use super::super::super::*;
use serde_json::Value;

pub(super) fn card_with_extra(
    this: &FarmDesktop,
    title: &str,
    extra: impl IntoElement,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let title = localized_section_title(title, &this.preferences.language);
    card_shell(cx).child(
        h_flex()
            .items_center()
            .justify_between()
            .child(div().font_medium().child(title))
            .child(extra),
    )
}

pub(super) fn section_card(
    this: &FarmDesktop,
    title: &str,
    key: &'static str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    section_card_with_extra(this, title, key, div(), cx)
}

pub(super) fn section_card_with_extra(
    this: &FarmDesktop,
    title: &str,
    key: &'static str,
    extra: impl IntoElement,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let title = localized_section_title(title, &this.preferences.language);
    card_shell(cx).child(
        h_flex()
            .items_center()
            .justify_between()
            .child(section_toggle(this, &title, key, cx))
            .child(extra),
    )
}

fn localized_section_title(title: &str, language: &str) -> String {
    let english = match title {
        "萌宠成长日记" => "Pet growth journal",
        "寻宝与护送" => "Treasure hunt and escort",
        "萌宠养成" => "Pet care",
        "夺宝锦囊" => "Plunder charms",
        "好友夺宝" => "Friend plunder",
        "互动与被夺日志" => "Interaction and plunder logs",
        "节令小礼" => "Seasonal gifts",
        "种子赠礼" => "Seed gifts",
        "成长手记" => "Growth journals",
        "拾物小铺" => "Found item shop",
        _ => return title.to_string(),
    };
    crate::i18n::text(language, title, english).into()
}

fn section_toggle(
    this: &FarmDesktop,
    title: &str,
    key: &'static str,
    cx: &mut Context<FarmDesktop>,
) -> Button {
    let expanded = this.activities.pet_diary_expanded_sections.contains(key);
    Button::new(format!("pet-diary-section-{key}"))
        .label(format!("{} {title}", if expanded { "⌄" } else { "›" }))
        .ghost()
        .on_click(cx.listener(move |this, _, _, cx| {
            if !this.activities.pet_diary_expanded_sections.remove(key) {
                this.activities.pet_diary_expanded_sections.insert(key.to_string());
            }
            cx.notify();
        }))
}

fn card_shell(cx: &mut Context<FarmDesktop>) -> Div {
    div()
        .v_flex()
        .gap_3()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .p_4()
}

pub(super) fn text(value: Option<&Value>, fallback: &str) -> String {
    value
        .map(value_to_action_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn number(value: Option<&Value>, fallback: i64) -> i64 {
    value
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
        .unwrap_or(fallback)
}

pub(super) fn is_true(value: Option<&Value>) -> bool {
    value.and_then(Value::as_bool) == Some(true)
}

pub(super) fn item_label(item: &Value, language: &str) -> String {
    let name = item.get("name").and_then(Value::as_str).unwrap_or_default();
    let id = text(item.get("id"), "?");
    let name = if name.is_empty() {
        format!("{} {id}", crate::i18n::text(language, "道具", "Item"))
    } else {
        name.to_string()
    };
    let count = text(item.get("count"), "");
    format!("{name} ×{count}")
}

pub(super) fn item_line(
    item: &Value,
    size: f32,
    language: &str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let image = item
        .get("image")
        .and_then(Value::as_str)
        .filter(|url| !url.is_empty())
        .map(|url| picture(url, size, cx));
    h_flex()
        .items_center()
        .gap_2()
        .when_some(image, |line, image| line.child(image))
        .child(div().text_sm().child(item_label(item, language)))
}

pub(super) fn picture(url: &str, size: f32, cx: &mut Context<FarmDesktop>) -> AnyElement {
    render_asset_image(url, size, cx)
}

pub(super) fn items_line(
    items: Option<&Value>,
    language: &str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let items = items.and_then(Value::as_array).cloned().unwrap_or_default();
    if items.is_empty() {
        return div();
    }
    h_flex()
        .flex_wrap()
        .gap_3()
        .children(items.iter().map(|item| item_line(item, 22., language, cx)))
}

pub(super) fn timestamp(value: Option<&Value>) -> Option<String> {
    let millis = value.and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))?;
    use chrono::TimeZone as _;
    chrono::Local
        .timestamp_millis_opt(millis)
        .single()
        .map(|time| time.format("%m-%d %H:%M").to_string())
}

#[cfg(test)]
mod tests {
    use super::item_label;
    use serde_json::json;

    #[test]
    fn item_labels_preserve_vue_truthy_names_and_empty_count_suffixes() {
        assert_eq!(item_label(&json!({"name":"123", "id":9, "count":2}), "zh-CN"), "123 ×2");
        assert_eq!(item_label(&json!({"name":"Apple", "id":9}), "zh-CN"), "Apple ×");
        assert_eq!(item_label(&json!({"id":9, "count":1}), "en-US"), "Item 9 ×1");
    }
}
