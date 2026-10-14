use std::rc::Rc;

use gpui_kit::component::theme::{Theme, ThemeConfig, ThemeConfigColors, ThemeMode};
use gpui_kit::{App, SharedString};

/// 产品配色集中在主题层，所有业务页从语义令牌读取。
pub(crate) fn init(cx: &mut App) {
    Theme::update(cx, |theme| {
        theme.light_theme = Rc::new(config(false));
        theme.dark_theme = Rc::new(config(true));
    });
    Theme::change(Theme::global(cx).mode, None, cx);
}

fn color(dark: bool, light: &'static str, night: &'static str) -> Option<SharedString> {
    Some(if dark { night } else { light }.into())
}

fn config(dark: bool) -> ThemeConfig {
    let neutral = color(dark, "#FFFFFF", "#292929");
    let foreground = color(dark, "#1D1D1F", "#F5F5F7");
    let accent = color(dark, "#E6F0FC", "#193B61");
    let blue = color(dark, "#0063CC", "#0A6CDA");
    let blue_hover = color(dark, "#0058B8", "#0875EA");
    let blue_active = color(dark, "#004E9F", "#005FC2");
    let mut colors = ThemeConfigColors::default();
    colors.background = color(dark, "#FFFFFF", "#1E1E1E");
    colors.foreground = foreground.clone();
    colors.sidebar = color(dark, "#F0F0F2", "#252526");
    colors.sidebar_foreground = foreground.clone();
    colors.sidebar_border = color(dark, "#DCDCE0", "#3A3A3C");
    colors.sidebar_accent = accent.clone();
    colors.sidebar_accent_foreground = foreground.clone();
    colors.sidebar_primary = blue.clone();
    colors.sidebar_primary_foreground = Some("#FFFFFF".into());
    colors.group_box = color(dark, "#F7F7F9", "#28282A");
    colors.group_box_foreground = foreground.clone();
    colors.border = color(dark, "#E1E1E5", "#3A3A3C");
    colors.input = color(dark, "#C5C5CB", "#55555A");
    colors.muted = color(dark, "#EEEEF1", "#353538");
    colors.muted_foreground = color(dark, "#636368", "#B5B5BB");
    colors.popover = neutral.clone();
    colors.popover_foreground = foreground.clone();
    colors.accent = accent.clone();
    colors.accent_foreground = foreground.clone();
    colors.primary = blue.clone();
    colors.primary_foreground = Some("#FFFFFF".into());
    colors.primary_hover = blue_hover.clone();
    colors.primary_active = blue_active.clone();
    colors.button_primary = blue.clone();
    colors.button_primary_foreground = Some("#FFFFFF".into());
    colors.button_primary_hover = blue_hover;
    colors.button_primary_active = blue_active;
    colors.button = neutral.clone();
    colors.button_foreground = foreground.clone();
    colors.button_hover = color(dark, "#F3F3F6", "#353537");
    colors.button_active = color(dark, "#E6E6EB", "#404044");
    colors.ring = blue.clone();
    colors.caret = blue;
    colors.link = color(dark, "#0063CC", "#69ACF5");
    colors.list = neutral.clone();
    colors.list_hover = color(dark, "#F3F3F6", "#323234");
    colors.list_active = accent.clone();
    colors.list_active_border = accent.clone();
    colors.table = neutral.clone();
    colors.table_head = color(dark, "#F7F7F9", "#28282A");
    colors.table_head_foreground = foreground;
    colors.table_even = color(dark, "#FAFAFC", "#242426");
    colors.table_hover = color(dark, "#F3F3F6", "#323234");
    colors.table_active = accent.clone();
    colors.table_active_border = accent;
    colors.table_row_border = color(dark, "#E8E8EC", "#38383A");
    ThemeConfig {
        name: if dark { "QQ Farm Dark" } else { "QQ Farm Light" }.into(),
        mode: if dark { ThemeMode::Dark } else { ThemeMode::Light },
        font_size: Some(14.),
        radius: Some(6),
        radius_lg: Some(10),
        colors,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::theme::{Theme, ThemeMode};
    use gpui_kit::TestAppContext;

    #[gpui_kit::test]
    fn both_appearances_keep_product_colors_after_switching(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            super::init(cx);
            for mode in [ThemeMode::Light, ThemeMode::Dark, ThemeMode::Light] {
                Theme::change(mode, None, cx);
                let theme = Theme::global(cx);
                assert!(theme.primary.s > 0.5, "accent must retain the blue product theme");
                assert_ne!(theme.background, theme.sidebar);
                assert_eq!(theme.font_size.as_f32(), 14.);
            }
        });
    }
}
