use super::super::*;

pub(super) fn reward_chip(reward: &serde_json::Value, cx: &mut Context<FarmDesktop>) -> AnyElement {
    let raw_name = reward.get("name").map(value_to_action_string).unwrap_or_default();
    let name = if raw_name.chars().all(|character| character.is_ascii_digit()) {
        String::new()
    } else {
        raw_name
    };
    let count = reward.get("count").map(value_to_action_string).filter(|count| count != "0");
    let mut chip = h_flex()
        .items_center()
        .gap_2()
        .rounded(cx.theme().radius)
        .bg(cx.theme().group_box)
        .px_2()
        .py_1();
    if let Some(image) = render_record_asset_preview_sized(reward, 24., cx) {
        chip = chip.child(image);
    }
    chip.child(div().text_xs().child(name))
        .when_some(count, |chip, count| {
            chip.child(
                div().text_xs().text_color(cx.theme().muted_foreground).child(format!("×{count}")),
            )
        })
        .into_any_element()
}
