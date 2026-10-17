use super::super::*;
use gpui_kit::assets::IconName;

impl FarmDesktop {
    pub(in crate::desktop) fn render_settings_ui(&self, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        div()
            .when(self.page == Page::Settings && self.page_section == "systemConfig", |panel| {
                panel.child(self.render_system_config_view(cx))
            })
            .when(self.page == Page::Settings && self.page_section == "ui", |panel| {
                panel.child(
                    div()
                        .v_flex()
                        .gap_3()
                        .rounded(cx.theme().radius_lg)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().group_box)
                        .p_4()
                        .child(div().font_semibold().child(
                            self.page.section_title_for("ui", language).unwrap_or("系统设置"),
                        ))
                        .child(
                            h_flex()
                                .id("settings-appearance-row")
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .w(px(64.))
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(crate::i18n::text(language, "主题", "Theme")),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(
                                            Button::new("appearance-system")
                                                .label(crate::i18n::text(
                                                    language,
                                                    "跟随系统",
                                                    "System",
                                                ))
                                                .when(
                                                    self.preferences.appearance == "system",
                                                    |button| button.primary(),
                                                )
                                                .when(
                                                    self.preferences.appearance != "system",
                                                    |button| button.ghost(),
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.set_appearance("system", window, cx)
                                                })),
                                        )
                                        .child(
                                            Button::new("appearance-light")
                                                .label(crate::i18n::text(language, "浅色", "Light"))
                                                .when(
                                                    self.preferences.appearance == "light",
                                                    |button| button.primary(),
                                                )
                                                .when(
                                                    self.preferences.appearance != "light",
                                                    |button| button.ghost(),
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.set_appearance("light", window, cx)
                                                })),
                                        )
                                        .child(
                                            Button::new("appearance-dark")
                                                .label(crate::i18n::text(language, "深色", "Dark"))
                                                .when(
                                                    self.preferences.appearance == "dark",
                                                    |button| button.primary(),
                                                )
                                                .when(
                                                    self.preferences.appearance != "dark",
                                                    |button| button.ghost(),
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.set_appearance("dark", window, cx)
                                                })),
                                        ),
                                ),
                        )
                        .child(
                            h_flex()
                                .id("settings-language-row")
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .w(px(64.))
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(crate::i18n::text(language, "语言", "Language")),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(
                                            Button::new("language-zh")
                                                .label("简体中文")
                                                .when(
                                                    self.preferences.language == "zh-CN",
                                                    |button| button.primary(),
                                                )
                                                .when(
                                                    self.preferences.language != "zh-CN",
                                                    |button| button.ghost(),
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.set_language("zh-CN", window, cx)
                                                })),
                                        )
                                        .child(
                                            Button::new("language-en")
                                                .label("English")
                                                .when(
                                                    self.preferences.language == "en-US",
                                                    |button| button.primary(),
                                                )
                                                .when(
                                                    self.preferences.language != "en-US",
                                                    |button| button.ghost(),
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.set_language("en-US", window, cx)
                                                })),
                                        ),
                                ),
                        )
                        .child(div().h(px(1.)).w_full().bg(cx.theme().border))
                        .child(
                            h_flex()
                                .id("settings-update-row")
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .w(px(64.))
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(crate::i18n::text(language, "更新", "Updates")),
                                )
                                .child(
                                    Button::new("settings-check-update")
                                        .icon(IconName::Download)
                                        .label(crate::i18n::text(
                                            language,
                                            "检查更新",
                                            "Check for updates",
                                        ))
                                        .ghost()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.dispatch_ui_command(&CheckForUpdates, window, cx)
                                        })),
                                ),
                        ),
                )
            })
            .when(self.page == Page::Settings, |panel| {
                panel.when_some(self.settings.save_notice.as_ref(), |panel, notice| {
                    let message = div()
                        .id("settings-save-notice")
                        .test_support()
                        .text_sm()
                        .text_color(if self.settings.save_notice_warning {
                            cx.theme().warning_foreground
                        } else {
                            cx.theme().success
                        })
                        .child(notice.clone());
                    panel.child(message)
                })
            })
    }
}

impl FarmDesktop {
    pub(in crate::desktop) fn render_setting_editor(&self, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        div().when_some(self.settings.edit_path.as_ref(), |panel, path| {
            panel.child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().group_box)
                    .p_3()
                    .child(if language == "en-US" {
                        format!("Edit setting · {path}")
                    } else {
                        format!("编辑设置 · {path}")
                    })
                    .child(
                        Input::new(&self.settings.edit_value).id("setting-edit-value").w(px(320.)),
                    )
                    .child(
                        Button::new("setting-edit-cancel")
                            .label(crate::i18n::text(language, "取消", "Cancel"))
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings.edit_path = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("setting-edit-save")
                            .label(crate::i18n::text(language, "应用", "Apply"))
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| this.save_setting_edit(cx))),
                    ),
            )
        })
    }
}
