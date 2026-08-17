//! 应用会话上下文。

use std::sync::Arc;

use qq_farm_core::runtime::engine::RuntimeEngine;

use crate::wx_login::WxLoginHub;

/// 应用上下文 — 持有运行时引擎与共享门面状态。
#[derive(Clone)]
pub struct AppContext {
    pub engine: Arc<RuntimeEngine>,
    pub wx_login: Arc<WxLoginHub>,
}

impl AppContext {
    #[must_use]
    pub fn new(engine: Arc<RuntimeEngine>) -> Self {
        Self {
            engine,
            wx_login: Arc::new(WxLoginHub::new()),
        }
    }

    #[must_use]
    pub fn with_wx_login(engine: Arc<RuntimeEngine>, wx_login: Arc<WxLoginHub>) -> Self {
        Self { engine, wx_login }
    }
}

impl std::fmt::Debug for AppContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppContext").finish_non_exhaustive()
    }
}
