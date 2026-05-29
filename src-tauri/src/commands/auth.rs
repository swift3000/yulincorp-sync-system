//! 认证命令：登录、登出、获取当前用户
//!
//! 安全措施：
//! - PBKDF2-SHA256 密码哈希（10万次迭代）
//! - UUID v4 session token
//! - 会话24小时过期

use crate::db::{self, DbPool};
use crate::validators::*;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub force_password_change: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
    pub user: Option<UserInfo>,
    pub token: Option<String>,
    pub must_change_password: bool,
}

/// 用户登录（PBKDF2验证 + 生成session token）
#[tauri::command]
pub fn login(pool: State<'_, DbPool>, req: LoginRequest) -> Result<LoginResponse, String> {
    validate_non_empty(&req.username, "用户名")?;
    validate_max_len(&req.username, 50, "用户名")?;
    validate_non_empty(&req.password, "密码")?;
    validate_max_len(&req.password, 100, "密码")?;

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;

    // 查询用户（统一错误消息，防止用户枚举）
    let mut stmt = conn
        .prepare("SELECT id, username, display_name, role, password_hash, is_active, force_password_change FROM users WHERE username = ?1")
        .map_err(|e| format!("SQL错误: {e}"))?;

    let user = stmt.query_row([&req.username], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, bool>(5)?,
            row.get::<_, bool>(6)?,
        ))
    });

    // 通用的认证失败响应（不区分账号不存在/密码错误/已禁用）
    let auth_fail = || LoginResponse {
        success: false,
        message: "账号或密码错误".into(),
        user: None,
        token: None,
        must_change_password: false,
    };

    match user {
        Err(_) => return Ok(auth_fail()),
        Ok((id, username, display_name, role, password_hash, is_active, force_pwd_change)) => {
            if !is_active {
                return Ok(auth_fail()); // 不再泄露用户存在信息
            }

            // PBKDF2 验证
            match db::verify_password(&req.password, &password_hash) {
                Ok(true) => {
                    // 生成 session token
                    let token = Uuid::new_v4().to_string();

                    // 清理过期 sessions + 插入新 session（一次写入）
                    conn.execute(
                        "DELETE FROM sessions WHERE user_id = ?1 AND expires_at < datetime('now','localtime')",
                        [&id],
                    )
                    .map_err(|e| format!("清理session失败: {e}"))?;

                    conn.execute(
                        "INSERT INTO sessions (user_id, token, expires_at) VALUES (?1, ?2, datetime('now','localtime','+24 hours'))",
                        rusqlite::params![id, token],
                    )
                    .map_err(|e| format!("创建session失败: {e}"))?;

                    log::info!("用户登录成功: {} ({})", username, role);

                    Ok(LoginResponse {
                        success: true,
                        message: "登录成功".into(),
                        user: Some(UserInfo {
                            id,
                            username,
                            display_name,
                            role,
                            force_password_change: force_pwd_change,
                        }),
                        token: Some(token),
                        must_change_password: force_pwd_change,
                    })
                }
                Ok(false) => Ok(auth_fail()),
                Err(e) => {
                    log::error!("密码验证失败: {}", e);
                    Ok(auth_fail())
                }
            }
        }
    }
}

/// 获取当前用户（通过 token 验证 session）
#[tauri::command]
pub fn get_current_user(
    pool: State<'_, DbPool>,
    token: Option<String>,
) -> Result<Option<UserInfo>, String> {
    let token = match token {
        Some(t) if !t.is_empty() => t,
        _ => return Ok(None),
    };

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;

    let mut stmt = conn
        .prepare(
        "SELECT u.id, u.username, u.display_name, u.role, u.force_password_change
         FROM users u
         JOIN sessions s ON u.id = s.user_id
         WHERE s.token = ?1 AND s.expires_at > datetime('now','localtime') AND u.is_active = 1",
        )
        .map_err(|e| format!("SQL错误: {e}"))?;

    match stmt.query_row([&token], |row| {
        Ok(UserInfo {
            id: row.get(0)?,
            username: row.get(1)?,
            display_name: row.get(2)?,
            role: row.get(3)?,
            force_password_change: row.get(4)?,
        })
    }) {
        Ok(user) => Ok(Some(user)),
        Err(_) => Ok(None), // token 无效或过期
    }
}

/// 用户登出（销毁 session）
#[tauri::command]
pub fn logout(pool: State<'_, DbPool>, token: Option<String>) -> Result<(), String> {
    if let Some(token) = token {
        let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
        conn.execute("DELETE FROM sessions WHERE token = ?1", [&token])
            .map_err(|e| format!("登出失败: {e}"))?;
    }
    Ok(())
}

// ── 用户管理（管理员功能）─────────────────────────────

