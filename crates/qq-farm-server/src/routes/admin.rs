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
//! 鉴权：阶段 2A-3 占位（adminRequired 暂放过），后续 commit 接 user_store 真实鉴权。

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok, ok_empty, AdminContext, ApiError, ApiResult};

/// 构造 admin 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        // 公告
        .route("/api/announcement", get(get_announcement))
        .route("/api/announcement/read", post(mark_announcement_read))
        .route("/api/admin/announcement", post(set_announcement))
        // 系统配置
        .route("/api/admin/device-presets", get(get_device_presets))
        .route("/api/admin/system-config", get(get_system_config).post(set_system_config))
        .route("/api/admin/system-config/reset", post(reset_system_config))
        // 卡密
        .route("/api/admin/cards", get(list_cards).post(create_card))
        .route("/api/admin/cards/batch-delete", post(batch_delete_cards))
        .route("/api/admin/cards/{code}", post(update_card).delete(delete_card))
        // 卡密领取
        .route("/api/card-claim/status", get(get_card_claim_status))
        .route("/api/admin/card-claim/status", post(set_card_claim_status))
        .route("/api/card-claim/claim", post(claim_card))
        .route("/api/admin/card-claim/records", get(get_card_claim_records))
        // 用户
        .route("/api/admin/users", get(list_users))
        .route("/api/admin/users-with-password", get(list_users_with_password))
        .route("/api/admin/users/{username}", post(create_user))
        .route("/api/admin/users/{username}/edit", post(edit_user))
        .route("/api/admin/users/{username}", delete(delete_user))
        .route("/api/admin/users/{username}/renew", post(admin_renew_user))
}

#[derive(Debug, Deserialize)]
struct AnnouncementBody {
    #[serde(default)]
    title: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    version: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ReadAnnouncementBody {
    username: String,
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
struct CreateUserBody {
    password: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    card_code: Option<String>,
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

async fn get_announcement(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let ann = qq_farm_core::models::store::global_config::get_announcement();
    ok(json!({ "ok": true, "announcement": ann }))
}

async fn mark_announcement_read(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<ReadAnnouncementBody>,
) -> ApiResult<serde_json::Value> {
    qq_farm_core::models::store::global_config::mark_announcement_read(&body.username);
    ok_empty()
}

async fn set_announcement(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<AnnouncementBody>,
) -> ApiResult<serde_json::Value> {
    let ann = qq_farm_core::models::store::global_config::Announcement {
        content: body.content,
        show_once: true,
        updated_at: chrono::Utc::now().timestamp_millis(),
    };
    qq_farm_core::models::store::global_config::set_announcement(ann);
    ok_empty()
}

async fn get_device_presets(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let presets = qq_farm_core::config::system_config::get_device_presets();
    ok(json!({ "ok": true, "presets": presets }))
}

async fn get_system_config(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let cfg = qq_farm_core::models::store::global_config::get_system_config();
    match cfg {
        Some(c) => ok(json!({ "ok": true, "systemConfig": c })),
        None => Ok(Json(json!({ "ok": true, "systemConfig": null }))),
    }
}

async fn set_system_config(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<SystemConfigBody>,
) -> ApiResult<serde_json::Value> {
    let cfg: qq_farm_core::config::system_config::SystemConfig =
        serde_json::from_value(body.rest).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    qq_farm_core::models::store::global_config::set_system_config(cfg);
    ok_empty()
}

async fn reset_system_config(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    qq_farm_core::models::store::global_config::reset_system_config();
    ok_empty()
}

async fn list_cards(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let cards = qq_farm_core::models::user_store::users::get_all_cards();
    ok(json!({ "ok": true, "cards": cards }))
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
        ok(json!({ "ok": true, "cards": cards }))
    } else {
        let card = qq_farm_core::models::user_store::users::create_card(
            &body.description, body.days, ctype,
        );
        ok(json!({ "ok": true, "card": card }))
    }
}

async fn batch_delete_cards(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<BatchDeleteBody>,
) -> ApiResult<serde_json::Value> {
    let codes: Vec<&str> = body.codes.iter().map(String::as_str).collect();
    let n = qq_farm_core::models::user_store::users::delete_cards_batch(&codes);
    ok(json!({ "ok": true, "deleted": n }))
}

async fn update_card(
    State(_ctx): State<Arc<AdminContext>>,
    Path(code): Path<String>,
    Json(body): Json<UpdateCardBody>,
) -> ApiResult<serde_json::Value> {
    let card = qq_farm_core::models::user_store::users::update_card(
        &code, body.enabled, body.days, body.description,
    );
    ok(json!({ "ok": true, "card": card }))
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
    ok(json!({ "ok": true, "status": status }))
}

async fn set_card_claim_status(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<CardClaimStatusBody>,
) -> ApiResult<serde_json::Value> {
    qq_farm_core::models::user_store::card_claim::set_status(
        body.enabled.unwrap_or(true),
        body.message.clone(),
    );
    ok_empty()
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
        Ok(c) => ok(json!({ "ok": true, "card": c })),
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
    ok(json!({ "ok": true, "users": users }))
}

async fn list_users_with_password(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    // 当前 API 不暴露密码（安全考虑）；返回同 list_users
    let users = qq_farm_core::models::user_store::users::get_all_users();
    ok(json!({ "ok": true, "users": users }))
}

async fn create_user(
    State(_ctx): State<Arc<AdminContext>>,
    Path(username): Path<String>,
    Json(body): Json<CreateUserBody>,
) -> ApiResult<serde_json::Value> {
    let role = body.role.unwrap_or_else(|| "user".to_string());
    let result = qq_farm_core::models::user_store::users::create_user_with_role(
        &username, &body.password, &role, body.card_code.as_deref().unwrap_or(""),
    );
    match result {
        Ok(u) => ok(json!({ "ok": true, "user": u })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e }))),
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
        Ok(u) => ok(json!({ "ok": true, "user": u })),
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
