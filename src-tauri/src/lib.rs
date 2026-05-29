//! 陕西昱霖 - 采购销售协同管理系统
//! Tauri 2.x 后端核心

mod commands;
mod db;
mod license;
mod validators;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_dir = app.path().app_data_dir().expect("无法获取应用数据目录");
            std::fs::create_dir_all(&app_dir).expect("无法创建应用数据目录");

            let db_path = app_dir.join("yulin.db");

            // 初始化表结构
            db::init_database(&db_path).expect("数据库初始化失败");

            // 创建连接池并注册为全局状态
            let pool = db::create_pool(&db_path).expect("数据库连接池创建失败");
            app.manage(pool);

            // License 验证
            license::verify_on_startup(app.handle().clone())?;

            log::info!("陕西昱霖协同管理系统启动成功");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::login,
            commands::auth::logout,
            commands::auth::get_current_user,
            commands::auth::change_password,
            commands::auth::list_users,
            commands::auth::create_user,
            commands::auth::update_user,
            commands::auth::delete_user,
            commands::auth::get_system_info,
            commands::dashboard::get_stats,
            commands::products::list_products,
            commands::products::get_product,
            commands::products::create_product,
            commands::products::update_product,
            commands::products::delete_product,
            commands::inventory::get_stock,
            commands::inventory::stock_in,
            commands::inventory::stock_out,
            commands::purchase::list_purchase_orders,
            commands::purchase::get_purchase_order,
            commands::purchase::create_purchase_order,
            commands::purchase::submit_purchase_order,
            commands::purchase::receive_purchase_order,
            commands::sales::list_sales_orders,
            commands::sales::get_sales_order,
            commands::sales::create_sales_order,
            commands::sales::confirm_sales_order,
            commands::sales::ship_sales_order,
            commands::suppliers::list_suppliers,
            commands::suppliers::get_supplier,
            commands::suppliers::create_supplier,
            commands::suppliers::update_supplier,
            commands::suppliers::delete_supplier,
            commands::customers::list_customers,
            commands::customers::get_customer,
            commands::customers::create_customer,
            commands::customers::update_customer,
            commands::customers::delete_customer,
            commands::brands::get_brand,
            commands::brands::list_brands,
            commands::brands::create_brand,
            commands::brands::update_brand,
            commands::brands::delete_brand,
            commands::categories::list_categories,
            commands::categories::create_category,
            commands::categories::update_category,
            commands::categories::delete_category,
            commands::contracts::list_contracts,
            commands::contracts::get_contract,
            commands::contracts::create_contract,
            commands::contracts::update_contract,
            commands::contracts::delete_contract,
            commands::contracts::change_contract_status,
            commands::contracts::add_payment_record,
            commands::contracts::delete_payment_record,
            commands::projects::list_projects,
            commands::projects::get_project,
            commands::projects::create_project,
            commands::projects::update_project,
            commands::projects::delete_project,
            commands::projects::change_project_status,
            commands::projects::update_project_phase,
            commands::finance::list_ar,
            commands::finance::list_ap,
            commands::finance::get_profit_summary,
            commands::finance::list_transactions,
            commands::finance::record_transaction,
            commands::finance::delete_transaction,
        ])
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
