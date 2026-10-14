use super::super::events;
use super::super::*;
use super::filter::{
    event_key, humanize_log_message, log_matches, should_follow_tail_after_scroll,
};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::ScrollWheelEvent;

const LOG_RENDER_CAP: usize = 300;

impl FarmDesktop {
    pub(in crate::desktop::dashboard) fn render_logs_panel(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let language = self.preferences.language.as_str();
        let log_keyword = self.dashboard.log_keyword.read(cx).value().trim().to_lowercase();
        let logs = self
            .rows
            .iter()
            .filter(|row| row.section == "logs" || row.section == "warnings")
            .filter(|row| {
                log_matches(
                    row,
                    &self.dashboard.module_filter,
                    &self.dashboard.event_filter,
                    &self.dashboard.level_filter,
                    &log_keyword,
                    language,
                )
            })
            .collect::<Vec<_>>();
        let hidden_count = logs.len().saturating_sub(LOG_RENDER_CAP);
        let visible_logs = if self.dashboard.logs_expanded || hidden_count == 0 {
            logs.as_slice()
        } else {
            &logs[hidden_count..]
        };
        let module_filter = self.dashboard.module_filter.clone();
        let level_filter = self.dashboard.level_filter.clone();
        let event_filter = self.dashboard.event_filter.clone();
        let event_options = events::EVENT_LABELS
            .iter()
            .filter(|(key, _, _)| !events::is_hidden(key))
            .map(|(key, _, _)| (key.to_string(), events::label(key, language)))
            .collect::<Vec<_>>();
        let all_events_label = crate::i18n::text(language, "所有事件", "All events").to_string();
        let weak = cx.entity().downgrade();
        let module_weak = weak.clone();
        let level_weak = weak.clone();
        let event_button = Button::new("dashboard-event-filter")
            .label(if event_filter.is_empty() {
                all_events_label.clone()
            } else {
                events::label(&event_filter, language)
            })
            .dropdown_caret(true)
            .dropdown_menu(move |menu, _, _| {
                let weak_for_all = weak.clone();
                let mut menu = menu.item(
                    PopupMenuItem::new(all_events_label.clone())
                        .checked(event_filter.is_empty())
                        .on_click(move |_, _, cx| {
                            let _ = weak_for_all.update(cx, |this, cx| {
                                this.dashboard.event_filter.clear();
                                cx.notify();
                            });
                        }),
                );
                for (key, label) in event_options.iter().cloned() {
                    let weak_for_item = weak.clone();
                    let checked = key == event_filter;
                    menu = menu.item(PopupMenuItem::new(label).checked(checked).on_click(
                        move |_, _, cx| {
                            let _ = weak_for_item.update(cx, |this, cx| {
                                this.dashboard.event_filter = key.clone();
                                cx.notify();
                            });
                        },
                    ));
                }
                menu
            });
        let module_options = [
            ("", crate::i18n::text(language, "所有模块", "All modules")),
            ("farm", crate::i18n::text(language, "农场", "Farm")),
            ("friend", crate::i18n::text(language, "好友", "Friends")),
            ("system", crate::i18n::text(language, "系统", "System")),
        ];
        let module_button = Button::new("dashboard-module-filter")
            .label(if module_filter.is_empty() {
                module_options[0].1
            } else {
                module_options
                    .iter()
                    .find(|(key, _)| *key == module_filter)
                    .map_or(module_options[0].1, |(_, label)| *label)
            })
            .dropdown_caret(true)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu;
                for (key, label) in module_options {
                    let weak = module_weak.clone();
                    let checked = key == module_filter;
                    menu = menu.item(PopupMenuItem::new(label).checked(checked).on_click(
                        move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.dashboard.module_filter = key.into();
                                cx.notify();
                            });
                        },
                    ));
                }
                menu
            });
        let level_options = [
            ("", crate::i18n::text(language, "所有等级", "All levels")),
            ("info", crate::i18n::text(language, "普通", "Info")),
            ("warn", crate::i18n::text(language, "警告", "Warnings")),
        ];
        let level_button = Button::new("dashboard-level-filter")
            .label(if level_filter.is_empty() {
                level_options[0].1
            } else {
                level_options
                    .iter()
                    .find(|(key, _)| *key == level_filter)
                    .map_or(level_options[0].1, |(_, label)| *label)
            })
            .dropdown_caret(true)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu;
                for (key, label) in level_options {
                    let weak = level_weak.clone();
                    let checked = key == level_filter;
                    menu = menu.item(PopupMenuItem::new(label).checked(checked).on_click(
                        move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.dashboard.level_filter = key.into();
                                cx.notify();
                            });
                        },
                    ));
                }
                menu
            });

        let log_rows: AnyElement = if logs.is_empty() {
            let empty_label =
                crate::i18n::text(language, "暂无实时事件", "No realtime events yet").to_string();
            div()
                .id("dashboard-empty-logs")
                .aria_label(empty_label.clone())
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(empty_label)
                .test_support()
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h_0()
                .relative()
                .vertical_scrollbar(&self.dashboard.logs_scroll)
                .child(
                    // Native overflow + track_scroll keeps the follow-tail handle
                    // connected; the gpui-component Scrollable wrapper owns a
                    // private handle that silently disconnected it.
                    div()
                        .id("dashboard-logs-scroll")
                        .size_full()
                        .v_flex()
                        .gap_2()
                        .track_scroll(&self.dashboard.logs_scroll)
                        .on_scroll_wheel(cx.listener(
                            |this, event: &ScrollWheelEvent, window, cx| {
                                let delta_y =
                                    event.delta.pixel_delta(window.line_height()).y.as_f32();
                                let offset_y = this.dashboard.logs_scroll.offset().y.as_f32();
                                let max_offset_y =
                                    this.dashboard.logs_scroll.max_offset().y.as_f32();
                                this.dashboard.logs_follow_tail = should_follow_tail_after_scroll(
                                    offset_y,
                                    max_offset_y,
                                    delta_y,
                                );
                                cx.notify();
                            },
                        ))
                        .overflow_y_scroll()
                        .children(visible_logs.iter().map(|row| render_log_row(row, language, cx)))
                        .when(hidden_count > 0 && !self.dashboard.logs_expanded, |list| {
                            list.child(
                                Button::new("dashboard-show-older-logs")
                                    .label(if language == "en-US" {
                                        format!("Show {hidden_count} earlier logs")
                                    } else {
                                        format!("显示更早的 {hidden_count} 条日志")
                                    })
                                    .primary()
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.dashboard.logs_expanded = true;
                                        cx.notify();
                                    })),
                            )
                        })
                        .when(
                            self.dashboard.logs_expanded && logs.len() > LOG_RENDER_CAP,
                            |list| {
                                list.child(
                                    Button::new("dashboard-collapse-logs")
                                        .label(crate::i18n::text(language, "收起", "Collapse"))
                                        .ghost()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.dashboard.logs_expanded = false;
                                            cx.notify();
                                        })),
                                )
                            },
                        ),
                )
                .into_any_element()
        };

        let logs_panel = div()
            .id("dashboard-logs-panel")
            .v_flex()
            .flex_1()
            .min_h(px(280.))
            .gap_3()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_4()
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_between()
                    .items_center()
                    .gap_2()
                    .child(div().font_medium().child(crate::i18n::text(
                        language,
                        "运行日志",
                        "Running logs",
                    )))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_1()
                            .child(module_button)
                            .child(event_button)
                            .child(level_button)
                            .child(
                                Input::new(&self.dashboard.log_keyword)
                                    .id("dashboard-log-search")
                                    .w(px(170.))
                                    .cleanable(true),
                            )
                            .child(
                                Button::new("dashboard-clear-logs")
                                    .label(crate::i18n::text(language, "清空", "Clear"))
                                    .ghost()
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.clear_dashboard_logs(cx)),
                                    ),
                            ),
                    ),
            )
            .child(div().h(px(1.)).bg(cx.theme().border))
            .child(log_rows)
            .test_support();

        logs_panel.into_any_element()
    }

    pub(in crate::desktop) fn clear_dashboard_logs(&mut self, cx: &mut Context<Self>) {
        if let Some(account_id) = self.selected_account.as_deref() {
            qq_farm_app::farm::clear_global_logs(&self.app, Some(account_id));
        }
        self.page_raw["logs"] = serde_json::json!([]);
        self.rows = pages::page_data(Page::Dashboard, self.page_raw.clone()).rows;
        self.dashboard.logs_expanded = false;
        self.dashboard.logs_follow_tail = true;
        self.dashboard.logs_scroll.scroll_to_bottom();
        cx.notify();
    }
}

