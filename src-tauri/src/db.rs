//! 数据库初始化与连接管理
//!
//! SQLite（rusqlite），应用启动创建表 + PBKDF2 密码哈希。

use ring::{pbkdf2, rand};
use ring::rand::SecureRandom;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

// PBKDF2 参数
const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_LEN: usize = 16;
const HASH_LEN: usize = 32;

/// 线程安全的数据库连接池（当前单连接）
pub type DbPool = Arc<Mutex<Connection>>;

/// 创建数据库连接池
pub fn create_pool(db_path: &Path) -> Result<DbPool, Box<dyn std::error::Error>> {
    let conn = Connection::open(db_path)?;
    Ok(Arc::new(Mutex::new(conn)))
}

/// 生成 PBKDF2 密码哈希（格式：base64_salt:base64_hash）
pub fn hash_password(password: &str) -> Result<String, Box<dyn std::error::Error>> {
    let rng = rand::SystemRandom::new();
    let mut salt = [0u8; SALT_LEN];
    rng.fill(&mut salt).map_err(|e| format!("RNG 失败: {e}"))?;

    let mut hash = [0u8; HASH_LEN];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        std::num::NonZeroU32::new(PBKDF2_ITERATIONS).unwrap(),
        &salt,
        password.as_bytes(),
        &mut hash,
    );

    Ok(format!(
        "{}:{}",
        base64_encode(&salt),
        base64_encode(&hash)
    ))
}

/// 验证密码是否匹配存储的哈希
pub fn verify_password(password: &str, stored_hash: &str) -> Result<bool, String> {
    let parts: Vec<&str> = stored_hash.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err("无效的密码哈希格式".into());
    }

    let salt = base64_decode(parts[0]).map_err(|e| format!("salt 解码失败: {e}"))?;
    let expected = base64_decode(parts[1]).map_err(|e| format!("hash 解码失败: {e}"))?;

    let mut hash = [0u8; HASH_LEN];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        std::num::NonZeroU32::new(PBKDF2_ITERATIONS).unwrap(),
        &salt,
        password.as_bytes(),
        &mut hash,
    );

    Ok(hash == expected.as_slice())
}

/// Base64 编码（URL-safe，无填充）
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// Base64 解码
fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|e| e.to_string())
}

