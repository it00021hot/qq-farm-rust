//! QQ 扫码登录（NapCat）门面 — 对齐 bot admin `qq-login-routes`。

use serde_json::{json, Value};

use qq_farm_core::models::store::global_config::{get_login_settings, set_login_settings};
use qq_farm_core::services::qq_login::NapCatClient;

use crate::error::AppResult;

/// 登录设置视图（供设置页展示）。
///
/// rust 桌面版微信登录始终可用，无开关概念；这里只暴露 QQ 扫码（NapCat）配置。
#[must_use]
pub fn login_settings_view() -> Value {
    let s = get_login_settings();
    json!({
        "qqQrLogin": s.qq_qr_login,
        "napCatEndpoint": s.nap_cat_endpoint,
        "napCatSignature": s.nap_cat_signature,
    })
}

/// 保存登录设置。
///
/// `wechat_qr_login` 是 bot 面板的概念，rust 桌面版不管理，保留已存值不动。
pub fn save_login_settings(settings: &Value) {
    let parse_bool = |key: &str, fallback: bool| -> bool {
        settings.get(key).and_then(Value::as_bool).unwrap_or(fallback)
    };
    let parse_string = |key: &str| -> String {
        settings.get(key).and_then(Value::as_str).unwrap_or_default().trim().to_string()
    };
    let mut current = get_login_settings();
    current.qq_qr_login = parse_bool("qqQrLogin", current.qq_qr_login);
    current.nap_cat_endpoint = parse_string("napCatEndpoint");
    current.nap_cat_signature = parse_string("napCatSignature");
    set_login_settings(current);
}

fn client() -> AppResult<NapCatClient> {
    Ok(NapCatClient::from_settings(&get_login_settings())?)
}

/// 创建 QQ 扫码登录任务（获取二维码）。
pub async fn create_login_task() -> AppResult<Value> {
    let task = client()?.create_login_task().await?;
    Ok(serde_json::to_value(&task).unwrap_or(Value::Null))
}

/// 轮询登录任务状态。
pub async fn query_login_status(task_id: &str) -> AppResult<Value> {
    let task = client()?.query_login_status(task_id).await?;
    Ok(serde_json::to_value(&task).unwrap_or(Value::Null))
}

/// 用 taskId 换 QQ 小程序授权 code（用于登录游戏）。
pub async fn get_miniapp_code(task_id: &str) -> AppResult<Value> {
    let code = client()?.get_miniapp_code(task_id).await?;
    Ok(json!({ "code": code }))
}

/// 取消登录任务。
pub async fn cancel_login_task(task_id: &str) -> AppResult<()> {
    client()?.cancel_login_task(task_id).await.map_err(crate::error::AppError::from_core)
}
