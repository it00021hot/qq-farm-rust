//! Admin 路由 — 用户 / 系统配置 / 公告 / 登录日志。
//!
//! 包含：
//! - 公告（announcement）— 管理员
//! - 系统配置（system-config）— 管理员
//! - 设备预设（device-presets）— 管理员
//! - 用户（users）— 管理员
//!
//! 鉴权：admin 路由走 `admin_required`。

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok, ok_data, AdminContext, ApiError, ApiResult};

/// 构造 admin 路由（带 admin 鉴权）
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/admin/announcement", post(set_announcement))
        // 系统配置
        .route("/api/admin/device-presets", get(get_device_presets))
        .route("/api/admin/system-config", get(get_system_config).post(set_system_config))
        .route("/api/admin/system-config/reset", post(reset_system_config))
        // 用户管理（admin）
        .route("/api/admin/users", get(list_users))
        .route("/api/admin/users-with-password", get(list_users_with_password))
        .route("/api/admin/users/{username}", post(create_user))
        .route("/api/admin/users/{username}/edit", post(edit_user))
        .route("/api/admin/users/{username}", delete(delete_user))
        // 登录日志（admin）
        .route(
            "/api/admin/login-logs",
            get(super::auth::admin_list_login_logs).delete(super::auth::admin_delete_login_logs),
        )
}

#[derive(Debug, Deserialize)]
struct AnnouncementBody {
    #[serde(default)]
    content: String,
    #[serde(default, alias = "showOnce")]
    show_once: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SystemConfigBody {
    #[serde(flatten)]
    rest: serde_json::Value,
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

async fn get_device_presets(State(_ctx): State<Arc<AdminContext>>) -> ApiResult<serde_json::Value> {
    let presets = qq_farm_core::config::system_config::get_device_presets();
    ok_data(presets)
}

async fn get_system_config(State(_ctx): State<Arc<AdminContext>>) -> ApiResult<serde_json::Value> {
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

async fn list_users(State(_ctx): State<Arc<AdminContext>>) -> ApiResult<serde_json::Value> {
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
    match qq_farm_core::models::user_store::users::update_user(&username, expires, body.enabled) {
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
        card_code: None,
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
