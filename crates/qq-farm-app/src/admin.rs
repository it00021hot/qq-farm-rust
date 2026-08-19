//! 本机运维门面（系统配置 / 设备预设）。

use qq_farm_core::config::system_config::SystemConfig;
use qq_farm_core::config::update_runtime_config;
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
