mod activities;
mod commerce;
mod filter;
mod personal;
mod rows;
mod state;
mod toolbar;

use state::ContentState;

use super::super::*;

impl FarmDesktop {
    pub(crate) fn render_content(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let state = ContentState::new(self);
        let language = self.preferences.language.as_str();
        // Vue hides the whole friends tab area and asks for an account when
        // none is selected; stale rows must not stay actionable.
        let friends_without_account = self.page == Page::Friends && self.selected_account.is_none();
        let row_contents = if self.page == Page::Settings
            || friends_without_account
            || state.personal_unavailable
            || state.personal_has_error
            || state.analytics_view
            || state.activity_directory
            || state.mall_catalog_view
            || state.mystery_shop_view
            || state.qixi_view
            || state.qingmei_view
            || state.constellation_view
            || state.solar_terms_view
            || state.season_view
            || state.weather_view
            || state.daily_gifts_view
            || state.personal_lands_view
            || state.personal_bag
            || state.personal_interaction_view
            || state.personal_illustrated_view
            || state.personal_pets_view
        {
            div().into_any_element()
        } else {
            let (visible_row_indices, rows_use_grid) = self.visible_row_indices(cx);
            self.render_row_contents(visible_row_indices, rows_use_grid, window, cx)
        };
        let mut page_views = Vec::new();
        if self.page == Page::Dashboard {
            page_views.push(self.render_dashboard_panel(cx).into_any_element());
        }
        page_views.extend(self.render_personal_content(&state, window, cx));
        page_views.extend(self.render_activity_content(&state, window, cx));
        page_views.extend(self.render_commerce_content(&state, cx));
        div()
            .v_flex()
            .size_full()
            .min_w_0()
            .bg(cx.theme().background)
            .child(self.render_content_toolbar(cx))
            .child(
                if state.personal_unavailable
                    || matches!(self.page, Page::Accounts | Page::Dashboard)
                    || (state.analytics_view && self.selected_account.is_none())
                    || friends_without_account
                {
                    div().into_any_element()
                } else {
                    self.render_page_sections(cx).into_any_element()
                },
            )
            .child(
                if state.personal_unavailable
                    || matches!(self.page, Page::Accounts | Page::Dashboard | Page::Analytics)
                    || friends_without_account
                {
                    div().into_any_element()
                } else {
                    self.render_page_controls(cx).into_any_element()
                },
            )
            .child(if self.page == Page::Accounts {
                self.render_accounts_page(window, cx).into_any_element()
            } else if state.analytics_view {
                self.render_analytics_view(cx)
            } else {
                let panel = div()
                    .v_flex()
                    .gap_3()
                    .p_6()
                    .flex_1()
                    .min_h_0()
                    .when(matches!(self.page, Page::Dashboard | Page::Personal), |panel| {
                        panel.when_some(
                            self.farm.lands.operation_notice.as_ref(),
                            |panel, (message, success)| {
                                panel.child(
                                    div()
                                        .id("farm-operation-feedback")
                                        .rounded(cx.theme().radius)
                                        .bg(cx.theme().group_box)
                                        .px_4()
                                        .py_3()
                                        .text_color(if *success {
                                            cx.theme().success
                                        } else {
                                            cx.theme().danger
                                        })
                                        .child(message.clone())
                                        .test_support(),
                                )
                            },
                        )
                    })
                    .children(page_views)
                    .child(self.render_settings_ui(cx))
                    .when(state.charity_view, |panel| panel.child(self.render_activity_rules(cx)))
                    .when_some(self.update_message.as_ref(), |panel, message| {
                        panel.child(
                            h_flex()
                                .id("update-notice")
                                .aria_label(message.clone())
                                .justify_between()
                                .items_center()
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().group_box)
                                .px_4()
                                .py_3()
                                .child(message.clone())
                                .test_support()
                                .when_some(self.release_url.as_ref(), |row, _| {
                                    row.child(
                                        Button::new("open-release")
                                            .label(crate::i18n::text(
                                                language,
                                                "打开发布页下载",
                                                "Download from release page",
                                            ))
                                            .primary()
                                            .on_click(cx.listener(|this, _, _, _| {
                                                this.open_release_page()
                                            })),
                                    )
                                }),
                        )
                    })
                    .child(self.render_game_config_overlays(cx))
                    .child(self.render_setting_editor(cx))
                    .when(self.loading, |panel| {
                        panel.child(
                            div()
                                .id("page-loading")
                                .aria_label(crate::i18n::text(language, "正在加载…", "Loading…"))
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(crate::i18n::text(language, "正在加载…", "Loading…"))
                                .test_support(),
                        )
                    })
                    .when_some(state.friend_section_error.as_ref(), |panel, error| {
                        let message = format!(
                            "{}: {error}",
                            crate::i18n::text(language, "加载失败", "Loading failed")
                        );
                        let error_panel = div()
                            .id("friend-section-error")
                            .aria_label(message.clone())
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().group_box)
                            .p_4()
                            .text_color(cx.theme().danger)
                            .child(message);
                        #[cfg(test)]
                        let error_panel = error_panel.test_support();
                        panel.child(error_panel)
                    })
                    .when_some(
                        self.error
                            .as_ref()
                            .filter(|_| !state.pet_diary_error && !state.weather_error),
                        |panel, error| {
                            let banner = div()
                                .id("page-error")
                                .aria_label(error.clone())
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().group_box)
                                .p_4()
                                .text_color(cx.theme().danger)
                                .child(error.clone())
                                .test_support();
                            panel.child(banner)
                        },
                    )
                    .when(
                        self.page != Page::Dashboard
                            && self.page != Page::Settings
                            && !state.personal_unavailable
                            && !state.personal_has_error
                            && self.page != Page::Analytics
                            // The game-config catalog table renders its own
                            // in-table empty state plus pagination, so the
                            // generic page empty would duplicate it.
                            && self.page != Page::GameConfig
                            && (self.rows.is_empty() || friends_without_account)
                            && !self.loading
                            && self.error.is_none()
                            && !state.charity_view
                            && !state.activity_directory
                            && !state.activity_shop_view
                            && !state.pet_diary_view
                            && !state.autumn_view
                            && !state.qixi_view
                            && !state.qingmei_view
                            && !state.constellation_view
                            && !state.solar_terms_view
                            && !state.season_view
                            && !state.weather_view
                            && !state.mall_catalog_view
                            && !state.mystery_shop_view
                            && !state.daily_gifts_view
                            && !state.personal_lands_view
                            && !state.personal_bag
                            && !state.personal_interaction_view
                            && !state.personal_illustrated_view
                            && !state.personal_pets_view
                            && state.friend_section_error.is_none(),
                        |panel| {
                            let empty_label = if friends_without_account {
                                crate::i18n::text(language, "请选择账号", "Select account")
                            } else {
                                match (self.page, self.page_section.as_str()) {
                                    (Page::Friends, "friends") => {
                                        crate::i18n::text(language, "无数据", "No Data")
                                    }
                                    (Page::Friends, "blacklist") => crate::i18n::text(
                                        language,
                                        "暂无黑名单好友",
                                        "No blacklisted friends",
                                    ),
                                    (Page::Friends, "records") => crate::i18n::text(
                                        language,
                                        "暂无访客记录",
                                        "No visitor records",
                                    ),
                                    _ => crate::i18n::text(
                                        language,
                                        "这里还没有可显示的内容",
                                        "Nothing to show here yet",
                                    ),
                                }
                            };
                            panel.child({
                                let label = div()
                                    .id("empty-state-label")
                                    .aria_label(empty_label.to_string())
                                    .v_flex()
                                    .gap_2()
                                    .items_center()
                                    .py_12()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(empty_label);
                                #[cfg(test)]
                                let label = label.test_support();
                                label.child(
                                    Button::new("empty-refresh")
                                        .label(crate::i18n::text(language, "刷新", "Refresh"))
                                        .ghost()
                                        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                                )
                            })
                        },
                    )
                    .when(
                        self.page != Page::Dashboard
                            && self.page != Page::Settings
                            && !state.personal_unavailable
                            && !state.personal_has_error
                            && self.page != Page::Analytics
                            && !state.charity_view
                            && !state.activity_shop_view
                            && !state.pet_diary_view
                            && !state.autumn_view
                            && !state.qixi_view
                            && !state.qingmei_view
                            && !state.constellation_view
                            && !state.solar_terms_view
                            && !state.season_view
                            && !state.weather_view
                            && !state.mall_catalog_view
                            && !state.mystery_shop_view
                            && !state.daily_gifts_view
                            && !state.personal_lands_view
                            && !state.personal_bag
                            && !state.personal_interaction_view
                            && !state.personal_illustrated_view
                            && !state.personal_pets_view,
                        |panel| panel.child(row_contents),
                    );
                if state.personal_illustrated_view {
                    panel.into_any_element()
                } else {
                    panel.overflow_y_scrollbar().into_any_element()
                }
            })
    }
}
