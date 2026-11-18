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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use qq_farm_core::runtime::engine::{EngineConfig, RuntimeEngine};

    use super::*;

    #[tokio::test]
    async fn stale_napcat_code_is_not_stored() {
        let engine = Arc::new(RuntimeEngine::assemble(EngineConfig {
            data_root: std::env::temp_dir(),
            max_workers: 0,
            ..EngineConfig::default()
        }));
        let context = AppContext::new(engine);
        let login = QqMiniappCode {
            code: "synthetic-napcat-code".to_owned(),
            nickname: "合成".to_owned(),
            app_id: QQ_MINIAPP_APP_ID.to_owned(),
        };
        let stored = store_code(&context, 4, 5, "task-1", "task-1", login, "合成")
            .await
            .expect("stale decision");
        assert!(!stored);
        let leaked = qq_farm_core::models::store::accounts::get_accounts()
            .iter()
            .any(|account| account.code == "synthetic-napcat-code");
        assert!(!leaked);
    }
}

#[must_use]
pub fn next_action(status: &str) -> QqScanAction {
    action_for_status(status)
}
