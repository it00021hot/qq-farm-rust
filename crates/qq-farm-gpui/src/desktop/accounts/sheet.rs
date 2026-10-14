use super::super::*;
use gpui_kit::component::{sheet::Sheet, WindowExt};

mod code_login;
mod wx_login;

impl FarmDesktop {
    pub(in crate::desktop) fn render_account_sheet(
        &self,
        sheet: Sheet,
        cx: &mut Context<Self>,
    ) -> Sheet {
        let editing = self.accounts.editing_id.is_some();
        let typed_code = self.accounts.code.read(cx).value().to_string();
        let (form_platform, _) = parse_account_login_input(&typed_code, &self.accounts.platform);
        sheet
            .size(rems(33.))
            .resizable(false)
            .overlay_closable(false)
            .title(self.render_account_sheet_header(editing, cx))
            .child(self.render_account_sheet_body(&form_platform, cx))
            .footer(self.render_account_sheet_footer(cx))
            .on_close(cx.listener(|this, _, _, cx| {
                this.close_account_form();
                cx.notify();
            }))
    }

    fn render_account_sheet_header(&self, editing: bool, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        div()
            .v_flex()
            .gap_1()
            .child(div().text_lg().font_semibold().child(crate::i18n::text(
                language,
                if editing { "编辑账号" } else { "新增账号" },
                if editing { "Edit Account" } else { "Add Account" },
            )))
            .child(div().text_sm().font_normal().text_color(cx.theme().muted_foreground).child(
                crate::i18n::text(
                    language,
                    if editing {
                        "更新登录方式或账号名称"
                    } else {
                        "选择输入登录凭证或使用微信授权"
                    },
                    if editing {
                        "Update the login method or account name"
                    } else {
                        "Enter a login credential or use WeChat authorization"
                    },
                ),
            ))
    }

    fn render_account_sheet_body(&self, form_platform: &str, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        div()
            .v_flex()
            .gap_4()
            .py_4()
            .child(div().text_sm().font_medium().child(crate::i18n::text(
                language,
                "账号备注",
                "Display Name",
            )))
            .child(Input::new(&self.accounts.name).id("account-name"))
            .child(self.render_account_login_tabs(cx))
            .when(self.accounts.login_tab == "code", |form| {
                self.render_account_code_fields(form, form_platform, cx)
            })
            .when(self.accounts.login_tab == "wx", |form| self.render_account_wx_fields(form, cx))
            .child(
                div().text_sm().font_medium().child(crate::i18n::text(language, "备注", "Remark")),
            )
            .child(
                div().id("account-remark").child(
                    Textarea::new(&self.accounts.remark)
                        .accessibility_id("account-remark-input")
                        .h(px(82.))
                        .w_full(),
                ),
            )
            .when_some(self.accounts.wx_error.as_ref(), |form, error| {
                form.child(
                    div()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().warning)
                        .p_3()
                        .text_sm()
                        .text_color(cx.theme().warning_foreground)
                        .child(super::localized_account_message(&self.preferences.language, error)),
                )
            })
            .when_some(self.accounts.form_error.as_ref(), |form, error| {
                form.child(
                    div()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().danger)
                        .p_3()
                        .text_sm()
                        .text_color(cx.theme().danger_foreground)
                        .child(super::localized_account_message(&self.preferences.language, error)),
                )
            })
    }

    fn render_account_login_tabs(&self, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        h_flex()
            .gap_2()
            .p_1()
            .rounded(cx.theme().radius)
            .bg(cx.theme().group_box)
            .child(
                Button::new("account-login-code")
                    .label(crate::i18n::text(language, "输入 code", "Enter code"))
                    .when(self.accounts.login_tab == "code", |button| button.primary())
                    .when(self.accounts.login_tab != "code", |button| button.ghost())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.accounts.login_tab = "code".into();
                        this.accounts.wx_generation = this.accounts.wx_generation.wrapping_add(1);
                        this.accounts.wx_task_id = None;
                        this.accounts.wx_session_id = None;
                        this.accounts.wx_quick_profile = None;
                        if let Some(path) = this.accounts.wx_qr_path.take() {
                            let _ = std::fs::remove_file(path);
                        }
                        this.accounts.wx_error = None;
                        this.accounts.wx_status.clear();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("account-login-wx")
                    .label(crate::i18n::text(language, "微信授权", "WeChat authorization"))
                    .when(self.accounts.login_tab == "wx", |button| button.primary())
                    .when(self.accounts.login_tab != "wx", |button| button.ghost())
                    .on_click(cx.listener(|this, _, _, cx| this.start_wx_auth_flow(cx))),
            )
    }

    fn render_account_sheet_footer(&self, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        h_flex()
            .justify_end()
            .gap_2()
            .w_full()
            .child(
                Button::new("account-cancel")
                    .label(crate::i18n::text(language, "取消", "Cancel"))
                    .ghost()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.close_account_form();
                        window.close_sheet(cx);
                        cx.notify();
                    })),
            )
            .when(self.accounts.login_tab == "code", |footer| {
                footer.child(
                    Button::new("account-save")
                        .label(crate::i18n::text(language, "确认", "Confirm"))
                        .primary()
                        .disabled(self.accounts.form_saving)
                        .on_click(cx.listener(|this, _, _, cx| this.save_account(cx))),
                )
            })
            .when(self.accounts.form_saving, |footer| {
                footer.child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                    super::account_status_text(
                        &self.preferences.language,
                        &self.accounts.wx_status,
                    ),
                ))
            })
    }
}
