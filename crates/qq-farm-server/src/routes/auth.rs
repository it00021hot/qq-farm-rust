//! Auth 路由 — 面板用户登录 / 注册（无需卡密）。

use std::sync::{Arc, OnceLock};
use std::time::Instant;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::context::{ok_data, ok_empty, AdminContext, ApiError, ApiResult};

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
        .route("/api/user/change-password", post(change_password))
        .route("/api/ping", get(ping))
        .route("/api/game-version", get(game_version))
        .route("/api/auth/validate", get(validate))
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

/// 登录：validate → bindSession → addLoginLog
async fn login(
    State(ctx): State<Arc<AdminContext>>,
    headers: HeaderMap,
    Json(body): Json<LoginBody>,
) -> Response {
    let client_ip = crate::middleware::extract_client_ip(&headers);
    let user_agent =
        headers.get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or("unknown").to_string();

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
    tracing::info!(elapsed_ms = started.elapsed().as_millis() as u64, "登录密码校验完成");

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

    let username = validation.username.clone().unwrap_or(body.username.clone());
    let role = validation.role.clone().unwrap_or_else(|| "user".to_string());

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

    let account_limit = validation
        .account_limit
        .unwrap_or(qq_farm_core::models::user_store::users::DEFAULT_ACCOUNT_LIMIT);
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
        qq_farm_core::models::user_store::users::register_user(&body.username, &body.password)
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
    if let Some(t) = headers.get("x-admin-token").and_then(|v| v.to_str().ok()) {
        ctx.sessions.delete(t);
    }
    ok_empty()
}

async fn get_me(
    State(ctx): State<Arc<AdminContext>>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    let token = headers.get("x-admin-token").and_then(|v| v.to_str().ok()).unwrap_or("");
    if let Some(info) = ctx.sessions.get(token) {
        let user = qq_farm_core::models::user_store::users::get_session_user(&info.username);
        let user_json = user
            .as_ref()
            .and_then(|u| serde_json::to_value(u).ok())
            .unwrap_or(json!({ "username": info.username, "role": info.role }));
        return ok_data(json!({
            "username": info.username,
            "role": info.role,
            "accountLimit": user_json.get("accountLimit").cloned().unwrap_or(
                json!(qq_farm_core::models::user_store::users::DEFAULT_ACCOUNT_LIMIT)
            ),
        }));
    }
    Err(ApiError::Unauthorized("missing or invalid token".to_string()))
}

async fn change_password(
    State(ctx): State<Arc<AdminContext>>,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordBody>,
) -> ApiResult<serde_json::Value> {
    let target_username = if let Some(u) = body.username.clone() {
        if !u.is_empty() {
            u
        } else {
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
    let token = headers.get("x-admin-token").and_then(|v| v.to_str().ok()).unwrap_or("");
    match ctx.sessions.get(token) {
        Some(_) => {
            ctx.sessions.touch(token);
            ok_data(json!({ "valid": true }))
        }
        None => Err(ApiError::Unauthorized("invalid token".to_string())),
    }
}
