//! 本机运维门面（用户 / 系统配置 / 公告 / 登录日志）。

use qq_farm_core::config::system_config::SystemConfig;
use qq_farm_core::config::update_runtime_config;
use qq_farm_core::models::store::global_config::Announcement;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};

/// 系统配置。
#[must_use]
pub fn get_system_config() -> Value {
    json!(qq_farm_core::models::store::global_config::get_system_config())
}

pub fn set_system_config(cfg: Value) -> AppResult<Value> {
    let sys: SystemConfig =
        serde_json::from_value(cfg).map_err(|e| AppError::BadRequest(e.to_string()))?;
    qq_farm_core::models::store::global_config::set_system_config(sys.clone());
    update_runtime_config(&sys);
    Ok(json!(sys))
}

pub fn reset_system_config() -> Value {
    qq_farm_core::models::store::global_config::reset_system_config();
    let default = qq_farm_core::config::get_default_system_config();
    update_runtime_config(&default);
    json!({
        "saved": Value::Null,
        "default": default,
        "current": qq_farm_core::config::get_runtime_config(),
    })
}

#[must_use]
pub fn device_presets() -> Value {
    json!(qq_farm_core::config::system_config::get_device_presets())
}

/// 公告。
pub fn set_announcement(content: &str, show_once: bool) {
    let ann = Announcement {
        content: content.to_string(),
        show_once,
        updated_at: chrono::Utc::now().timestamp_millis(),
    };
    qq_farm_core::models::store::global_config::set_announcement(ann);
}

#[must_use]
pub fn get_announcement() -> Value {
    json!(qq_farm_core::models::store::global_config::get_announcement())
}

/// 用户列表。
#[must_use]
pub fn list_users() -> Value {
    json!(qq_farm_core::models::user_store::users::get_all_users())
}

pub fn delete_user(username: &str) -> bool {
    qq_farm_core::models::user_store::users::delete_user(username)
}

/// 登录日志。
#[must_use]
pub fn login_logs(limit: usize, offset: usize) -> Value {
    let (logs, total) =
        qq_farm_core::models::user_store::auth::get_login_logs(limit.max(1), offset);
    json!({ "logs": logs, "total": total })
}

pub fn clear_login_logs() {
    qq_farm_core::models::user_store::auth::clear_login_logs();
}
