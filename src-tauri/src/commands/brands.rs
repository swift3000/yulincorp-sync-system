//! 品牌管理命令

use crate::commands::auth::{verify_auth, AllowRoles};
use crate::db::DbPool;
use crate::validators::*;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct Brand {
    pub id: i64,
    pub name: String,
    pub logo_path: String,
    pub website: String,
    pub description: String,
    pub is_active: bool,
    pub created_at: String,
}

fn row_to_brand(row: &rusqlite::Row<'_>) -> rusqlite::Result<Brand> {
    Ok(Brand {
        id: row.get(0)?,
        name: row.get(1)?,
        logo_path: row.get(2)?,
        website: row.get(3)?,
        description: row.get(4)?,
        is_active: row.get(5)?,
        created_at: row.get(6)?,
    })
}

#[tauri::command]
pub fn get_brand(pool: State<'_, DbPool>, token: Option<String>, id: i64) -> Result<Brand, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    validate_positive_i64(id, "品牌ID")?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let brand = conn.query_row(
        "SELECT id,name,logo_path,website,description,is_active,created_at FROM brands WHERE id=?1 AND is_active=1",
        [id],
        row_to_brand,
    ).map_err(|e| format!("品牌不存在: {e}"))?;
    Ok(brand)
}

#[tauri::command]
pub fn list_brands(pool: State<'_, DbPool>, token: Option<String>, keyword: Option<String>) -> Result<Vec<Brand>, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;

    let items: Vec<Brand> = if let Some(ref kw) = keyword {
        let pattern = format!("%{kw}%");
        let mut stmt = conn
            .prepare(
                "SELECT id,name,logo_path,website,description,is_active,created_at
                 FROM brands WHERE name LIKE ?1 AND is_active = 1 ORDER BY id ASC",
            )
            .map_err(|e| format!("SQL错误: {e}"))?;
        let result: Vec<Brand> = stmt
            .query_map([&pattern], row_to_brand)
            .map_err(|e| format!("查询错误: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        result
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id,name,logo_path,website,description,is_active,created_at
                 FROM brands WHERE is_active = 1 ORDER BY id ASC",
            )
            .map_err(|e| format!("SQL错误: {e}"))?;
        let result: Vec<Brand> = stmt
            .query_map([], row_to_brand)
            .map_err(|e| format!("查询错误: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };
    Ok(items)
}

#[tauri::command]
pub fn create_brand(pool: State<'_, DbPool>, token: Option<String>, name: String) -> Result<Brand, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    validate_string(&name, "品牌名称", 100)?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    conn.execute("INSERT INTO brands (name) VALUES (?1)", [&name])
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                "品牌名称已存在".to_string()
            } else {
                format!("创建失败: {e}")
            }
        })?;
    let id = conn.last_insert_rowid();
    log::info!("品牌已创建: {} (id={})", name, id);
    Ok(Brand {
        id,
        name,
        logo_path: String::new(),
        website: String::new(),
        description: String::new(),
        is_active: true,
        created_at: String::new(),
    })
}

#[tauri::command]
pub fn update_brand(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
    name: String,
    website: Option<String>,
    description: Option<String>,
) -> Result<(), String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_positive_i64(id, "品牌ID")?;
    validate_string(&name, "品牌名称", 100)?;
    validate_option_max_len(&website, 200, "网址")?;
    validate_option_max_len(&description, 500, "品牌描述")?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let affected = conn
        .execute(
            "UPDATE brands SET name=?1,website=?2,description=?3 WHERE id=?4",
            rusqlite::params![name, website.unwrap_or_default(), description.unwrap_or_default(), id],
        )
        .map_err(|e| format!("更新失败: {e}"))?;
    if affected == 0 {
        return Err("品牌不存在".into());
    }
    Ok(())
}

#[tauri::command]
pub fn delete_brand(pool: State<'_, DbPool>, token: Option<String>, id: i64) -> Result<(), String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_positive_i64(id, "品牌ID")?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    conn.execute(
        "UPDATE brands SET is_active=0 WHERE id=?1",
        [id],
    )
    .map_err(|e| format!("删除失败: {e}"))?;
    log::info!("品牌已软删除: id={}", id);
    Ok(())
}
