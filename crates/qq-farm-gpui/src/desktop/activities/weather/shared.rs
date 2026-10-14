use super::super::super::*;

pub(super) fn text(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(in crate::desktop::activities) fn number(value: Option<&serde_json::Value>) -> Option<i64> {
    value.and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
}

pub(in crate::desktop::activities) fn flag(value: &serde_json::Value, key: &str) -> bool {
    value.get(key).and_then(serde_json::Value::as_bool) == Some(true)
}

pub(super) fn gate(snapshot: &serde_json::Value, key: &str) -> bool {
    snapshot.pointer(&format!("/actions/{key}/enabled")).and_then(serde_json::Value::as_bool)
        != Some(false)
}

pub(super) fn item_label(item: &serde_json::Value, language: &str) -> String {
    text(item, "name").filter(|name| !name.chars().all(|c| c.is_ascii_digit())).unwrap_or_else(
        || {
            format!(
                "{} {}",
                crate::i18n::text(language, "物品", "Item"),
                item.get("id").map(value_to_action_string).unwrap_or_else(|| "--".into())
            )
        },
    )
}

pub(super) fn item_summary(item: &serde_json::Value, language: &str) -> String {
    let count = item.get("count").map(value_to_action_string).unwrap_or_default();
    format!(
        "{}{}",
        item_label(item, language),
        if count.is_empty() { count } else { format!(" ×{count}") }
    )
}

pub(super) fn image(record: &serde_json::Value, cx: &App) -> AnyElement {
    render_record_asset_preview_sized(record, cx.theme().font_size.as_f32() * 2.4, cx)
        .unwrap_or_else(|| {
            div()
                .size(rems(2.4))
                .rounded(cx.theme().radius)
                .bg(cx.theme().muted)
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child("--")
                .into_any_element()
        })
}

pub(super) fn panel(cx: &App) -> Div {
    div()
        .v_flex()
        .gap_3()
        .min_w_0()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().group_box)
        .p_4()
}

pub(super) fn muted(label: impl Into<SharedString>, cx: &App) -> Div {
    div().text_sm().text_color(cx.theme().muted_foreground).child(label.into())
}

pub(super) fn empty(label: &str, cx: &App) -> Div {
    muted(label.to_string(), cx).py_4().text_center()
}

pub(super) fn list<'a>(value: &'a serde_json::Value, key: &str) -> &'a [serde_json::Value] {
    value.get(key).and_then(serde_json::Value::as_array).map(Vec::as_slice).unwrap_or_default()
}

pub(super) fn action_notice(action: &str, result: &serde_json::Value, language: &str) -> String {
    let title = match action {
        "summon" => crate::i18n::text(language, "雷雨召唤成功", "Thunderstorm summoned"),
        "exchange" => crate::i18n::text(language, "采集瓶兑换成功", "Collector exchanged"),
        "collect" => crate::i18n::text(language, "采雨成功", "Rain collected"),
        "frog" => crate::i18n::text(language, "青蛙使坏成功", "Frog mischief completed"),
        "cloud" => crate::i18n::text(language, "乌云使坏成功", "Cloud mischief completed"),
        _ => crate::i18n::text(language, "研究推进成功", "Research advanced"),
    };
    let source = result.get("rewards").or_else(|| result.pointer("/use/rewards"));
    let rewards = source
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| item_summary(item, language))
        .collect::<Vec<_>>()
        .join(if language == "en-US" { ", " } else { "、" });
    let land = (action == "cloud")
        .then(|| result.get("landId").map(value_to_action_string))
        .flatten()
        .filter(|land| !land.is_empty())
        .map(|land| {
            if language == "en-US" {
                format!(" (Plot {land})")
            } else {
                format!("（地块 {land}）")
            }
        })
        .unwrap_or_default();
    let reward_separator = if language == "en-US" { ": " } else { "：" };
    format!(
        "{title}{land}{}",
        if rewards.is_empty() { rewards } else { format!("{reward_separator}{rewards}") }
    )
}

#[cfg(test)]
mod tests {
    use super::action_notice;
    use serde_json::json;

    #[test]
    fn weather_reward_notice_uses_localized_fallbacks_and_separators() {
        let result = json!({"rewards": [{"id": 1, "count": 2}, {"id": 3}]});

        assert_eq!(
            action_notice("summon", &result, "en-US"),
            "Thunderstorm summoned: Item 1 ×2, Item 3"
        );
        assert_eq!(action_notice("summon", &result, "zh-CN"), "雷雨召唤成功：物品 1 ×2、物品 3");
    }
}
