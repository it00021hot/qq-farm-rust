use chrono::TimeZone;
use gpui_kit::component::ActiveTheme;
use gpui_kit::{App, Hsla};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GiftStatusKind {
    /// 领取失败（bot fecf531：错误可见，优先级高于一切）
    Error,
    /// 部分领取失败
    Partial,
    NotActivated,
    /// 已开通会员但当前无礼包
    NoGift,
    Done,
    Waiting,
    Disabled,
}

impl GiftStatusKind {
    pub(super) fn from_gift(gift: &serde_json::Value) -> Self {
        let key = gift.get("key").and_then(serde_json::Value::as_str).unwrap_or_default();
        // 失败态优先（对齐 bot DailyOverview：错误不得被「已完成」掩盖）
        match gift.get("result").and_then(serde_json::Value::as_str) {
            Some("error") => return Self::Error,
            Some("partial") => return Self::Partial,
            _ => {}
        }
        let not_activated = match key {
            "vip_daily_gift" => {
                let is_vip = gift.get("isVip").and_then(serde_json::Value::as_bool);
                let has_gift = gift.get("hasGift").and_then(serde_json::Value::as_bool);
                is_vip == Some(false) || (is_vip.is_none() && has_gift == Some(false))
            }
            "month_card_gift" => {
                gift.get("hasCard").and_then(serde_json::Value::as_bool) == Some(false)
            }
            _ => false,
        };
        if not_activated {
            Self::NotActivated
        } else if key == "vip_daily_gift"
            && gift.get("isVip").and_then(serde_json::Value::as_bool) == Some(true)
            && gift.get("hasGift").and_then(serde_json::Value::as_bool) == Some(false)
        {
            Self::NoGift
        } else if gift_done(gift) {
            Self::Done
        } else if gift.get("enabled").and_then(serde_json::Value::as_bool).unwrap_or(false) {
            Self::Waiting
        } else {
            Self::Disabled
        }
    }

    pub(super) fn color(self, cx: &App) -> Hsla {
        match self {
            Self::Done => cx.theme().success,
            Self::Waiting => cx.theme().info,
            Self::Error | Self::Partial => cx.theme().danger,
            Self::NotActivated | Self::NoGift | Self::Disabled => cx.theme().muted_foreground,
        }
    }
}

pub(super) fn gift_icon(key: &str) -> &'static str {
    match key {
        "task_claim" => "✅",
        "email_rewards" => "📧",
        "mall_free_gifts" => "🛍️",
        "daily_share" => "📤",
        "vip_daily_gift" => "⭐",
        "month_card_gift" => "📅",
        _ => "🎁",
    }
}

pub(super) fn gift_done(gift: &serde_json::Value) -> bool {
    gift.get("doneToday").and_then(serde_json::Value::as_bool).unwrap_or(false)
}

pub(super) fn gift_status(gift: &serde_json::Value, language: &str) -> String {
    let (chinese, english) = match GiftStatusKind::from_gift(gift) {
        GiftStatusKind::Error => ("领取失败", "Claim failed"),
        GiftStatusKind::Partial => ("部分领取失败", "Partially failed"),
        GiftStatusKind::NotActivated => ("未开通", "Not activated"),
        GiftStatusKind::NoGift => ("暂无礼包", "No gift available"),
        GiftStatusKind::Done => ("今日已完成", "Done today"),
        GiftStatusKind::Waiting => ("等待执行", "Waiting"),
        GiftStatusKind::Disabled => ("未开启", "Disabled"),
    };
    crate::i18n::text(language, chinese, english).to_string()
}

pub(super) fn gift_detail(gift: &serde_json::Value, language: &str) -> String {
    let key = gift.get("key").and_then(serde_json::Value::as_str).unwrap_or_default();
    let result = gift.get("result").and_then(serde_json::Value::as_str).unwrap_or_default();
    // 失败态：优先展示具体错误（bot 面板以 tooltip 展示 lastError）
    if result == "error" || result == "partial" {
        if let Some(error) = gift.get("lastError").and_then(serde_json::Value::as_str) {
            if !error.is_empty() {
                return error.to_string();
            }
        }
        let timestamp = value_i64(gift.get("lastAt"));
        return if timestamp > 0 {
            match chrono::Local.timestamp_millis_opt(timestamp).single() {
                Some(time) => {
                    crate::i18n::text(language, "最近检查", "Last checked").to_string()
                        + " "
                        + &time.format("%H:%M").to_string()
                }
                None => String::new(),
            }
        } else {
            crate::i18n::text(language, "等待下次检查", "Waiting for next check").into()
        };
    }
    match key {
        "vip_daily_gift"
            if gift.get("isVip").and_then(serde_json::Value::as_bool) == Some(false) =>
        {
            return crate::i18n::text(
                language,
                "未开通 QQ 会员或无每日礼包",
                "QQ membership or a daily gift is not active",
            )
            .into();
        }
        "vip_daily_gift"
            if gift.get("isVip").and_then(serde_json::Value::as_bool) == Some(true)
                && gift.get("hasGift").and_then(serde_json::Value::as_bool) == Some(false) =>
        {
            return crate::i18n::text(
                language,
                "已开通会员，当前没有可领取礼包",
                "Membership active but no gift available",
            )
            .into();
        }
        "month_card_gift"
            if gift.get("hasCard").and_then(serde_json::Value::as_bool) == Some(false) =>
        {
            return crate::i18n::text(
                language,
                "未购买月卡或已过期",
                "Month card not purchased or expired",
            )
            .into();
        }
        _ => {}
    }
    let total = value_i64(gift.get("totalCount"));
    let current = value_i64(gift.get("completedCount"));
    if total > 0 {
        return progress_label(current, total, language);
    }
    let timestamp = value_i64(gift.get("lastAt"));
    if timestamp > 0 && gift_done(gift) {
        if let Some(time) = chrono::Local.timestamp_millis_opt(timestamp).single() {
            return crate::i18n::text(language, "完成时间", "Done at").to_string()
                + " "
                + &time.format("%H:%M").to_string();
        }
    }
    String::new()
}

