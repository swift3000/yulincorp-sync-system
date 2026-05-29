//! 项目管理命令 — 全生命周期 + 阶段追踪 + 文档关联

use crate::commands::auth::{verify_auth, AllowRoles};
use crate::db::DbPool;
use serde::{Deserialize, Serialize};
use tauri::State;

// ── 数据结构 ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectPhase {
    pub id: i64,
    pub project_id: i64,
    pub phase_name: String,
    pub status: String,
    pub start_date: String,
    pub end_date: String,
    pub notes: String,
    pub sort_order: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectDocument {
    pub id: i64,
    pub project_id: i64,
    pub phase_id: Option<i64>,
    pub doc_name: String,
    pub doc_type: String,
    pub file_path: String,
    pub file_size: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub project_code: String,
    pub name: String,
    pub customer_id: Option<i64>,
    pub customer_name: String,
    pub status: String,
    pub budget: f64,
    pub actual_cost: f64,
    pub start_date: String,
    pub end_date: String,
    pub handled_by: Option<i64>,
    pub handler_name: String,
    pub notes: String,
    pub phases: Vec<ProjectPhase>,
    pub documents: Vec<ProjectDocument>,
}

// ── 列表 ──────────────────────────────────────────────

#[tauri::command]
pub fn list_projects(
    pool: State<'_, DbPool>,
    token: Option<String>,
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

    let mut where_clauses = vec!["p.is_deleted = 0".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref s) = status {
        where_clauses.push(format!("p.status = ?{}", params.len() + 1));
        params.push(Box::new(s.clone()));
    }
    if let Some(ref kw) = search {
        let idx = params.len() + 1;
        where_clauses.push(format!(
            "(p.name LIKE ?{0} OR p.project_code LIKE ?{0})", idx
        ));
        params.push(Box::new(format!("%{}%", kw)));
    }

    let where_sql = where_clauses.join(" AND ");
    let count_sql = format!("SELECT COUNT(*) FROM projects p WHERE {}", where_sql);
    let total: i64 = {
        let mut stmt = conn.prepare(&count_sql).map_err(|e| e.to_string())?;
        let pr: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        stmt.query_row(pr.as_slice(), |row| row.get(0)).map_err(|e| e.to_string())?
    };

    let query_sql = format!(
        "SELECT p.id, p.project_code, p.name, p.customer_id, COALESCE(c.name,''),
                p.status, p.budget, p.actual_cost, p.start_date, p.end_date,
                p.handled_by, COALESCE(u.display_name,''), p.notes
         FROM projects p
         LEFT JOIN customers c ON p.customer_id = c.id
         LEFT JOIN users u ON p.handled_by = u.id
         WHERE {} ORDER BY p.id DESC LIMIT ?{} OFFSET ?{}",
        where_sql,
        params.len() + 1,
        params.len() + 2
    );

    params.push(Box::new(page_size));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&query_sql).map_err(|e| e.to_string())?;
    let pr: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let items: Vec<Project> = stmt
        .query_map(pr.as_slice(), |row| {
            Ok(Project {
                id: row.get(0)?,
                project_code: row.get(1)?,
                name: row.get(2)?,
                customer_id: row.get(3)?,
                customer_name: row.get(4)?,
                status: row.get(5)?,
                budget: row.get(6)?,
                actual_cost: row.get(7)?,
                start_date: row.get(8)?,
                end_date: row.get(9)?,
                handled_by: row.get(10)?,
                handler_name: row.get(11)?,
                notes: row.get(12)?,
                phases: vec![],
                documents: vec![],
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "items": items, "total": total, "page": page, "page_size": page_size
    }))
}

// ── 详情（含阶段和文档）───────────────────────────────

#[tauri::command]
pub fn get_project(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
) -> Result<Project, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;
    get_project_inner(&conn, id)
}

