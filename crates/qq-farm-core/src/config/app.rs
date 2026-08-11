//! 应用全局配置。
//!
//! 启动时从环境变量 / CLI 参数构造，运行时基本只读。
//!
//! 1:1 对应原 `core/src/config/config.ts` 的 `CONFIG` + `RuntimeConfig` 高层参数。

use serde::{Deserialize, Serialize};

/// 应用全局配置（启动参数）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Web 面板监听端口
    pub admin_port: u16,
    /// 管理员密码（明文，运行时与全局配置里 hash 比对）
    pub admin_password: Option<String>,
    /// 日志级别
    pub log_level: String,
    /// 数据目录
    pub data_dir: String,
    /// 时区
    pub timezone: String,
    /// 心跳间隔（毫秒）
    pub heartbeat_interval_ms: i64,
    /// 农场检查间隔（毫秒）
    pub farm_check_interval_ms: i64,
    /// 好友检查间隔（毫秒）
    pub friend_check_interval_ms: i64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            admin_port: std::env::var("ADMIN_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3007),
            admin_password: std::env::var("ADMIN_PASSWORD").ok(),
            log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            data_dir: std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string()),
            timezone: std::env::var("TIMEZONE").unwrap_or_else(|_| "Asia/Shanghai".to_string()),
            heartbeat_interval_ms: 25_000,
            farm_check_interval_ms: 3_000,
            friend_check_interval_ms: 12_000,
        }
    }
}

impl AppConfig {
    /// 从环境变量构造
    #[must_use]
    pub fn from_env() -> Self {
        Self::default()
    }
}
