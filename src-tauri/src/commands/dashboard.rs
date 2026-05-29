//! 仪表盘命令 — 一次查询获取所有统计数据

use crate::commands::auth::{verify_auth, AllowRoles};
use crate::db::DbPool;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardStats {
    pub today_sales: f64,
    pub pending_purchase: i64,
    pub stock_alerts: i64,
    pub project_count: i64,
    pub total_customers: i64,
    pub total_suppliers: i64,
    pub monthly_revenue: f64,
    pub monthly_profit: f64,
}

#[tauri::command]
pub fn get_stats(pool: State<'_, DbPool>, token: Option<String>) -> Result<DashboardStats, String> {
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    verify_auth(&pool, &token, AllowRoles::Any)?;

    // 一条 SQL 查询全部统计数据，减少往返
    let (today_sales,) = conn
        .query_row(
            "SELECT COALESCE(SUM(total_amount), 0) FROM sales_orders
             WHERE date(order_date) = date('now','localtime') AND status != 'cancelled'",
            [],
            |row| Ok((row.get::<_, f64>(0)?,)),
        )
        .unwrap_or((0.0,));

    let (pending_purchase,) = conn
        .query_row(
            "SELECT COALESCE(COUNT(*), 0) FROM purchase_orders WHERE status = 'submitted'",
            [],
            |row| Ok((row.get::<_, i64>(0)?,)),
        )
        .unwrap_or((0,));

    let (stock_alerts,) = conn
        .query_row(
            "SELECT COALESCE(COUNT(*), 0) FROM inventory i
             JOIN products p ON i.product_id = p.id
             WHERE i.quantity <= p.min_stock AND p.is_active = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?,)),
        )
        .unwrap_or((0,));

    let (project_count,) = conn
        .query_row(
            "SELECT COALESCE(COUNT(*), 0) FROM projects WHERE status NOT IN ('completed','cancelled')",
            [],
            |row| Ok((row.get::<_, i64>(0)?,)),
        )
        .unwrap_or((0,));

    let (total_customers,) = conn
        .query_row(
            "SELECT COALESCE(COUNT(*), 0) FROM customers WHERE is_active = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?,)),
        )
        .unwrap_or((0,));

    let (total_suppliers,) = conn
        .query_row(
            "SELECT COALESCE(COUNT(*), 0) FROM suppliers WHERE is_active = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?,)),
        )
        .unwrap_or((0,));

    let (monthly_revenue, monthly_profit) = conn
        .query_row(
            "SELECT COALESCE(SUM(total_amount), 0), COALESCE(SUM(profit), 0)
             FROM sales_orders
             WHERE date(order_date) >= date('now','localtime','start of month')
             AND status != 'cancelled'",
            [],
            |row| Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?)),
        )
        .unwrap_or((0.0, 0.0));

    Ok(DashboardStats {
        today_sales,
        pending_purchase,
        stock_alerts,
        project_count,
        total_customers,
        total_suppliers,
        monthly_revenue,
        monthly_profit,
    })
}
