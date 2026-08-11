//! `tracing` 初始化。
//!
//! 由 server / cli 在启动时调用一次。

use std::sync::Once;

static INIT: Once = Once::new();

/// 初始化日志系统（幂等）
pub fn init() {
    INIT.call_once(|| {
        use tracing_subscriber::{fmt, EnvFilter};
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,qq_farm_core=debug"));
        fmt().with_env_filter(filter).with_target(true).init();
    });
}
