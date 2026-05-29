//! License 商业授权验证模块
//!
//! 功能：
//! - 应用启动时验证 License
//! - 定期心跳校验（每 24 小时）
//! - 72 小时离线宽容期
//! - 过期/吊销后的功能限制

use tauri::AppHandle;

/// 启动时验证 License
///
/// TODO v0.2: 实现完整的在线验证 + 离线缓存逻辑
pub fn verify_on_startup(_app: AppHandle) -> Result<(), String> {
    // 当前为开发阶段占位实现
    // 后续版本将：
    // 1. 读取本地缓存的 License Key
    // 2. 调用激活服务器 /api/verify 接口
    // 3. 验证 Ed25519 签名
    // 4. 检查到期日期
    // 5. 如网络不可用，回退到离线缓存（72h 宽容期）
    // 6. License 无效时限制功能并弹窗提示

    log::info!("License 验证模块已加载（开发模式，跳过验证）");
    Ok(())
}

/// License 信息结构
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LicenseInfo {
    pub license_key: String,
    pub customer_name: String,
    pub max_users: i32,
    pub max_machines: i32,
    pub expiry_date: String,
    pub features: String,
    pub status: String,
}

/// 获取当前 License 信息（从本地缓存读取）
pub fn get_cached_license() -> Option<LicenseInfo> {
    // TODO: 从 license_info 表读取
    None
}
