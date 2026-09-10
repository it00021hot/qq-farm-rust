//! QQ 扫码登录（NapCat）命令 — 薄适配 → `qq-farm-app::qq_login`。

use serde_json::Value;

use qq_farm_app::qq_login;

use crate::error::{IpcError, IpcResult};

/// 登录设置视图（扫码开关 / NapCat 配置）。
#[tauri::command]
pub fn get_qq_login_settings() -> IpcResult<Value> {
    Ok(qq_login::login_settings_view())
}

/// 保存登录设置。
#[tauri::command]
pub fn save_qq_login_settings(settings: Value) -> IpcResult<Value> {
    qq_login::save_login_settings(&settings);
    Ok(qq_login::login_settings_view())
}

/// 创建 QQ 扫码登录任务（获取二维码）。
#[tauri::command]
pub async fn qq_login_create_task() -> IpcResult<Value> {
    qq_login::create_login_task().await.map_err(IpcError::from)
}

/// 轮询 QQ 扫码登录任务状态。
#[tauri::command]
pub async fn qq_login_task_status(task_id: String) -> IpcResult<Value> {
    qq_login::query_login_status(&task_id).await.map_err(IpcError::from)
}

/// 用 taskId 换 QQ 小程序授权 code。
#[tauri::command]
pub async fn qq_login_miniapp_code(task_id: String) -> IpcResult<Value> {
    qq_login::get_miniapp_code(&task_id).await.map_err(IpcError::from)
}

/// 取消 QQ 扫码登录任务。
#[tauri::command]
pub async fn qq_login_cancel_task(task_id: String) -> IpcResult<Value> {
    qq_login::cancel_login_task(&task_id).await.map_err(IpcError::from)?;
    Ok(serde_json::json!({ "ok": true }))
}
