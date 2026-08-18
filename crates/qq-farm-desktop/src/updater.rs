//! GitHub Releases 自动更新（Tauri updater + 原生对话框，对齐 Wails 行为）。

use std::time::Duration;

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

const STARTUP_DELAY: Duration = Duration::from_secs(5);
const BACKGROUND_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MANUAL_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// 启动约 5 秒后静默检查；有更新才弹窗。
pub fn setup(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_DELAY).await;
        run_check(handle, false, BACKGROUND_TIMEOUT).await;
    });
}

/// 菜单/托盘「检查更新」：总是给出结果。
pub fn check_for_updates(app: &AppHandle, show_up_to_date: bool) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        run_check(handle, show_up_to_date, MANUAL_TIMEOUT).await;
    });
}

async fn run_check(app: AppHandle, show_up_to_date: bool, timeout: Duration) {
    match tokio::time::timeout(timeout, check_inner(&app)).await {
        Ok(Ok(Some(version))) => prompt_and_install(app, version).await,
        Ok(Ok(None)) => {
            if show_up_to_date {
                app.dialog()
                    .message("已是最新版本")
                    .title("软件更新")
                    .kind(MessageDialogKind::Info)
                    .blocking_show();
            }
        }
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "updater check failed");
            if show_up_to_date {
                app.dialog()
                    .message(format!("检查更新失败：{e}"))
                    .title("软件更新")
                    .kind(MessageDialogKind::Error)
                    .blocking_show();
            }
        }
        Err(_) => {
            tracing::warn!("updater check timed out");
            if show_up_to_date {
                app.dialog()
                    .message("检查更新超时")
                    .title("软件更新")
                    .kind(MessageDialogKind::Error)
                    .blocking_show();
            }
        }
    }
}

async fn check_inner(app: &AppHandle) -> Result<Option<String>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(update.version.clone())),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

async fn prompt_and_install(app: AppHandle, version: String) {
    let go = app
        .dialog()
        .message(format!("发现新版本 {version}，是否立即更新？"))
        .title("软件更新")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom("立即更新".to_string(), "稍后".to_string()))
        .blocking_show();
    if !go {
        return;
    }

    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            app.dialog()
                .message(format!("无法启动更新：{e}"))
                .title("软件更新")
                .kind(MessageDialogKind::Error)
                .blocking_show();
            return;
        }
    };
    let update = match updater.check().await {
        Ok(Some(u)) => u,
        Ok(None) => {
            app.dialog()
                .message("已是最新版本")
                .title("软件更新")
                .kind(MessageDialogKind::Info)
                .blocking_show();
            return;
        }
        Err(e) => {
            app.dialog()
                .message(format!("检查更新失败：{e}"))
                .title("软件更新")
                .kind(MessageDialogKind::Error)
                .blocking_show();
            return;
        }
    };

    if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
        tracing::error!(error = %e, "updater install failed");
        app.dialog()
            .message(format!("安装更新失败：{e}"))
            .title("软件更新")
            .kind(MessageDialogKind::Error)
            .blocking_show();
        return;
    }
    app.restart();
}
