//! 供应商管理命令

use crate::commands::auth::{verify_auth, AllowRoles};
use crate::db::DbPool;
use crate::validators::*;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct Supplier {
    pub id: i64,
    pub name: String,
    pub contact_person: String,
    pub phone: String,
    pub email: String,
    pub address: String,
    pub bank_account: String,
    pub tax_id: String,
    pub notes: String,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SupplierListResponse {
    pub items: Vec<Supplier>,
    pub total: i64,
}

#[tauri::command]
pub fn list_suppliers(
    pool: State<'_, DbPool>,
    token: Option<String>,
    keyword: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<SupplierListResponse, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    validate_keyword(&keyword)?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;

    let (total, items) = if let Some(ref kw) = keyword {
        let pattern = format!("%{kw}%");
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM suppliers WHERE name LIKE ?1 AND is_active = 1",
                [&pattern],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let mut stmt = conn
            .prepare(
                "SELECT id,name,contact_person,phone,email,address,bank_account,tax_id,notes,is_active,created_at
                 FROM suppliers WHERE name LIKE ?1 AND is_active = 1
                 ORDER BY id DESC LIMIT ?2 OFFSET ?3",
            )
            .map_err(|e| format!("SQL错误: {e}"))?;

        let items = stmt
            .query_map(rusqlite::params![pattern, page_size, offset], row_to_supplier)
            .map_err(|e| format!("查询错误: {e}"))?
            .filter_map(|r| r.ok())
            .collect();

        (total, items)
    } else {
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM suppliers WHERE is_active = 1", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        let mut stmt = conn
            .prepare(
                "SELECT id,name,contact_person,phone,email,address,bank_account,tax_id,notes,is_active,created_at
                 FROM suppliers WHERE is_active = 1 ORDER BY id DESC LIMIT ?1 OFFSET ?2",
            )
            .map_err(|e| format!("SQL错误: {e}"))?;

        let items = stmt
            .query_map([page_size, offset], row_to_supplier)
            .map_err(|e| format!("查询错误: {e}"))?
            .filter_map(|r| r.ok())
            .collect();

        (total, items)
    };

    Ok(SupplierListResponse { items, total })
}

#[tauri::command]
pub fn get_supplier(pool: State<'_, DbPool>, token: Option<String>, id: i64) -> Result<Option<Supplier>, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    conn.query_row(
        "SELECT id,name,contact_person,phone,email,address,bank_account,tax_id,notes,is_active,created_at
         FROM suppliers WHERE id=?1",
        [id],
        row_to_supplier,
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        _ => Err(format!("查询错误: {e}")),
    })
}

#[tauri::command]
pub fn create_supplier(
    pool: State<'_, DbPool>,
    token: Option<String>,
    data: CreateSupplierRequest,
) -> Result<Supplier, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    validate_string(&data.name, "供应商名称", 100)?;
    validate_max_len(&data.contact_person, 50, "联系人")?;
    validate_max_len(&data.phone, 30, "电话")?;
    validate_max_len(&data.email, 100, "邮箱")?;
    validate_max_len(&data.address, 200, "地址")?;
    validate_max_len(&data.notes, 500, "备注")?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    conn.execute(
        "INSERT INTO suppliers (name,contact_person,phone,email,address,bank_account,tax_id,notes)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![
            data.name, data.contact_person, data.phone, data.email,
            data.address, data.bank_account, data.tax_id, data.notes,
        ],
    )
    .map_err(|e| format!("创建失败: {e}"))?;

    let id = conn.last_insert_rowid();
    log::info!("供应商已创建: {} (id={})", data.name, id);

    // 直接从当前连接读取（避免 borrow checker 冲突）
    conn.query_row(
        "SELECT id,name,contact_person,phone,email,address,bank_account,tax_id,notes,is_active,created_at
         FROM suppliers WHERE id=?1",
        [id],
        row_to_supplier,
    )
    .map_err(|e| format!("创建后读取失败: {e}"))
}

#[tauri::command]
pub fn update_supplier(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
    data: UpdateSupplierRequest,
) -> Result<Supplier, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_positive_i64(id, "供应商ID")?;
    validate_string(&data.name, "供应商名称", 100)?;
    validate_max_len(&data.contact_person, 50, "联系人")?;
    validate_max_len(&data.phone, 30, "电话")?;
    validate_max_len(&data.email, 100, "邮箱")?;
    validate_max_len(&data.address, 200, "地址")?;
    validate_max_len(&data.notes, 500, "备注")?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let affected = conn
        .execute(
            "UPDATE suppliers SET name=?1,contact_person=?2,phone=?3,email=?4,address=?5,
             bank_account=?6,tax_id=?7,notes=?8,updated_at=datetime('now','localtime')
             WHERE id=?9",
            rusqlite::params![
                data.name, data.contact_person, data.phone, data.email,
                data.address, data.bank_account, data.tax_id, data.notes, id,
            ],
        )
        .map_err(|e| format!("更新失败: {e}"))?;

    if affected == 0 {
        return Err("供应商不存在".into());
    }
    conn.query_row(
        "SELECT id,name,contact_person,phone,email,address,bank_account,tax_id,notes,is_active,created_at
         FROM suppliers WHERE id=?1",
        [id],
        row_to_supplier,
    )
    .map_err(|e| format!("更新后读取失败: {e}"))
}

#[tauri::command]
pub fn delete_supplier(pool: State<'_, DbPool>, token: Option<String>, id: i64) -> Result<(), String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_positive_i64(id, "供应商ID")?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let affected = conn
        .execute(
            "UPDATE suppliers SET is_active=0,updated_at=datetime('now','localtime') WHERE id=?1",
            [id],
        )
        .map_err(|e| format!("删除失败: {e}"))?;
    if affected == 0 {
        return Err("供应商不存在".into());
    }
    log::info!("供应商已软删除: id={}", id);
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct CreateSupplierRequest {
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
    pub bank_account: String,
    #[serde(default)]
    pub tax_id: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSupplierRequest {
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
    pub bank_account: String,
    #[serde(default)]
    pub tax_id: String,
    #[serde(default)]
    pub notes: String,
}

fn row_to_supplier(row: &rusqlite::Row<'_>) -> rusqlite::Result<Supplier> {
    Ok(Supplier {
        id: row.get(0)?,
        name: row.get(1)?,
        contact_person: row.get(2)?,
        phone: row.get(3)?,
        email: row.get(4)?,
        address: row.get(5)?,
        bank_account: row.get(6)?,
        tax_id: row.get(7)?,
        notes: row.get(8)?,
        is_active: row.get(9)?,
        created_at: row.get(10)?,
    })
}
