use super::super::super::super::*;

fn resolve_avatar_source(raw: &str) -> Option<gpui_kit::ImageSource> {
    let normalized = raw.strip_prefix("//").map(|url| format!("https://{url}"));
    crate::assets::resolve_image_source(normalized.as_deref().unwrap_or(raw)).ok()
}

pub(super) fn render_avatar(
    url: Option<&str>,
    initial: char,
    id: impl Into<String>,
    size: f32,
    cx: &App,
) -> AnyElement {
    let id = id.into();
    let background = cx.theme().background;
    let fallback = move || {
        let avatar = div()
            .id(id.clone())
            .aria_label(format!("Avatar {initial}"))
            .size(px(size))
            .rounded_full()
            .bg(background)
            .flex()
            .items_center()
            .justify_center()
            .font_semibold()
            .child(initial.to_string());
        #[cfg(test)]
        let avatar = avatar.test_support();
        avatar.into_any_element()
    };

    match url.and_then(resolve_avatar_source) {
        Some(source) => img(source)
            .size(px(size))
            .rounded_full()
            .object_fit(ObjectFit::Cover)
            .with_fallback(fallback)
            .into_any_element(),
        None => fallback(),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_avatar_source;
    use gpui_kit::ImageSource;

    #[test]
    fn visitor_and_friend_avatars_accept_vue_image_sources() {
        assert!(matches!(
            resolve_avatar_source("https://example.com/avatar.png"),
            Some(ImageSource::Resource(_))
        ));
        assert!(matches!(
            resolve_avatar_source("//example.com/avatar.png"),
            Some(ImageSource::Resource(_))
        ));
        assert!(matches!(
            resolve_avatar_source("data:image/png;base64,AQID"),
            Some(ImageSource::Image(_))
        ));
        assert!(resolve_avatar_source("file:///tmp/avatar.png").is_none());
    }
}
