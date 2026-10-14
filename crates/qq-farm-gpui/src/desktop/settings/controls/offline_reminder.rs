use super::super::super::*;

use gpui_kit::component::input::InputContentType;
use gpui_kit::component::select::Select;

impl FarmDesktop {
    pub(super) fn append_offline_reminder_controls(
        &self,
        mut controls: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        let reminder = self.settings.draft.get("offlineReminder");
        let provider = reminder
            .and_then(|value| value.get("provider"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("none");
        let is_qq_bot = provider == "qq_bot";
        let is_ding_talk = provider == "ding_talk";
        let bound = self
            .page_raw
            .pointer("/qqBotBinding/bound")
            .and_then(serde_json::Value::as_bool)
            .or_else(|| {
                reminder
                    .and_then(|value| value.pointer("/qqBotBinding/userOpenid"))
                    .and_then(serde_json::Value::as_str)
                    .map(|openid| !openid.trim().is_empty())
            })
            .unwrap_or(false);
        let nickname = self
            .page_raw
            .pointer("/qqBotBinding/binding/nickname")
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                reminder
                    .and_then(|value| value.pointer("/qqBotBinding/nickname"))
                    .and_then(serde_json::Value::as_str)
            })
            .unwrap_or("");
        let invite_url = self
            .page_raw
            .pointer("/qqBotBinding/botInviteUrl")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let credentials_ready = !self.settings.qq_bot_app_id.read(cx).value().trim().is_empty()
            && !self.settings.qq_bot_secret.read(cx).value().trim().is_empty();
        let test_ready = offline_test_ready(
            provider,
            &self.settings.offline_endpoint.read(cx).value(),
            &self.settings.offline_token.read(cx).value(),
            bound,
        );

        let mut panel = div()
            .v_flex()
            .gap_3()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_4()
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .child(settings_text(&self.preferences.language, "机器人类型")),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Select::new(&self.settings.offline_provider)
                                .id("offline-reminder-provider")
                                .w_full()
                                .accessibility_label(settings_text(
                                    &self.preferences.language,
                                    "机器人类型",
                                ))
                                .disabled(self.loading),
                        ),
                    )
                    .child(
                        Button::new("open-qq-bot-docs")
                            .label(settings_text(&self.preferences.language, "官网"))
                            .disabled(!is_qq_bot)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Err(error) =
                                    FarmDesktop::open_external_url("https://bot.q.qq.com/wiki/")
                                {
                                    this.error = Some(crate::i18n::error_with_detail(
                                        &this.preferences.language,
                                        "无法打开链接",
                                        "Could not open the link",
                                        error,
                                    ));
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .child({
                let hint = settings_text(
                    &self.preferences.language,
                    "绑定 QQ 机器人后，账号下线、上线、应用宝授权失效时会自动推送，无需配置标题和内容。",
                );
                div()
                    .id("offline-reminder-hint")
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .aria_label(hint.clone())
                    .child(hint)
                    .test_support()
            });

        if is_qq_bot {
            let waiting = self.settings.qq_bot_session_id.is_some();
            let status_label = if bound {
                settings_text(&self.preferences.language, "已绑定")
            } else if waiting {
                settings_text(&self.preferences.language, "等待扫码")
            } else {
                settings_text(&self.preferences.language, "未绑定")
            };
            let mut status_tag = div().rounded_full().px_2().py_1().text_xs();
            status_tag = if bound {
                status_tag.bg(cx.theme().success.opacity(0.16)).text_color(cx.theme().success)
            } else {
                status_tag.bg(cx.theme().muted)
            };
            let status_tag = status_tag.child(status_label);
            panel = panel
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(labeled_field(
                            settings_text(&self.preferences.language, "AppID"),
                            Input::new(&self.settings.qq_bot_app_id)
                                .id("settings-qqbot-app-id")
                                .w(px(420.)),
                        ))
                        .child(labeled_field(
                            settings_text(&self.preferences.language, "AppSecret"),
                            Input::new(&self.settings.qq_bot_secret)
                                .id("settings-qqbot-secret")
                                .w(px(420.))
                                .content_type(InputContentType::Password)
                                .mask_toggle(),
                        ))
                        .child({
                            let hint = settings_text(
                                &self.preferences.language,
                                "填写 AppID 和 AppSecret 后点绑定，再用手机 QQ 打开机器人发任意消息完成绑定。绑定后自动推送：下线、上线、应用宝授权二维码。",
                            );
                            div()
                                .id("qq-bot-hint")
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .aria_label(hint.clone())
                                .child(hint)
                                .test_support()
                        }),
                )
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .child(settings_text(&self.preferences.language, "绑定状态")),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(status_tag)
                                .when(!nickname.is_empty(), |row| {
                                    row.child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(nickname.to_string()),
                                    )
                                }),
                        )
                        .when_some(self.settings.qq_bot_status.as_ref(), |section, status| {
                            section.child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(settings_text(&self.preferences.language, status)),
                            )
                        })
                        .when_some(self.settings.qq_bot_qr_path.clone(), |section, path| {
                            section.child(
                                div()
                                    .v_flex()
                                    .gap_2()
                                    .child(
                                        img(path)
                                            .id("qq-bot-bind-qr")
                                            .w(px(180.))
                                            .h(px(180.))
                                            .object_fit(ObjectFit::Contain)
                                            .test_support(),
                                    )
                                    .child({
                                        let hint = settings_text(
                                                &self.preferences.language,
                                                "如已填写邀请链接，可扫码打开机器人资料卡，再发任意消息。",
                                            );
                                        div()
                                            .id("qq-bot-bind-scan-hint")
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .aria_label(hint.clone())
                                            .child(hint)
                                            .test_support()
                                    }),
                            )
                        })
                        .when(
                            self.settings.qq_bot_session_id.is_some()
                                && self.settings.qq_bot_qr_path.is_none(),
                            |section| {
                                section.child(
                                    {
                                        let hint = settings_text(
                                            &self.preferences.language,
                                            "等待绑定中：请用手机 QQ 打开该机器人，发送任意消息（无需扫本页二维码）。",
                                        );
                                        div()
                                            .id("qq-bot-bind-manual-hint")
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .aria_label(hint.clone())
                                            .child(hint)
                                            .test_support()
                                    },
                                )
                            },
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Button::new("qqbot-bind-start")
                                        .label(if self.settings.qq_bot_session_id.is_some() {
                                            settings_text(&self.preferences.language, "等待扫码…")
                                        } else {
                                            settings_text(
                                                &self.preferences.language,
                                                "绑定 QQ 通知",
                                            )
                                        })
                                        .primary()
                                        .loading(self.settings.qq_bot_bind_loading)
                                        .disabled(
                                            !credentials_ready
                                                || self.settings.qq_bot_session_id.is_some()
                                                || self.settings.qq_bot_bind_loading
                                                || self.loading,
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.start_qq_bot_binding(cx)
                                        })),
                                )
                                .child(
                                    Button::new("qqbot-open-bot")
                                        .label(settings_text(
                                            &self.preferences.language,
                                            "打开机器人",
                                        ))
                                        .ghost()
                                        .disabled(invite_url.is_empty())
                                        .on_click(
                                            cx.listener(|this, _, _, _| this.open_bot_invite()),
                                        ),
                                )
                                .child(
                                    Button::new("qqbot-bind-unbind")
                                        .label(settings_text(&self.preferences.language, "解绑"))
                                        .ghost()
                                        .disabled(!bound || self.loading)
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.unbind_qq_bot(cx)),
                                        ),
                                ),
                        ),
                );
        } else if is_ding_talk {
            panel = panel
                .child(labeled_field(
                    settings_text(&self.preferences.language, "Webhook 地址"),
                    Input::new(&self.settings.offline_endpoint)
                        .id("settings-dingtalk-endpoint")
                        .aria_label(settings_text(&self.preferences.language, "Webhook 地址"))
                        .w(px(640.)),
                ))
                .child(labeled_field(
                    settings_text(&self.preferences.language, "Access Token"),
                    Input::new(&self.settings.offline_token)
                        .id("settings-dingtalk-token")
                        .aria_label(settings_text(&self.preferences.language, "Access Token"))
                        .w(px(640.))
                        .content_type(InputContentType::Password)
                        .mask_toggle(),
                ))
                .child(labeled_field(
                    settings_text(&self.preferences.language, "加签密钥"),
                    Input::new(&self.settings.offline_secret)
                        .id("settings-dingtalk-secret")
                        .aria_label(settings_text(&self.preferences.language, "加签密钥"))
                        .w(px(640.))
                        .content_type(InputContentType::Password)
                        .mask_toggle(),
                ))
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                    settings_text(
                        &self.preferences.language,
                        "Webhook 地址与 Access Token 二选一；机器人安全设置选择「加签」时填写加签密钥。",
                    ),
                ));
        } else if provider == "wechat_bot" {
            panel = panel.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(settings_text(&self.preferences.language, "微信机器人暂未实现。")),
            );
        }

        let test_button = Button::new("test-offline-reminder")
            .label(settings_text(&self.preferences.language, "测试通知"))
            .ghost()
            .loading(self.settings.offline_test_loading)
            .disabled(!test_ready || self.loading)
            .on_click(cx.listener(|this, _, _, cx| this.test_offline_reminder(cx)));
        let save_button = Button::new("save-offline-reminder")
            .label(settings_text(&self.preferences.language, "保存下线提醒"))
            .primary()
            .disabled(self.loading || self.settings.offline_test_loading)
            .on_click(cx.listener(|this, _, _, cx| this.save_offline_reminder(cx)));

        panel = panel.child(
            h_flex()
                .justify_end()
                .gap_2()
                .border_t_1()
                .border_color(cx.theme().border)
                .pt_3()
                .child(test_button)
                .child(save_button),
        );
        controls = controls.child(panel);
        controls
    }
}

fn labeled_field(label: String, input: impl IntoElement) -> impl IntoElement {
    div().v_flex().gap_1().child(div().text_sm().child(label)).child(input)
}

fn offline_test_ready(provider: &str, endpoint: &str, token: &str, bound: bool) -> bool {
    match provider {
        "qq_bot" => bound,
        "ding_talk" => !endpoint.trim().is_empty() || !token.trim().is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::offline_test_ready;

    #[test]
    fn offline_test_button_follows_provider_setup_requirements() {
        assert!(!offline_test_ready("none", "", "", false));
        assert!(!offline_test_ready("qq_bot", "", "", false));
        assert!(offline_test_ready("qq_bot", "", "", true));
        assert!(!offline_test_ready("ding_talk", "", "", false));
        assert!(offline_test_ready("ding_talk", "https://hook", "", false));
        assert!(offline_test_ready("ding_talk", "", "token", false));
        assert!(!offline_test_ready("wechat_bot", "https://hook", "token", true));
    }
}
