//! Auth 路由 — 13 端点。
//!
//! 1:1 对应原 `controllers/admin/auth-routes.ts`（327 行）。

use std::sync::{Arc, OnceLock};
use std::time::Instant;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::context::{ok, ok_data, ok_empty, AdminContext, ApiError, ApiResult};

static STARTED_AT: OnceLock<Instant> = OnceLock::new();

fn server_uptime_secs() -> f64 {
    STARTED_AT.get_or_init(Instant::now).elapsed().as_secs_f64()
}

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
        // /api/admin/login-logs 已在 admin::router() 中通过 super::auth::admin_list_login_logs 暴露，这里不再加
        .route("/api/card/info/{code}", get(card_info))
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
    #[serde(alias = "cardCode", alias = "card_code")]
    card_code: String,
}

#[derive(Debug, Deserialize)]
struct RenewBody {
    #[serde(alias = "cardCode", alias = "card_code")]
    card_code: String,
    #[serde(default)]
    username: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChangePasswordBody {
    /// 可选：admin 修改他人密码时填；普通用户改自己密码不填，从 token 推断
    #[serde(default)]
    username: Option<String>,
    #[serde(alias = "oldPassword", alias = "old_password")]
    old_password: String,
    #[serde(alias = "newPassword", alias = "new_password")]
    new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginLogsQuery {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
}

/// 登录（1:1 对应原 TS）：validate → banned/expired 二次校验 → bindSession → addLoginLog
async fn login(
    State(ctx): State<Arc<AdminContext>>,
    headers: HeaderMap,
    Json(body): Json<LoginBody>,
) -> Response {
    let client_ip = crate::middleware::extract_client_ip(&headers);
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let username = body.username.clone();
    let password = body.password.clone();
    let ip = client_ip.clone();
    let started = Instant::now();
    let validation = match tokio::task::spawn_blocking(move || {
        qq_farm_core::models::user_store::users::validate_user(&username, &password, &ip)
    })
    .await
    {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": "登录校验中断" })),
            )
                .into_response();
        }
    };
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis() as u64,
        "登录密码校验完成"
    );

    // 失败分支：error 字段 + 写失败日志
    if let Some(err) = &validation.error {
        let (status, payload) = match err.as_str() {
            "rate_limit" => (
                StatusCode::TOO_MANY_REQUESTS,
                json!({
                    "ok": false,
                    "error": validation.message,
                    "errorType": err,
                    "remainingMs": validation.remaining_ms,
                }),
            ),
            "locked" => (
                StatusCode::LOCKED,
                json!({
                    "ok": false,
                    "error": validation.message,
                    "errorType": err,
                    "remainingMs": validation.remaining_ms,
                }),
            ),
            _ => (
                StatusCode::UNAUTHORIZED,
                json!({
                    "ok": false,
                    "error": validation.message.clone().unwrap_or_else(|| "用户名或密码错误".to_string()),
                    "errorType": err,
                }),
            ),
        };
        qq_farm_core::models::user_store::auth::add_login_log(json!({
            "event": "login_failed",
            "username": body.username,
            "errorType": err,
            "ip": client_ip,
            "userAgent": user_agent,
        }));
        return (status, Json(payload)).into_response();
    }

    // validate_user 成功时：username/role/card 都在 validation 里
    let username = validation.username.clone().unwrap_or(body.username.clone());
    let role = validation.role.clone().unwrap_or_else(|| "user".to_string());

    // banned 检查
    if role != "admin" {
        if let Some(card) = &validation.card {
            if !card.enabled {
                qq_farm_core::models::user_store::auth::add_login_log(json!({
                    "event": "login_failed",
                    "username": username,
                    "errorType": "banned",
                    "ip": client_ip,
                    "userAgent": user_agent,
                }));
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "ok": false,
                        "error": "账号已被封禁，请联系管理员",
                    })),
                )
                    .into_response();
            }
            if let Some(expires_at) = card.expires_at {
                if expires_at < now_ms() {
                    qq_farm_core::models::user_store::auth::add_login_log(json!({
                        "event": "login_failed",
                        "username": username,
                        "errorType": "expired",
                        "ip": client_ip,
                        "userAgent": user_agent,
                    }));
                    return (
                        StatusCode::FORBIDDEN,
                        Json(json!({
                            "ok": false,
                            "error": "账号已过期，请续费后重新登录",
                        })),
                    )
                        .into_response();
                }
            }
        }
    }

    // 发 token + bindSession
    let token = Uuid::new_v4().to_string();
    ctx.sessions.create(token.clone(), username.clone(), role.clone());

    qq_farm_core::models::user_store::auth::add_login_log(json!({
        "event": "login_success",
        "username": username,
        "errorType": null,
        "ip": client_ip,
        "userAgent": user_agent,
    }));

    tracing::info!(username = %username, role = %role, ip = %client_ip, "登录成功");

    let card_json = validation
        .card
        .as_ref()
        .and_then(|_| serde_json::to_value(&validation.card).ok());
    let account_limit = validation.account_limit.unwrap_or(0);
    let user_obj = qq_farm_core::models::user_store::users::get_session_user(&username);
    let user_json = user_obj
        .as_ref()
        .and_then(|u| serde_json::to_value(u).ok())
        .unwrap_or(json!({ "username": username, "role": role }));

    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "data": {
                "token": token,
                "role": role,
                "card": card_json,
                "accountLimit": account_limit,
                "user": user_json,
                "username": username,
                "mustChangePassword": user_obj.as_ref().and_then(|u| u.must_change_password).unwrap_or(false),
            }
        })),
    )
        .into_response()
}

