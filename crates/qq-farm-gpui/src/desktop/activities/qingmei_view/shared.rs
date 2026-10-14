use super::super::super::*;

pub(super) fn text(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn number(value: Option<&serde_json::Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str()?.parse().ok())
            .or_else(|| value.as_f64().map(|number| number as i64))
    })
}

pub(super) fn item_row(item: &serde_json::Value) -> DisplayRow {
    DisplayRow {
        id: "qingmei-item".into(),
        section: "qingmei".into(),
        title: text(item, "name").unwrap_or_default(),
        detail: String::new(),
        state: String::new(),
        record: item.clone(),
    }
}

pub(super) fn image(item: &serde_json::Value, size: f32, language: &str, cx: &App) -> AnyElement {
    render_asset_preview_sized(&item_row(item), size, cx).unwrap_or_else(|| {
        div()
            .size(px(size))
            .flex()
            .items_center()
            .justify_center()
            .rounded(cx.theme().radius)
            .bg(cx.theme().background)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(crate::i18n::text(language, "原料", "Ingredient"))
            .into_any_element()
    })
}

pub(super) fn panel(cx: &App) -> Div {
    div()
        .v_flex()
        .gap_3()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().group_box)
        .p_4()
}

pub(super) fn rules(activity: &serde_json::Value, language: &str, cx: &App) -> Div {
    let Some(rules) = activity.get("rules") else { return div() };
    let paragraphs = rules
        .get("paragraphs")
        .into_iter()
        .chain(rules.pointer("/tips/txt"))
        .chain(rules.get("lines"))
        .filter_map(serde_json::Value::as_array)
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if paragraphs.is_empty() {
        return div();
    }
    let title = text(rules, "title")
        .unwrap_or_else(|| crate::i18n::text(language, "活动规则", "Activity rules").into());
    div()
        .v_flex()
        .gap_2()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().group_box)
        .p_4()
        .child(div().font_medium().child(title))
        .children(
            paragraphs
                .into_iter()
                .map(|line| div().text_sm().text_color(cx.theme().muted_foreground).child(line)),
        )
}
