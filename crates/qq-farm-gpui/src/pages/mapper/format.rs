use serde_json::Value;

use crate::pages::Page;

pub(super) fn row_section(page: Page, prefix: &str) -> String {
    let root = prefix.split('/').next().unwrap_or(prefix);
    if page != Page::Settings {
        return root.into();
    }
    match root {
        "offlineReminder" => "offlineReminder".into(),
        "systemConfig" | "devicePresets" | "qqBotBinding" => "systemConfig".into(),
        "ui" => "ui".into(),
        "intervals" | "friendQuietHours" | "stealDelaySeconds" | "plantOrderRandom"
        | "plantDelaySeconds" => "strategy".into(),
        "friendBlacklist"
        | "autoAcceptFriendMinLevel"
        | "autoAcceptRequireOwnLevel"
        | "autoAcceptHarvestStealEnabled"
        | "autoAcceptHarvestStealHarvest"
        | "autoAcceptHarvestStealSteal" => "friendBlacklist".into(),
        "strategy"
        | "plantingStrategy"
        | "preferredSeed"
        | "preferredSeedId"
        | "bagSeedPriority"
        | "bagSeedLandTypes"
        | "bagSeedFallbackStrategy"
        | "bagSeedMultiLandReservationEnabled" => "strategy".into(),
        _ => "automation".into(),
    }
}

pub(super) fn detail_at(value: &Value) -> String {
    ["level", "count", "price", "gold", "status", "description"]
        .iter()
        .filter_map(|key| {
            value
                .get(*key)
                .filter(|field| !field.is_null())
                .map(|field| format!("{} {}", humanize_key(key), value_string(field)))
        })
        .collect::<Vec<_>>()
        .join("  ·  ")
}

pub(super) fn state_at(value: &Value) -> String {
    if value.get("running").and_then(Value::as_bool).unwrap_or(false) {
        "运行中".into()
    } else if value.get("running").and_then(Value::as_bool) == Some(false) {
        "已停止".into()
    } else if value.get("isCompleted").and_then(Value::as_bool).unwrap_or(false) {
        "已完成".into()
    } else {
        String::new()
    }
}

pub(super) fn string_at(value: &Value, keys: &[&str], fallback: &str) -> String {
    keys.iter()
        .find_map(|key| {
            value.pointer(&format!("/{}", key.replace('.', "/"))).and_then(|field| match field {
                Value::String(text) => Some(text.clone()),
                Value::Number(number) => Some(number.to_string()),
                _ => None,
            })
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn number_at(value: &Value, keys: &[&str]) -> i64 {
    keys.iter()
        .find_map(|key| {
            value.pointer(&format!("/{}", key.replace('.', "/"))).and_then(Value::as_i64)
        })
        .unwrap_or_default()
}

pub(super) fn value_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => if *value { "是" } else { "否" }.into(),
        Value::Array(items) => format!("{} 项", items.len()),
        Value::Object(items) => format!("{} 项", items.len()),
        Value::Null => String::new(),
    }
}

pub(super) fn humanize_key(key: &str) -> String {
    match key {
        "online" => "在线".into(),
        "level" => "等级".into(),
        "exp" => "经验".into(),
        "gold" => "金币".into(),
        "running" => "运行".into(),
        "platform" => "平台".into(),
        _ => key.to_string(),
    }
}
