//! 分类管理命令

use crate::commands::auth::{verify_auth, AllowRoles};
use crate::db::DbPool;
use crate::validators::*;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub sort_order: i64,
    pub created_at: String,
}

fn row_to_category(row: &rusqlite::Row<'_>) -> rusqlite::Result<Category> {
    Ok(Category {
        id: row.get(0)?,
        name: row.get(1)?,
        parent_id: row.get(2)?,
        sort_order: row.get(3)?,
        created_at: row.get(4)?,
    })
}

/// 获取分类树（全部，带层级结构）
#[tauri::command]
pub fn list_categories(pool: State<'_, DbPool>, token: Option<String>) -> Result<Vec<Category>, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT id,name,parent_id,sort_order,created_at
             FROM categories ORDER BY sort_order, id ASC",
        )
        .map_err(|e| format!("SQL错误: {e}"))?;

    let items: Vec<Category> = stmt
        .query_map([], row_to_category)
        .map_err(|e| format!("查询错误: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(items)
}

#[tauri::command]
pub fn create_category(
    pool: State<'_, DbPool>,
    token: Option<String>,
    name: String,
    parent_id: Option<i64>,
) -> Result<Category, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    validate_string(&name, "分类名称", 50)?;
    validate_option_positive_i64(&parent_id, "父分类")?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    conn.execute(
        "INSERT INTO categories (name,parent_id) VALUES (?1,?2)",
        rusqlite::params![name, parent_id],
    )
    .map_err(|e| format!("创建失败: {e}"))?;

    let id = conn.last_insert_rowid();
    Ok(Category {
        id,
        name,
        parent_id,
        sort_order: 0,
        created_at: String::new(),
    })
}

#[tauri::command]
pub fn update_category(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
    name: String,
    parent_id: Option<i64>,
) -> Result<(), String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_positive_i64(id, "分类ID")?;
    validate_string(&name, "分类名称", 50)?;
    validate_option_positive_i64(&parent_id, "父分类")?;

    // 防止循环引用
    if let Some(pid) = parent_id {
        if pid == id {
            return Err("分类不能设为自己的子分类".into());
        }
        // 向上遍历父节点链，检查是否形成环
        let check_conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
        let mut current = pid;
        for _ in 0..100 {
            if current == id {
                return Err("不能将分类移到自己的子分类下（会形成循环引用）".into());
            }
            let parent: Option<i64> = check_conn.query_row(
                "SELECT parent_id FROM categories WHERE id = ?1",
                [current], |r| r.get(0),
            ).ok().flatten();
            match parent {
                Some(p) => current = p,
                None => break,
            }
        }
        drop(check_conn);
    }

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let affected = conn
        .execute(
            "UPDATE categories SET name=?1,parent_id=?2 WHERE id=?3",
            rusqlite::params![name, parent_id, id],
        )
        .map_err(|e| format!("更新失败: {e}"))?;
    if affected == 0 {
        return Err("分类不存在".into());
    }
    Ok(())
}

#[tauri::command]
pub fn delete_category(pool: State<'_, DbPool>, token: Option<String>, id: i64) -> Result<(), String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    validate_positive_i64(id, "分类ID")?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    // 检查是否有子分类
    let child_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM categories WHERE parent_id=?1",
            [id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if child_count > 0 {
        return Err("该分类下还有子分类，请先删除子分类".into());
    }
    conn.execute("DELETE FROM categories WHERE id=?1", [id])
        .map_err(|e| format!("删除失败: {e}"))?;
    Ok(())
}
