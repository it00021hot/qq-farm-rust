use super::super::super::*;

impl FarmDesktop {
    pub(super) fn render_account_code_fields(
        &self,
        form: Div,
        form_platform: &str,
        cx: &mut Context<Self>,
    ) -> Div {
        let language = &self.preferences.language;
        let code = self.accounts.code.read(cx).value().to_string();
        form.child(div().text_sm().font_medium().child(crate::i18n::text(
            language,
            "Code / 登录 URL",
            "Code / Login URL",
        )))
        .child(Input::new(&self.accounts.code).id("account-code"))
        .child(div().text_sm().text_color(cx.theme().muted_foreground).child(crate::i18n::text(
            language,
            "支持粘贴 QQ / 微信登录链接，平台会自动识别。",
            "Paste a QQ or WeChat login URL; the platform is detected automatically.",
        )))
        .when_some(super::super::account_login_url_hint(&code, language), |form, hint| {
            let hint = div()
                .id("account-login-url-hint")
                .text_sm()
                .text_color(cx.theme().blue)
                .aria_label(hint.clone())
                .child(hint);
            #[cfg(test)]
            let hint = hint.test_support();
            form.child(hint)
        })
        .child(div().text_sm().font_medium().child(crate::i18n::text(language, "平台", "Platform")))
        .child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("account-form-platform-qq")
                        .label("QQ")
                        .when(form_platform == "qq", |button| button.primary())
                        .when(form_platform != "qq", |button| button.ghost())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.accounts.platform = "qq".into();
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("account-form-platform-wx")
                        .label(crate::i18n::text(language, "微信", "WeChat"))
                        .when(form_platform == "wx", |button| button.primary())
                        .when(form_platform != "wx", |button| button.ghost())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.accounts.platform = "wx".into();
                            cx.notify();
                        })),
                ),
        )
    }
}
