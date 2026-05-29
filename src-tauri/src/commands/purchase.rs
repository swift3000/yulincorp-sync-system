//! 采购管理命令 — 创建/提交/收货 + 库存联动入库

use crate::commands::auth::{verify_auth, AllowRoles};
use crate::db::DbPool;
use crate::validators::*;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct PurchaseOrder {
    pub id: i64,
    pub order_no: String,
    pub supplier_id: i64,
    pub supplier_name: String,
    pub total_amount: f64,
    pub status: String,
    pub order_date: String,
    pub expected_date: String,
    pub notes: String,
    pub items_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PurchaseOrderDetail {
    pub id: i64,
    pub order_no: String,
    pub supplier_id: i64,
    pub supplier_name: String,
    pub total_amount: f64,
    pub status: String,
    pub order_date: String,
    pub expected_date: String,
    pub notes: String,
    pub items: Vec<PurchaseOrderItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PurchaseOrderItem {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub sku: String,
    pub quantity: i64,
    pub unit_price: f64,
    pub total_price: f64,
    pub received_quantity: i64,
}

fn row_to_order(row: &rusqlite::Row<'_>) -> rusqlite::Result<PurchaseOrder> {
    Ok(PurchaseOrder {
        id: row.get(0)?,
        order_no: row.get(1)?,
        supplier_id: row.get(2)?,
        supplier_name: row.get(3)?,
        total_amount: row.get(4)?,
        status: row.get(5)?,
        order_date: row.get(6)?,
        expected_date: row.get(7)?,
        notes: row.get(8)?,
        items_count: row.get(9)?,
    })
}

fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<PurchaseOrderItem> {
    Ok(PurchaseOrderItem {
        id: row.get(0)?,
        product_id: row.get(1)?,
        product_name: row.get(2)?,
        sku: row.get(3)?,
        quantity: row.get(4)?,
        unit_price: row.get(5)?,
        total_price: row.get(6)?,
        received_quantity: row.get(7)?,
    })
}

#[tauri::command]
pub fn list_purchase_orders(
    pool: State<'_, DbPool>,
    token: Option<String>,
    status: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<Vec<PurchaseOrder>, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;

    let mut conditions = vec!["1=1".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
    if let Some(s) = &status {
        if !s.is_empty() && s != "all" {
            conditions.push(format!("po.status = ?{}", params.len() + 1));
            params.push(Box::new(s.clone()));
        }
    }

    let sql = format!(
        "SELECT po.id, po.order_no, po.supplier_id, s.name, po.total_amount, po.status,
                po.order_date, po.expected_date, po.notes,
                (SELECT COUNT(*) FROM purchase_order_items WHERE order_id = po.id) as items_count
         FROM purchase_orders po
         JOIN suppliers s ON po.supplier_id = s.id
         WHERE {} ORDER BY po.id DESC LIMIT ?{} OFFSET ?{}",
        conditions.join(" AND "),
        params.len() + 1, params.len() + 2,
    );
    params.push(Box::new(page_size));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("SQL错误: {e}"))?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let orders: Vec<PurchaseOrder> = stmt
        .query_map(param_refs.as_slice(), row_to_order)
        .map_err(|e| format!("查询错误: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(orders)
}

#[tauri::command]
pub fn get_purchase_order(pool: State<'_, DbPool>, token: Option<String>, id: i64) -> Result<Option<PurchaseOrderDetail>, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;

    let order = conn.query_row(
        "SELECT po.id, po.order_no, po.supplier_id, s.name, po.total_amount, po.status,
                po.order_date, po.expected_date, po.notes
         FROM purchase_orders po JOIN suppliers s ON po.supplier_id = s.id WHERE po.id=?1",
        [id],
        |row| Ok(PurchaseOrderDetail {
            id: row.get(0)?, order_no: row.get(1)?, supplier_id: row.get(2)?,
            supplier_name: row.get(3)?, total_amount: row.get(4)?, status: row.get(5)?,
            order_date: row.get(6)?, expected_date: row.get(7)?, notes: row.get(8)?,
            items: vec![],
        }),
    );

    match order {
        Ok(mut o) => {
            let mut stmt = conn.prepare(
                "SELECT poi.id, poi.product_id, p.name, p.sku, poi.quantity,
                        poi.unit_price, poi.total_price, poi.received_quantity
                 FROM purchase_order_items poi JOIN products p ON poi.product_id = p.id
                 WHERE poi.order_id=?1",
            ).map_err(|e| format!("SQL错误: {e}"))?;
            o.items = stmt.query_map([id], row_to_item)
                .map_err(|e| format!("查询错误: {e}"))?
                .filter_map(|r| r.ok()).collect();
            Ok(Some(o))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("查询错误: {e}")),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreatePurchaseOrderRequest {
    pub supplier_id: i64,
    pub expected_date: Option<String>,
    pub notes: Option<String>,
    pub items: Vec<OrderItemInput>,
}

#[derive(Debug, Deserialize)]
pub struct OrderItemInput {
    pub product_id: i64,
    pub quantity: i64,
    pub unit_price: f64,
}

/// 生成订单号：PO-年月日-序号
fn gen_order_no(conn: &rusqlite::Connection) -> String {
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM purchase_orders WHERE order_date = date('now','localtime')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    format!("PO-{}-{:04}", today, count + 1)
}

#[tauri::command]
pub fn create_purchase_order(
    pool: State<'_, DbPool>,
    token: Option<String>,
    data: CreatePurchaseOrderRequest,
) -> Result<PurchaseOrderDetail, String> {
    let user = verify_auth(&pool, &token, AllowRoles::Any)?;

    validate_positive_i64(data.supplier_id, "供应商")?;
    for item in &data.items {
        validate_positive_i64(item.product_id, "产品ID")?;
        validate_positive_f64(item.unit_price, "单价")?;
    }

    if data.items.is_empty() {
        return Err("请至少添加一项采购明细".into());
    }
    if data.items.iter().any(|i| i.quantity <= 0 || i.unit_price < 0.0) {
        return Err("明细数量和单价不能为负数".into());
    }
    if data.items.len() > 200 {
        return Err("单次最多添加200行明细".into());
    }

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let order_no = gen_order_no(&conn);

    let total: f64 = data.items.iter().map(|i| i.quantity as f64 * i.unit_price).sum();

    conn.execute(
        "INSERT INTO purchase_orders (order_no, supplier_id, total_amount, status, expected_date, notes, created_by)
         VALUES (?1, ?2, ?3, 'draft', ?4, ?5, ?6)",
        rusqlite::params![order_no, data.supplier_id, total, data.expected_date, data.notes, user.id],
    )
    .map_err(|e| format!("创建失败: {e}"))?;

    let order_id = conn.last_insert_rowid();

    // 批量插入明细（单条循环，SQLite 单连接下安全）
    for item in &data.items {
        conn.execute(
            "INSERT INTO purchase_order_items (order_id, product_id, quantity, unit_price, total_price)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![order_id, item.product_id, item.quantity, item.unit_price, item.quantity as f64 * item.unit_price],
        )
        .map_err(|e| format!("插入明细失败: {e}"))?;
    }

    log::info!("采购单已创建: {} (总金额={}, 操作人:{})", order_no, total, user.username);

    // 内联读取（避免 borrow checker 冲突）
    let mut detail = conn.query_row(
        "SELECT po.id, po.order_no, po.supplier_id, s.name, po.total_amount, po.status,
                po.order_date, po.expected_date, po.notes
         FROM purchase_orders po JOIN suppliers s ON po.supplier_id = s.id WHERE po.id=?1",
        [order_id],
        |row| Ok(PurchaseOrderDetail {
            id: row.get(0)?, order_no: row.get(1)?, supplier_id: row.get(2)?,
            supplier_name: row.get(3)?, total_amount: row.get(4)?, status: row.get(5)?,
            order_date: row.get(6)?, expected_date: row.get(7)?, notes: row.get(8)?,
            items: vec![],
        }),
    ).map_err(|e| format!("创建后读取失败: {e}"))?;

    // 读取明细
    let mut stmt = conn.prepare(
        "SELECT poi.id, poi.product_id, p.name, p.sku, poi.quantity,
                poi.unit_price, poi.total_price, poi.received_quantity
         FROM purchase_order_items poi JOIN products p ON poi.product_id = p.id
         WHERE poi.order_id=?1",
    ).map_err(|e| format!("SQL错误: {e}"))?;
    detail.items = stmt.query_map([order_id], row_to_item)
        .map_err(|e| format!("查询明细失败: {e}"))?
        .filter_map(|r| r.ok()).collect();

    Ok(detail)
}

#[tauri::command]
pub fn submit_purchase_order(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
) -> Result<(), String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;

    validate_positive_i64(id, "采购单ID")?;

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let affected = conn
        .execute(
            "UPDATE purchase_orders SET status='submitted', updated_at=datetime('now','localtime')
             WHERE id=?1 AND status='draft'",
            [id],
        )
        .map_err(|e| format!("提交失败: {e}"))?;
    if affected == 0 { return Err("只能提交草稿状态的采购单".into()); }
    log::info!("采购单已提交: id={}", id);
    Ok(())
}

/// 收货 — 更新收货数量 + 自动入库
#[tauri::command]
pub fn receive_purchase_order(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
    item_received: Vec<ItemReceived>,
) -> Result<(), String> {
    let _user = verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;

    validate_positive_i64(id, "采购单ID")?;
    if item_received.is_empty() {
        return Err("收货明细不能为空".into());
    }
    for item in &item_received {
        validate_positive_i64(item.received_qty, "收货数量")?;
    }

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;

    // 检查订单状态
    let status: String = conn.query_row(
        "SELECT status FROM purchase_orders WHERE id=?1", [id], |row| row.get(0),
    ).map_err(|_| "采购单不存在".to_string())?;
    if status != "submitted" && status != "received" {
        return Err(format!("只能对已提交的采购单收货，当前状态：{}", status));
    }

    for item in &item_received {
        // 检查是否超收
        let (ordered, received): (i64, i64) = conn.query_row(
            "SELECT quantity, received_quantity FROM purchase_order_items WHERE id=?1 AND order_id=?2",
            rusqlite::params![item.item_id, id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|e| format!("采购明细(ID={})不存在: {}", item.item_id, e))?;

        if received + item.received_qty > ordered {
            return Err(format!(
                "收货数量超出订单量：明细ID={}，已收{}，本次收{}，订单量{}",
                item.item_id, received, item.received_qty, ordered
            ));
        }

        // 更新收货数量
        conn.execute(
            "UPDATE purchase_order_items SET received_quantity = received_quantity + ?1 WHERE id=?2 AND order_id=?3",
            rusqlite::params![item.received_qty, item.item_id, id],
        )
        .map_err(|e| format!("更新收货数量失败: {e}"))?;

        // 自动入库
        conn.execute(
            "INSERT INTO inventory (product_id, warehouse_id, quantity) VALUES (?1, 1, 0)
             ON CONFLICT(product_id, warehouse_id) DO NOTHING",
            [item.product_id],
        )
        .map_err(|e| format!("初始化库存失败: {e}"))?;

        conn.execute(
            "UPDATE inventory SET quantity = quantity + ?1, updated_at = datetime('now','localtime')
             WHERE product_id = ?2 AND warehouse_id = 1",
            rusqlite::params![item.received_qty, item.product_id],
        )
        .map_err(|e| format!("入库失败: {e}"))?;
    }

    // 检查是否全部收货
    let pending: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM purchase_order_items WHERE order_id=?1 AND quantity > received_quantity",
            [id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if pending == 0 {
        conn.execute(
            "UPDATE purchase_orders SET status='received', received_date=date('now','localtime'), updated_at=datetime('now','localtime')
             WHERE id=?1",
            [id],
        )
        .map_err(|e| format!("更新状态失败: {e}"))?;
    }

    log::info!("采购单收货: id={}, 共{}项", id, item_received.len());
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ItemReceived {
    pub item_id: i64,
    pub product_id: i64,
    pub received_qty: i64,
}
