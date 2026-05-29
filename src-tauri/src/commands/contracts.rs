//! 合同管理命令 — CRUD + 状态流转 + 收付款跟踪

use crate::commands::auth::{verify_auth, AllowRoles};
use crate::db::DbPool;
use crate::validators::*;
use serde::{Deserialize, Serialize};
use tauri::State;

// ── 数据结构 ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Contract {
    pub id: i64,
    pub contract_code: String,
    pub name: String,
    pub direction: String,
    pub supplier_id: Option<i64>,
    pub supplier_name: String,
    pub customer_id: Option<i64>,
    pub customer_name: String,
    pub total_amount: f64,
    pub paid_amount: f64,
    pub unpaid_amount: f64,
    pub sign_date: String,
    pub start_date: String,
    pub end_date: String,
    pub payment_method: String,
    pub payment_terms: String,
    pub brand_ids: String,
    pub fulfillment_status: String,
    pub status: String,
    pub file_path: String,
    pub notes: String,
    pub handled_by: Option<i64>,
    pub handler_name: String,
    pub payments: Vec<PaymentRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentRecord {
    pub id: i64,
    pub contract_id: i64,
    pub amount: f64,
    pub payment_date: String,
    pub payment_method: String,
    pub notes: String,
}

// ── 列表查询 ──────────────────────────────────────────

#[tauri::command]
pub fn list_contracts(
    pool: State<'_, DbPool>,
    token: Option<String>,
    direction: Option<String>,
    status: Option<String>,
    search: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<serde_json::Value, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;

    let mut where_clauses = vec!["c.is_deleted = 0".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref d) = direction {
        where_clauses.push(format!("c.direction = ?{}", params.len() + 1));
        params.push(Box::new(d.clone()));
    }
    if let Some(ref s) = status {
        where_clauses.push(format!("c.status = ?{}", params.len() + 1));
        params.push(Box::new(s.clone()));
    }
    if let Some(ref kw) = search {
        let idx = params.len() + 1;
        where_clauses.push(format!(
            "(c.name LIKE ?{0} OR c.contract_code LIKE ?{0})",
            idx
        ));
        params.push(Box::new(format!("%{}%", kw)));
    }

    let where_sql = where_clauses.join(" AND ");

    let count_sql = format!(
        "SELECT COUNT(*) FROM contracts c WHERE {}",
        where_sql
    );
    let total: i64 = {
        let mut stmt = conn.prepare(&count_sql).map_err(|e| e.to_string())?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        stmt.query_row(param_refs.as_slice(), |row| row.get(0))
            .map_err(|e| e.to_string())?
    };

    let query_sql = format!(
        "SELECT c.id, c.contract_code, c.name, c.direction,
                c.supplier_id, COALESCE(s.name,'') AS supplier_name,
                c.customer_id, COALESCE(cu.name,'') AS customer_name,
                c.total_amount, c.paid_amount,
                c.total_amount - c.paid_amount AS unpaid_amount,
                c.sign_date, c.start_date, c.end_date,
                c.payment_method, c.payment_terms, c.brand_ids,
                c.fulfillment_status, c.status, c.file_path, c.notes,
                c.handled_by, COALESCE(u.display_name,'') AS handler_name
         FROM contracts c
         LEFT JOIN suppliers s ON c.supplier_id = s.id
         LEFT JOIN customers cu ON c.customer_id = cu.id
         LEFT JOIN users u ON c.handled_by = u.id
         WHERE {}
         ORDER BY c.id DESC
         LIMIT ?{} OFFSET ?{}",
        where_sql,
        params.len() + 1,
        params.len() + 2
    );

    params.push(Box::new(page_size));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&query_sql).map_err(|e| e.to_string())?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let items: Vec<Contract> = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok(Contract {
                id: row.get(0)?,
                contract_code: row.get(1)?,
                name: row.get(2)?,
                direction: row.get(3)?,
                supplier_id: row.get(4)?,
                supplier_name: row.get(5)?,
                customer_id: row.get(6)?,
                customer_name: row.get(7)?,
                total_amount: row.get(8)?,
                paid_amount: row.get(9)?,
                unpaid_amount: row.get(10)?,
                sign_date: row.get(11)?,
                start_date: row.get(12)?,
                end_date: row.get(13)?,
                payment_method: row.get(14)?,
                payment_terms: row.get(15)?,
                brand_ids: row.get(16)?,
                fulfillment_status: row.get(17)?,
                status: row.get(18)?,
                file_path: row.get(19)?,
                notes: row.get(20)?,
                handled_by: row.get(21)?,
                handler_name: row.get(22)?,
                payments: vec![],
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "page_size": page_size
    }))
}

