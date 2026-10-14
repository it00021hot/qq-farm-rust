use super::*;

pub(super) fn render_crop(land: &serde_json::Value, cx: &App) -> AnyElement {
    if land
        .get("seedImage")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|image| !image.is_empty())
    {
        render_record_asset_preview_sized(land, 48., cx)
            .map(|image| div().flex().items_center().justify_center().h(px(48.)).child(image))
            .unwrap_or_else(|| div().flex().items_center().justify_center().h(px(48.)).child("🌱"))
            .into_any_element()
    } else {
        div()
            .flex()
            .items_center()
            .justify_center()
            .h(px(48.))
            .text_xl()
            .text_color(cx.theme().muted_foreground)
            .child("🌱")
            .into_any_element()
    }
}

pub(in crate::desktop) fn render_countdown(
    remaining: i64,
    total: i64,
    progress: f32,
    phase: &str,
    cx: &App,
) -> Div {
    if remaining <= 0 {
        return div()
            .text_center()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(if phase.is_empty() { "-" } else { phase }.to_string());
    }

    let mut countdown = div()
        .text_center()
        .text_xs()
        .text_color(cx.theme().warning)
        .child(format_land_countdown(remaining));
    if total > 0 {
        countdown =
            countdown.child(
                div().h(px(5.)).w_full().rounded_full().bg(cx.theme().background).child(
                    div().h_full().w(relative(progress)).rounded_full().bg(cx.theme().primary),
                ),
            );
    }
    countdown
}
