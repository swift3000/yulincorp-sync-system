//! 产品管理命令 — 完整 CRUD + 关联品牌/分类/JOIN查询

use crate::commands::auth::{verify_auth, AllowRoles};
use crate::db::DbPool;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub sku: String,
    pub brand_id: Option<i64>,
    pub brand_name: Option<String>,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub unit: String,
    pub spec: String,
    pub purchase_price: f64,
    pub sale_price: f64,
    pub min_stock: i64,
    pub is_active: bool,
    pub notes: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProductListResponse {
    pub products: Vec<Product>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

fn row_to_product(row: &rusqlite::Row<'_>) -> rusqlite::Result<Product> {
    Ok(Product {
        id: row.get(0)?,
        name: row.get(1)?,
        sku: row.get(2)?,
        brand_id: row.get(3)?,
        brand_name: row.get(4)?,
        category_id: row.get(5)?,
        category_name: row.get(6)?,
        unit: row.get(7)?,
        spec: row.get(8)?,
        purchase_price: row.get(9)?,
        sale_price: row.get(10)?,
        min_stock: row.get(11)?,
        is_active: row.get(12)?,
        notes: row.get(13)?,
        created_at: row.get(14)?,
    })
}

#[tauri::command]
pub fn list_products(
    pool: State<'_, DbPool>,
    token: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
    keyword: Option<String>,
    brand_id: Option<i64>,
    category_id: Option<i64>,
) -> Result<ProductListResponse, String> {
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;

    // 动态构建 WHERE 子句
    let mut conditions = vec!["p.is_active = 1".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

    if let Some(kw) = &keyword {
        if !kw.is_empty() {
            conditions.push(format!("(p.name LIKE ?{} OR p.sku LIKE ?{})", params.len() + 1, params.len() + 2));
            let pattern = format!("%{kw}%");
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern));
        }
    }
    if let Some(bid) = brand_id {
        conditions.push(format!("p.brand_id = ?{}", params.len() + 1));
        params.push(Box::new(bid));
    }
    if let Some(cid) = category_id {
        conditions.push(format!("p.category_id = ?{}", params.len() + 1));
        params.push(Box::new(cid));
    }

    let where_clause = conditions.join(" AND ");

    // 查询总数
    let count_sql = format!(
        "SELECT COUNT(*) FROM products p WHERE {where_clause}"
    );
    let total: i64 = {
        let mut stmt = conn.prepare(&count_sql).map_err(|e| format!("SQL错误: {e}"))?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        stmt.query_row(param_refs.as_slice(), |row| row.get(0)).unwrap_or(0)
    };

    // 查询数据（JOIN 品牌和分类，一次搞定）
    let data_sql = format!(
        "SELECT p.id, p.name, p.sku, p.brand_id, b.name, p.category_id, c.name,
                p.unit, p.spec, p.purchase_price, p.sale_price, p.min_stock,
                p.is_active, p.notes, p.created_at
         FROM products p
         LEFT JOIN brands b ON p.brand_id = b.id
         LEFT JOIN categories c ON p.category_id = c.id
         WHERE {where_clause}
         ORDER BY p.id DESC LIMIT ?{} OFFSET ?{}",
        params.len() + 1,
        params.len() + 2,
    );
    params.push(Box::new(page_size));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&data_sql).map_err(|e| format!("SQL错误: {e}"))?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let products: Vec<Product> = stmt
        .query_map(param_refs.as_slice(), row_to_product)
        .map_err(|e| format!("查询错误: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(ProductListResponse { products, total, page, page_size })
}

#[tauri::command]
pub fn get_product(pool: State<'_, DbPool>, token: Option<String>, id: i64) -> Result<Option<Product>, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;
    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    conn.query_row(
        "SELECT p.id, p.name, p.sku, p.brand_id, b.name, p.category_id, c.name,
                p.unit, p.spec, p.purchase_price, p.sale_price, p.min_stock,
                p.is_active, p.notes, p.created_at
         FROM products p
         LEFT JOIN brands b ON p.brand_id = b.id
         LEFT JOIN categories c ON p.category_id = c.id
         WHERE p.id=?1",
        [id],
        row_to_product,
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        _ => Err(format!("查询错误: {e}")),
    })
}

#[derive(Debug, Deserialize)]
pub struct CreateProductRequest {
    pub name: String,
    pub sku: String,
    pub brand_id: Option<i64>,
    pub category_id: Option<i64>,
    pub unit: Option<String>,
    pub spec: Option<String>,
    pub purchase_price: Option<f64>,
    pub sale_price: Option<f64>,
    pub min_stock: Option<i64>,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn create_product(
    pool: State<'_, DbPool>,
    token: Option<String>,
    data: CreateProductRequest,
) -> Result<Product, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;

    if data.name.len() > 256 || data.sku.len() > 64 {
        return Err("产品名称或SKU长度超出限制".into());
    }

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;

