//! Auth 路由 — 13 端点。
//!
//! 1:1 对应原 `controllers/admin/auth-routes.ts`（327 行）。

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok, ok_empty, AdminContext, ApiError, ApiResult};

/// 构造 auth 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/login", post(login))
        .route("/api/register", post(register))
        .route("/api/logout", post(logout))
        .route("/api/user/me", get(get_me))
        .route("/api/user/renew", post(renew))
        .route("/api/user/change-password", post(change_password))
        .route("/api/ping", get(ping))
        .route("/api/game-version", get(game_version))
        .route("/api/auth/validate", get(validate))
        .route("/api/scheduler", get(scheduler))
        .route("/api/admin/login-logs", get(get_login_logs).delete(delete_login_logs))
        .route("/api/card/info/:code", get(card_info))
}

#[derive(Debug, Deserialize)]
struct LoginBody {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct RegisterBody {
    username: String,
    password: String,
    card_code: String,
}

#[derive(Debug, Deserialize)]
struct RenewBody {
    card_code: String,
    #[serde(default)]
    username: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChangePasswordBody {
    username: String,
    old_password: String,
    new_password: String,
}

#[derive(Debug, Deserialize)]
struct LoginLogsQuery {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct CardQuery {
    code: String,
}

async fn login(
    State(_ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<LoginBody>,
) -> ApiResult<serde_json::Value> {
    let ip = crate::middleware::extract_client_ip(&headers);
    let result = qq_farm_core::models::user_store::users::validate_user(
        &body.username,
        &body.password,
        &ip,
    );
    let mut value = serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = value.as_object_mut() {
        let ok_flag = result.error.is_none();
        obj.insert("ok".to_string(), serde_json::json!(ok_flag));
    }
    Ok(Json(value))
}

async fn register(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<RegisterBody>,
) -> ApiResult<serde_json::Value> {
    let result = qq_farm_core::models::user_store::users::register_user(
        &body.username,
        &body.password,
        &body.card_code,
    );
    match result {
        Ok(user) => ok(json!({ "ok": true, "user": user })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e }))),
    }
}

async fn logout(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    ok_empty()
}

async fn get_me(
    State(_ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<serde_json::Value> {
    let username = headers
        .get("x-username")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let user = qq_farm_core::models::user_store::users::get_session_user(username);
    match user {
        Some(u) => ok(json!({ "ok": true, "user": u })),
        None => Ok(Json(json!({ "ok": false, "error": "Unauthorized" }))),
    }
}

async fn renew(
    State(_ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<RenewBody>,
) -> ApiResult<serde_json::Value> {
    let username = body
        .username
        .clone()
        .or_else(|| headers.get("x-username").and_then(|v| v.to_str().ok().map(String::from)))
        .unwrap_or_default();
    if username.is_empty() {
        return Err(ApiError::BadRequest("missing username".to_string()));
    }
    let result = qq_farm_core::models::user_store::users::renew_user(&username, &body.card_code);
    match result {
        Ok(r) => ok(json!({ "ok": true, "card": r.card, "addedSec": r.added_sec.unwrap_or(0) })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e }))),
    }
}

async fn change_password(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<ChangePasswordBody>,
) -> ApiResult<serde_json::Value> {
    let result = qq_farm_core::models::user_store::users::change_password(
        &body.username,
        &body.old_password,
        &body.new_password,
    );
    match result {
        Ok(()) => ok_empty(),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e }))),
    }
}

async fn ping(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    ok(json!({ "ok": true, "pong": true }))
}

async fn game_version(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    // 阶段 2A 占位（实际从 game_config 读 latest）
    ok(json!({ "ok": true, "version": "1.0.0" }))
}

async fn validate(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    ok(json!({ "ok": true }))
}

async fn scheduler(
    State(ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let state = ctx.engine.runtime_state();
    let workers: Vec<_> = state
        .workers
        .lock()
        .iter()
        .map(|(id, w)| serde_json::json!({ "accountId": id, "name": w.account_name }))
        .collect();
    let global_logs = state.global_logs.lock().len();
    let account_logs = state.account_logs.lock().len();
    ok(json!({
        "ok": true,
        "workers": workers,
        "globalLogCount": global_logs,
        "accountLogCount": account_logs,
    }))
}

async fn get_login_logs(
    State(_ctx): State<Arc<AdminContext>>,
    Query(q): Query<LoginLogsQuery>,
) -> ApiResult<serde_json::Value> {
    let limit = q.limit.unwrap_or(100);
    let offset = q.offset.unwrap_or(0);
    let (logs, total) = qq_farm_core::models::user_store::auth::get_login_logs(limit, offset);
    ok(json!({ "ok": true, "logs": logs, "total": total }))
}

async fn delete_login_logs(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    qq_farm_core::models::user_store::auth::clear_login_logs();
    ok_empty()
}

async fn card_info(
    State(_ctx): State<Arc<AdminContext>>,
    Path(code): Path<String>,
) -> ApiResult<serde_json::Value> {
    let _q = CardQuery { code: code.clone() };
    let cards = qq_farm_core::models::user_store::users::get_all_cards();
    let card = cards.into_iter().find(|c| c.code == code);
    match card {
        Some(c) => ok(json!({ "ok": true, "card": c })),
        None => Ok(Json(json!({ "ok": false, "error": "card not found" }))),
    }
}
