//! 服务端进程配置（端口、CORS、并发、TTL）。

use qq_farm_core::constants::{DEFAULT_GATEWAY_ORIGIN, WX_LOGIN_TASK_TTL_MS};

/// HTTP / Socket.IO 服务配置
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub admin_port: u16,
    pub max_workers: usize,
    pub cors_origins: Vec<String>,
    pub gateway_origin: String,
    pub wx_login_task_ttl_ms: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            admin_port: 3007,
            max_workers: 16,
            cors_origins: vec![
                "http://localhost:5173".into(),
                "http://localhost:3000".into(),
                "http://127.0.0.1:5173".into(),
            ],
            gateway_origin: DEFAULT_GATEWAY_ORIGIN.to_string(),
            wx_login_task_ttl_ms: WX_LOGIN_TASK_TTL_MS,
        }
    }
}

impl ServerConfig {
    /// 从环境变量加载（缺省走 [`Default`]）
    #[must_use]
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(p) = std::env::var("ADMIN_PORT") {
            if let Ok(n) = p.parse() {
                cfg.admin_port = n;
            }
        }
        if let Ok(p) = std::env::var("FARM_MAX_WORKERS") {
            if let Ok(n) = p.parse() {
                cfg.max_workers = n;
            }
        }
        if let Ok(raw) = std::env::var("FARM_CORS_ORIGINS") {
            let parsed: Vec<String> = raw
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            if !parsed.is_empty() {
                cfg.cors_origins = parsed;
            }
        }
        if let Ok(o) = std::env::var("FARM_GATEWAY_ORIGIN") {
            if !o.trim().is_empty() {
                cfg.gateway_origin = o;
            }
        }
        cfg
    }

    /// 是否允许该 Origin（无 Origin 时允许 `*` 语义由调用方处理）
    #[must_use]
    pub fn allows_origin(&self, origin: &str) -> bool {
        self.cors_origins.iter().any(|o| o == origin)
    }
}
