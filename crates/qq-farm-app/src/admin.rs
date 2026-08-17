//! 本机运维门面（卡密 / 用户 / 系统配置 / 登录日志）。

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

/// 卡密列表。
#[must_use]
pub fn list_cards() -> Value {
    json!(qq_farm_core::models::user_store::users::get_all_cards())
}

pub fn create_card(
    description: &str,
    days: i64,
    card_type: Option<&str>,
    count: Option<i64>,
) -> Value {
    let ty = card_type.unwrap_or("time");
    let n = count.unwrap_or(1).max(1);
    if n <= 1 {
        json!(qq_farm_core::models::user_store::users::create_card(
            description, days, ty
        ))
    } else {
        json!(qq_farm_core::models::user_store::users::create_cards_batch(
            description, days, n, ty
        ))
    }
}

pub fn update_card(
    code: &str,
    enabled: Option<bool>,
    days: Option<i64>,
    description: Option<String>,
) -> AppResult<Value> {
    let card =
        qq_farm_core::models::user_store::users::update_card(code, enabled, days, description)
            .ok_or_else(|| AppError::NotFound(format!("card not found: {code}")))?;
    Ok(json!(card))
}

pub fn delete_card(code: &str) -> bool {
    qq_farm_core::models::user_store::users::delete_card(code)
}

pub fn batch_delete_cards(codes: &[String]) -> usize {
    let refs: Vec<&str> = codes.iter().map(String::as_str).collect();
    qq_farm_core::models::user_store::users::delete_cards_batch(&refs)
}

/// 用户列表。
#[must_use]
pub fn list_users() -> Value {
    json!(qq_farm_core::models::user_store::users::get_all_users())
}

pub fn delete_user(username: &str) -> bool {
    qq_farm_core::models::user_store::users::delete_user(username)
}

pub fn renew_user(username: &str, card_code: &str) -> AppResult<Value> {
    qq_farm_core::models::user_store::users::renew_user(username, card_code)
        .map(|u| json!(u))
        .map_err(AppError::BadRequest)
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

/// 卡密领取配置。
#[must_use]
pub fn card_claim_status() -> Value {
    json!({ "enabled": qq_farm_core::models::user_store::card_claim::get_card_claim_status() })
}

pub fn set_card_claim_status(enabled: bool) -> Value {
    json!({
        "enabled": qq_farm_core::models::user_store::card_claim::set_card_claim_status(enabled)
    })
}

#[must_use]
pub fn card_claim_records() -> Value {
    json!(qq_farm_core::models::user_store::card_claim::get_card_claim_records())
}
