//! Application metadata and the mobile release check (desktop keeps its signed updater).

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    platform: &'static str,
}

#[tauri::command]
pub fn get_app_info(app: tauri::AppHandle) -> AppInfo {
    AppInfo { version: app.package_info().version.to_string(), platform: std::env::consts::OS }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateResult {
    Native,
    #[allow(dead_code)]
    #[serde(rename_all = "camelCase")]
    Release {
        version: String,
        available: bool,
        has_apk: bool,
        release_url: String,
    },
}

#[tauri::command]
pub async fn check_app_update(app: tauri::AppHandle) -> Result<UpdateResult, String> {
    #[cfg(desktop)]
    {
        crate::updater::check_manually(app).await;
        Ok(UpdateResult::Native)
    }
    #[cfg(mobile)]
    {
        crate::release::check(&app.package_info().version.to_string()).await
    }
}
