//! 应用事件总线 — 包装 core RuntimeEvent。

use qq_farm_core::runtime::runtime_state::RuntimeEvent;
use tokio::sync::broadcast;

use crate::session::AppContext;

/// 应用层事件（当前直接包装 RuntimeEvent，后续可扩展 GPUI 专用变体）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct AppEvent(pub RuntimeEvent);

impl AppEvent {
    #[must_use]
    pub fn into_inner(self) -> RuntimeEvent {
        self.0
    }

    #[must_use]
    pub fn as_inner(&self) -> &RuntimeEvent {
        &self.0
    }
}

impl From<RuntimeEvent> for AppEvent {
    fn from(e: RuntimeEvent) -> Self {
        Self(e)
    }
}

impl From<AppEvent> for RuntimeEvent {
    fn from(e: AppEvent) -> Self {
        e.0
    }
}

impl AppContext {
    /// 订阅运行时事件。
    ///
    /// GPUI / desktop 客户端应通过此方法订阅，并将收到的 [`RuntimeEvent`] 包装为 [`AppEvent`]。
    /// HTTP server 的 Socket.IO 转发仍可直接使用 `RuntimeEngine::runtime_state().subscribe()`。
    pub fn subscribe_events(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.engine.subscribe_runtime_events()
    }
}
