//! 数据目录、关于框、关窗进托盘、显示主窗口。

use std::path::Path;

use tauri::menu::MenuId;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::updater;

pub const ID_SHOW_MAIN: &str = "show-main";
pub const ID_OPEN_DATA_DIR: &str = "open-data-dir";
pub const ID_CHECK_UPDATE: &str = "check-update";
pub const ID_ABOUT: &str = "about";
pub const ID_QUIT: &str = "quit";

pub fn handle_menu_event(app: &AppHandle, id: &MenuId) {
    match id.as_ref() {
        ID_SHOW_MAIN => show_main_window(app),
        ID_OPEN_DATA_DIR => open_data_dir(),
        ID_CHECK_UPDATE => updater::check_for_updates(app, true),
        ID_ABOUT => show_about(app),
        ID_QUIT => app.exit(0),
        _ => {}
    }
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn toggle_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn install_close_to_tray(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let hidden = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = hidden.hide();
        }
    });
}

pub fn open_data_dir() {
    let dir = qq_farm_core::config::paths::ensure_data_dir()
        .unwrap_or_else(|_| qq_farm_core::config::paths::get_data_dir());
    if let Err(e) = open_path(&dir) {
        tracing::warn!(error = %e, path = %dir.display(), "open data dir failed");
    }
}

fn open_path(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(path).spawn()?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open").arg(path).spawn()?;
    }
    Ok(())
}

pub fn show_about(app: &AppHandle) {
    let version = app.package_info().version.to_string();
    app.dialog()
        .message(format!("QQ Farm desktop\nVersion {version}"))
        .title("QQ Farm")
        .kind(MessageDialogKind::Info)
        .blocking_show();
}
