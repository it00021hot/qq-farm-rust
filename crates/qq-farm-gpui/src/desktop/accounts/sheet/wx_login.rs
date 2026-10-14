use super::super::super::*;

impl FarmDesktop {
    pub(super) fn render_account_wx_fields(&self, form: Div, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        let local_profile = self.accounts.wx_quick_profile.as_ref();
        let qr_path = self.accounts.wx_qr_path.clone();
        let local = self.accounts.wx_mode == "local";
        let profile_name = local_profile
            .map(|profile| {
                if profile.2.trim().is_empty() {
                    crate::i18n::text(language, "本机微信", "WeChat for desktop").to_string()
                } else {
                    profile.2.clone()
                }
            })
            .unwrap_or_default();
        let avatar = local_profile
            .map(|profile| profile.3.trim())
            .filter(|avatar| !avatar.is_empty())
            .map(|avatar| {
                let avatar = div()
                    .id("wx-local-avatar")
                    .size(px(72.))
                    .rounded_full()
                    .overflow_hidden()
                    .child(render_asset_image(avatar, 72., cx));
                #[cfg(test)]
                let avatar = avatar.test_support();
                avatar.into_any_element()
            })
            .unwrap_or_else(|| {
                let avatar = div()
                    .id("wx-local-avatar-fallback")
                    .text_3xl()
                    .font_semibold()
                    .text_color(cx.theme().blue)
                    .child("微");
                #[cfg(test)]
                let avatar = avatar.test_support();
                avatar.into_any_element()
            });

        form.child(
            div()
                .v_flex()
                .items_center()
                .justify_center()
                .gap_3()
                .w_full()
                .min_h(px(260.))
                .rounded(cx.theme().radius_lg)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().group_box)
                .p_5()
                .when(local, |panel| {
                    panel
                        .child(avatar)
                        .when(!profile_name.is_empty(), |panel| {
                            panel.child(div().font_medium().child(profile_name.clone()))
                        })
                        .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                            if self.accounts.wx_status.is_empty() {
                                crate::i18n::text(
                                    language,
                                    "正在检测本机微信…",
                                    "Checking for WeChat on this device…",
                                )
                                .to_string()
                            } else {
                                super::super::account_status_text(
                                    language,
                                    &self.accounts.wx_status,
                                )
                            },
                        ))
                        .when_some(local_profile, |panel, profile| {
                            let port = profile.0;
                            panel.child(
                                Button::new("wx-local-authorize")
                                    .label(crate::i18n::text(
                                        language,
                                        "微信快捷登录",
                                        "Continue with WeChat",
                                    ))
                                    .success()
                                    .disabled(self.accounts.form_saving)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let bounds = window.bounds();
                                        let x = (bounds.origin.x.as_f32()
                                            + bounds.size.width.as_f32() / 2.
                                            - 180.)
                                            .round()
                                            as i32;
                                        let y = (bounds.origin.y.as_f32()
                                            + bounds.size.height.as_f32() / 2.
                                            - 132.)
                                            .round()
                                            as i32;
                                        let _ = port;
                                        this.authorize_local_wechat(x, y, cx);
                                    })),
                            )
                        })
                        .child(
                            Button::new("wx-switch-qr")
                                .label(crate::i18n::text(
                                    language,
                                    "切换扫码登录",
                                    "Use QR code instead",
                                ))
                                .ghost()
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.start_wx_qr_flow(None, cx)),
                                ),
                        )
                })
                .when(!local, |panel| {
                    panel
                        .child(
                            div()
                                .w(px(220.))
                                .h(px(220.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().background)
                                .when_some(qr_path.clone(), |qr, path| {
                                    qr.child(
                                        img(path)
                                            .w(px(208.))
                                            .h(px(208.))
                                            .object_fit(ObjectFit::Contain),
                                    )
                                })
                                .when(qr_path.is_none(), |qr| {
                                    qr.child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(crate::i18n::text(
                                                language,
                                                "二维码加载中…",
                                                "Loading QR code…",
                                            )),
                                    )
                                }),
                        )
                        .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                            if self.accounts.wx_status.is_empty() {
                                crate::i18n::text(
                                    language,
                                    "等待微信扫码",
                                    "Waiting for WeChat scan",
                                )
                                .to_string()
                            } else {
                                super::super::account_status_text(
                                    language,
                                    &self.accounts.wx_status,
                                )
                            },
                        ))
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new("wx-refresh-qr")
                                        .label(crate::i18n::text(
                                            language,
                                            "刷新二维码",
                                            "Refresh QR code",
                                        ))
                                        .ghost()
                                        .disabled(self.accounts.form_saving)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.start_wx_qr_flow(None, cx)
                                        })),
                                )
                                .child(
                                    Button::new("wx-detect-local")
                                        .label(crate::i18n::text(
                                            language,
                                            "重新检测本机微信",
                                            "Check for WeChat again",
                                        ))
                                        .ghost()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.start_wx_auth_flow(cx)
                                        })),
                                ),
                        )
                }),
        )
    }
}
