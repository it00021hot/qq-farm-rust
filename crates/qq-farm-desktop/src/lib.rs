//! QQ Farm Tauri v2 桌面适配层。
//!
//! 仅依赖 `qq-farm-app` / `qq-farm-core`；不依赖 `qq-farm-server`，不把 Tauri 泄漏进 app。

mod commands;
mod error;
mod events;
mod state;

use std::sync::Arc;

use tauri::Manager;

use crate::state::DesktopState;

/// 桌面端进程入口（由 `main` / 移动端入口调用）。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    qq_farm_core::utils::logger::init();
    dotenvy::dotenv().ok();

    // RuntimeEngine / AppEvent 桥依赖当前线程的 Tokio runtime（与旧 GPUI 入口一致）。
    let runtime = Box::leak(Box::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("qq-farm-tokio")
            .build()
            .expect("tokio runtime"),
    ));
    let _enter = runtime.enter();

    let max_workers = std::env::var("MAX_WORKERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(16);
    let app_ctx = Arc::new(qq_farm_app::bootstrap::assemble_app_context(
        max_workers,
        "https://game.qq.com",
    ));
    let desktop = DesktopState::new(app_ctx);

    tauri::Builder::default()
        .manage(desktop)
        .setup(|app| {
            let handle = app.handle().clone();
            let state = app.state::<DesktopState>().inner().clone();
            events::spawn_event_bridge(handle, state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::desktop_ready,
            commands::get_snapshot,
            commands::list_accounts,
            commands::get_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running qq-farm-desktop");
}
