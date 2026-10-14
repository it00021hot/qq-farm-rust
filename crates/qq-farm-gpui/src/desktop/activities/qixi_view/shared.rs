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
    js_number(value).filter(|number| number.is_finite()).map(f64::trunc).map(|number| number as i64)
}

pub(super) fn js_number(value: Option<&serde_json::Value>) -> Option<f64> {
    match value? {
        serde_json::Value::Null => Some(0.),
        serde_json::Value::Bool(value) => Some(if *value { 1. } else { 0. }),
        serde_json::Value::Number(value) => value.as_f64(),
        serde_json::Value::String(value) if value.trim().is_empty() => Some(0.),
        serde_json::Value::String(value) => value.trim().parse().ok(),
        serde_json::Value::Array(values) if values.is_empty() => Some(0.),
        _ => None,
    }
}

pub(super) fn js_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(value) => value.as_f64().is_some_and(|number| number != 0.),
        serde_json::Value::String(value) => !value.is_empty(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => true,
    }
}

pub(super) fn item_row(item: &serde_json::Value) -> DisplayRow {
    DisplayRow {
        id: "qixi-item".into(),
        section: "qixi".into(),
        title: text(item, "name").unwrap_or_default(),
        detail: String::new(),
        state: String::new(),
        record: item.clone(),
    }
}

pub(super) fn item_image(
    item: &serde_json::Value,
    size: f32,
    language: &str,
    cx: &App,
) -> AnyElement {
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
            .child(crate::i18n::text(language, "道具", "Item"))
            .into_any_element()
    })
}

pub(super) fn item_label(
    item: &serde_json::Value,
    fallback: &str,
    english_fallback: &str,
    language: &str,
) -> String {
    match text(item, "name") {
        Some(name) if name.chars().all(|character| character.is_ascii_digit()) => {
            if fallback.is_empty() {
                name
            } else {
                crate::i18n::text(language, fallback, english_fallback).to_string()
            }
        }
        Some(name) => name,
        None => crate::i18n::text(language, fallback, english_fallback).to_string(),
    }
}

pub(super) fn display_count(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else { return "0".into() };
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(false) => "0".into(),
        serde_json::Value::String(value) if value.is_empty() => "0".into(),
        serde_json::Value::Number(value) if value.as_f64() == Some(0.) => "0".into(),
        serde_json::Value::Number(value) if value.as_i64().is_some() => {
            value.as_i64().unwrap_or_default().to_string()
        }
        serde_json::Value::Number(value) => {
            let number = value.as_f64().unwrap_or_default();
            if number.fract() == 0. {
                format!("{number:.0}")
            } else {
                value.to_string()
            }
        }
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Bool(value) => value.to_string(),
        value => value.to_string(),
    }
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

pub(super) fn rules(
    activity: &serde_json::Value,
    expanded: bool,
    language: &str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
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
        .unwrap_or_else(|| crate::i18n::text(language, "活动说明", "Event info").into());
    let mut panel = div()
        .v_flex()
        .gap_2()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().group_box)
        .p_4()
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(div().font_medium().child(title))
                .child(
                    Button::new("qixi-rules-toggle")
                        .ghost()
                        .label(if expanded {
                            crate::i18n::text(language, "收起", "Collapse")
                        } else {
                            crate::i18n::text(language, "展开", "Expand")
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.activities.qixi_rules_expanded =
                                !this.activities.qixi_rules_expanded;
                            cx.notify();
                        })),
                ),
        );
    if expanded {
        panel = panel.children(
            paragraphs
                .into_iter()
                .map(|line| div().text_sm().text_color(cx.theme().muted_foreground).child(line)),
        );
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::{display_count, item_label};
    use serde_json::json;

    #[test]
    fn qixi_counts_keep_vue_truthy_strings_and_default_false_values_to_zero() {
        assert_eq!(display_count(Some(&json!("2.50"))), "2.50");
        assert_eq!(display_count(Some(&json!(2.5))), "2.5");
        assert_eq!(display_count(Some(&json!(2.0))), "2");
        assert_eq!(display_count(Some(&json!(0))), "0");
        assert_eq!(display_count(Some(&json!(""))), "0");
        assert_eq!(display_count(None), "0");
    }

    #[test]
    fn qixi_item_names_use_fallback_only_when_provided() {
        assert_eq!(item_label(&json!({"name": "123"}), "", "", "en-US"), "123");
        assert_eq!(
            item_label(&json!({"name": "123"}), "Feathers", "Feathers", "en-US"),
            "Feathers"
        );
        assert_eq!(item_label(&json!({"name": "Star feather"}), "", "", "en-US"), "Star feather");
        assert_eq!(item_label(&json!({}), "Feathers", "Feathers", "en-US"), "Feathers");
    }

    #[test]
    fn qixi_numeric_and_truthy_helpers_follow_javascript_inputs() {
        assert_eq!(super::number(Some(&json!("1.9"))), Some(1));
        assert_eq!(super::js_number(Some(&json!("0.5"))), Some(0.5));
        assert_eq!(super::js_number(Some(&json!(null))), Some(0.));
        assert!(!super::js_truthy(&json!("")));
        assert!(super::js_truthy(&json!(" ")));
    }
}