// ── 详情查询（含收付款记录）───────────────────────────

/// 内部查询（不验证权限，由调用方保证已验权）
fn get_contract_inner(conn: &rusqlite::Connection, id: i64) -> Result<Contract, String> {
    let contract = conn
        .query_row(
            "SELECT c.id, c.contract_code, c.name, c.direction,
                    c.supplier_id, COALESCE(s.name,''),
                    c.customer_id, COALESCE(cu.name,''),
                    c.total_amount, c.paid_amount,
                    c.total_amount - c.paid_amount,
                    c.sign_date, c.start_date, c.end_date,
                    c.payment_method, c.payment_terms, c.brand_ids,
                    c.fulfillment_status, c.status, c.file_path, c.notes,
                    c.handled_by, COALESCE(u.display_name,'')
             FROM contracts c
             LEFT JOIN suppliers s ON c.supplier_id = s.id
             LEFT JOIN customers cu ON c.customer_id = cu.id
             LEFT JOIN users u ON c.handled_by = u.id
             WHERE c.id = ?1 AND c.is_deleted = 0",
            [id],
            |row| {
                Ok(Contract {
                    id: row.get(0)?,
                    contract_code: row.get(1)?,
                    name: row.get(2)?,
                    direction: row.get(3)?,
                    supplier_id: row.get(4)?,
                    supplier_name: row.get(5)?,
                    customer_id: row.get(6)?,
                    customer_name: row.get(7)?,
                    total_amount: row.get(8)?,
                    paid_amount: row.get(9)?,
                    unpaid_amount: row.get(10)?,
                    sign_date: row.get(11)?,
                    start_date: row.get(12)?,
                    end_date: row.get(13)?,
                    payment_method: row.get(14)?,
                    payment_terms: row.get(15)?,
                    brand_ids: row.get(16)?,
                    fulfillment_status: row.get(17)?,
                    status: row.get(18)?,
                    file_path: row.get(19)?,
                    notes: row.get(20)?,
                    handled_by: row.get(21)?,
                    handler_name: row.get(22)?,
                    payments: vec![],
                })
            },
        )
        .map_err(|e| format!("合同不存在: {}", e))?;

    // 查询收付款记录
    let mut stmt = conn
        .prepare(
            "SELECT id, contract_id, amount, payment_date, payment_method, notes
             FROM payment_records WHERE contract_id = ?1 ORDER BY payment_date DESC",
        )
        .map_err(|e| e.to_string())?;

    let payments: Vec<PaymentRecord> = stmt
        .query_map([id], |row| {
            Ok(PaymentRecord {
                id: row.get(0)?,
                contract_id: row.get(1)?,
                amount: row.get(2)?,
                payment_date: row.get(3)?,
                payment_method: row.get(4)?,
                notes: row.get::<_, String>(5).unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(Contract {
        payments,
        ..contract
    })
}

/// 公开 Tauri 命令：需登录验证
#[tauri::command]
pub fn get_contract(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
) -> Result<Contract, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;
    get_contract_inner(&conn, id)
}

// ── 创建合同 ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateContractInput {
    pub name: String,
    pub direction: String,
    pub supplier_id: Option<i64>,
    pub customer_id: Option<i64>,
    pub total_amount: f64,
    pub sign_date: String,
    pub start_date: String,
    pub end_date: String,
    pub payment_method: String,
    pub payment_terms: String,
    pub brand_ids: String,
    pub file_path: String,
    pub notes: String,
}

#[tauri::command]
pub fn create_contract(
    pool: State<'_, DbPool>,
    token: Option<String>,
    input: CreateContractInput,
) -> Result<Contract, String> {
    let user = verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;

    validate_string(&input.name, "合同名称", 200)?;
    validate_direction(&input.direction)?;
    validate_non_negative_f64(input.total_amount, "合同金额")?;
    validate_date(&input.sign_date, "签订日期")?;
    validate_date(&input.start_date, "开始日期")?;
    validate_date(&input.end_date, "结束日期")?;
    validate_date_range(&input.start_date, &input.end_date)?;
    validate_non_empty(&input.payment_method, "付款方式")?;
    validate_max_len(&input.notes, 1000, "备注")?;
    if input.direction == "purchase" {
        validate_option_positive_i64(&input.supplier_id, "供应商")?;
    }
    if input.direction == "sales" {
        validate_option_positive_i64(&input.customer_id, "客户")?;
    }

    // 校验方向与关联方一致性
    if input.direction == "purchase" && input.supplier_id.is_none() {
        return Err("采购合同必须关联供应商".into());
    }
    if input.direction == "sales" && input.customer_id.is_none() {
        return Err("销售合同必须关联客户".into());
    }

    // 生成合同编号: CT-purchase/sales-YYYYMMDD-NNNN
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    let prefix = format!(
        "CT-{}-{}",
        if input.direction == "purchase" { "CG" } else { "XS" },
        today
    );

    let seq: i64 = conn
        .query_row(
            "SELECT COUNT(*) + 1 FROM contracts WHERE contract_code LIKE ?1",
            [format!("{}%", prefix)],
            |row| row.get(0),
        )
        .unwrap_or(1);

    let contract_code = format!("{}-{:04}", prefix, seq);
    let brand_ids = if input.brand_ids.is_empty() {
        "[]".to_string()
    } else {
        input.brand_ids
    };

    conn.execute(
        "INSERT INTO contracts (contract_code, name, direction,
         supplier_id, customer_id, total_amount, sign_date, start_date, end_date,
         payment_method, payment_terms, brand_ids, file_path, notes, handled_by)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        rusqlite::params![
            contract_code,
            input.name,
            input.direction,
            input.supplier_id,
            input.customer_id,
            input.total_amount,
            input.sign_date,
            input.start_date,
            input.end_date,
            input.payment_method,
            input.payment_terms,
            brand_ids,
            input.file_path,
            input.notes,
            user.id,
        ],
    )
    .map_err(|e| format!("创建合同失败: {}", e))?;

    let new_id = conn.last_insert_rowid();
    get_contract_inner(&conn, new_id)
}

// ── 更新合同 ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UpdateContractInput {
    pub name: Option<String>,
    pub direction: Option<String>,
    pub supplier_id: Option<i64>,
    pub customer_id: Option<i64>,
    pub total_amount: Option<f64>,
    pub sign_date: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub payment_method: Option<String>,
    pub payment_terms: Option<String>,
    pub brand_ids: Option<String>,
    pub fulfillment_status: Option<String>,
    pub file_path: Option<String>,
    pub notes: Option<String>,
    pub handled_by: Option<i64>,
}

