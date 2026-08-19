//! Admin 路由 — 系统配置 / 设备预设。
//!
//! 鉴权：admin 路由走 `admin_required`。

use std::sync::Arc;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok_data, AdminContext, ApiError, ApiResult};

/// 构造 admin 路由（带 admin 鉴权）
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/admin/device-presets", get(get_device_presets))
        .route("/api/admin/system-config", get(get_system_config).post(set_system_config))
        .route("/api/admin/system-config/reset", post(reset_system_config))
        .route("/api/settings/device-presets", get(get_device_presets))
        .route("/api/settings/system-config", get(get_system_config).post(set_system_config))
        .route("/api/settings/system-config/reset", post(reset_system_config))
}

#[derive(Debug, Deserialize)]
struct SystemConfigBody {
    #[serde(flatten)]
    rest: serde_json::Value,
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
