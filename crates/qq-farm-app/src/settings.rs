//! 设置面板编排。

use qq_farm_core::models::store::global_config::{
    effective_qq_bot_credentials, set_qq_bot_credentials, NotificationProvider, OfflineReminder,
    QqBotCredentials,
};
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::session::AppContext;

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
    if merged.provider == NotificationProvider::DingTalk {
        // 钉钉：endpoint 与 token 二选一；endpoint 非法直接 400 提示
        if merged.endpoint.trim().is_empty() && merged.token.trim().is_empty() {
            return Ok(json!({
                "ok": false, "code": "missing_endpoint",
                "msg": "请填写钉钉 Webhook 地址或 Access Token"
            }));
        }
        let result = qq_farm_core::services::push::send_dingtalk(
            &merged.endpoint,
            &merged.token,
            &merged.secret,
            "测试通知",
            "这是一条来自 QQ Farm 桌面端的钉钉测试消息",
        )
        .await;
        return match result {
            Ok(()) => Ok(json!({ "ok": true, "msg": "钉钉测试消息已发送" })),
            Err(e) => Ok(json!({ "ok": false, "code": "send_failed", "msg": e })),
        };
    }
    if merged.provider != NotificationProvider::QqBot {
        return Ok(
            json!({ "ok": false, "code": "not_configured", "msg": "未启用 QQ 官方机器人通知" }),
        );
    }
    let Some(send_config) = merged.send_config() else {
        return Ok(json!({ "ok": false, "code": "not_bound", "msg": "请先扫码绑定 QQ 通知" }));
    };
    let result = ctx.engine.qq_bot().send_text(&send_config, "", "测试通知：下线").await;
    serde_json::to_value(result).map_err(|e| AppError::Internal(e.to_string()))
}
