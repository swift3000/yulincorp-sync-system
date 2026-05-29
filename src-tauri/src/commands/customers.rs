//! 客户管理命令

use crate::commands::auth::{verify_auth, AllowRoles};
use crate::db::DbPool;
use crate::validators::*;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct Customer {
    pub id: i64,
    pub name: String,
    pub contact_person: String,
    pub phone: String,
    pub email: String,
    pub address: String,
    pub tax_id: String,
    pub notes: String,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CustomerListResponse {
    pub items: Vec<Customer>,
    pub total: i64,
}

fn row_to_customer(row: &rusqlite::Row<'_>) -> rusqlite::Result<Customer> {
    Ok(Customer {
        id: row.get(0)?,
        name: row.get(1)?,
        contact_person: row.get(2)?,
        phone: row.get(3)?,
        email: row.get(4)?,
        address: row.get(5)?,
        tax_id: row.get(6)?,
        notes: row.get(7)?,
        is_active: row.get(8)?,
        created_at: row.get(9)?,
    })
}

#[tauri::command]
pub fn list_customers(
    pool: State<'_, DbPool>,
    token: Option<String>,
    keyword: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<CustomerListResponse, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    validate_keyword(&keyword)?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;
    let columns = "id,name,contact_person,phone,email,address,tax_id,notes,is_active,created_at";

    let (total, items) = if let Some(ref kw) = keyword {
        let pattern = format!("%{kw}%");
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM customers WHERE name LIKE ?1 AND is_active = 1",
                [&pattern],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {columns} FROM customers WHERE name LIKE ?1 AND is_active = 1
                 ORDER BY id DESC LIMIT ?2 OFFSET ?3"
            ))
            .map_err(|e| format!("SQL错误: {e}"))?;
        let items: Vec<Customer> = stmt
            .query_map(rusqlite::params![pattern, page_size, offset], row_to_customer)
            .map_err(|e| format!("查询错误: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        (total, items)
    } else {
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM customers WHERE is_active = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {columns} FROM customers WHERE is_active = 1
                 ORDER BY id DESC LIMIT ?1 OFFSET ?2"
            ))
            .map_err(|e| format!("SQL错误: {e}"))?;
        let items: Vec<Customer> = stmt
            .query_map([page_size, offset], row_to_customer)
            .map_err(|e| format!("查询错误: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        (total, items)
    };

    Ok(CustomerListResponse { items, total })
}

#[tauri::command]
pub fn get_customer(pool: State<'_, DbPool>, token: Option<String>, id: i64) -> Result<Option<Customer>, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    conn.query_row(
        "SELECT id,name,contact_person,phone,email,address,tax_id,notes,is_active,created_at
         FROM customers WHERE id=?1",
        [id],
        row_to_customer,
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        _ => Err(format!("查询错误: {e}")),
    })
}

#[tauri::command]
pub fn create_customer(
    pool: State<'_, DbPool>,
    token: Option<String>,
    data: CreateCustomerRequest,
) -> Result<Customer, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    validate_string(&data.name, "客户名称", 100)?;
    validate_max_len(&data.contact_person, 50, "联系人")?;
    validate_max_len(&data.phone, 30, "电话")?;
    validate_max_len(&data.email, 100, "邮箱")?;
    validate_max_len(&data.address, 200, "地址")?;
    validate_max_len(&data.notes, 500, "备注")?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    conn.execute(
        "INSERT INTO customers (name,contact_person,phone,email,address,tax_id,notes)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![
            data.name, data.contact_person, data.phone, data.email,
            data.address, data.tax_id, data.notes,
        ],
    )
    .map_err(|e| format!("创建失败: {e}"))?;

    let id = conn.last_insert_rowid();
    log::info!("客户已创建: {} (id={})", data.name, id);

    conn.query_row(
        "SELECT id,name,contact_person,phone,email,address,tax_id,notes,is_active,created_at
         FROM customers WHERE id=?1",
        [id],
        row_to_customer,
    )
    .map_err(|e| format!("创建后读取失败: {e}"))
}

#[tauri::command]
pub fn update_customer(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
    data: UpdateCustomerRequest,
) -> Result<Customer, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_positive_i64(id, "客户ID")?;
    validate_string(&data.name, "客户名称", 100)?;
    validate_max_len(&data.contact_person, 50, "联系人")?;
    validate_max_len(&data.phone, 30, "电话")?;
    validate_max_len(&data.email, 100, "邮箱")?;
    validate_max_len(&data.address, 200, "地址")?;
    validate_max_len(&data.notes, 500, "备注")?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let affected = conn
        .execute(
            "UPDATE customers SET name=?1,contact_person=?2,phone=?3,email=?4,address=?5,
             tax_id=?6,notes=?7,updated_at=datetime('now','localtime') WHERE id=?8",
            rusqlite::params![
                data.name, data.contact_person, data.phone, data.email,
                data.address, data.tax_id, data.notes, id,
            ],
        )
        .map_err(|e| format!("更新失败: {e}"))?;
    if affected == 0 {
        return Err("客户不存在".into());
    }
    conn.query_row(
        "SELECT id,name,contact_person,phone,email,address,tax_id,notes,is_active,created_at
         FROM customers WHERE id=?1",
        [id],
        row_to_customer,
    )
    .map_err(|e| format!("更新后读取失败: {e}"))
}

#[tauri::command]
pub fn delete_customer(pool: State<'_, DbPool>, token: Option<String>, id: i64) -> Result<(), String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_positive_i64(id, "客户ID")?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let affected = conn
        .execute(
            "UPDATE customers SET is_active=0,updated_at=datetime('now','localtime') WHERE id=?1",
            [id],
        )
        .map_err(|e| format!("删除失败: {e}"))?;
    if affected == 0 {
        return Err("客户不存在".into());
    }
    log::info!("客户已软删除: id={}", id);
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct CreateCustomerRequest {
    pub name: String,
    #[serde(default)]
    pub contact_person: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub tax_id: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCustomerRequest {
    pub name: String,
    #[serde(default)]
    pub contact_person: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub tax_id: String,
    #[serde(default)]
    pub notes: String,
}
