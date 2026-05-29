//! 财务报表命令 — 应收应付 + 利润分析 + 收付款记录

use crate::commands::auth::{verify_auth, AllowRoles};
use crate::db::DbPool;
use crate::validators::*;
use serde::{Deserialize, Serialize};
use tauri::State;

// ── 数据结构 ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ARItem {
    pub customer_id: i64,
    pub customer_name: String,
    pub total_sales: f64,
    pub total_received: f64,
    pub balance: f64,
    pub last_transaction_date: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct APItem {
    pub supplier_id: i64,
    pub supplier_name: String,
    pub total_purchases: f64,
    pub total_paid: f64,
    pub balance: f64,
    pub last_transaction_date: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfitSummary {
    pub total_revenue: f64,
    pub total_cost: f64,
    pub gross_profit: f64,
    pub gross_margin: f64,
    pub period_start: String,
    pub period_end: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentTransaction {
    pub id: i64,
    pub transaction_type: String,
    pub reference_type: String,
    pub reference_id: Option<i64>,
    pub party_type: String,
    pub party_id: i64,
    pub party_name: String,
    pub amount: f64,
    pub payment_method: String,
    pub transaction_date: String,
    pub notes: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionListResult {
    pub items: Vec<PaymentTransaction>,
    pub total_count: i64,
    pub total_amount: f64,
}

// ── 应收账款 ──────────────────────────────────────────

#[tauri::command]
pub fn list_ar(
    pool: State<'_, DbPool>,
    token: Option<String>,
) -> Result<Vec<ARItem>, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.name,
                    COALESCE(SUM(CASE WHEN so.status != 'draft' THEN so.total_amount ELSE 0 END), 0) AS total_sales,
                    COALESCE((
                        SELECT SUM(pt.amount) FROM payment_transactions pt
                        WHERE pt.party_type = 'customer' AND pt.party_id = c.id
                        AND pt.transaction_type = 'receipt' AND pt.is_deleted = 0
                    ), 0) AS total_received,
                    MAX(pt2.transaction_date) AS last_date
             FROM customers c
             LEFT JOIN sales_orders so ON so.customer_id = c.id
             LEFT JOIN payment_transactions pt2 ON pt2.party_id = c.id AND pt2.party_type = 'customer' AND pt2.is_deleted = 0
             WHERE c.is_active = 1
             GROUP BY c.id, c.name
             HAVING total_sales > 0 OR total_received > 0
             ORDER BY (total_sales - total_received) DESC"
        )
        .map_err(|e| e.to_string())?;

    let items: Vec<ARItem> = stmt
        .query_map([], |row| {
            let total_sales: f64 = row.get(2)?;
            let total_received: f64 = row.get(3)?;
            Ok(ARItem {
                customer_id: row.get(0)?,
                customer_name: row.get(1)?,
                total_sales,
                total_received,
                balance: total_sales - total_received,
                last_transaction_date: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(items)
}

// ── 应付账款 ──────────────────────────────────────────

#[tauri::command]
pub fn list_ap(
    pool: State<'_, DbPool>,
    token: Option<String>,
) -> Result<Vec<APItem>, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.name,
                    COALESCE(SUM(CASE WHEN po.status != 'draft' THEN po.total_amount ELSE 0 END), 0) AS total_purchases,
                    COALESCE((
                        SELECT SUM(pt.amount) FROM payment_transactions pt
                        WHERE pt.party_type = 'supplier' AND pt.party_id = s.id
                        AND pt.transaction_type = 'payment' AND pt.is_deleted = 0
                    ), 0) AS total_paid,
                    MAX(pt2.transaction_date) AS last_date
             FROM suppliers s
             LEFT JOIN purchase_orders po ON po.supplier_id = s.id
             LEFT JOIN payment_transactions pt2 ON pt2.party_id = s.id AND pt2.party_type = 'supplier' AND pt2.is_deleted = 0
             WHERE s.is_active = 1
             GROUP BY s.id, s.name
             HAVING total_purchases > 0 OR total_paid > 0
             ORDER BY (total_purchases - total_paid) DESC"
        )
        .map_err(|e| e.to_string())?;

    let items: Vec<APItem> = stmt
        .query_map([], |row| {
            let total_purchases: f64 = row.get(2)?;
            let total_paid: f64 = row.get(3)?;
            Ok(APItem {
                supplier_id: row.get(0)?,
                supplier_name: row.get(1)?,
                total_purchases,
                total_paid,
                balance: total_purchases - total_paid,
                last_transaction_date: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(items)
}

// ── 利润分析 ──────────────────────────────────────────

#[tauri::command]
pub fn get_profit_summary(
    pool: State<'_, DbPool>,
    token: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
) -> Result<ProfitSummary, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;

    if let Some(ref d) = start_date {
        validate_date_format(d, "开始日期")?;
    }
    if let Some(ref d) = end_date {
        validate_date_format(d, "结束日期")?;
    }
    if let (Some(ref s), Some(ref e)) = (&start_date, &end_date) {
        validate_date_range(s, e)?;
    }

    let start = start_date.unwrap_or_else(|| "2000-01-01".to_string());
    let end = end_date.unwrap_or_else(|| "2099-12-31".to_string());

    let result = conn
        .query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN so.status IN ('confirmed','shipped') THEN so.total_amount ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN so.status IN ('confirmed','shipped') THEN soi.quantity * soi.cost_price ELSE 0 END), 0)
             FROM sales_orders so
             LEFT JOIN sales_order_items soi ON soi.order_id = so.id
             WHERE so.order_date BETWEEN ?1 AND ?2",
            rusqlite::params![start, end],
            |row| {
                let revenue: f64 = row.get(0)?;
                let cost: f64 = row.get(1)?;
                let profit = revenue - cost;
                let margin = if revenue > 0.0 { (profit / revenue) * 100.0 } else { 0.0 };
                Ok(ProfitSummary {
                    total_revenue: revenue,
                    total_cost: cost,
                    gross_profit: profit,
                    gross_margin: (margin * 100.0).round() / 100.0,
                    period_start: start.clone(),
                    period_end: end.clone(),
                })
            },
        )
        .map_err(|e| format!("查询利润数据失败: {}", e))?;

    Ok(result)
}

// ── 收付款记录列表 ────────────────────────────────────

#[tauri::command]
pub fn list_transactions(
    pool: State<'_, DbPool>,
    token: Option<String>,
    transaction_type: Option<String>,
    party_type: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<TransactionListResult, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(50).min(200);
    let offset = (page - 1) * page_size;

    let mut where_clauses = vec!["pt.is_deleted = 0".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref t) = transaction_type {
        where_clauses.push(format!("pt.transaction_type = ?{}", params.len() + 1));
        params.push(Box::new(t.clone()));
    }
    if let Some(ref p) = party_type {
        where_clauses.push(format!("pt.party_type = ?{}", params.len() + 1));
        params.push(Box::new(p.clone()));
    }

    let where_sql = where_clauses.join(" AND ");

    let query_sql = format!(
        "SELECT pt.id, pt.transaction_type, pt.reference_type, pt.reference_id,
                pt.party_type, pt.party_id,
                CASE WHEN pt.party_type='customer' THEN COALESCE(c.name,'')
                     ELSE COALESCE(s.name,'') END AS party_name,
                pt.amount, pt.payment_method, pt.transaction_date, pt.notes
         FROM payment_transactions pt
         LEFT JOIN customers c ON pt.party_type='customer' AND pt.party_id=c.id
         LEFT JOIN suppliers s ON pt.party_type='supplier' AND pt.party_id=s.id
         WHERE {} ORDER BY pt.id DESC
         LIMIT ?{} OFFSET ?{}",
        where_sql,
        params.len() + 1,
        params.len() + 2
    );

    let count_sql = format!("SELECT COUNT(*), COALESCE(SUM(amount),0) FROM payment_transactions pt WHERE {}", where_sql);
    let (total_count, total_amount): (i64, f64) = {
        let mut stmt = conn.prepare(&count_sql).map_err(|e| e.to_string())?;
        let pr: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        stmt.query_row(pr.as_slice(), |row| Ok((row.get(0)?, row.get(1)?))).map_err(|e| e.to_string())?
    };

    params.push(Box::new(page_size));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&query_sql).map_err(|e| e.to_string())?;
    let pr: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let items: Vec<PaymentTransaction> = stmt
        .query_map(pr.as_slice(), |row| {
            Ok(PaymentTransaction {
                id: row.get(0)?, transaction_type: row.get(1)?,
                reference_type: row.get(2)?, reference_id: row.get(3)?,
                party_type: row.get(4)?, party_id: row.get(5)?,
                party_name: row.get(6)?, amount: row.get(7)?,
                payment_method: row.get(8)?, transaction_date: row.get(9)?,
                notes: row.get(10)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(TransactionListResult { items, total_count, total_amount })
}

// ── 记录收付款 ────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RecordTransactionInput {
    pub transaction_type: String,
    pub reference_type: String,
    pub reference_id: Option<i64>,
    pub party_type: String,
    pub party_id: i64,
    pub amount: f64,
    pub payment_method: String,
    pub transaction_date: String,
    pub notes: String,
}

#[tauri::command]
pub fn record_transaction(
    pool: State<'_, DbPool>,
    token: Option<String>,
    input: RecordTransactionInput,
) -> Result<PaymentTransaction, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_transaction_type(&input.transaction_type)?;
    validate_party_type(&input.party_type)?;
    validate_positive_i64(input.party_id, "交易对象")?;
    validate_non_empty(&input.payment_method, "付款方式")?;
    validate_date(&input.transaction_date, "交易日期")?;
    validate_max_len(&input.notes, 1000, "备注")?;
    if input.amount <= 0.0 { return Err("金额必须大于0".into()); }

    let conn = pool.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO payment_transactions (transaction_type, reference_type, reference_id, party_type, party_id, amount, payment_method, transaction_date, notes)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![
            input.transaction_type, input.reference_type, input.reference_id,
            input.party_type, input.party_id, input.amount,
            input.payment_method, input.transaction_date, input.notes,
        ],
    )
    .map_err(|e| format!("记录失败: {}", e))?;

    let new_id = conn.last_insert_rowid();
    let (party_name,): (String,) = if input.party_type == "customer" {
        conn.query_row("SELECT name FROM customers WHERE id = ?1", [input.party_id], |row| Ok((row.get(0)?,))).unwrap_or(("未知".into(),))
    } else {
        conn.query_row("SELECT name FROM suppliers WHERE id = ?1", [input.party_id], |row| Ok((row.get(0)?,))).unwrap_or(("未知".into(),))
    };

    Ok(PaymentTransaction {
        id: new_id, transaction_type: input.transaction_type,
        reference_type: input.reference_type, reference_id: input.reference_id,
        party_type: input.party_type, party_id: input.party_id,
        party_name, amount: input.amount,
        payment_method: input.payment_method,
        transaction_date: input.transaction_date,
        notes: input.notes,
    })
}

// ── 软删除交易记录 ────────────────────────────────────

#[tauri::command]
pub fn delete_transaction(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
) -> Result<(), String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_positive_i64(id, "交易记录ID")?;
    let conn = pool.lock().map_err(|e| e.to_string())?;
    let affected = conn.execute(
        "UPDATE payment_transactions SET is_deleted = 1 WHERE id = ?1 AND is_deleted = 0",
        [id],
    ).map_err(|e| e.to_string())?;
    if affected == 0 { return Err("记录不存在或已删除".into()); }
    Ok(())
}
