use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const UI_PREFERENCES_FILE: &str = "gpui-ui.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPreferences {
    pub appearance: String,
    pub language: String,
    pub sidebar_collapsed: bool,
    pub sidebar_width: u32,
    pub selected_page: String,
    pub selected_account: Option<String>,
    pub window_width: u32,
    pub window_height: u32,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            appearance: "system".into(),
            language: "zh-CN".into(),
            sidebar_collapsed: false,
            sidebar_width: 224,
            selected_page: "dashboard".into(),
            selected_account: None,
            window_width: 1440,
            window_height: 900,
        }
    }
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("FARM_DATA_DIR")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_data_dir().unwrap_or_else(|| PathBuf::from("data")))
}

pub fn prepare_environment() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    let path = data_dir();
    std::fs::create_dir_all(&path)?;
    if env_is_unset("FARM_DATA_DIR") {
        std::env::set_var("FARM_DATA_DIR", &path);
    }
    let resources = crate::assets::resource_root();
    set_env_if_missing("QQ_FARM_ASSET_DIR", &resources);
    set_env_if_missing("FARM_GAME_CONFIG_DIR", &resources.join("assets/game_config"));
    set_env_if_missing("TSDK_WASM_PATH", &engine_wasm_path(&resources, "tsdk.wasm"));
    set_env_if_missing("TSDK_WX_WASM_PATH", &engine_wasm_path(&resources, "tsdk-wx.wasm"));
    Ok(())
}

fn engine_wasm_path(resources: &std::path::Path, filename: &str) -> PathBuf {
    let packaged = resources.join(filename);
    if packaged.is_file() {
        packaged
    } else {
        resources.join("assets").join(filename)
    }
}

fn set_env_if_missing(name: &str, value: &std::path::Path) {
    if env_is_unset(name) {
        std::env::set_var(name, value);
    }
}

fn env_is_unset(name: &str) -> bool {
    std::env::var(name).ok().filter(|value| !value.is_empty()).is_none()
}

pub fn load_ui_preferences() -> UiPreferences {
    load_ui_preferences_from(&data_dir())
}

pub(crate) fn load_ui_preferences_from(directory: &std::path::Path) -> UiPreferences {
    let path = directory.join(UI_PREFERENCES_FILE);
    std::fs::read(path)
        .ok()
        .and_then(|contents| serde_json::from_slice(&contents).ok())
        .unwrap_or_default()
}

pub(crate) fn save_ui_preferences(
    directory: &std::path::Path,
    preferences: &UiPreferences,
) -> std::io::Result<()> {
    let path = directory.join(UI_PREFERENCES_FILE);
    let temporary_path = path.with_extension("json.tmp");
    let contents = serde_json::to_vec_pretty(preferences).map_err(std::io::Error::other)?;
    std::fs::write(&temporary_path, contents)?;
    std::fs::rename(temporary_path, path)
}

fn default_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        dirs::data_local_dir().map(|path| path.join("QQFarmRust"))
    }
    #[cfg(not(windows))]
    {
        dirs::data_dir().map(|path| path.join("QQFarmRust"))
    }
}

#[cfg(test)]
mod tests {
    use super::engine_wasm_path;
    use std::path::Path;

    #[test]
    fn engine_wasm_paths_match_checkout_and_packaged_layouts() {
        let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for filename in ["tsdk.wasm", "tsdk-wx.wasm"] {
            let path = engine_wasm_path(&checkout, filename);
            assert!(path.is_file(), "checkout {filename}");
            let packaged_root = checkout.join("assets");
            assert_eq!(engine_wasm_path(&packaged_root, filename), packaged_root.join(filename));
        }
    }
}