#[tauri::command]
pub fn update_contract(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
    input: UpdateContractInput,
) -> Result<Contract, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_positive_i64(id, "合同ID")?;
    let conn = pool.lock().map_err(|e| e.to_string())?;

    let mut sets = vec!["updated_at = datetime('now','localtime')".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref v) = input.name {
        validate_string(v, "合同名称", 200)?;
        sets.push(format!("name = ?{}", params.len() + 1));
        params.push(Box::new(v.clone()));
    }
    if let Some(ref v) = input.direction {
        validate_direction(v)?;
        sets.push(format!("direction = ?{}", params.len() + 1));
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = input.supplier_id {
        validate_positive_i64(v, "供应商")?;
        sets.push(format!("supplier_id = ?{}", params.len() + 1));
        params.push(Box::new(v));
    }
    if let Some(v) = input.customer_id {
        validate_positive_i64(v, "客户")?;
        sets.push(format!("customer_id = ?{}", params.len() + 1));
        params.push(Box::new(v));
    }
    if let Some(v) = input.total_amount {
        validate_non_negative_f64(v, "合同金额")?;
        sets.push(format!("total_amount = ?{}", params.len() + 1));
        params.push(Box::new(v));
    }
    if let Some(ref v) = input.sign_date {
        validate_date(v, "签订日期")?;
        sets.push(format!("sign_date = ?{}", params.len() + 1));
        params.push(Box::new(v.clone()));
    }
    if let Some(ref v) = input.start_date {
        validate_date(v, "开始日期")?;
        sets.push(format!("start_date = ?{}", params.len() + 1));
        params.push(Box::new(v.clone()));
    }
    if let Some(ref v) = input.end_date {
        validate_date(v, "结束日期")?;
        sets.push(format!("end_date = ?{}", params.len() + 1));
        params.push(Box::new(v.clone()));
    }
    if let Some(ref v) = input.payment_method {
        validate_non_empty(v, "付款方式")?;
        sets.push(format!("payment_method = ?{}", params.len() + 1));
        params.push(Box::new(v.clone()));
    }
    if let Some(ref v) = input.payment_terms {
        sets.push(format!("payment_terms = ?{}", params.len() + 1));
        params.push(Box::new(v.clone()));
    }
    if let Some(ref v) = input.brand_ids {
        sets.push(format!("brand_ids = ?{}", params.len() + 1));
        params.push(Box::new(v.clone()));
    }
    if let Some(ref v) = input.fulfillment_status {
        sets.push(format!("fulfillment_status = ?{}", params.len() + 1));
        params.push(Box::new(v.clone()));
    }
    if let Some(ref v) = input.file_path {
        sets.push(format!("file_path = ?{}", params.len() + 1));
        params.push(Box::new(v.clone()));
    }
    if let Some(ref v) = input.notes {
        validate_max_len(v, 1000, "备注")?;
        sets.push(format!("notes = ?{}", params.len() + 1));
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = input.handled_by {
        validate_positive_i64(v, "经办人")?;
        sets.push(format!("handled_by = ?{}", params.len() + 1));
        params.push(Box::new(v));
    }

    if sets.len() == 1 {
        return Err("没有需要更新的字段".into());
    }

    // 交叉校验：direction 与 supplier_id/customer_id 的一致性
    // 首先读取当前合同的 direction（如果用户没有提供新的 direction）
    let effective_direction = if let Some(ref d) = input.direction {
        d.clone()
    } else {
        conn.query_row(
            "SELECT direction FROM contracts WHERE id=?1 AND is_deleted=0",
            [id], |r| r.get(0),
        ).map_err(|_| "合同不存在".to_string())?
    };
    if effective_direction == "purchase" && input.customer_id.is_some() {
        return Err("采购合同不能关联客户".into());
    }
    if effective_direction == "sales" && input.supplier_id.is_some() {
        return Err("销售合同不能关联供应商".into());
    }

    params.push(Box::new(id));
    let sql = format!(
        "UPDATE contracts SET {} WHERE id = ?{} AND is_deleted = 0",
        sets.join(", "),
        params.len()
    );

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn
        .execute(&sql, param_refs.as_slice())
        .map_err(|e| format!("更新合同失败: {}", e))?;

    if affected == 0 {
        return Err("合同不存在或已删除".into());
    }

    get_contract_inner(&conn, id)
}

// ── 软删除 ────────────────────────────────────────────

#[tauri::command]
pub fn delete_contract(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
) -> Result<(), String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;
    let affected = conn
        .execute(
            "UPDATE contracts SET is_deleted = 1, updated_at = datetime('now','localtime')
             WHERE id = ?1 AND is_deleted = 0",
            [id],
        )
        .map_err(|e| e.to_string())?;
    if affected == 0 {
        return Err("合同不存在或已删除".into());
    }
    Ok(())
}

