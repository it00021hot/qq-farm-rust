//! 设置面板编排。

use qq_farm_core::models::store::global_config::{
    effective_qq_bot_credentials, set_qq_bot_credentials, NotificationProvider, OfflineReminder,
    QqBotCredentials,
};
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::session::AppContext;

/// 设置面板聚合（对齐 web `/api/settings`）。
#[must_use]
pub fn settings_panel(account_id: &str, username: &str) -> Value {
    use qq_farm_core::models::store::account_config as cfg;
    let id = if account_id.is_empty() { None } else { Some(account_id) };
    let intervals = cfg::get_intervals(id);
    let strategy = cfg::get_planting_strategy(id);
    let preferred = cfg::get_preferred_seed(id);
    let quiet = cfg::get_friend_quiet_hours(id);
    let automation = cfg::get_automation(id);
    let snap = cfg::get_config_snapshot(id).config;
    let ui = qq_farm_core::models::store::global_config::get_ui();
    let offline = offline_reminder_view(Some(username));
    json!({
        "intervals": {
            "farm": intervals.farm,
            "farmMin": intervals.farm_min,
            "farmMax": intervals.farm_max,
            "helpMin": intervals.help_min,
            "helpMax": intervals.help_max,
            "stealMin": intervals.steal_min,
            "stealMax": intervals.steal_max,
        },
        "strategy": strategy,
        "plantingStrategy": strategy,
        "preferredSeed": preferred,
        "friendQuietHours": quiet,
        "automation": automation,
        "stealDelaySeconds": snap.steal_delay_seconds,
        "plantOrderRandom": snap.plant_order_random,
        "plantDelaySeconds": snap.plant_delay_seconds,
        "fertilizerBuyOrganicCount": snap.fertilizer_buy_organic_count,
        "fertilizerBuyOrganicThresholdHours": snap.fertilizer_buy_organic_threshold_hours,
        "fertilizerBuyNormalCount": snap.fertilizer_buy_normal_count,
        "fertilizerBuyNormalThresholdHours": snap.fertilizer_buy_normal_threshold_hours,
        "fertilizerBuyCheckIntervalMinutes": snap.fertilizer_buy_check_interval_minutes,
        "bagSeedPriority": cfg::get_bag_seed_priority(id),
        "bagSeedFallbackStrategy": cfg::get_bag_seed_fallback_strategy(id),
        "friendBlacklist": cfg::get_friend_blacklist(id),
        "plantBlacklist": cfg::get_plant_blacklist(id),
        "ui": ui,
        "offlineReminder": offline,
    })
}

/// 保存设置快照并可选 reload worker。
pub fn save_settings(ctx: &AppContext, account_id: &str, snapshot: Value) -> AppResult<Value> {
    if account_id.is_empty() {
        return Err(AppError::BadRequest("Missing account id".into()));
    }
    let _ = qq_farm_core::models::store::account_config::apply_config_snapshot(
        snapshot,
        Some(account_id),
        true,
    );
    let rev = ctx.engine.runtime_state().next_config_revision();
    let running = ctx.engine.has_worker(account_id);
    if running {
        ctx.engine.reload_worker_config(account_id);
    }
    let mut data = settings_panel(account_id, "");
    if let Some(obj) = data.as_object_mut() {
        obj.insert("saved".to_string(), json!(true));
        obj.insert("configRevision".to_string(), json!(rev));
        obj.insert("status".to_string(), json!(if running { "confirmed" } else { "stopped" }));
        obj.insert("stopped".to_string(), json!(!running));
        obj.insert("confirmed".to_string(), json!(running));
    }
    Ok(json!({
        "saved": true,
        "stopped": !running,
        "confirmed": running,
        "status": if running { "confirmed" } else { "stopped" },
        "data": data,
    }))
}

/// 默认账号配置。
#[must_use]
pub fn default_settings() -> Value {
    json!(qq_farm_core::models::store::account_config::get_default_account_config())
}

/// 读取离线提醒（用户覆盖优先，否则全局默认）。
#[must_use]
pub fn get_offline_reminder(username: Option<&str>) -> OfflineReminder {
    if let Some(u) = username.filter(|s| !s.is_empty()) {
        qq_farm_core::models::store::global_config::get_user_offline_reminder(u)
            .unwrap_or_else(qq_farm_core::models::store::global_config::get_offline_reminder)
    } else {
        qq_farm_core::models::store::global_config::get_offline_reminder()
    }
}

/// 设置面板用的离线提醒 JSON（附带机器人凭据，供填写 AppID/AppSecret）。
#[must_use]
pub fn offline_reminder_view(username: Option<&str>) -> Value {
    let reminder = get_offline_reminder(username);
    let mut value = serde_json::to_value(reminder).unwrap_or_else(|_| json!({}));
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "qqBot".into(),
            serde_json::to_value(effective_qq_bot_credentials()).unwrap_or_else(|_| json!({})),
        );
    }
    value
}

/// 设置离线提醒（全局或用户）。
pub fn set_offline_reminder(username: Option<&str>, cfg: Value) {
    if let Some(raw) = cfg.get("qqBot").cloned() {
        if let Ok(credentials) = serde_json::from_value::<QqBotCredentials>(raw) {
            if credentials.is_complete() {
                set_qq_bot_credentials(credentials);
            }
        }
    }
    let Ok(mut reminder) = serde_json::from_value::<OfflineReminder>(cfg) else {
        tracing::warn!("offline reminder payload deserialize failed; keep existing");
        return;
    };
    // 保存凭据时前端可能带上空 binding；勿覆盖已绑定的 openid。
    let existing = get_offline_reminder(username);
    if reminder.qq_bot_binding.user_openid.trim().is_empty() && existing.qq_bot_binding.is_bound() {
        reminder.qq_bot_binding = existing.qq_bot_binding;
        if reminder.provider == NotificationProvider::None {
            reminder.provider = existing.provider;
        }
    }
    if let Some(u) = username.filter(|s| !s.is_empty()) {
        qq_farm_core::models::store::global_config::set_user_offline_reminder(u, reminder);
    } else {
        qq_farm_core::models::store::global_config::set_offline_reminder(reminder);
    }
}

/// 测试离线提醒推送。
pub async fn test_offline_reminder(
    ctx: &AppContext,
    username: Option<&str>,
    cfg: Value,
) -> AppResult<Value> {
    if let Some(raw) = cfg.get("qqBot").cloned() {
        if let Ok(credentials) = serde_json::from_value::<QqBotCredentials>(raw) {
            if credentials.is_complete() {
                set_qq_bot_credentials(credentials);
            }
        }
    }
    let base = get_offline_reminder(username);
    let merged: OfflineReminder = serde_json::from_value(cfg).unwrap_or(base);
    if merged.provider == NotificationProvider::WechatBot {
        return Ok(json!({ "ok": false, "code": "not_implemented", "msg": "微信机器人暂未实现" }));
    }
    if merged.provider != NotificationProvider::QqBot {
        return Ok(json!({ "ok": false, "code": "not_configured", "msg": "未启用 QQ 官方机器人通知" }));
    }
    let Some(send_config) = merged.send_config() else {
        return Ok(json!({ "ok": false, "code": "not_bound", "msg": "请先扫码绑定 QQ 通知" }));
    };
    let result = ctx.engine.qq_bot().send_text(&send_config, "", "测试通知：下线").await;
    serde_json::to_value(result).map_err(|e| AppError::Internal(e.to_string()))
}
