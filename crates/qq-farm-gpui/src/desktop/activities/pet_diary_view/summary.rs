use super::super::super::*;
use serde_json::Value;

use super::common::{card_with_extra, picture, text, timestamp};

fn balance_label(item: &Value) -> String {
    let name = item.get("name").and_then(Value::as_str).filter(|name| !name.is_empty());
    let id = item.get("id").map(value_to_action_string).unwrap_or_default();
    let label = name.map(str::to_string).unwrap_or(id);
    let count = match item.get("count") {
        None | Some(Value::Null) => "未知".to_string(),
        Some(value) => value_to_action_string(value),
    };
    format!("{label} ×{count}")
}

pub(super) fn render(this: &FarmDesktop, snapshot: &Value, cx: &mut Context<FarmDesktop>) -> Div {
    let language = &this.preferences.language;
    let has_snapshot = !snapshot.is_null();
    let active = snapshot.get("active").and_then(Value::as_bool) == Some(true);
    let start = timestamp(snapshot.get("startTime")).unwrap_or_else(|| "--".into());
    let end = timestamp(snapshot.get("endTime")).unwrap_or_else(|| "--".into());
    let warnings = snapshot.get("warnings").and_then(Value::as_array).cloned().unwrap_or_default();
    let balances = snapshot.get("balances");

    let refresh = Button::new("pet-diary-refresh")
        .label(crate::i18n::text(language, "刷新", "Refresh"))
        .ghost()
        .loading(this.loading)
        .disabled(this.loading || this.selected_account.is_none())
        .on_click(cx.listener(|this, _, _, cx| this.refresh_pet_diary_snapshot(cx)));
    let mut header = h_flex()
        .items_center()
        .flex_wrap()
        .gap_2()
        .child(div().font_semibold().child(text(
            snapshot.get("title"),
            crate::i18n::text(language, "萌宠成长日记", "Pet growth journal"),
        )))
        .child(
            div()
                .rounded_full()
                .bg(if active { cx.theme().success } else { cx.theme().group_box })
                .px_2()
                .py_1()
                .text_xs()
                .text_color(if active {
                    cx.theme().success_foreground
                } else {
                    cx.theme().muted_foreground
                })
                .child(if active {
                    crate::i18n::text(language, "进行中", "In progress")
                } else {
                    crate::i18n::text(language, "未开放", "Not open")
                }),
        );
    if has_snapshot {
        header = header.child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{start} ~ {end}")),
        );
    }
    let mut panel = card_with_extra(this, "萌宠成长日记", refresh, cx).child(header);
    if has_snapshot {
        let balances = balances.and_then(Value::as_array).cloned().unwrap_or_default();
        if !balances.is_empty() {
            panel = panel.child(h_flex().flex_wrap().items_center().gap_3().children(
                balances.iter().map(|balance| {
                    let image = balance
                        .get("image")
                        .and_then(Value::as_str)
                        .filter(|url| !url.is_empty())
                        .map(|url| picture(url, 18., cx));
                    h_flex()
                        .items_center()
                        .gap_1()
                        .when_some(image, |line, image| line.child(image))
                        .child(div().text_sm().child(balance_label(balance)))
                }),
            ));
        }
    }

    if !warnings.is_empty() {
        panel = panel.child(
            div()
                .v_flex()
                .gap_1()
                .rounded(cx.theme().radius)
                .bg(cx.theme().group_box)
                .p_3()
                .children(warnings.iter().map(|warning| {
                    div().text_sm().text_color(cx.theme().warning).child(
                        warning
                            .as_str()
                            .unwrap_or(crate::i18n::text(
                                language,
                                "活动数据读取不完整",
                                "Activity data is incomplete.",
                            ))
                            .to_string(),
                    )
                })),
        );
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::balance_label;
    use serde_json::json;

    #[test]
    fn balance_labels_match_vue_name_and_count_fallbacks() {
        assert_eq!(balance_label(&json!({"name": "金币", "id": 1, "count": 5})), "金币 ×5");
        assert_eq!(balance_label(&json!({"id": 7, "count": 0})), "7 ×0");
        assert_eq!(balance_label(&json!({"name": "金币", "id": 1})), "金币 ×未知");
        assert_eq!(balance_label(&json!({"name": "", "id": "sand"})), "sand ×未知");
    }
}
