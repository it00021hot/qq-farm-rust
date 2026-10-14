use super::super::super::*;

pub(super) fn text_at(value: &serde_json::Value, path: &str) -> String {
    value
        .pointer(path)
        .map(value_to_action_string)
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "0".into())
}

pub(super) fn reward_label(item: &serde_json::Value, fallback: &str) -> String {
    let name = item.get("name").map(value_to_action_string).unwrap_or_default();
    if !name.is_empty() && !name.chars().all(|ch| ch.is_ascii_digit()) {
        name
    } else if !fallback.is_empty() {
        fallback.to_owned()
    } else {
        name
    }
}

pub(super) fn reward_line(
    item: &serde_json::Value,
    fallback: &str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let label = reward_label(item, fallback);
    let count = item.get("count").map(value_to_action_string).filter(|count| count != "0");
    h_flex()
        .items_center()
        .gap_2()
        .when_some(render_record_asset_preview_sized(item, 32., cx), |row, image| row.child(image))
        .child(div().min_w_0().text_sm().child(label))
        .when_some(count, |row, count| {
            row.child(
                div().text_sm().text_color(cx.theme().muted_foreground).child(format!("×{count}")),
            )
        })
}

pub(super) fn reward_line_plain(
    item: &serde_json::Value,
    fallback: &str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let label = reward_label(item, fallback);
    h_flex()
        .items_center()
        .gap_2()
        .when_some(render_record_asset_preview_sized(item, 32., cx), |row, image| row.child(image))
        .child(div().min_w_0().text_sm().child(label))
}

pub(super) fn metric_card(
    title: &str,
    value: String,
    item: Option<&serde_json::Value>,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    div()
        .v_flex()
        .flex_1()
        .min_w(px(170.))
        .gap_2()
        .rounded(cx.theme().radius_lg)
        .bg(cx.theme().group_box)
        .p_4()
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .when_some(
                    item.and_then(|item| render_record_asset_preview_sized(item, 36., cx)),
                    |row, image| row.child(image),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(title.to_string()),
                ),
        )
        .child(div().text_lg().font_semibold().child(value))
}
