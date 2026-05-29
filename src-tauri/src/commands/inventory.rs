//! 库存管理命令 — 原子 UPDATE 防负库存、出入库日志

use crate::commands::auth::{verify_auth, AllowRoles};
use crate::db::DbPool;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct StockItem {
    pub product_id: i64,
    pub product_name: String,
    pub sku: String,
    pub quantity: i64,
    pub locked_quantity: i64,
    pub available_quantity: i64,
    pub min_stock: i64,
    pub unit: String,
    pub updated_at: String,
}

fn row_to_stock(row: &rusqlite::Row<'_>) -> rusqlite::Result<StockItem> {
    let quantity: i64 = row.get(3)?;
    let locked: i64 = row.get(4)?;
    Ok(StockItem {
        product_id: row.get(0)?,
        product_name: row.get(1)?,
        sku: row.get(2)?,
        quantity,
        locked_quantity: locked,
        available_quantity: quantity - locked,
        min_stock: row.get(5)?,
        unit: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

#[tauri::command]
pub fn get_stock(
    pool: State<'_, DbPool>,
    token: Option<String>,
    keyword: Option<String>,
    low_stock_only: Option<bool>,
) -> Result<Vec<StockItem>, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;

    // 确保所有产品都有库存记录（首次查询时自动初始化）
    conn.execute(
        "INSERT OR IGNORE INTO inventory (product_id, warehouse_id, quantity)
         SELECT id, 1, 0 FROM products WHERE is_active = 1",
        [],
    )
    .map_err(|e| format!("初始化库存失败: {e}"))?;

    let mut sql = String::from(
        "SELECT p.id, p.name, p.sku, i.quantity, i.locked_quantity,
                p.min_stock, p.unit, i.updated_at
         FROM inventory i
         JOIN products p ON i.product_id = p.id
         WHERE p.is_active = 1",
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

    if low_stock_only.unwrap_or(false) {
        sql.push_str(" AND i.quantity <= p.min_stock");
    }
    if let Some(kw) = &keyword {
        let kw = kw.trim();
        if !kw.is_empty() && kw.len() <= 100 {
            sql.push_str(&format!(
                " AND (p.name LIKE ?{} OR p.sku LIKE ?{})",
                params.len() + 1,
                params.len() + 2
            ));
            let pattern = format!("%{kw}%");
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern));
        }
    }
    sql.push_str(" ORDER BY p.id ASC");

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("SQL错误: {e}"))?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let items: Vec<StockItem> = stmt
        .query_map(param_refs.as_slice(), row_to_stock)
        .map_err(|e| format!("查询错误: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(items)
}

/// 入库 — 原子增加库存数量
#[tauri::command]
pub fn stock_in(
    pool: State<'_, DbPool>,
    token: Option<String>,
    product_id: i64,
    quantity: i64,
    warehouse_id: Option<i64>,
    notes: Option<String>,
) -> Result<(), String> {
    let user = verify_auth(&pool, &token, AllowRoles::Any)?;

    if quantity <= 0 {
        return Err("入库数量必须大于0".into());
    }
    if quantity > 1_000_000 {
        return Err("单次入库数量不能超过100万".into());
    }
    let wh = warehouse_id.unwrap_or(1);

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;

    // 验证产品存在
    let product_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM products WHERE id=?1 AND is_deleted=0",
        [product_id], |r| r.get::<_, i64>(0),
    ).map(|c| c > 0).unwrap_or(false);
    if !product_exists {
        return Err(format!("产品ID={}不存在，无法入库", product_id));
    }

    // 确保库存记录存在
    conn.execute(
        "INSERT OR IGNORE INTO inventory (product_id, warehouse_id, quantity) VALUES (?1, ?2, 0)",
        rusqlite::params![product_id, wh],
    )
    .map_err(|e| format!("初始化库存记录失败: {e}"))?;

    // 原子 UPDATE（SQLite 单写者保证原子性）
    let affected = conn.execute(
        "UPDATE inventory SET quantity = quantity + ?1, updated_at = datetime('now','localtime')
         WHERE product_id = ?2 AND warehouse_id = ?3",
        rusqlite::params![quantity, product_id, wh],
    )
    .map_err(|e| format!("入库失败: {e}"))?;

    if affected == 0 {
        return Err(format!("入库失败：产品ID={}的库存记录不存在", product_id));
    }

    log_operation(&conn, user.id, product_id, "stock_in", quantity, notes.unwrap_or_default())?;

    log::info!("入库: product_id={}, +{} (仓库{}, 操作人:{})", product_id, quantity, wh, user.username);
    Ok(())
}

/// 出库 — 原子扣减，禁止负库存
#[tauri::command]
pub fn stock_out(
    pool: State<'_, DbPool>,
    token: Option<String>,
    product_id: i64,
    quantity: i64,
    warehouse_id: Option<i64>,
    notes: Option<String>,
) -> Result<(), String> {
    let user = verify_auth(&pool, &token, AllowRoles::Any)?;

    if quantity <= 0 {
        return Err("出库数量必须大于0".into());
    }
    if quantity > 1_000_000 {
        return Err("单次出库数量不能超过100万".into());
    }
    let wh = warehouse_id.unwrap_or(1);

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;

    // 原子检查 + 扣减（SQLite 单写者保证不会超卖）
    let affected = conn
        .execute(
            "UPDATE inventory SET quantity = quantity - ?1, updated_at = datetime('now','localtime')
             WHERE product_id = ?2 AND warehouse_id = ?3 AND quantity >= ?1",
            rusqlite::params![quantity, product_id, wh],
        )
        .map_err(|e| format!("出库失败: {e}"))?;

    if affected == 0 {
        return Err("库存不足，无法完成出库".into());
    }

    log_operation(&conn, user.id, product_id, "stock_out", -quantity, notes.unwrap_or_default())?;

    log::info!("出库: product_id={}, -{} (仓库{}, 操作人:{})", product_id, quantity, wh, user.username);
    Ok(())
}

fn log_operation(
    conn: &rusqlite::Connection,
    user_id: i64,
    product_id: i64,
    action: &str,
    quantity: i64,
    notes: String,
) -> Result<(), String> {
    let after_data = serde_json::json!({
        "quantity_change": quantity,
        "notes": notes,
    })
    .to_string();

    conn.execute(
        "INSERT INTO operation_logs (user_id, action, module, record_id, after_data)
         VALUES (?1, ?2, 'inventory', ?3, ?4)",
        rusqlite::params![user_id, format!("{action}: {}{}", if quantity > 0 { "+" } else { "" }, quantity), product_id, after_data],
    )
    .map_err(|e| format!("日志记录失败: {e}"))?;
    Ok(())
}
