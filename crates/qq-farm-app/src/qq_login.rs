//! 可选 NapCat QQ 扫码门面。成功后的 code 交给现有账号登录。

use qq_farm_core::models::store::global_config::{
    get_qq_login_settings, set_qq_login_settings, QqLoginSettings,
};
use qq_farm_core::services::napcat_login::{self, QqLoginTask, QqMiniappCode, QqScanAction};

use crate::accounts::{upsert_account, AclPolicy, UpsertAccountRequest};
use crate::error::{AppError, AppResult};
use crate::session::AppContext;

pub use qq_farm_core::services::napcat_login::{
    action_for_status, flow_still_current, QQ_MINIAPP_APP_ID,
};

#[must_use]
pub fn settings() -> QqLoginSettings {
    get_qq_login_settings()
}

pub fn save_settings(settings: QqLoginSettings) -> AppResult<QqLoginSettings> {
    set_qq_login_settings(settings, true).map_err(AppError::BadRequest)
}

pub async fn create_task() -> AppResult<QqLoginTask> {
    napcat_login::create_saved_login_task().await.map_err(bad_request)
}

pub async fn query_status(task_id: &str) -> AppResult<QqLoginTask> {
    napcat_login::query_saved_login_status(task_id).await.map_err(bad_request)
}

pub async fn miniapp_code(task_id: &str) -> AppResult<QqMiniappCode> {
    napcat_login::get_saved_miniapp_code(task_id).await.map_err(bad_request)
}

pub async fn cancel(task_id: &str) -> AppResult<()> {
    napcat_login::cancel_saved_login_task(task_id).await.map_err(bad_request)
}

pub async fn take_code(
    started: u64,
    current: u64,
    task_id: &str,
    active_task: &str,
) -> AppResult<Option<QqMiniappCode>> {
    if !flow_still_current(started, current, task_id, active_task) {
        return Ok(None);
    }
    Ok(Some(miniapp_code(task_id).await?))
}

pub async fn store_code(
    ctx: &AppContext,
    started: u64,
    current: u64,
    task_id: &str,
    active_task: &str,
    login: QqMiniappCode,
    name: &str,
) -> AppResult<bool> {
    if !flow_still_current(started, current, task_id, active_task) {
        return Ok(false);
    }
    let display = if name.trim().is_empty() { login.nickname } else { name.trim().to_owned() };
    upsert_account(
        ctx,
        &AclPolicy::LocalOwner,
        UpsertAccountRequest {
            code: Some(login.code),
            name: (!display.is_empty()).then(|| display.clone()),
            remark: (!name.trim().is_empty()).then(|| name.trim().to_owned()),
            platform: Some("qq".to_owned()),
            username: Some("local".to_owned()),
            ..UpsertAccountRequest::default()
        },
    )
    .await?;
    Ok(true)
}

fn bad_request(error: napcat_login::NapCatLoginError) -> AppError {
    AppError::BadRequest(error.to_string())
}

#[must_use]
pub fn next_action(status: &str) -> QqScanAction {
    action_for_status(status)
}
