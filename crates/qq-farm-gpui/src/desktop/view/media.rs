use super::super::*;

pub(crate) fn render_asset_preview(row: &DisplayRow, cx: &App) -> Option<AnyElement> {
    render_asset_preview_sized(row, 42., cx)
}

pub(crate) fn render_asset_preview_sized(
    row: &DisplayRow,
    size: f32,
    cx: &App,
) -> Option<AnyElement> {
    render_record_asset_preview_sized(&row.record, size, cx)
}

pub(crate) fn render_record_asset_preview_sized(
    record: &serde_json::Value,
    size: f32,
    cx: &App,
) -> Option<AnyElement> {
    let raw = ["image", "seedImage", "assetName", "iconRes"].into_iter().find_map(|key| {
        record.get(key).and_then(serde_json::Value::as_str).filter(|raw| !raw.is_empty())
    })?;
    Some(render_asset_image(raw, size, cx))
}

pub(crate) fn render_asset_image(raw: &str, size: f32, cx: &App) -> AnyElement {
    let radius = cx.theme().radius;
    let background = cx.theme().group_box;
    let foreground = cx.theme().muted_foreground;
    let language = cx
        .try_global::<crate::i18n::ActiveLanguage>()
        .map(|language| language.0.as_str())
        .unwrap_or("zh-CN")
        .to_owned();
    match crate::assets::resolve_image_source(raw) {
        Ok(source) => img(source)
            .size(px(size))
            .rounded(radius)
            .object_fit(ObjectFit::Contain)
            .with_fallback(move || missing_asset(size, radius, background, foreground, &language))
            .into_any_element(),
        Err(_) => missing_asset(size, radius, background, foreground, &language),
    }
}

fn missing_asset(
    size: f32,
    radius: Pixels,
    background: Hsla,
    foreground: Hsla,
    language: &str,
) -> AnyElement {
    div()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded(radius)
        .bg(background)
        .text_xs()
        .text_color(foreground)
        .child(missing_asset_label(language))
        .into_any_element()
}

fn missing_asset_label(language: &str) -> &'static str {
    crate::i18n::text(language, "缺图", "Image unavailable")
}

#[cfg(test)]
mod tests {
    use super::missing_asset_label;

    #[test]
    fn image_failure_state_follows_the_selected_language() {
        assert_eq!(missing_asset_label("zh-CN"), "缺图");
        assert_eq!(missing_asset_label("en-US"), "Image unavailable");
    }
}
