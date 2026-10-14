use super::super::*;
use super::move_account_column;
use gpui_kit::base::v_flex;
use gpui_kit::component::popover::Popover;
use gpui_kit::component::select::Select;

impl FarmDesktop {
    pub(super) fn render_account_filters(&self, cx: &mut Context<Self>) -> Div {
        let column_settings = self.accounts.columns.clone();
        let all_columns_visible = column_settings.iter().all(|column| column.visible);
        let desktop = cx.entity().downgrade();
        let desktop_open = desktop.clone();
        let language = self.preferences.language.clone();
        let content_language = language.clone();
        let text = |chinese, english| crate::i18n::text(&language, chinese, english);
        let column_picker = Popover::new("account-column-settings")
            // Vue's column setting uses placement="bottom-end": the picker
            // drops below the trigger instead of covering the toolbar.
            .anchor(gpui_kit::Anchor::TopRight)
            .trigger(
                Button::new("account-column-settings-trigger")
                    .label(crate::i18n::text(&language, "列设置", "Columns"))
                    .ghost(),
            )
            .open(self.accounts.column_picker_open)
            .on_open_change(move |open, _, cx| {
                let _ = desktop_open.update(cx, |this, cx| {
                    this.accounts.column_picker_open = *open;
                    cx.notify();
                });
            })
            .content(move |_, _, _| {
                let desktop_all = desktop.clone();
                let all_label = crate::i18n::text(&content_language, "全选", "Select all");
                v_flex()
                    .w(px(320.))
                    .gap_2()
                    .child(
                        Checkbox::new("account-columns-select-all")
                            .label(all_label)
                            .checked(all_columns_visible)
                            .on_change(move |checked, _, cx| {
                                let _ = desktop_all.update(cx, |this, cx| {
                                    for column in &mut this.accounts.columns {
                                        column.visible = *checked;
                                    }
                                    cx.notify();
                                });
                            }),
                    )
                    .children(column_settings.iter().enumerate().map(|(index, column)| {
                        let desktop_visible = desktop.clone();
                        let desktop_pin = desktop.clone();
                        let desktop_up = desktop.clone();
                        let desktop_down = desktop.clone();
                        let title = column.id.title(&content_language);
                        let pin_label = column.pin.label(&content_language);
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Checkbox::new(format!(
                                    "account-column-visible-{}",
                                    column.id.key()
                                ))
                                .label(title)
                                .checked(column.visible)
                                .on_change(
                                    move |checked, _, cx| {
                                        let _ = desktop_visible.update(cx, |this, cx| {
                                            if let Some(column) =
                                                this.accounts.columns.get_mut(index)
                                            {
                                                column.visible = *checked;
                                            }
                                            cx.notify();
                                        });
                                    },
                                ),
                            )
                            .child(div().flex_1())
                            .child(
                                Button::new(format!("account-column-pin-{}", column.id.key()))
                                    .label(pin_label)
                                    .ghost()
                                    .accessibility_label(format!("{}: {}", title, pin_label))
                                    .on_click(move |_, _, cx| {
                                        let _ = desktop_pin.update(cx, |this, cx| {
                                            if let Some(column) =
                                                this.accounts.columns.get_mut(index)
                                            {
                                                column.pin = column.pin.next();
                                            }
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child(
                                Button::new(format!("account-column-up-{}", column.id.key()))
                                    .label("↑")
                                    .ghost()
                                    .accessibility_label(if content_language == "en-US" {
                                        format!("Move {title} up")
                                    } else {
                                        format!("将{title}上移")
                                    })
                                    .disabled(index == 0)
                                    .on_click(move |_, _, cx| {
                                        let _ = desktop_up.update(cx, |this, cx| {
                                            move_account_column(
                                                &mut this.accounts.columns,
                                                index,
                                                -1,
                                            );
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child(
                                Button::new(format!("account-column-down-{}", column.id.key()))
                                    .label("↓")
                                    .ghost()
                                    .accessibility_label(if content_language == "en-US" {
                                        format!("Move {title} down")
                                    } else {
                                        format!("将{title}下移")
                                    })
                                    .disabled(index + 1 == column_settings.len())
                                    .on_click(move |_, _, cx| {
                                        let _ = desktop_down.update(cx, |this, cx| {
                                            move_account_column(
                                                &mut this.accounts.columns,
                                                index,
                                                1,
                                            );
                                            cx.notify();
                                        });
                                    }),
                            )
                    }))
            });
        let search_panel = div()
            .id("account-search-panel")
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
                            .w(px(92.))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(text("关键词", "Keyword")),
                    )
                    .child(Input::new(&self.accounts.keyword).id("account-search").flex_1()),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(92.))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(text("平台", "Platform")),
                    )
                    .child(
                        Select::new(&self.accounts.platform_selector)
                            .id("account-platform-select")
                            .w(px(240.))
                            .accessibility_label(text("平台", "Platform")),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(92.))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(text("运行状态", "Run status")),
                    )
                    .child(
                        Select::new(&self.accounts.run_status_selector)
                            .id("account-run-status-select")
                            .w(px(240.))
                            .accessibility_label(text("运行状态", "Run Status")),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(92.))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(text("授权状态", "Authorization")),
                    )
                    .child(
                        Select::new(&self.accounts.auth_status_selector)
                            .id("account-auth-status-select")
                            .w(px(240.))
                            .accessibility_label(text("授权状态", "Authorization")),
                    ),
            )
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("account-search-reset")
                            .label(text("重置", "Reset"))
                            .ghost()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.reset_account_filters(window, cx)
                            })),
                    )
                    .child(
                        Button::new("account-search-submit")
                            .label(text("搜索", "Search"))
                            .primary()
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| this.apply_account_filters(cx))),
                    ),
            );
        #[cfg(test)]
        let search_panel = search_panel.test_support();

        div()
            .v_flex()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        Button::new("account-search-toggle")
                            .label(if self.accounts.search_open {
                                text("收起搜索", "Hide search")
                            } else {
                                text("搜索", "Search")
                            })
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.accounts.search_open = !this.accounts.search_open;
                                cx.notify();
                            })),
                    )
                    .child(column_picker),
            )
            .when(self.accounts.search_open, |panel| panel.child(search_panel))
    }
}
