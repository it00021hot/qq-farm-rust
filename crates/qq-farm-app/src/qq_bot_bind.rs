//! QQ Bot 扫码绑定编排。

use qq_farm_core::models::store::global_config::{
    apply_qq_bot_binding, clear_qq_bot_binding, effective_qq_bot_credentials, gateway_qq_bot_config,
    get_offline_reminder, get_user_offline_reminder, QqBotBinding,
};
use qq_farm_core::services::qrlogin::qr_png_data_url;
use qq_farm_core::services::qq_bot::{BindPollResult, BindStartResult};
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::session::AppContext;

/// 启动 QQ Bot 扫码绑定。
pub fn start_qq_bot_bind(ctx: &AppContext, username: &str) -> AppResult<BindStartResult> {
    let username = username.trim();
    if username.is_empty() {
        return Err(AppError::BadRequest("缺少用户名".into()));
    }
    let credentials = effective_qq_bot_credentials();
    if !credentials.is_complete() {
        return Err(AppError::BadRequest("请先填写 QQ 机器人 AppID 和 AppSecret".into()));
    }
    ctx.engine
        .qq_bot()
        .reconcile_background(gateway_qq_bot_config());
    let invite_url = credentials.invite_url();
    let qr_data_url = if invite_url.is_empty() {
        String::new()
    } else {
        qr_png_data_url(&invite_url)
    };
    Ok(ctx.engine.qq_bot().bind_sessions().start_session(username, &invite_url, &qr_data_url))
}

/// 轮询 QQ Bot 绑定状态。
#[must_use]
pub fn poll_qq_bot_bind(ctx: &AppContext, session_id: &str) -> BindPollResult {
    ctx.engine.qq_bot().bind_sessions().poll(session_id)
}

/// 解绑 QQ Bot 通知。
pub fn unbind_qq_bot(username: &str) {
    let username = username.trim();
    if username.is_empty() {
        return;
    }
    clear_qq_bot_binding(username);
}

/// 绑定状态摘要（供设置页展示）。
#[must_use]
pub fn qq_bot_bind_status(username: Option<&str>) -> Value {
    let credentials = effective_qq_bot_credentials();
    let reminder = username
        .filter(|u| !u.is_empty())
        .and_then(get_user_offline_reminder)
        .unwrap_or_else(get_offline_reminder);
    let binding = reminder.qq_bot_binding.clone();
    json!({
        "credentialsConfigured": credentials.is_complete(),
        "bound": binding.is_bound(),
        "binding": binding,
        "botInviteUrl": credentials.invite_url(),
    })
}

/// 应用显式绑定结果（测试/内部用）。
pub fn save_qq_bot_binding(username: &str, binding: QqBotBinding) {
    apply_qq_bot_binding(username, binding);
}

/// 启动时恢复已保存的 openid 映射。
pub fn restore_saved_bindings(ctx: &AppContext) {
    let bind_sessions = ctx.engine.qq_bot().bind_sessions();
    let state = qq_farm_core::models::store::global_config::state();
    let guard = state.read();
    for (username, reminder) in &guard.user_offline_reminders {
        if reminder.qq_bot_binding.is_bound() {
            bind_sessions.register_binding(username, &reminder.qq_bot_binding);
        }
    }
}
