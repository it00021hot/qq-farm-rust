//! 应用会话上下文。

use std::sync::Arc;

use qq_farm_core::runtime::engine::RuntimeEngine;

/// 应用上下文 — 持有运行时引擎。
#[derive(Clone)]
pub struct AppContext {
    pub engine: Arc<RuntimeEngine>,
}

impl AppContext {
    #[must_use]
    pub fn new(engine: Arc<RuntimeEngine>) -> Self {
        Self { engine }
    }
}

impl std::fmt::Debug for AppContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppContext").finish_non_exhaustive()
    }
}
