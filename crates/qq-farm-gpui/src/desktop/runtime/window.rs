use super::*;
use gpui_kit::component::theme::{Theme, ThemeMode};

pub(in crate::desktop) fn observe_window(window: &mut Window, cx: &mut Context<FarmDesktop>) {
    cx.observe_window_bounds(window, |this, window, _| {
        let bounds = window.bounds();
        let width = bounds.size.width.as_f32().round().max(0.0) as u32;
        let height = bounds.size.height.as_f32().round().max(0.0) as u32;
        if width >= 1100
            && height >= 720
            && (this.preferences.window_width != width || this.preferences.window_height != height)
        {
            this.preferences.window_width = width;
            this.preferences.window_height = height;
            this.persist_ui_preferences();
        }
    })
    .detach();
    cx.observe_window_appearance(window, |this, window, cx| {
        if this.preferences.appearance == "system" {
            Theme::change(ThemeMode::from(window.appearance()), Some(window), cx);
        }
    })
    .detach();
}
