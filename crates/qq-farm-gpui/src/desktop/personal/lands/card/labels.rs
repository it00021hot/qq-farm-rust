use super::*;

pub(in crate::desktop) fn soil_label(level: i64, language: &str) -> Option<&'static str> {
    match level {
        0 => Some(crate::i18n::text(language, "普通", "Normal")),
        1 => Some(crate::i18n::text(language, "黄土地", "Yellow soil")),
        2 => Some(crate::i18n::text(language, "红土地", "Red soil")),
        3 => Some(crate::i18n::text(language, "黑土地", "Black soil")),
        4 => Some(crate::i18n::text(language, "金土地", "Golden soil")),
        5 => Some(crate::i18n::text(language, "紫金土地", "Purple gold soil")),
        _ => None,
    }
}

pub(in crate::desktop) fn land_status_label(
    status: &str,
    occupied: bool,
    language: &str,
) -> String {
    if occupied {
        return crate::i18n::text(language, "合种占用", "Joint occupied").into();
    }
    match status {
        "harvestable" => crate::i18n::text(language, "可收获", "Harvestable").into(),
        "growing" => crate::i18n::text(language, "生长中", "Growing").into(),
        "empty" => crate::i18n::text(language, "空闲", "Empty").into(),
        "dead" => crate::i18n::text(language, "枯死", "Dead").into(),
        "locked" => crate::i18n::text(language, "未解锁", "Locked").into(),
        "stealable" => crate::i18n::text(language, "可偷", "Stealable").into(),
        "" => "-".into(),
        other => other.into(),
    }
}

pub(in crate::desktop) fn land_status_color(status: &str, occupied: bool, cx: &App) -> Hsla {
    if occupied {
        return cx.theme().info;
    }
    match status {
        "harvestable" => cx.theme().warning,
        "dead" => cx.theme().danger,
        "growing" => cx.theme().success,
        "stealable" => cx.theme().info,
        _ => cx.theme().muted_foreground,
    }
}

pub(super) fn land_badge(label: String, cx: &App) -> Div {
    land_badge_with_color(label, cx.theme().muted_foreground, cx)
}

pub(in crate::desktop) fn land_badge_with_color(label: String, color: Hsla, cx: &App) -> Div {
    div()
        .rounded(cx.theme().radius)
        .bg(cx.theme().background)
        .px_2()
        .py_1()
        .text_xs()
        .text_color(color)
        .child(label)
}

pub(super) fn json_number(value: Option<&serde_json::Value>) -> i64 {
    value
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse::<i64>().ok()))
        .unwrap_or_default()
}
