#![recursion_limit = "256"]

mod application;
mod assets;
mod conflict;
mod desktop;
mod http;
mod i18n;
mod pages;
mod runtime;
mod theme;
mod tray;
mod update;

use std::sync::Arc;
use std::time::Duration;

use qq_farm_app::runtime_lease::{EngineLease, EngineLeaseError};

fn main() {
    #[cfg(feature = "ui-preview")]
    if std::env::args_os().nth(1).is_some_and(|argument| argument == "--render-preview") {
        let directory =
            std::env::args_os().nth(2).expect("--render-preview requires an output directory");
        desktop::render_previews(std::path::Path::new(&directory)).expect("render GPUI preview");
        return;
    }
    if let Err(error) = runtime::prepare_environment() {
        eprintln!("QQ Farm could not prepare its data directory: {error}");
        return;
    }
    let data_dir = runtime::data_dir();
    let lease = match EngineLease::acquire_for(&data_dir, "gpui") {
        Ok(lease) => Arc::new(lease),
        Err(error) => {
            if matches!(error, EngineLeaseError::AlreadyRunning) {
                if let Some(owner) = EngineLease::wake_existing_owner(&data_dir) {
                    if owner == "gpui" {
                        return;
                    }
                    conflict::show(owner, runtime::load_ui_preferences().language);
                    return;
                }
            }
            eprintln!("QQ Farm could not open its data directory: {error}");
            return;
        }
    };
    qq_farm_app::bootstrap::load_persisted_stores();
    let ui_preferences = runtime::load_ui_preferences();
    let tokio_runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("qq-farm-gpui-tokio")
            .build()
            .expect("create Tokio runtime"),
    );
    let app_context = {
        let _enter = tokio_runtime.enter();
        let app_context = qq_farm_app::bootstrap::assemble_app_context(16, "https://game.qq.com");
        // 与 Tauri 版启动路径一致：已授权微信账号在延迟与错峰后自动重连。
        app_context.engine.schedule_wx_authorized_start();
        Arc::new(app_context)
    };

    application::run(
        Arc::clone(&app_context),
        Arc::clone(&tokio_runtime),
        Arc::clone(&lease),
        ui_preferences,
    );
    tokio_runtime.block_on(qq_farm_app::lifecycle::shutdown(&app_context, Duration::from_secs(5)));
    drop(app_context);
    if let Ok(runtime) = Arc::try_unwrap(tokio_runtime) {
        runtime.shutdown_timeout(Duration::from_secs(1));
    }
    drop(lease);
}
