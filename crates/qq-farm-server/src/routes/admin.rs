//! Admin 路由 — 22 端点。
//!
//! 1:1 对应原 `controllers/admin/admin-routes.ts`（405 行）。
//!
//! 包含：
//! - 公告（announcement）— 公开 + 管理员
//! - 系统配置（system-config）— 管理员
//! - 设备预设（device-presets）— 管理员
//! - 卡密（cards）— 管理员
//! - 卡密领取（card-claim）— 公开 + 管理员
//! - 用户（users）— 管理员
//!
//! 鉴权：admin 路由走 `admin_required`；公开卡密领取走 `public_router`。

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok, ok_data, ok_empty, AdminContext, ApiError, ApiResult};

/// 构造 admin 路由（带 admin 鉴权）
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/admin/announcement", post(set_announcement))
        // 系统配置
        .route("/api/admin/device-presets", get(get_device_presets))
        .route("/api/admin/system-config", get(get_system_config).post(set_system_config))
        .route("/api/admin/system-config/reset", post(reset_system_config))
        // 卡密
        .route("/api/admin/cards", get(list_cards).post(create_card))
        .route("/api/admin/cards/batch-delete", post(batch_delete_cards))
        .route("/api/admin/cards/{code}", post(update_card).delete(delete_card))
        // 卡密领取配置（admin）
        .route("/api/admin/card-claim/status", post(set_card_claim_status))
        .route("/api/admin/card-claim/records", get(get_card_claim_records))
        // 用户管理（admin）
        .route("/api/admin/users", get(list_users))
        .route("/api/admin/users-with-password", get(list_users_with_password))
        .route("/api/admin/users/{username}", post(create_user))
        .route("/api/admin/users/{username}/edit", post(edit_user))
        .route("/api/admin/users/{username}", delete(delete_user))
        .route("/api/admin/users/{username}/renew", post(admin_renew_user))
        // 登录日志（admin）
        .route(
            "/api/admin/login-logs",
            get(super::auth::admin_list_login_logs).delete(super::auth::admin_delete_login_logs),
        )
}

/// 公开卡密领取（无需 admin）
pub fn public_router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/card-claim/status", get(get_card_claim_status))
        .route("/api/card-claim/claim", post(claim_card))
}