fn get_project_inner(conn: &rusqlite::Connection, id: i64) -> Result<Project, String> {
    let mut project = conn
        .query_row(
            "SELECT p.id, p.project_code, p.name, p.customer_id, COALESCE(c.name,''),
                    p.status, p.budget, p.actual_cost, p.start_date, p.end_date,
                    p.handled_by, COALESCE(u.display_name,''), p.notes
             FROM projects p
             LEFT JOIN customers c ON p.customer_id = c.id
             LEFT JOIN users u ON p.handled_by = u.id
             WHERE p.id = ?1 AND p.is_deleted = 0",
            [id],
            |row| {
                Ok(Project {
                    id: row.get(0)?,
                    project_code: row.get(1)?,
                    name: row.get(2)?,
                    customer_id: row.get(3)?,
                    customer_name: row.get(4)?,
                    status: row.get(5)?,
                    budget: row.get(6)?,
                    actual_cost: row.get(7)?,
                    start_date: row.get(8)?,
                    end_date: row.get(9)?,
                    handled_by: row.get(10)?,
                    handler_name: row.get(11)?,
                    notes: row.get(12)?,
                    phases: vec![],
                    documents: vec![],
                })
            },
        )
        .map_err(|e| format!("项目不存在: {}", e))?;

    // 加载阶段
    let mut stmt = conn
        .prepare("SELECT id, project_id, phase_name, status, start_date, end_date, notes, sort_order FROM project_phases WHERE project_id = ?1 ORDER BY sort_order")
        .map_err(|e| e.to_string())?;
    project.phases = stmt
        .query_map([id], |row| {
            Ok(ProjectPhase {
                id: row.get(0)?, project_id: row.get(1)?, phase_name: row.get(2)?,
                status: row.get(3)?, start_date: row.get(4)?, end_date: row.get(5)?,
                notes: row.get(6)?, sort_order: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // 加载文档
    let mut stmt2 = conn
        .prepare("SELECT id, project_id, phase_id, doc_name, doc_type, file_path, file_size FROM project_documents WHERE project_id = ?1 ORDER BY id DESC")
        .map_err(|e| e.to_string())?;
    project.documents = stmt2
        .query_map([id], |row| {
            Ok(ProjectDocument {
                id: row.get(0)?, project_id: row.get(1)?, phase_id: row.get(2)?,
                doc_name: row.get(3)?, doc_type: row.get(4)?, file_path: row.get(5)?,
                file_size: row.get::<_, i64>(6).unwrap_or(0),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(project)
}

// ── 创建 ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateProjectInput {
    pub name: String,
    pub customer_id: Option<i64>,
    pub budget: f64,
    pub start_date: String,
    pub end_date: String,
    pub notes: String,
}

#[tauri::command]
pub fn create_project(
    pool: State<'_, DbPool>,
    token: Option<String>,
    input: CreateProjectInput,
) -> Result<Project, String> {
    let user = verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;

    let today = chrono::Local::now().format("%Y%m%d").to_string();
    let prefix = format!("PRJ-{}", today);
    let seq: i64 = conn
        .query_row(
            "SELECT COUNT(*) + 1 FROM projects WHERE project_code LIKE ?1",
            [format!("{}%", prefix)],
            |row| row.get(0),
        )
        .unwrap_or(1);
    let project_code = format!("{}-{:04}", prefix, seq);

    conn.execute(
        "INSERT INTO projects (project_code, name, customer_id, budget, start_date, end_date, handled_by, notes)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![
            project_code, input.name, input.customer_id, input.budget,
            input.start_date, input.end_date, user.id, input.notes,
        ],
    )
    .map_err(|e| format!("创建项目失败: {}", e))?;

    let new_id = conn.last_insert_rowid();

    // 自动创建默认阶段
    let phases = ["招投标", "方案设计", "采购执行", "施工实施", "验收交付", "维保服务"];
    for (i, name) in phases.iter().enumerate() {
        conn.execute(
            "INSERT INTO project_phases (project_id, phase_name, sort_order) VALUES (?1,?2,?3)",
            rusqlite::params![new_id, name, i as i64],
        )
        .ok();
    }

    get_project_inner(&conn, new_id)
}

// ── 更新 ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UpdateProjectInput {
    pub name: Option<String>,
    pub customer_id: Option<i64>,
    pub budget: Option<f64>,
    pub actual_cost: Option<f64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub handled_by: Option<i64>,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn update_project(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
    input: UpdateProjectInput,
) -> Result<Project, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;

    let mut sets = vec!["updated_at = datetime('now','localtime')".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref v) = input.name { sets.push(format!("name = ?{}", params.len()+1)); params.push(Box::new(v.clone())); }
    if let Some(v) = input.customer_id { sets.push(format!("customer_id = ?{}", params.len()+1)); params.push(Box::new(v)); }
    if let Some(v) = input.budget { sets.push(format!("budget = ?{}", params.len()+1)); params.push(Box::new(v)); }
    if let Some(v) = input.actual_cost { sets.push(format!("actual_cost = ?{}", params.len()+1)); params.push(Box::new(v)); }
    if let Some(ref v) = input.start_date { sets.push(format!("start_date = ?{}", params.len()+1)); params.push(Box::new(v.clone())); }
    if let Some(ref v) = input.end_date { sets.push(format!("end_date = ?{}", params.len()+1)); params.push(Box::new(v.clone())); }
    if let Some(v) = input.handled_by { sets.push(format!("handled_by = ?{}", params.len()+1)); params.push(Box::new(v)); }
    if let Some(ref v) = input.notes { sets.push(format!("notes = ?{}", params.len()+1)); params.push(Box::new(v.clone())); }

    if sets.len() == 1 { return Err("没有需要更新的字段".into()); }

    params.push(Box::new(id));
    let sql = format!("UPDATE projects SET {} WHERE id = ?{} AND is_deleted = 0", sets.join(", "), params.len());
    let pr: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, pr.as_slice()).map_err(|e| format!("更新失败: {}", e))?;
    if affected == 0 { return Err("项目不存在或已删除".into()); }

    get_project_inner(&conn, id)
}

// ── 软删除 ────────────────────────────────────────────

#[tauri::command]
pub fn delete_project(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
) -> Result<(), String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;
    let affected = conn.execute(
        "UPDATE projects SET is_deleted = 1, updated_at = datetime('now','localtime') WHERE id = ?1 AND is_deleted = 0",
        [id],
    ).map_err(|e| e.to_string())?;
    if affected == 0 { return Err("项目不存在或已删除".into()); }
    Ok(())
}

// ── 状态流转 ──────────────────────────────────────────

#[tauri::command]
pub fn change_project_status(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
    new_status: String,
) -> Result<Project, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;

    let current: String = conn
        .query_row("SELECT status FROM projects WHERE id = ?1 AND is_deleted = 0", [id], |row| row.get(0))
        .map_err(|e| format!("项目不存在: {}", e))?;

    let valid = match (current.as_str(), new_status.as_str()) {
        ("bidding", "design") | ("bidding", "cancelled") => true,
        ("design", "execution") | ("design", "cancelled") => true,
        ("execution", "delivery") | ("execution", "cancelled") => true,
        ("delivery", "maintenance") | ("delivery", "completed") | ("delivery", "cancelled") => true,
        ("maintenance", "completed") => true,
        _ => false,
    };

    if !valid { return Err(format!("不允许从 {} 变更为 {}", current, new_status)); }

    conn.execute("UPDATE projects SET status = ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
        rusqlite::params![new_status, id]).map_err(|e| e.to_string())?;

    get_project_inner(&conn, id)
}

// ── 阶段管理 ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UpdatePhaseInput {
    pub phase_name: Option<String>,
    pub status: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn update_project_phase(
    pool: State<'_, DbPool>,
    token: Option<String>,
    phase_id: i64,
    input: UpdatePhaseInput,
) -> Result<ProjectPhase, String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;

    let mut sets: Vec<String> = vec![];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref v) = input.phase_name { sets.push(format!("phase_name=?{}", params.len()+1)); params.push(Box::new(v.clone())); }
    if let Some(ref v) = input.status { sets.push(format!("status=?{}", params.len()+1)); params.push(Box::new(v.clone())); }
    if let Some(ref v) = input.start_date { sets.push(format!("start_date=?{}", params.len()+1)); params.push(Box::new(v.clone())); }
    if let Some(ref v) = input.end_date { sets.push(format!("end_date=?{}", params.len()+1)); params.push(Box::new(v.clone())); }
    if let Some(ref v) = input.notes { sets.push(format!("notes=?{}", params.len()+1)); params.push(Box::new(v.clone())); }

    if sets.is_empty() { return Err("没有需要更新的字段".into()); }

    params.push(Box::new(phase_id));
    let sql = format!("UPDATE project_phases SET {} WHERE id = ?{}", sets.join(","), params.len());
    let pr: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, pr.as_slice()).map_err(|e| format!("更新阶段失败: {}", e))?;

    let phase = conn.query_row(
        "SELECT id, project_id, phase_name, status, start_date, end_date, notes, sort_order FROM project_phases WHERE id = ?1",
        [phase_id],
        |row| Ok(ProjectPhase {
            id: row.get(0)?, project_id: row.get(1)?, phase_name: row.get(2)?,
            status: row.get(3)?, start_date: row.get(4)?, end_date: row.get(5)?,
            notes: row.get(6)?, sort_order: row.get(7)?,
        }),
    ).map_err(|e| e.to_string())?;

    Ok(phase)
}
