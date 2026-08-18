//! 打包后的数据目录与资源路径（对齐 Wails：可写目录走 OS app data，wasm 走 bundle）。

use std::path::PathBuf;

use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

fn env_unset(key: &str) -> bool {
    std::env::var(key).ok().filter(|s| !s.is_empty()).is_none()
}

fn default_os_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        dirs::data_local_dir().map(|d| d.join("QQFarm"))
    }
    #[cfg(not(windows))]
    {
        dirs::data_dir().map(|d| d.join("QQFarm"))
    }
}

/// `dotenv` + release 默认 `FARM_DATA_DIR`。须在 `logger::init` 之前调用。
pub fn prepare_data_dir() {
    dotenvy::dotenv().ok();
    if env_unset("FARM_DATA_DIR") && !cfg!(debug_assertions) {
        if let Some(dir) = default_os_data_dir() {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!("create data dir failed ({}): {e}", dir.display());
            }
            std::env::set_var("FARM_DATA_DIR", &dir);
        }
    }
}

/// 安装包内把 TSDK / game_config 指到 Tauri resource dir。`tauri dev` 保持仓库根路径。
pub fn apply_bundled_resource_env(app: &AppHandle) {
    if cfg!(dev) {
        return;
    }
    if env_unset("TSDK_WASM_PATH") {
        if let Ok(p) = app.path().resolve("assets/tsdk.wasm", BaseDirectory::Resource) {
            if p.is_file() {
                std::env::set_var("TSDK_WASM_PATH", &p);
            } else {
                tracing::warn!(path = %p.display(), "bundled tsdk.wasm missing");
            }
        }
    }
    if env_unset("FARM_GAME_CONFIG_DIR") {
        if let Ok(p) = app.path().resolve("assets/game_config", BaseDirectory::Resource) {
            if p.is_dir() {
                std::env::set_var("FARM_GAME_CONFIG_DIR", &p);
            } else {
                tracing::warn!(path = %p.display(), "bundled game_config missing");
            }
        }
    }
}