// ── 状态流转 ──────────────────────────────────────────

#[tauri::command]
pub fn change_contract_status(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
    new_status: String,
) -> Result<Contract, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_positive_i64(id, "合同ID")?;
    let conn = pool.lock().map_err(|e| e.to_string())?;

    // 获取当前状态
    let current_status: String = conn
        .query_row(
            "SELECT status FROM contracts WHERE id = ?1 AND is_deleted = 0",
            [id],
            |row| row.get(0),
        )
        .map_err(|e| format!("合同不存在: {}", e))?;

    // 状态流转规则
    let valid = match (current_status.as_str(), new_status.as_str()) {
        ("draft", "active") => true,
        ("active", "fulfilling") => true,
        ("active", "terminated") => true,
        ("fulfilling", "completed") => true,
        ("fulfilling", "terminated") => true,
        ("active", "expired") => true,
        ("fulfilling", "expired") => true,
        _ => false,
    };

    if !valid {
        return Err(format!(
            "不允许从 {} 变更为 {}",
            current_status, new_status
        ));
    }

    conn.execute(
        "UPDATE contracts SET status = ?1, updated_at = datetime('now','localtime')
         WHERE id = ?2 AND is_deleted = 0",
        rusqlite::params![new_status, id],
    )
    .map_err(|e| e.to_string())?;

    get_contract_inner(&conn, id)
}

