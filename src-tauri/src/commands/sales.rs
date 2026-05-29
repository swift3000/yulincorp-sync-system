//! 销售管理命令 — 创建/确认/发货 + 库存联动出库 + 利润计算

use crate::commands::auth::{verify_auth, AllowRoles};
use crate::db::DbPool;
use crate::validators::*;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct SalesOrder {
    pub id: i64,
    pub order_no: String,
    pub customer_id: i64,
    pub customer_name: String,
    pub total_amount: f64,
    pub profit: f64,
    pub status: String,
    pub order_date: String,
    pub delivery_date: Option<String>,
    pub items_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SalesOrderDetail {
    pub id: i64,
    pub order_no: String,
    pub customer_id: i64,
    pub customer_name: String,
    pub total_amount: f64,
    pub profit: f64,
    pub status: String,
    pub order_date: String,
    pub delivery_date: Option<String>,
    pub notes: String,
    pub items: Vec<SalesOrderItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SalesOrderItem {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub sku: String,
    pub quantity: i64,
    pub unit_price: f64,
    pub cost_price: f64,
    pub total_price: f64,
    pub profit: f64,
}

fn row_to_sales_order(row: &rusqlite::Row<'_>) -> rusqlite::Result<SalesOrder> {
    Ok(SalesOrder {
        id: row.get(0)?, order_no: row.get(1)?, customer_id: row.get(2)?,
        customer_name: row.get(3)?, total_amount: row.get(4)?, profit: row.get(5)?,
        status: row.get(6)?, order_date: row.get(7)?, delivery_date: row.get(8)?,
        items_count: row.get(9)?,
    })
}

fn row_to_sales_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<SalesOrderItem> {
    Ok(SalesOrderItem {
        id: row.get(0)?, product_id: row.get(1)?, product_name: row.get(2)?,
        sku: row.get(3)?, quantity: row.get(4)?, unit_price: row.get(5)?,
        cost_price: row.get(6)?, total_price: row.get(7)?, profit: row.get(8)?,
    })
}

#[tauri::command]
pub fn list_sales_orders(
    pool: State<'_, DbPool>,
    token: Option<String>,
    status: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<Vec<SalesOrder>, String> {
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;

    let mut conditions = vec!["1=1".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
    if let Some(s) = &status {
        if !s.is_empty() && s != "all" {
            conditions.push(format!("so.status = ?{}", params.len() + 1));
            params.push(Box::new(s.clone()));
        }
    }

    let sql = format!(
        "SELECT so.id, so.order_no, so.customer_id, c.name, so.total_amount, so.profit,
                so.status, so.order_date, so.delivery_date,
                (SELECT COUNT(*) FROM sales_order_items WHERE order_id = so.id) as items_count
         FROM sales_orders so JOIN customers c ON so.customer_id = c.id
         WHERE {} ORDER BY so.id DESC LIMIT ?{} OFFSET ?{}",
        conditions.join(" AND "), params.len() + 1, params.len() + 2,
    );
    params.push(Box::new(page_size));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("SQL错误: {e}"))?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let orders: Vec<SalesOrder> = stmt.query_map(param_refs.as_slice(), row_to_sales_order)
        .map_err(|e| format!("查询错误: {e}"))?
        .filter_map(|r| r.ok()).collect();
    Ok(orders)
}

#[tauri::command]
pub fn get_sales_order(pool: State<'_, DbPool>, token: Option<String>, id: i64) -> Result<Option<SalesOrderDetail>, String> {
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let order = conn.query_row(
        "SELECT so.id, so.order_no, so.customer_id, c.name, so.total_amount, so.profit,
                so.status, so.order_date, so.delivery_date, so.notes
         FROM sales_orders so JOIN customers c ON so.customer_id = c.id WHERE so.id=?1",
        [id],
        |row| Ok(SalesOrderDetail {
            id: row.get(0)?, order_no: row.get(1)?, customer_id: row.get(2)?,
            customer_name: row.get(3)?, total_amount: row.get(4)?, profit: row.get(5)?,
            status: row.get(6)?, order_date: row.get(7)?, delivery_date: row.get(8)?,
            notes: row.get(9)?, items: vec![],
        }),
    );
    match order {
        Ok(mut o) => {
            let mut stmt = conn.prepare(
                "SELECT soi.id, soi.product_id, p.name, p.sku, soi.quantity,
                        soi.unit_price, soi.cost_price, soi.total_price, soi.profit
                 FROM sales_order_items soi JOIN products p ON soi.product_id = p.id
                 WHERE soi.order_id=?1",
            ).map_err(|e| format!("SQL错误: {e}"))?;
            o.items = stmt.query_map([id], row_to_sales_item)
                .map_err(|e| format!("查询错误: {e}"))?
                .filter_map(|r| r.ok()).collect();
            Ok(Some(o))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("查询错误: {e}")),
    }
}

fn gen_sales_order_no(conn: &rusqlite::Connection) -> String {
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sales_orders WHERE order_date = date('now','localtime')",
        [], |row| row.get(0),
    ).unwrap_or(0);
    format!("SO-{}-{:04}", today, count + 1)
}

#[derive(Debug, Deserialize)]
pub struct CreateSalesOrderRequest {
    pub customer_id: i64,
    pub delivery_date: Option<String>,
    pub notes: Option<String>,
    pub items: Vec<SalesItemInput>,
}

#[derive(Debug, Deserialize)]
pub struct SalesItemInput {
    pub product_id: i64,
    pub quantity: i64,
    pub unit_price: f64,
    /// 成本价（从产品资料自动获取，也可手动指定）
    pub cost_price: Option<f64>,
}

#[tauri::command]
pub fn create_sales_order(
    pool: State<'_, DbPool>,
    token: Option<String>,
    data: CreateSalesOrderRequest,
) -> Result<SalesOrderDetail, String> {
    let user = verify_auth(&pool, &token, AllowRoles::Any)?;

    validate_positive_i64(data.customer_id, "客户")?;
    for item in &data.items {
        validate_positive_i64(item.product_id, "产品ID")?;
        validate_positive_f64(item.unit_price, "单价")?;
        if let Some(cp) = item.cost_price {
            validate_non_negative_f64(cp, "成本价")?;
        }
    }

    if data.items.is_empty() {
        return Err("请至少添加一项销售明细".into());
    }
    if data.items.iter().any(|i| i.quantity <= 0 || i.unit_price < 0.0) {
        return Err("明细数量和单价不能为负数".into());
    }
    if data.items.len() > 200 {
        return Err("单次最多添加200行明细".into());
    }

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;

    let order_no = gen_sales_order_no(&conn);
    let mut total_amount = 0.0;
    let mut total_profit = 0.0;

    // 批量查询成本价（一次IN查询替代逐条查询）
    let cost_cache = if data.items.iter().any(|i| i.cost_price.is_none()) {
        let ids: Vec<String> = data.items.iter()
            .filter(|i| i.cost_price.is_none())
            .map(|i| i.product_id.to_string())
            .collect();
        if !ids.is_empty() {
            let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!("SELECT id, purchase_price FROM products WHERE id IN ({})", placeholders);
            let mut stmt = conn.prepare(&sql).ok();
            let mut cache = std::collections::HashMap::new();
            if let Some(ref mut stmt) = stmt {
                let params: Vec<&dyn rusqlite::types::ToSql> = ids.iter()
                    .map(|s| s as &dyn rusqlite::types::ToSql)
                    .collect();
                let _ = stmt.query_map(params.as_slice(), |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
                }).map(|rows| {
                    for r in rows {
                        if let Ok((id, price)) = r { cache.insert(id, price); }
                    }
                });
            }
            cache
        } else {
            std::collections::HashMap::new()
        }
    } else {
        std::collections::HashMap::new()
    };

    conn.execute(
        "INSERT INTO sales_orders (order_no, customer_id, total_amount, profit, status, delivery_date, notes, created_by)
         VALUES (?1, ?2, 0, 0, 'draft', ?3, ?4, ?5)",
        rusqlite::params![order_no, data.customer_id, data.delivery_date, data.notes, user.id],
    ).map_err(|e| format!("创建失败: {e}"))?;

    let order_id = conn.last_insert_rowid();

    for item in &data.items {
        let cost = item.cost_price.unwrap_or(*cost_cache.get(&item.product_id).unwrap_or(&0.0));
        let line_total = item.quantity as f64 * item.unit_price;
        let line_profit = line_total - item.quantity as f64 * cost;

        conn.execute(
            "INSERT INTO sales_order_items (order_id, product_id, quantity, unit_price, cost_price, total_price, profit)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![order_id, item.product_id, item.quantity, item.unit_price, cost, line_total, line_profit],
        ).map_err(|e| format!("插入明细失败: {e}"))?;

        total_amount += line_total;
        total_profit += line_profit;
    }

    // 更新汇总金额
    conn.execute(
        "UPDATE sales_orders SET total_amount=?1, profit=?2 WHERE id=?3",
        rusqlite::params![total_amount, total_profit, order_id],
    ).map_err(|e| format!("更新汇总失败: {e}"))?;

    log::info!("销售单已创建: {} (金额={}, 毛利={})", order_no, total_amount, total_profit);

    let mut detail = conn.query_row(
        "SELECT so.id, so.order_no, so.customer_id, c.name, so.total_amount, so.profit,
                so.status, so.order_date, so.delivery_date, so.notes
         FROM sales_orders so JOIN customers c ON so.customer_id = c.id WHERE so.id=?1",
        [order_id],
        |row| Ok(SalesOrderDetail {
            id: row.get(0)?, order_no: row.get(1)?, customer_id: row.get(2)?,
            customer_name: row.get(3)?, total_amount: row.get(4)?, profit: row.get(5)?,
            status: row.get(6)?, order_date: row.get(7)?, delivery_date: row.get(8)?,
            notes: row.get(9)?, items: vec![],
        }),
    ).map_err(|e| format!("创建后读取失败: {e}"))?;

    let mut stmt = conn.prepare(
        "SELECT soi.id, soi.product_id, p.name, p.sku, soi.quantity,
                soi.unit_price, soi.cost_price, soi.total_price, soi.profit
         FROM sales_order_items soi JOIN products p ON soi.product_id = p.id
         WHERE soi.order_id=?1",
    ).map_err(|e| format!("SQL错误: {e}"))?;
    detail.items = stmt.query_map([order_id], row_to_sales_item)
        .map_err(|e| format!("查询明细失败: {e}"))?
        .filter_map(|r| r.ok()).collect();

    Ok(detail)
}

/// 确认销售单（草稿→已确认）
#[tauri::command]
pub fn confirm_sales_order(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
) -> Result<(), String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;

    validate_positive_i64(id, "销售单ID")?;

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let affected = conn.execute(
        "UPDATE sales_orders SET status='confirmed', updated_at=datetime('now','localtime')
         WHERE id=?1 AND status='draft'",
        [id],
    ).map_err(|e| format!("确认失败: {e}"))?;
    if affected == 0 { return Err("只能确认草稿状态的销售单".into()); }
    log::info!("销售单已确认: id={}", id);
    Ok(())
}

