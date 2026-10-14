use gpui_kit::base::StyledExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::*;

fn conflict_copy(owner: &str, language: &str) -> (String, String, String) {
    let owner = match owner {
        "tauri" => crate::i18n::text(language, "Tauri 桌面端", "Tauri desktop client"),
        "gpui" => crate::i18n::text(language, "GPUI 桌面端", "GPUI desktop client"),
        _ => crate::i18n::text(language, "另一个桌面端", "Another desktop client"),
    };
    let title = crate::i18n::text(language, "QQ Farm 已在运行", "QQ Farm is already running");
    let description = if language == "en-US" {
        format!("{owner} is using this data directory. The existing window was brought forward.")
    } else {
        format!("{owner}正在使用此数据目录，已唤起现有窗口。请关闭此提示后继续使用。")
    };
    let button = crate::i18n::text(language, "确定", "OK");
    (title.into(), description, button.into())
}

struct ConflictNotice {
    owner: String,
    language: String,
}

impl Render for ConflictNotice {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let (title, description, button) = conflict_copy(&self.owner, &self.language);
        div()
            .v_flex()
            .gap_3()
            .p_6()
            .child(div().text_lg().font_semibold().child(title))
            .child(description)
            .child(
                Button::new("close-notice").label(button).primary().on_click(|_, _, cx| cx.quit()),
            )
    }
}

pub fn show(owner: String, language: String) {
    application().run(move |cx| {
        init(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(420.), px(180.)), cx)),
            window_min_size: Some(size(px(420.), px(180.))),
            app_id: Some("com.qqfarm.gpui.conflict".into()),
            ..Default::default()
        };
        open_window(options, cx, |_, cx| cx.new(|_| ConflictNotice { owner, language }))
            .expect("open data directory conflict notice");
    });
}

#[cfg(test)]
mod tests {
    use super::conflict_copy;

    #[test]
    fn data_directory_conflict_notice_follows_the_selected_language() {
        assert_eq!(
            conflict_copy("tauri", "zh-CN"),
            (
                "QQ Farm 已在运行".into(),
                "Tauri 桌面端正在使用此数据目录，已唤起现有窗口。请关闭此提示后继续使用。".into(),
                "确定".into(),
            )
        );
        assert_eq!(
            conflict_copy("tauri", "en-US"),
            (
                "QQ Farm is already running".into(),
                "Tauri desktop client is using this data directory. The existing window was brought forward.".into(),
                "OK".into(),
            )
        );
        assert_eq!(
            conflict_copy("unknown", "en-US").1,
            "Another desktop client is using this data directory. The existing window was brought forward."
        );
    }
}
