//! 应用配置（启动参数、服务端口、日志级别等）。
//!
//! 阶段 0：仅定义结构体和默认值。运行时加载逻辑留到阶段 1 引入 `config` crate。

use serde::{Deserialize, Serialize};

/// 应用全局配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Web 面板监听端口
    pub admin_port: u16,

    /// 日志级别
    pub log_level: String,

    /// 数据目录（账号、用户、卡密等运行时持久化文件）
    pub data_dir: String,

    /// 时区
    pub timezone: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            admin_port: 3007,
            log_level: "info".to_string(),
            data_dir: "./data".to_string(),
            timezone: "Asia/Shanghai".to_string(),
        }
    }
}
