use super::super::*;
use gpui_kit::TestSupportExt;

impl FarmDesktop {
    pub(in crate::desktop) fn personal_page_unavailable(&self) -> bool {
        self.page == Page::Personal
            && (self.selected_account.is_none()
                || self.page_raw.get("connected").and_then(serde_json::Value::as_bool)
                    == Some(false))
    }

    pub(in crate::desktop) fn render_personal_status(&self, cx: &mut Context<Self>) -> Div {
        let error = self
            .page_raw
            .get("sectionErrors")
            .and_then(|errors| errors.get(&self.page_section))
            .and_then(serde_json::Value::as_str);
        let text = crate::i18n::text;
        let language = &self.preferences.language;
        div()
            .v_flex()
            .gap_3()
            .when(self.personal_page_unavailable(), |panel| {
                let label = if self.selected_account.is_none() {
                    text(language, "请选择账号", "Select account")
                } else {
                    text(
                        language,
                        "账号未运行，请先启动账号",
                        "Account is not running. Please start it first.",
                    )
                };
                panel.child(
                    div()
                        .id("personal-unavailable")
                        .aria_label(label.to_string())
                        .py_12()
                        .text_center()
                        .text_color(cx.theme().muted_foreground)
                        .child(label)
                        .test_support(),
                )
            })
            .when_some(error, |panel, error| {
                // Section failures share the banner row used by the page level
                // error and update notices: message on the left, action on the right.
                panel.child(
                    h_flex()
                        .id("personal-section-error")
                        .aria_label(error.to_string())
                        .justify_between()
                        .items_center()
                        .gap_3()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().group_box)
                        .px_4()
                        .py_3()
                        .text_color(cx.theme().danger)
                        .child(error.to_string())
                        .child(
                            Button::new("personal-section-retry")
                                .label(text(language, "重试", "Retry"))
                                .ghost()
                                .disabled(self.loading)
                                .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                        )
                        .test_support(),
                )
            })
    }
}
