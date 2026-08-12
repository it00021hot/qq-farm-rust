//! Account 路由 — 13 端点。
//!
//! 1:1 对应原 `controllers/admin/account-routes.ts`（495 行）。

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok, ok_empty, AdminContext, ApiError, ApiResult};

/// 构造 account 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/accounts", get(list_accounts).post(create_account))
        .route("/api/account/remark", post(remark_account))
        .route("/api/accounts/{id}", delete(delete_account))
        .route("/api/account-logs", get(get_account_logs))
        .route("/api/logs", get(get_logs).delete(delete_logs))
        .route("/api/settings", get(get_settings).post(save_settings))
        .route("/api/settings/default", get(get_default_settings))
        .route("/api/settings/theme", post(set_theme))
        .route("/api/settings/offline-reminder", post(set_offline_reminder))
        .route("/api/settings/offline-reminder/test", post(test_offline_reminder))
}

#[derive(Debug, Deserialize)]
struct RemarkBody {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct CreateAccountBody {
    name: String,
    code: String,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    qq: Option<String>,
    #[serde(default)]
    uin: Option<String>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    username: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AccountLogsQuery {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LogsQuery {
    #[serde(default)]
    keyword: Option<String>,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    module: Option<String>,
    #[serde(default)]
    event: Option<String>,
    #[serde(default)]
    is_warn: Option<bool>,
    #[serde(default)]
    time_from: Option<String>,
    #[serde(default)]
    time_to: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SettingsBody {
    #[serde(default)]
    account_id: Option<String>,
    #[serde(flatten)]
    rest: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ThemeBody {
    theme: String,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OfflineReminderBody {
    #[serde(default)]
    account_id: Option<String>,
    #[serde(flatten)]
    cfg: serde_json::Value,
}

async fn list_accounts(
    State(ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let accounts = qq_farm_core::models::store::accounts::get_accounts();
    let _ = ctx;
    ok(json!({ "ok": true, "accounts": accounts }))
}

async fn create_account(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<CreateAccountBody>,
) -> ApiResult<serde_json::Value> {
    let platform = body.platform.unwrap_or_else(|| "qq".to_string());
    let acc = qq_farm_core::models::store::accounts::Account {
        id: String::new(),
        name: body.name,
        code: body.code,
        platform,
        qq: body.qq.unwrap_or_default(),
        uin: body.uin.unwrap_or_default(),
        avatar: body.avatar.unwrap_or_default(),
        username: body.username.unwrap_or_default(),
        created_at: 0,
        updated_at: 0,
    };
    let saved = qq_farm_core::models::store::accounts::add_or_update_account(acc);
    ok(json!({ "ok": true, "account": saved }))
}

async fn remark_account(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<RemarkBody>,
) -> ApiResult<serde_json::Value> {
    let accounts = qq_farm_core::models::store::accounts::get_accounts();
    let acc = accounts
        .into_iter()
        .find(|a| a.id == body.id)
        .ok_or_else(|| ApiError::NotFound(format!("account not found: {}", body.id)))?;
    let updated = qq_farm_core::models::store::accounts::Account {
        name: body.name,
        ..acc
    };
    let saved = qq_farm_core::models::store::accounts::add_or_update_account(updated);
    ok(json!({ "ok": true, "account": saved }))
}

async fn delete_account(
    State(_ctx): State<Arc<AdminContext>>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let _ = qq_farm_core::models::store::accounts::delete_account(&id);
    ok_empty()
}

async fn get_account_logs(
    State(ctx): State<Arc<AdminContext>>,
    Query(q): Query<AccountLogsQuery>,
) -> ApiResult<serde_json::Value> {
    let state = ctx.engine.runtime_state();
    let logs = state.account_logs.lock().clone();
    let filtered: Vec<_> = if let Some(target) = q.account_id.as_deref() {
        logs.into_iter().filter(|l| l.account_id == target).collect()
    } else {
        logs
    };
    let limit = q.limit.unwrap_or(100);
    let limited: Vec<_> = filtered.into_iter().rev().take(limit).collect();
    ok(json!({ "ok": true, "logs": limited }))
}

async fn get_logs(
    State(ctx): State<Arc<AdminContext>>,
    Query(q): Query<LogsQuery>,
) -> ApiResult<serde_json::Value> {
    let state = ctx.engine.runtime_state();
    let logs = state.global_logs.lock().clone();
    let filters = qq_farm_core::runtime::runtime_state::LogFilters {
        keyword: q.keyword,
        tag: q.tag,
        module: q.module,
        event: q.event,
        is_warn: q.is_warn,
        time_from: q.time_from,
        time_to: q.time_to,
    };
    let mut filtered = state.filter_logs(&logs, &filters);
    if let Some(limit) = q.limit {
        filtered.truncate(limit);
    }
    ok(json!({ "ok": true, "logs": filtered }))
}

async fn delete_logs(
    State(ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let state = ctx.engine.runtime_state();
    state.global_logs.lock().clear();
    ok_empty()
}

async fn get_settings(
    State(ctx): State<Arc<AdminContext>>,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = q.account_id.as_deref().unwrap_or("");
    let snapshot = qq_farm_core::models::store::account_config::get_config_snapshot(Some(id));
    let mut value = serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = value.as_object_mut() {
        obj.insert("__revision".to_string(), serde_json::json!(ctx.engine.runtime_state().config_revision()));
    }
    Ok(Json(json!({ "ok": true, "settings": value })))
}

async fn save_settings(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<SettingsBody>,
) -> ApiResult<serde_json::Value> {
    let id = body.account_id.as_deref().unwrap_or("");
    let _updated = qq_farm_core::models::store::account_config::apply_config_snapshot(
        body.rest,
        Some(id),
        true,
    );
    ok_empty()
}

async fn get_default_settings(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let cfg = qq_farm_core::models::store::account_config::get_default_account_config();
    ok(json!({ "ok": true, "default": cfg }))
}

async fn set_theme(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<ThemeBody>,
) -> ApiResult<serde_json::Value> {
    let _ = body.account_id;
    qq_farm_core::models::store::global_config::set_ui_theme(&body.theme);
    ok_empty()
}

async fn set_offline_reminder(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<OfflineReminderBody>,
) -> ApiResult<serde_json::Value> {
    let cfg: qq_farm_core::models::store::global_config::OfflineReminder =
        serde_json::from_value(body.cfg).unwrap_or_default();
    let username = body.account_id.as_deref().unwrap_or("");
    if username.is_empty() {
        qq_farm_core::models::store::global_config::set_offline_reminder(cfg);
    } else {
        qq_farm_core::models::store::global_config::set_user_offline_reminder(username, cfg);
    }
    ok_empty()
}

async fn test_offline_reminder(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<OfflineReminderBody>,
) -> ApiResult<serde_json::Value> {
    // 占位：直接转发到 reminder service
    let _svc = _ctx.engine.relogin_reminder();
    let _payload = qq_farm_core::runtime::relogin_reminder::OfflineReminderPayload {
        account_id: body.account_id.clone().unwrap_or_default(),
        account_name: body.account_id.unwrap_or_default(),
        reason: "test".to_string(),
        ..Default::default()
    };
    // 真实触发：留 2B 联调（需要运行时触发）
    ok_empty()
}

#[derive(Debug, Deserialize)]
struct AccountQuery {
    #[serde(default)]
    account_id: Option<String>,
}