#[derive(Debug, Serialize)]
pub struct UserRecord {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserInput {
    pub username: String,
    pub password: String,
    pub display_name: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserInput {
    pub display_name: Option<String>,
    pub role: Option<String>,
    pub password: Option<String>,
    pub is_active: Option<bool>,
}

/// 用户列表
#[tauri::command]
pub fn list_users(
    pool: State<'_, DbPool>,
    token: Option<String>,
) -> Result<Vec<UserRecord>, String> {
    verify_auth(&pool, &token, AllowRoles::AdminOnly)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, username, display_name, role, is_active, created_at FROM users ORDER BY id")
        .map_err(|e| e.to_string())?;
    let users = stmt
        .query_map([], |row| {
            Ok(UserRecord {
                id: row.get(0)?,
                username: row.get(1)?,
                display_name: row.get(2)?,
                role: row.get(3)?,
                is_active: row.get::<_, i64>(4)? == 1,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(users)
}

/// 创建用户
#[tauri::command]
pub fn create_user(
    pool: State<'_, DbPool>,
    token: Option<String>,
    input: CreateUserInput,
) -> Result<UserRecord, String> {
    verify_auth(&pool, &token, AllowRoles::AdminOnly)?;
    if input.username.is_empty() || input.password.is_empty() {
        return Err("用户名和密码不能为空".into());
    }
    validate_role(&input.role)?;
    validate_max_len(&input.username, 50, "用户名")?;
    validate_max_len(&input.password, 100, "密码")?;
    if input.password.len() < 6 {
        return Err("密码至少需要6个字符".into());
    }
    validate_non_empty(&input.display_name, "显示名称")?;
    validate_max_len(&input.display_name, 50, "显示名称")?;

    let conn = pool.lock().map_err(|e| e.to_string())?;

    // 检查用户名唯一性
    let exists: bool = conn
        .query_row("SELECT COUNT(*) FROM users WHERE username = ?1", [&input.username], |row| row.get::<_, i64>(0))
        .map(|c| c > 0)
        .unwrap_or(false);
    if exists { return Err("用户名已存在".into()); }

    let password_hash = db::hash_password(&input.password).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO users (username, password_hash, display_name, role) VALUES (?1,?2,?3,?4)",
        rusqlite::params![input.username, password_hash, input.display_name, input.role],
    ).map_err(|e| format!("创建用户失败: {}", e))?;

    let new_id = conn.last_insert_rowid();
    let user = conn.query_row(
        "SELECT id, username, display_name, role, is_active, created_at FROM users WHERE id = ?1",
        [new_id],
        |row| Ok(UserRecord {
            id: row.get(0)?, username: row.get(1)?, display_name: row.get(2)?,
            role: row.get(3)?, is_active: row.get::<_, i64>(4)? == 1, created_at: row.get(5)?,
        }),
    ).map_err(|e| e.to_string())?;
    Ok(user)
}

/// 更新用户
#[tauri::command]
pub fn update_user(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
    input: UpdateUserInput,
) -> Result<UserRecord, String> {
    verify_auth(&pool, &token, AllowRoles::AdminOnly)?;
    validate_positive_i64(id, "用户ID")?;
    if let Some(ref role) = input.role {
        validate_role(role)?;
    }
    if let Some(ref name) = input.display_name {
        validate_non_empty(name, "显示名称")?;
        validate_max_len(name, 50, "显示名称")?;
    }
    if let Some(ref pw) = input.password {
        if pw.len() < 6 {
            return Err("密码至少需要6个字符".into());
        }
        validate_max_len(pw, 100, "密码")?;
    }

    let conn = pool.lock().map_err(|e| e.to_string())?;

    let mut sets: Vec<String> = vec![];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref v) = input.display_name { sets.push(format!("display_name=?{}", params.len()+1)); params.push(Box::new(v.clone())); }
    if let Some(ref v) = input.role { sets.push(format!("role=?{}", params.len()+1)); params.push(Box::new(v.clone())); }
    if let Some(ref v) = input.password {
        let hash = db::hash_password(v).map_err(|e| e.to_string())?;
        sets.push(format!("password_hash=?{}", params.len()+1)); params.push(Box::new(hash));
    }
    if let Some(v) = input.is_active {
        sets.push(format!("is_active=?{}", params.len()+1)); params.push(Box::new(if v { 1i64 } else { 0i64 }));
    }

    if sets.is_empty() { return Err("没有需要更新的字段".into()); }

    params.push(Box::new(id));
    let sql = format!("UPDATE users SET {} WHERE id = ?{}", sets.join(","), params.len());
    let pr: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, pr.as_slice()).map_err(|e| format!("更新失败: {}", e))?;

    let user = conn.query_row(
        "SELECT id, username, display_name, role, is_active, created_at FROM users WHERE id = ?1",
        [id],
        |row| Ok(UserRecord {
            id: row.get(0)?, username: row.get(1)?, display_name: row.get(2)?,
            role: row.get(3)?, is_active: row.get::<_, i64>(4)? == 1, created_at: row.get(5)?,
        }),
    ).map_err(|e| e.to_string())?;
    Ok(user)
}

/// 删除用户（软删除，禁用账号）
#[tauri::command]
pub fn delete_user(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
) -> Result<(), String> {
    verify_auth(&pool, &token, AllowRoles::AdminOnly)?;
    validate_positive_i64(id, "用户ID")?;
    let conn = pool.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE users SET is_active = 0 WHERE id = ?1 AND id != 1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 获取系统信息
#[tauri::command]
pub fn get_system_info(
    pool: State<'_, DbPool>,
    token: Option<String>,
) -> Result<serde_json::Value, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| e.to_string())?;

    let user_count: i64 = conn.query_row("SELECT COUNT(*) FROM users WHERE is_active=1", [], |r| r.get(0)).unwrap_or(0);
    let db_size: i64 = conn.query_row("SELECT page_count * page_size FROM pragma_page_count, pragma_page_size", [], |r| r.get(0)).unwrap_or(0);

    Ok(serde_json::json!({
        "version": "0.1.0",
        "db_size_bytes": db_size,
        "user_count": user_count,
        "platform": std::env::consts::OS,
    }))
}

// ════════════════════════════════════════════════════════════
//  权限验证中间件 —— 所有敏感命令调用此函数确认身份
// ════════════════════════════════════════════════════════════

/// 允许的角色集合
#[derive(Clone)]
pub enum AllowRoles {
    /// 允许所有登录用户（employee / boss / admin）
    Any,
    /// 仅允许老板和管理员（boss / admin）
    BossOrAdmin,
    /// 仅管理员
    AdminOnly,
}

/// 统一权限验证：验证 token 有效性并检查角色
/// 返回 (user_id, user_info) 或错误信息
pub fn verify_auth(
    pool: &DbPool,
    token: &Option<String>,
    allow: AllowRoles,
) -> Result<UserInfo, String> {
    let token = token
        .as_ref()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "未登录，请先登录".to_string())?;