/// 初始化数据库：创建所有表 + 默认管理员（PBKDF2）
pub fn init_database(db_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::open(db_path)?;

    // ── 性能优化 PRAGMA ──
    // WAL 模式: 读写并发 + 崩溃安全
    conn.execute_batch("PRAGMA journal_mode=WAL")?;
    // 外键约束
    conn.execute_batch("PRAGMA foreign_keys=ON")?;
    // UTF-8 编码
    conn.execute_batch("PRAGMA encoding='UTF-8'")?;
    // 缓存 32MB（默认 2MB），减少磁盘IO
    conn.execute_batch("PRAGMA cache_size=-8000")?;
    // WAL 模式同步: NORMAL 足够安全，写入速度提升 2-3x
    conn.execute_batch("PRAGMA synchronous=NORMAL")?;
    // 临时表/排序使用内存而非磁盘
    conn.execute_batch("PRAGMA temp_store=MEMORY")?;
    // 并发读繁忙等待 5 秒（而非立即失败）
    conn.execute_batch("PRAGMA busy_timeout=5000")?;
    // 内存映射 I/O 256MB，加速大表读取
    conn.execute_batch("PRAGMA mmap_size=268435456")?;
    // WAL 自动检查点大小 4096 页
    conn.execute_batch("PRAGMA wal_autocheckpoint=4096")?;
    // WAL 文件大小上限 64MB
    conn.execute_batch("PRAGMA journal_size_limit=67108864")?;
    // ANALYZE 采样行数限制
    conn.execute_batch("PRAGMA analysis_limit=1000")?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            display_name TEXT NOT NULL DEFAULT '',
            role TEXT NOT NULL CHECK(role IN ('employee','boss','admin')) DEFAULT 'employee',
            is_active INTEGER NOT NULL DEFAULT 1,
            force_password_change INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            token TEXT UNIQUE NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            expires_at TEXT NOT NULL
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            parent_id INTEGER REFERENCES categories(id),
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS brands (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            logo_path TEXT DEFAULT '',
            website TEXT DEFAULT '',
            description TEXT DEFAULT '',
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS products (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            sku TEXT UNIQUE NOT NULL,
            brand_id INTEGER REFERENCES brands(id),
            category_id INTEGER REFERENCES categories(id),
            unit TEXT NOT NULL DEFAULT '个',
            spec TEXT DEFAULT '',
            purchase_price REAL NOT NULL DEFAULT 0 CHECK(purchase_price >= 0),
            sale_price REAL NOT NULL DEFAULT 0 CHECK(sale_price >= 0),
            min_stock INTEGER NOT NULL DEFAULT 0 CHECK(min_stock >= 0),
            is_active INTEGER NOT NULL DEFAULT 1,
            notes TEXT DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS inventory (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            product_id INTEGER NOT NULL REFERENCES products(id),
            warehouse_id INTEGER NOT NULL DEFAULT 1,
            quantity INTEGER NOT NULL DEFAULT 0 CHECK(quantity >= 0),
            locked_quantity INTEGER NOT NULL DEFAULT 0 CHECK(locked_quantity >= 0),
            updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            UNIQUE(product_id, warehouse_id)
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS suppliers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            contact_person TEXT DEFAULT '',
            phone TEXT DEFAULT '',
            email TEXT DEFAULT '',
            address TEXT DEFAULT '',
            bank_account TEXT DEFAULT '',
            tax_id TEXT DEFAULT '',
            notes TEXT DEFAULT '',
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS customers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            contact_person TEXT DEFAULT '',
            phone TEXT DEFAULT '',
            email TEXT DEFAULT '',
            address TEXT DEFAULT '',
            tax_id TEXT DEFAULT '',
            notes TEXT DEFAULT '',
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS purchase_orders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            order_no TEXT UNIQUE NOT NULL,
            supplier_id INTEGER NOT NULL REFERENCES suppliers(id),
            total_amount REAL NOT NULL DEFAULT 0,
            status TEXT NOT NULL CHECK(status IN ('draft','submitted','received','cancelled')) DEFAULT 'draft',
            order_date TEXT NOT NULL DEFAULT (date('now','localtime')),
            expected_date TEXT DEFAULT '',
            received_date TEXT DEFAULT '',
            notes TEXT DEFAULT '',
            created_by INTEGER REFERENCES users(id),
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS purchase_order_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            order_id INTEGER NOT NULL REFERENCES purchase_orders(id) ON DELETE CASCADE,
            product_id INTEGER NOT NULL REFERENCES products(id),
            quantity INTEGER NOT NULL CHECK(quantity > 0),
            unit_price REAL NOT NULL CHECK(unit_price >= 0),
            total_price REAL NOT NULL CHECK(total_price >= 0),
            received_quantity INTEGER NOT NULL DEFAULT 0 CHECK(received_quantity >= 0)
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sales_orders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            order_no TEXT UNIQUE NOT NULL,
            customer_id INTEGER NOT NULL REFERENCES customers(id),
            total_amount REAL NOT NULL DEFAULT 0,
            profit REAL NOT NULL DEFAULT 0,
            status TEXT NOT NULL CHECK(status IN ('draft','confirmed','shipped','cancelled')) DEFAULT 'draft',
            order_date TEXT NOT NULL DEFAULT (date('now','localtime')),
            delivery_date TEXT DEFAULT '',
            notes TEXT DEFAULT '',
            created_by INTEGER REFERENCES users(id),
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sales_order_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            order_id INTEGER NOT NULL REFERENCES sales_orders(id) ON DELETE CASCADE,
            product_id INTEGER NOT NULL REFERENCES products(id),
            quantity INTEGER NOT NULL CHECK(quantity > 0),
            unit_price REAL NOT NULL CHECK(unit_price >= 0),
            cost_price REAL NOT NULL DEFAULT 0 CHECK(cost_price >= 0),
            total_price REAL NOT NULL CHECK(total_price >= 0),
            profit REAL NOT NULL DEFAULT 0
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS contracts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            contract_code TEXT UNIQUE NOT NULL,
            name TEXT NOT NULL,
            direction TEXT NOT NULL CHECK(direction IN ('purchase','sales')),
            supplier_id INTEGER REFERENCES suppliers(id),
            customer_id INTEGER REFERENCES customers(id),
            total_amount REAL NOT NULL DEFAULT 0 CHECK(total_amount >= 0),
            paid_amount REAL NOT NULL DEFAULT 0 CHECK(paid_amount >= 0),
            sign_date TEXT DEFAULT '',
            start_date TEXT DEFAULT '',
            end_date TEXT DEFAULT '',
            payment_method TEXT DEFAULT '',
            payment_terms TEXT DEFAULT '',
            brand_ids TEXT DEFAULT '[]',
            fulfillment_status TEXT NOT NULL DEFAULT 'pending' CHECK(fulfillment_status IN ('pending','partial','completed')),
            status TEXT NOT NULL CHECK(status IN ('draft','active','fulfilling','completed','terminated','expired')) DEFAULT 'draft',
            file_path TEXT DEFAULT '',
            notes TEXT DEFAULT '',
            handled_by INTEGER REFERENCES users(id),
            is_deleted INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS payment_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            contract_id INTEGER NOT NULL REFERENCES contracts(id) ON DELETE CASCADE,
            amount REAL NOT NULL CHECK(amount > 0),
            payment_date TEXT NOT NULL DEFAULT (date('now','localtime')),
            payment_method TEXT DEFAULT '',
            notes TEXT DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_code TEXT UNIQUE NOT NULL,
            name TEXT NOT NULL,
            customer_id INTEGER REFERENCES customers(id),
            status TEXT NOT NULL CHECK(status IN ('bidding','design','execution','delivery','maintenance','completed','cancelled')) DEFAULT 'bidding',
            budget REAL NOT NULL DEFAULT 0 CHECK(budget >= 0),
            actual_cost REAL NOT NULL DEFAULT 0 CHECK(actual_cost >= 0),
            start_date TEXT DEFAULT '',
            end_date TEXT DEFAULT '',
            handled_by INTEGER REFERENCES users(id),
            notes TEXT DEFAULT '',
            is_deleted INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS project_phases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            phase_name TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('pending','in_progress','completed')) DEFAULT 'pending',
            start_date TEXT DEFAULT '',
            end_date TEXT DEFAULT '',
            notes TEXT DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS project_documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            phase_id INTEGER REFERENCES project_phases(id),
            doc_name TEXT NOT NULL,
            doc_type TEXT DEFAULT '',
            file_path TEXT DEFAULT '',
            ocr_text TEXT DEFAULT '',
            file_size INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS operation_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER REFERENCES users(id),
            action TEXT NOT NULL,
            module TEXT NOT NULL,
            record_id INTEGER,
            before_data TEXT DEFAULT '',
            after_data TEXT DEFAULT '',
            ip_address TEXT DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS payment_transactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            transaction_type TEXT NOT NULL CHECK(transaction_type IN ('receipt','payment')),
            reference_type TEXT NOT NULL,
            reference_id INTEGER,
            party_type TEXT NOT NULL CHECK(party_type IN ('customer','supplier')),
            party_id INTEGER NOT NULL,
            amount REAL NOT NULL CHECK(amount > 0),
            payment_method TEXT DEFAULT '',
            transaction_date TEXT NOT NULL DEFAULT (date('now','localtime')),
            notes TEXT DEFAULT '',
            is_deleted INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS license_info (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            license_key TEXT NOT NULL,
            customer_name TEXT NOT NULL,
            max_users INTEGER NOT NULL DEFAULT 1,
            max_machines INTEGER NOT NULL DEFAULT 3,
            expiry_date TEXT NOT NULL,
            features TEXT NOT NULL DEFAULT '基础功能',
            last_verify_at TEXT DEFAULT '',
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )"
    )?;

    // ════════════════════════════════════════════════════════════
    //  性能索引 —— 覆盖所有外键及高频 WHERE/ORDER BY 列
    // ════════════════════════════════════════════════════════════

    // ── 会话（登录验证）──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id)"
    )?;

    // ── 产品（清单+筛选）──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_products_brand_id ON products(brand_id)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_products_category_id ON products(category_id)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_products_is_active ON products(is_active)"
    )?;

    // ── 采购订单 ──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_purchase_orders_supplier_id ON purchase_orders(supplier_id)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_purchase_orders_status ON purchase_orders(status)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_purchase_orders_order_date ON purchase_orders(order_date)"
    )?;

    // ── 采购明细 ──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_purchase_items_order_id ON purchase_order_items(order_id)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_purchase_items_product_id ON purchase_order_items(product_id)"
    )?;

    // ── 销售订单 ──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_sales_orders_customer_id ON sales_orders(customer_id)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_sales_orders_status ON sales_orders(status)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_sales_orders_order_date ON sales_orders(order_date)"
    )?;

    // ── 销售明细 ──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_sales_items_order_id ON sales_order_items(order_id)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_sales_items_product_id ON sales_order_items(product_id)"
    )?;

    // ── 库存 ──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_inventory_product_id ON inventory(product_id)"
    )?;

    // ── 合同 ──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_contracts_supplier_id ON contracts(supplier_id)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_contracts_customer_id ON contracts(customer_id)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_contracts_handled_by ON contracts(handled_by)"
    )?;
    // 复合索引: 软删除+方向+状态 —— 覆盖 list_contracts 最常见筛选
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_contracts_del_dir_status ON contracts(is_deleted, direction, status)"
    )?;

    // ── 付款记录 ──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_payment_records_contract_id ON payment_records(contract_id)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_payment_records_date ON payment_records(payment_date)"
    )?;

    // ── 项目 ──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_projects_customer_id ON projects(customer_id)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_projects_handled_by ON projects(handled_by)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_projects_del_status ON projects(is_deleted, status)"
    )?;

    // ── 项目阶段 ──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_project_phases_project_id ON project_phases(project_id)"
    )?;

    // ── 项目文档 ──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_project_docs_project_id ON project_documents(project_id)"
    )?;

    // ── 收付款流水（财务核心表）──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_payment_tx_party ON payment_transactions(party_type, party_id)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_payment_tx_type ON payment_transactions(transaction_type)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_payment_tx_date ON payment_transactions(transaction_date)"
    )?;
    // 复合索引: 财务报表 AR/AP 查询核心
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_payment_tx_finance ON payment_transactions(party_type, party_id, is_deleted, transaction_type)"
    )?;

    // ── 操作日志 ──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_op_logs_user_id ON operation_logs(user_id)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_op_logs_module ON operation_logs(module)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_op_logs_created_at ON operation_logs(created_at)"
    )?;

    // ── 供应商/客户/品牌/分类（常用筛选）──
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_suppliers_is_active ON suppliers(is_active)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_customers_is_active ON customers(is_active)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_brands_is_active ON brands(is_active)"
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_categories_parent_id ON categories(parent_id)"
    )?;

    // ── 统计信息更新 ──
    conn.execute_batch("ANALYZE")?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM users WHERE username = 'admin'",
        [],
        |row| row.get(0),
    )?;

    if count == 0 {
        let pwd_hash = hash_password("yulin2024!@#")
            .map_err(|e| format!("管理员密码哈希失败: {e}"))?;
        conn.execute(
            "INSERT INTO users (username, password_hash, display_name, role, force_password_change)
             VALUES ('admin', ?1, '系统管理员', 'admin', 1)",
            [&pwd_hash],
        )?;
        log::info!("默认管理员已创建 (PBKDF2-SHA256, 10万次迭代)【首次登录需修改密码】");
    }

    // WAL checkpoint + 统计优化
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    conn.execute_batch("PRAGMA optimize")?;

    log::info!("数据库初始化完成: {}", db_path.display());
    Ok(())
}
