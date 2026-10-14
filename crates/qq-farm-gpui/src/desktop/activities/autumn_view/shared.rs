use super::super::super::*;

pub(super) fn header(activity: &serde_json::Value, mode: &str, language: &str, cx: &App) -> Div {
    let image = if mode == "happy" {
        "/activity-assets/autumn/happy.png"
    } else {
        "/activity-assets/autumn/wish.png"
    };
    let image_row = item_row(&serde_json::json!({"image": image}));
    let title = text(activity, "title").unwrap_or_else(|| {
        crate::i18n::text(
            language,
            if mode == "happy" { "快乐不独享" } else { "秋祈良愿" },
            if mode == "happy" { "Shared Happiness" } else { "Autumn Wishes" },
        )
        .to_owned()
    });
    let active = activity.get("active").and_then(serde_json::Value::as_bool).unwrap_or(false);
    let status = crate::i18n::text(
        language,
        if active { "进行中" } else { "已结束" },
        if active { "Active" } else { "Ended" },
    );
    h_flex()
        .items_center()
        .gap_3()
        .child(render_asset_preview_sized(&image_row, 40., cx).unwrap_or_else(|| {
            div()
                .size(px(40.))
                .rounded(cx.theme().radius)
                .bg(cx.theme().group_box)
                .into_any_element()
        }))
        .child(div().font_medium().child(title))
        .child(
            div()
                .rounded(cx.theme().radius)
                .bg(if active { cx.theme().success } else { cx.theme().group_box })
                .px_2()
                .py_1()
                .text_xs()
                .text_color(if active {
                    cx.theme().success_foreground
                } else {
                    cx.theme().muted_foreground
                })
                .child(status),
        )
        .child(div().flex_1().text_xs().text_color(cx.theme().muted_foreground).child(format!(
            "{} ~ {}",
            format_time(activity.get("startTime")),
            format_time(activity.get("endTime"))
        )))
}

pub(super) fn rules(activity: &serde_json::Value, cx: &App) -> Div {
    let rules = activity.get("rules").and_then(serde_json::Value::as_array);
    let mut panel = div().v_flex().gap_1();
    if let Some(rules) = rules {
        for rule in rules.iter().filter_map(serde_json::Value::as_str) {
            panel = panel.child(
                div().text_xs().text_color(cx.theme().muted_foreground).child(rule.to_string()),
            );
        }
    }
    panel
}

pub(super) fn item_card(item: &serde_json::Value, language: &str, cx: &App) -> Div {
    let image = item_image(item, 32., cx);
    let name =
        text(item, "name").unwrap_or_else(|| crate::i18n::text(language, "奖励", "Reward").into());
    let count = number(item.get("count")).unwrap_or_default();
    h_flex().items_center().gap_2().child(image).child(
        div().v_flex().min_w_0().child(div().text_sm().child(name)).child(
            div().text_xs().text_color(cx.theme().muted_foreground).child(format!("×{count}")),
        ),
    )
}

pub(super) fn status_chip(label: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .rounded(cx.theme().radius)
        .bg(cx.theme().background)
        .px_2()
        .py_1()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(label.into())
}

pub(super) fn status_chip_tinted(
    label: impl Into<SharedString>,
    foreground: impl Into<gpui_kit::Hsla>,
    background: impl Into<gpui_kit::Fill>,
    cx: &App,
) -> Div {
    div()
        .rounded(cx.theme().radius)
        .bg(background)
        .px_2()
        .py_1()
        .text_xs()
        .text_color(foreground)
        .child(label.into())
}

pub(super) fn item_image(item: &serde_json::Value, size: f32, cx: &App) -> AnyElement {
    render_asset_preview_sized(&item_row(item), size, cx).unwrap_or_else(|| {
        div().size(px(size)).rounded(cx.theme().radius).bg(cx.theme().background).into_any_element()
    })
}

pub(super) fn item_row(item: &serde_json::Value) -> DisplayRow {
    DisplayRow {
        id: "autumn-item".into(),
        section: "autumn".into(),
        title: text(item, "name").unwrap_or_default(),
        detail: String::new(),
        state: String::new(),
        record: item.clone(),
    }
}

pub(super) fn text(item: &serde_json::Value, key: &str) -> Option<String> {
    item.get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn number(value: Option<&serde_json::Value>) -> Option<i64> {
    value.and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
}

pub(super) fn format_time(value: Option<&serde_json::Value>) -> String {
    let Some(timestamp) = number(value).filter(|timestamp| *timestamp > 0) else {
        return "--".into();
    };
    use chrono::TimeZone as _;
    chrono::Local
        .timestamp_millis_opt(timestamp)
        .single()
        .map(|time| time.format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "--".into())
}

pub(super) fn surface(cx: &App) -> Div {
    div()
        .v_flex()
        .gap_3()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().group_box)
        .p_4()
}