async fn register(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<RegisterBody>,
) -> ApiResult<serde_json::Value> {
    let result = tokio::task::spawn_blocking(move || {
        qq_farm_core::models::user_store::users::register_user(
            &body.username,
            &body.password,
            &body.card_code,
        )
    })
    .await
    .map_err(|_| ApiError::Internal("注册中断".to_string()))?;
    match result {
        Ok(user) => ok_data(user),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e }))),
    }
}

async fn logout(
    State(ctx): State<Arc<AdminContext>>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    // 优先从 body 解析 token；fallback 到 header
    if let Some(t) = headers.get("x-admin-token").and_then(|v| v.to_str().ok()) {
        ctx.sessions.delete(t);
    }
    ok_empty()
}

async fn get_me(
    State(ctx): State<Arc<AdminContext>>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    let token = headers
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if let Some(info) = ctx.sessions.get(token) {
        let user = qq_farm_core::models::user_store::users::get_session_user(&info.username);
        let user_json = user
            .as_ref()
            .and_then(|u| serde_json::to_value(u).ok())
            .unwrap_or(json!({ "username": info.username, "role": info.role }));
        return ok_data(json!({
            "username": info.username,
            "role": info.role,
            "card": user_json.get("card").cloned().unwrap_or(json!(null)),
            "accountLimit": user_json.get("accountLimit").cloned().unwrap_or(json!(2)),
        }));
    }
    Err(ApiError::Unauthorized("missing or invalid token".to_string()))
}

async fn renew(
    State(_ctx): State<Arc<AdminContext>>,
    headers: HeaderMap,
    Json(body): Json<RenewBody>,
) -> ApiResult<serde_json::Value> {
    let username = body
        .username
        .clone()
        .or_else(|| {
            headers
                .get("x-username")
                .and_then(|v| v.to_str().ok().map(String::from))
        })
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
    State(ctx): State<Arc<AdminContext>>,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordBody>,
) -> ApiResult<serde_json::Value> {
    // 解析 target username：body.username 优先；否则从 x-admin-token 取
    let target_username = if let Some(u) = body.username.clone() {
        if !u.is_empty() {
            u
        } else {
            // fallback to token
            headers
                .get("x-admin-token")
                .and_then(|v| v.to_str().ok())
                .and_then(|t| ctx.sessions.get_username(t))
                .unwrap_or_default()
        }
    } else {
        headers
            .get("x-admin-token")
            .and_then(|v| v.to_str().ok())
            .and_then(|t| ctx.sessions.get_username(t))
            .unwrap_or_default()
    };
    if target_username.is_empty() {
        return Ok(Json(json!({ "ok": false, "error": "未登录或 username 缺失" })));
    }
    let result = qq_farm_core::models::user_store::users::change_password(
        &target_username,
        &body.old_password,
        &body.new_password,
    );
    if result.is_ok() {
        // 改密成功 → 让该用户所有 session 失效
        ctx.sessions.invalidate_by_username(&target_username);
    }
    match result {
        Ok(()) => ok_empty(),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e }))),
    }
}

async fn ping(State(_ctx): State<Arc<AdminContext>>) -> ApiResult<serde_json::Value> {
    ok_data(json!({
        "ok": true,
        "uptime": server_uptime_secs(),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn game_version(State(_ctx): State<Arc<AdminContext>>) -> ApiResult<serde_json::Value> {
    let cv = qq_farm_core::config::get_runtime_config().client_version;
    Ok(Json(json!({ "ok": true, "clientVersion": cv })))
}

/// /api/auth/validate：真验证 token 是否存在 + 触碰更新 last_active
async fn validate(
    State(ctx): State<Arc<AdminContext>>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    let token = headers
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    match ctx.sessions.get(token) {
        Some(_) => {
            ctx.sessions.touch(token);
            ok_data(json!({ "valid": true }))
        }
        None => Err(ApiError::Unauthorized("invalid token".to_string())),
    }
}

async fn scheduler(State(ctx): State<Arc<AdminContext>>) -> ApiResult<serde_json::Value> {
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
    ok_data(json!({ "logs": logs, "total": total }))
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
    let cards = qq_farm_core::models::user_store::users::get_all_cards();
    let card = cards.into_iter().find(|c| c.code == code);
    match card {
        Some(c) => ok_data(json!({
            "type": c.card_type,
            "days": c.days,
            "description": c.description,
        })),
        None => Ok(Json(json!({ "ok": false, "error": "card not found" }))),
    }
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// admin 用的 login-logs GET（从 admin 路由引用）
pub async fn admin_list_login_logs(
    State(_ctx): State<Arc<AdminContext>>,
    Query(q): Query<LoginLogsQuery>,
) -> ApiResult<serde_json::Value> {
    get_login_logs(State(_ctx), Query(q)).await
}

/// admin 用的 login-logs DELETE
pub async fn admin_delete_login_logs(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    delete_login_logs(State(_ctx)).await
}