    // 检查 SKU 唯一性
    let exists: bool = conn
        .query_row("SELECT COUNT(*) FROM products WHERE sku=?1", [&data.sku], |row| row.get::<_, i64>(0))
        .map(|c| c > 0)
        .unwrap_or(false);
    if exists {
        return Err("SKU已存在".into());
    }

    conn.execute(
        "INSERT INTO products (name,sku,brand_id,category_id,unit,spec,purchase_price,sale_price,min_stock,notes)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        rusqlite::params![
            data.name, data.sku, data.brand_id, data.category_id,
            data.unit.unwrap_or_else(|| "个".into()), data.spec.unwrap_or_default(),
            data.purchase_price.unwrap_or(0.0), data.sale_price.unwrap_or(0.0),
            data.min_stock.unwrap_or(0), data.notes.unwrap_or_default(),
        ],
    )
    .map_err(|e| format!("创建失败: {e}"))?;

    let id = conn.last_insert_rowid();
    log::info!("产品已创建: {} (id={})", data.name, id);

    conn.query_row(
        "SELECT p.id, p.name, p.sku, p.brand_id, b.name, p.category_id, c.name,
                p.unit, p.spec, p.purchase_price, p.sale_price, p.min_stock,
                p.is_active, p.notes, p.created_at
         FROM products p
         LEFT JOIN brands b ON p.brand_id = b.id
         LEFT JOIN categories c ON p.category_id = c.id
         WHERE p.id=?1",
        [id],
        row_to_product,
    )
    .map_err(|e| format!("创建后读取失败: {e}"))
}

#[derive(Debug, Deserialize)]
pub struct UpdateProductRequest {
    pub name: Option<String>,
    pub sku: Option<String>,
    pub brand_id: Option<i64>,
    pub category_id: Option<i64>,
    pub unit: Option<String>,
    pub spec: Option<String>,
    pub purchase_price: Option<f64>,
    pub sale_price: Option<f64>,
    pub min_stock: Option<i64>,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn update_product(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
    data: UpdateProductRequest,
) -> Result<Product, String> {
    verify_auth(&pool, &token, AllowRoles::Any)?;

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;

    // 动态构建 UPDATE（只更新提供的字段）
    let mut sets = vec!["updated_at = datetime('now','localtime')".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

    if let Some(v) = &data.name { sets.push(format!("name = ?{}", params.len() + 1)); params.push(Box::new(v.clone())); }
    if let Some(v) = &data.sku { sets.push(format!("sku = ?{}", params.len() + 1)); params.push(Box::new(v.clone())); }
    if let Some(v) = data.brand_id { sets.push(format!("brand_id = ?{}", params.len() + 1)); params.push(Box::new(v)); }
    if let Some(v) = data.category_id { sets.push(format!("category_id = ?{}", params.len() + 1)); params.push(Box::new(v)); }
    if let Some(v) = &data.unit { sets.push(format!("unit = ?{}", params.len() + 1)); params.push(Box::new(v.clone())); }
    if let Some(v) = &data.spec { sets.push(format!("spec = ?{}", params.len() + 1)); params.push(Box::new(v.clone())); }
    if let Some(v) = data.purchase_price { sets.push(format!("purchase_price = ?{}", params.len() + 1)); params.push(Box::new(v)); }
    if let Some(v) = data.sale_price { sets.push(format!("sale_price = ?{}", params.len() + 1)); params.push(Box::new(v)); }
    if let Some(v) = data.min_stock { sets.push(format!("min_stock = ?{}", params.len() + 1)); params.push(Box::new(v)); }
    if let Some(v) = &data.notes { sets.push(format!("notes = ?{}", params.len() + 1)); params.push(Box::new(v.clone())); }

    params.push(Box::new(id));
    let sql = format!(
        "UPDATE products SET {} WHERE id = ?{}",
        sets.join(", "),
        params.len()
    );
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("更新失败: {e}"))?;
    if affected == 0 { return Err("产品不存在".into()); }

    conn.query_row(
        "SELECT p.id, p.name, p.sku, p.brand_id, b.name, p.category_id, c.name,
                p.unit, p.spec, p.purchase_price, p.sale_price, p.min_stock,
                p.is_active, p.notes, p.created_at
         FROM products p
         LEFT JOIN brands b ON p.brand_id = b.id
         LEFT JOIN categories c ON p.category_id = c.id
         WHERE p.id=?1",
        [id],
        row_to_product,
    )
    .map_err(|e| format!("更新后读取失败: {e}"))
}

#[tauri::command]
pub fn delete_product(
    pool: State<'_, DbPool>,
    token: Option<String>,
    id: i64,
) -> Result<(), String> {
    verify_auth(&pool, &token, AllowRoles::BossOrAdmin)?;

    let conn = pool.lock().map_err(|e| format!("数据库锁错误: {e}"))?;
    let affected = conn
        .execute("UPDATE products SET is_active=0,updated_at=datetime('now','localtime') WHERE id=?1", [id])
        .map_err(|e| format!("删除失败: {e}"))?;
    if affected == 0 { return Err("产品不存在".into()); }
    log::info!("产品已软删除: id={}", id);
    Ok(())
}