#[derive(Debug, Deserialize)]
struct AnnouncementBody {
    #[serde(default)]
    title: String,
    #[serde(default)]
    content: String,
    #[serde(default, alias = "showOnce")]
    show_once: Option<bool>,
    #[serde(default)]
    version: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct SystemConfigBody {
    #[serde(flatten)]
    rest: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct CardBody {
    description: String,
    days: i64,
    #[serde(default)]
    card_type: Option<String>,
    #[serde(default)]
    count: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct BatchDeleteBody {
    codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateCardBody {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    days: Option<i64>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CardClaimStatusBody {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaimCardBody {
    ua: String,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateUserBody {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default, alias = "expiresAt")]
    expires_at: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct EditUserBody {
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    card_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AdminRenewBody {
    card_code: String,
}

async fn set_announcement(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<AnnouncementBody>,
) -> ApiResult<serde_json::Value> {
    let ann = qq_farm_core::models::store::global_config::Announcement {
        content: body.content,
        show_once: body.show_once.unwrap_or(true),
        updated_at: chrono::Utc::now().timestamp_millis(),
    };
    qq_farm_core::models::store::global_config::set_announcement(ann.clone());
    ok_data(ann)
}

async fn get_device_presets(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let presets = qq_farm_core::config::system_config::get_device_presets();
    ok_data(presets)
}

async fn get_system_config(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let saved = qq_farm_core::models::store::global_config::get_system_config();
    let default = qq_farm_core::config::get_default_system_config();
    let current = qq_farm_core::config::get_runtime_config();
    ok_data(json!({
        "saved": saved,
        "default": default,
        "current": current,
    }))
}

async fn set_system_config(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<SystemConfigBody>,
) -> ApiResult<serde_json::Value> {
    let cfg: qq_farm_core::config::system_config::SystemConfig =
        serde_json::from_value(body.rest).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    qq_farm_core::models::store::global_config::set_system_config(cfg.clone());
    qq_farm_core::config::update_runtime_config(&cfg);
    let current = qq_farm_core::config::get_runtime_config();
    ok_data(json!({ "saved": cfg, "current": current }))
}

async fn reset_system_config(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    qq_farm_core::models::store::global_config::reset_system_config();
    let saved = qq_farm_core::config::get_default_system_config();
    qq_farm_core::config::update_runtime_config(&saved);
    let current = qq_farm_core::config::get_runtime_config();
    ok_data(json!({ "saved": saved, "current": current }))
}

async fn list_cards(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let cards = qq_farm_core::models::user_store::users::get_all_cards();
    ok_data(cards)
}

async fn create_card(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<CardBody>,
) -> ApiResult<serde_json::Value> {
    let ctype = body.card_type.as_deref().unwrap_or("time");
    let count = body.count.unwrap_or(1);
    if count > 1 {
        let cards = qq_farm_core::models::user_store::users::create_cards_batch(
            &body.description, body.days, count, ctype,
        );
        Ok(Json(json!({
            "ok": true,
            "data": cards,
            "batch": true,
            "count": cards.len(),
        })))
    } else {
        let card = qq_farm_core::models::user_store::users::create_card(
            &body.description, body.days, ctype,
        );
        ok_data(card)
    }
}

async fn batch_delete_cards(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<BatchDeleteBody>,
) -> ApiResult<serde_json::Value> {
    let codes: Vec<&str> = body.codes.iter().map(String::as_str).collect();
    let n = qq_farm_core::models::user_store::users::delete_cards_batch(&codes);
    let not_found = body.codes.len().saturating_sub(n);
    Ok(Json(json!({
        "ok": true,
        "deletedCount": n,
        "notFoundCount": not_found,
    })))
}

async fn update_card(
    State(_ctx): State<Arc<AdminContext>>,
    Path(code): Path<String>,
    Json(body): Json<UpdateCardBody>,
) -> ApiResult<serde_json::Value> {
    let card = qq_farm_core::models::user_store::users::update_card(
        &code, body.enabled, body.days, body.description,
    );
    match card {
        Some(c) => ok_data(c),
        None => Err(ApiError::NotFound("卡密不存在".to_string())),
    }
}

async fn delete_card(
    State(_ctx): State<Arc<AdminContext>>,
    Path(code): Path<String>,
) -> ApiResult<serde_json::Value> {
    let r = qq_farm_core::models::user_store::users::delete_card(&code);
    ok(json!({ "ok": r }))
}

async fn get_card_claim_status(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let status = qq_farm_core::models::user_store::card_claim::get_status();
    Ok(Json(json!({ "ok": true, "enabled": status.enabled })))
}

async fn set_card_claim_status(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<CardClaimStatusBody>,
) -> ApiResult<serde_json::Value> {
    qq_farm_core::models::user_store::card_claim::set_status(
        body.enabled.unwrap_or(true),
        body.message.clone(),
    );
    Ok(Json(json!({ "ok": true, "enabled": body.enabled.unwrap_or(true) })))
}

async fn claim_card(
    State(_ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<ClaimCardBody>,
) -> ApiResult<serde_json::Value> {
    let ua = if body.ua.is_empty() {
        headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string()
    } else {
        body.ua
    };
    let r = qq_farm_core::models::user_store::card_claim::claim_card_by_ua(
        &ua, body.username.as_deref(),
    );
    match r {
        Ok(c) => Ok(Json(json!({
            "ok": true,
            "cardCode": c.card_code,
            "days": c.days,
            "description": c.description,
        }))),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e }))),
    }
}

async fn get_card_claim_records(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let records = qq_farm_core::models::user_store::card_claim::get_card_claim_records();
    ok(json!({ "ok": true, "records": records }))
}

async fn list_users(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let users = qq_farm_core::models::user_store::users::get_all_users();
    ok_data(users)
}

async fn list_users_with_password(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let users = qq_farm_core::models::user_store::users::get_all_users();
    ok_data(users)
}

async fn create_user(
    State(_ctx): State<Arc<AdminContext>>,
    Path(username): Path<String>,
    Json(body): Json<UpdateUserBody>,
) -> ApiResult<serde_json::Value> {
    let expires = match &body.expires_at {
        None => None,
        Some(serde_json::Value::Null) => Some(None),
        Some(v) => Some(v.as_i64()),
    };
    match qq_farm_core::models::user_store::users::update_user(
        &username,
        expires,
        body.enabled,
    ) {
        Some(u) => ok_data(u),
        None => Err(ApiError::NotFound("用户不存在".to_string())),
    }
}

async fn edit_user(
    State(_ctx): State<Arc<AdminContext>>,
    Path(username): Path<String>,
    Json(body): Json<EditUserBody>,
) -> ApiResult<serde_json::Value> {
    let updates = qq_farm_core::models::user_store::users::EditUpdates {
        new_username: None,
        password: body.password,
        account_limit: None,
        is_permanent: false,
        expires_at: None,
        role: body.role,
        enabled: body.enabled,
        card_code: body.card_code,
    };
    let result = qq_farm_core::models::user_store::users::edit_user(&username, updates);
    match result {
        Ok(u) => ok_data(u),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e }))),
    }
}

async fn delete_user(
    State(_ctx): State<Arc<AdminContext>>,
    Path(username): Path<String>,
) -> ApiResult<serde_json::Value> {
    let r = qq_farm_core::models::user_store::users::delete_user(&username);
    ok(json!({ "ok": r }))
}

async fn admin_renew_user(
    State(_ctx): State<Arc<AdminContext>>,
    Path(username): Path<String>,
    Json(body): Json<AdminRenewBody>,
) -> ApiResult<serde_json::Value> {
    let r = qq_farm_core::models::user_store::users::renew_user(&username, &body.card_code);
    match r {
        Ok(ret) => ok(json!({ "ok": true, "card": ret.card, "addedSec": ret.added_sec.unwrap_or(0) })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e }))),
    }
}
