//! `AppEvent` / `RuntimeEvent` → 前端 `app-event`。

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use qq_farm_app::events::AppEvent;
use qq_farm_core::runtime::runtime_state::RuntimeEvent;

use crate::state::DesktopState;

/// 推送给前端的事件载荷。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopAppEvent {
    pub kind: String,
    pub account_id: Option<String>,
    pub account_name: Option<String>,
    pub message: Option<String>,
}

impl From<&RuntimeEvent> for DesktopAppEvent {
    fn from(ev: &RuntimeEvent) -> Self {
        match ev {
            RuntimeEvent::Log(entry) => Self {
                kind: "log".into(),
                account_id: entry.account_id.clone(),
                account_name: entry.account_name.clone(),
                message: Some(entry.msg.clone()),
            },
            RuntimeEvent::AccountLog(entry) => Self {
                kind: "account_log".into(),
                account_id: Some(entry.account_id.clone()),
                account_name: Some(entry.account_name.clone()),
                message: Some(entry.msg.clone()),
            },
            RuntimeEvent::Status {
                account_id,
                account_name,
                ..
            } => Self {
                kind: "status".into(),
                account_id: Some(account_id.clone()),
                account_name: Some(account_name.clone()),
                message: None,
            },
            RuntimeEvent::WorkerLog {
                account_id,
                account_name,
                ..
            } => Self {
                kind: "worker_log".into(),
                account_id: Some(account_id.clone()),
                account_name: Some(account_name.clone()),
                message: None,
            },
        }
    }
}

/// 在后台任务中订阅 runtime 事件并 emit 到所有窗口。
pub fn spawn_event_bridge(app: AppHandle, state: DesktopState) {
    tauri::async_runtime::spawn(async move {
        let mut rx = state.app.subscribe_events();
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let _ = AppEvent::from(ev.clone());
                    let payload = DesktopAppEvent::from(&ev);
                    if let Err(e) = app.emit("app-event", &payload) {
                        tracing::warn!(error = %e, "emit app-event failed");
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