// ── 添加收付款记录 ────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AddPaymentInput {
    pub contract_id: i64,
    pub amount: f64,
    pub payment_date: String,
    pub payment_method: String,
    pub notes: String,
}

#[tauri::command]
pub fn add_payment_record(
    pool: State<'_, DbPool>,
    token: Option<String>,
    input: AddPaymentInput,
) -> Result<Contract, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_positive_i64(input.contract_id, "合同ID")?;
    validate_date(&input.payment_date, "付款日期")?;
    validate_non_empty(&input.payment_method, "付款方式")?;
    validate_max_len(&input.notes, 500, "备注")?;
    if input.amount <= 0.0 {
        return Err("付款金额必须大于0".into());
    }

    let conn = pool.lock().map_err(|e| e.to_string())?;

    // 检查是否超额付款
    let (total, paid): (f64, f64) = conn.query_row(
        "SELECT total_amount, paid_amount FROM contracts WHERE id=?1 AND is_deleted=0",
        [input.contract_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| format!("合同不存在: {e}"))?;

    if paid + input.amount > total + 0.01 {
        return Err(format!(
            "付款金额超出合同余额：合同总金额=¥{:.2}，已付=¥{:.2}，本次付款=¥{:.2}，超出=¥{:.2}",
            total, paid, input.amount, paid + input.amount - total
        ));
    }

    // 插入付款记录
    conn.execute(
        "INSERT INTO payment_records (contract_id, amount, payment_date, payment_method, notes)
         VALUES (?1,?2,?3,?4,?5)",
        rusqlite::params![
            input.contract_id,
            input.amount,
            input.payment_date,
            input.payment_method,
            input.notes,
        ],
    )
    .map_err(|e| format!("添加付款记录失败: {}", e))?;

    // 更新合同已付金额
    conn.execute(
        "UPDATE contracts SET paid_amount = (
            SELECT COALESCE(SUM(amount), 0) FROM payment_records WHERE contract_id = ?1
        ), updated_at = datetime('now','localtime')
        WHERE id = ?1",
        [input.contract_id],
    )
    .map_err(|e| e.to_string())?;

    // 自动更新履行状态
    let (total, paid): (f64, f64) = conn
        .query_row(
            "SELECT total_amount, paid_amount FROM contracts WHERE id = ?1",
            [input.contract_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let new_fulfillment = if paid >= total {
        "completed"
    } else if paid > 0.0 {
        "partial"
    } else {
        "pending"
    };

    conn.execute(
        "UPDATE contracts SET fulfillment_status = ?1 WHERE id = ?2",
        rusqlite::params![new_fulfillment, input.contract_id],
    )
    .map_err(|e| e.to_string())?;

    let contract_id = input.contract_id;
    get_contract_inner(&conn, contract_id)
}

// ── 删除收付款记录 ────────────────────────────────────

#[tauri::command]
pub fn delete_payment_record(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
) -> Result<Contract, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_positive_i64(id, "付款记录ID")?;
    let conn = pool.lock().map_err(|e| e.to_string())?;

    let contract_id: i64 = conn
        .query_row(
            "SELECT contract_id FROM payment_records WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|e| format!("付款记录不存在: {}", e))?;

    conn.execute("DELETE FROM payment_records WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;

    // 重新计算已付金额
    conn.execute(
        "UPDATE contracts SET paid_amount = (
            SELECT COALESCE(SUM(amount), 0) FROM payment_records WHERE contract_id = ?1
        ), updated_at = datetime('now','localtime')
        WHERE id = ?1",
        [contract_id],
    )
    .map_err(|e| e.to_string())?;

    // 更新履行状态
    let (total, paid): (f64, f64) = conn
        .query_row(
            "SELECT total_amount, paid_amount FROM contracts WHERE id = ?1",
            [contract_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let new_fulfillment = if paid >= total {
        "completed"
    } else if paid > 0.0 {
        "partial"
    } else {
        "pending"
    };

    conn.execute(
        "UPDATE contracts SET fulfillment_status = ?1 WHERE id = ?2",
        rusqlite::params![new_fulfillment, contract_id],
    )
    .map_err(|e| e.to_string())?;

    get_contract_inner(&conn, contract_id)
}