pub(super) fn progress_label(current: i64, total: i64, language: &str) -> String {
    if language == "en-US" {
        format!("Progress: {current}/{total}")
    } else {
        format!("进度：{current}/{total}")
    }
}

pub(super) fn value_i64(value: Option<&serde_json::Value>) -> i64 {
    value
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
        .unwrap_or_default()
}

pub(super) fn task_progress_is_empty(task: &serde_json::Value) -> bool {
    fn js_falsy(value: Option<&serde_json::Value>) -> bool {
        match value {
            None | Some(serde_json::Value::Null) => true,
            Some(serde_json::Value::Bool(value)) => !value,
            Some(serde_json::Value::Number(value)) => value.as_f64() == Some(0.0),
            Some(serde_json::Value::String(value)) => value.is_empty(),
            Some(serde_json::Value::Array(_) | serde_json::Value::Object(_)) => false,
        }
    }

    js_falsy(task.get("progress")) && js_falsy(task.get("totalProgress"))
}

#[cfg(test)]
mod tests {
    use super::{gift_detail, gift_icon, gift_status, task_progress_is_empty, GiftStatusKind};

    #[test]
    fn daily_gift_labels_follow_vue_status_and_subscription_rules() {
        assert_eq!(
            gift_status(
                &serde_json::json!({"key":"vip_daily_gift", "isVip":false, "hasGift":false}),
                "zh-CN"
            ),
            "未开通"
        );
        // bot fecf531：失败态优先，错误详情进 detail
        assert_eq!(
            gift_status(
                &serde_json::json!({"key":"vip_daily_gift", "result":"error", "doneToday":false}),
                "zh-CN"
            ),
            "领取失败"
        );
        assert_eq!(
            gift_detail(
                &serde_json::json!({"key":"vip_daily_gift", "result":"error", "lastError":"领取会员礼包失败: 超时"}),
                "en-US"
            ),
            "领取会员礼包失败: 超时"
        );
        assert_eq!(
            gift_status(
                &serde_json::json!({"key":"vip_daily_gift", "isVip":true, "hasGift":false}),
                "en-US"
            ),
            "No gift available"
        );
        assert_eq!(
            gift_status(&serde_json::json!({"key":"task_claim", "doneToday":true}), "en-US"),
            "Done today"
        );
        assert_eq!(
            gift_detail(&serde_json::json!({"key":"month_card_gift", "hasCard":false}), "en-US"),
            "Month card not purchased or expired"
        );
        assert_eq!(gift_icon("daily_share"), "📤");
    }

    #[test]
    fn daily_gift_status_kind_matches_vue_badge_priority() {
        // bot fecf531：result=error 优先于一切（包括 doneToday）
        assert_eq!(
            GiftStatusKind::from_gift(&serde_json::json!({
                "key":"vip_daily_gift", "result":"error", "doneToday":true, "enabled":true
            })),
            GiftStatusKind::Error
        );
        assert_eq!(
            GiftStatusKind::from_gift(&serde_json::json!({
                "key":"vip_daily_gift", "result":"partial", "doneToday":true
            })),
            GiftStatusKind::Partial
        );
        // isVip=false 未开通；isVip 缺失时回退 hasGift
        assert_eq!(
            GiftStatusKind::from_gift(&serde_json::json!({
                "key":"vip_daily_gift", "isVip":false, "hasGift":false, "doneToday":true, "enabled":true
            })),
            GiftStatusKind::NotActivated
        );
        assert_eq!(
            GiftStatusKind::from_gift(&serde_json::json!({
                "key":"vip_daily_gift", "hasGift":false, "doneToday":true, "enabled":true
            })),
            GiftStatusKind::NotActivated
        );
        assert_eq!(
            GiftStatusKind::from_gift(&serde_json::json!({
                "key":"vip_daily_gift", "isVip":true, "hasGift":false, "doneToday":true
            })),
            GiftStatusKind::NoGift
        );
        assert_eq!(
            GiftStatusKind::from_gift(&serde_json::json!({"key":"task_claim", "doneToday":true})),
            GiftStatusKind::Done
        );
        assert_eq!(
            GiftStatusKind::from_gift(&serde_json::json!({"key":"task_claim", "enabled":true})),
            GiftStatusKind::Waiting
        );
        assert_eq!(
            GiftStatusKind::from_gift(&serde_json::json!({"key":"task_claim", "enabled":false})),
            GiftStatusKind::Disabled
        );
    }

    #[test]
    fn growth_task_empty_progress_uses_vue_javascript_truthiness() {
        assert!(task_progress_is_empty(&serde_json::json!({})));
        assert!(task_progress_is_empty(&serde_json::json!({"progress":0,"totalProgress":false})));
        assert!(!task_progress_is_empty(&serde_json::json!({"progress":"0","totalProgress":0})));
        assert!(task_progress_is_empty(&serde_json::json!({"progress":"","totalProgress":0})));
    }
}