    let conn = pool.lock().map_err(|e| format!("数据库错误: {e}"))?;

    let user = conn
        .query_row(
            "SELECT u.id, u.username, u.display_name, u.role, u.force_password_change
             FROM users u
             JOIN sessions s ON u.id = s.user_id
             WHERE s.token = ?1 AND s.expires_at > datetime('now','localtime') AND u.is_active = 1",
            [token],
            |row| {
                Ok(UserInfo {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    display_name: row.get(2)?,
                    role: row.get(3)?,
                    force_password_change: row.get(4)?,
                })
            },
        )
        .map_err(|_| "登录已过期，请重新登录".to_string())?;

    // 检查角色权限
    let allowed = match &allow {
        AllowRoles::Any => true,
        AllowRoles::BossOrAdmin => user.role == "boss" || user.role == "admin",
        AllowRoles::AdminOnly => user.role == "admin",
    };

    if !allowed {
        return Err("权限不足，请联系管理员".into());
    }

    Ok(user)
}

/// 修改密码（支持首次登录强制改密）
#[tauri::command]
pub fn change_password(
    pool: State<'_, DbPool>,
    token: Option<String>,
    old_password: String,
    new_password: String,
) -> Result<(), String> {
    validate_non_empty(&old_password, "旧密码")?;

    if new_password.len() < 6 {
        return Err("新密码长度不能少于6位".into());
    }

    let user = verify_auth(&pool, &token, AllowRoles::Any)?;

    let conn = pool.lock().map_err(|e| format!("数据库错误: {e}"))?;

    // 非强制改密时验证旧密码
    if !user.force_password_change {
        let stored: String = conn
            .query_row(
                "SELECT password_hash FROM users WHERE id = ?1",
                [user.id],
                |row| row.get(0),
            )
            .map_err(|_| "用户不存在".to_string())?;

        if !db::verify_password(&old_password, &stored).unwrap_or(false) {
            return Err("旧密码错误".into());
        }
    }

    let new_hash = db::hash_password(&new_password).map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE users SET password_hash = ?1, force_password_change = 0, updated_at = datetime('now','localtime') WHERE id = ?2",
        rusqlite::params![new_hash, user.id],
    )
    .map_err(|e| format!("修改密码失败: {e}"))?;

    log::info!("用户 {} 密码已修改", user.username);
    Ok(())
}
