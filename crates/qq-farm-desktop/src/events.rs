//! `AppEvent` / `RuntimeEvent` → 前端 `app-event`（web 信封：type / payload / accountId）。

use tauri::{AppHandle, Emitter};

use qq_farm_app::events::AppEvent;

use crate::state::DesktopState;

pub use qq_farm_app::events::PanelRealtimeEvent as DesktopAppEvent;

/// 在后台任务中订阅 runtime 事件并 emit 到所有窗口。
pub fn spawn_event_bridge(app: AppHandle, state: DesktopState) {
    tauri::async_runtime::spawn(async move {
        let mut rx = state.app.subscribe_events();
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    for payload in AppEvent::from(ev).to_realtime() {
                        if let Err(e) = app.emit("app-event", &payload) {
                            tracing::warn!(error = %e, "emit app-event failed");
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(skipped = n, "desktop event bridge lagged");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::info!("desktop event bridge closed");
                    break;
                }
            }
        }
    });
}