/// 发货 — 确认后扣减库存
#[tauri::command]
pub fn ship_sales_order(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
) -> Result<(), String> {
    let _user = verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;

    validate_positive_i64(id, "销售单ID")?;

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;

    // 检查状态
    let status: String = conn.query_row(
        "SELECT status FROM sales_orders WHERE id=?1", [id], |row| row.get(0),
    ).map_err(|_| "销售单不存在".to_string())?;
    if status != "confirmed" { return Err("只能对已确认的销售单发货".into()); }

    // 获取所有明细
    let mut stmt = conn.prepare(
        "SELECT soi.product_id, soi.quantity, soi.id FROM sales_order_items soi WHERE soi.order_id=?1"
    ).map_err(|e| format!("SQL错误: {e}"))?;

    let items: Vec<(i64, i64, i64)> = stmt.query_map([id], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    }).map_err(|e| format!("查询明细失败: {e}"))?
    .filter_map(|r| r.ok()).collect();

    // 预检查：确保所有产品库存充足
    for (product_id, qty, _item_id) in &items {
        let available: i64 = conn.query_row(
            "SELECT quantity FROM inventory WHERE product_id = ?1 AND warehouse_id = 1",
            [*product_id], |r| r.get(0),
        ).unwrap_or(0);
        if available < *qty {
            return Err(format!(
                "库存不足：产品ID={}，可用库存{}，需要{}，无法完成出库",
                product_id, available, qty
            ));
        }
    }

    // 全部检查通过后批量扣减
    for (product_id, qty, _item_id) in &items {
        conn.execute(
            "UPDATE inventory SET quantity = quantity - ?1, updated_at = datetime('now','localtime')
             WHERE product_id = ?2 AND warehouse_id = 1 AND quantity >= ?1",
            rusqlite::params![qty, product_id],
        ).map_err(|e| format!("库存扣减失败(product_id={}): {}", product_id, e))?;
    }

    // 更新状态
    conn.execute(
        "UPDATE sales_orders SET status='shipped', delivery_date=date('now','localtime'), updated_at=datetime('now','localtime')
         WHERE id=?1",
        [id],
    ).map_err(|e| format!("更新状态失败: {e}"))?;

    log::info!("销售单已发货: id={}, 扣减{}项库存", id, items.len());
    Ok(())
}