fn render_log_row(row: &DisplayRow, language: &str, cx: &Context<FarmDesktop>) -> impl IntoElement {
    let tag = row.record.get("tag").and_then(serde_json::Value::as_str).unwrap_or("系统");
    let event_key = event_key(row);
    let event = events::label(event_key, language);
    let display_tag = match tag {
        "农场" => crate::i18n::text(language, "农场", "Farm"),
        "系统" => crate::i18n::text(language, "系统", "System"),
        "好友" => crate::i18n::text(language, "好友", "Friends"),
        "错误" => crate::i18n::text(language, "错误", "Error"),
        "心跳" => crate::i18n::text(language, "心跳", "Heartbeat"),
        other => other,
    };
    let message = humanize_log_message(&row.title, language);
    let time = row.record.get("time").and_then(serde_json::Value::as_str).unwrap_or("");
    let is_warn = row.section == "warnings";
    let tag_color = match tag {
        "错误" => cx.theme().danger,
        "系统" => cx.theme().primary,
        "好友" => cx.theme().warning,
        _ => cx.theme().green,
    };
    h_flex()
        .items_start()
        .gap_2()
        .border_b_1()
        .border_color(cx.theme().border)
        .pb_2()
        .text_sm()
        .child(
            div()
                .w(px(72.))
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(time.to_string()),
        )
        .child(
            div()
                .rounded_full()
                .bg(cx.theme().background)
                .px_2()
                .py_1()
                .text_xs()
                .text_color(tag_color)
                .child(display_tag.to_string()),
        )
        .when(events::should_show_chip(tag, event_key), |line| {
            line.child(
                div()
                    .rounded_full()
                    .bg(cx.theme().background)
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(cx.theme().primary)
                    .child(event),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(if is_warn { cx.theme().danger } else { cx.theme().foreground })
                .child(message),
        )
}
