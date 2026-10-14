use super::super::super::*;
use super::model::{entries, period, sort_records, status};

impl FarmDesktop {
    pub(in crate::desktop) fn render_activity_directory(&self, cx: &mut Context<Self>) -> Div {
        if self.selected_account.is_none() {
            return div().py_12().text_color(cx.theme().muted_foreground).child(crate::i18n::text(
                &self.preferences.language,
                "请选择账号查看活动",
                "Select an account to view activities",
            ));
        }

        let query = self.search.read(cx).value().trim().to_lowercase();
        let now = chrono::Local::now().timestamp_millis();
        let language = self.preferences.language.as_str();
        let mut records = entries(&self.page_raw, language);
        records.retain(|record| {
            query.is_empty()
                || record
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|name| name.to_lowercase().contains(&query))
        });
        sort_records(&mut records, now);

        if records.is_empty() {
            return div()
                .v_flex()
                .items_center()
                .gap_3()
                .py_12()
                .text_color(cx.theme().muted_foreground)
                .child(if self.loading {
                    crate::i18n::text(language, "正在加载活动…", "Loading activities…")
                } else {
                    crate::i18n::text(language, "当前没有可用活动", "No activities available")
                });
        }

        div().h_flex().flex_wrap().gap_3().children(records.iter().enumerate().map(
            |(index, record)| {
                let title = record
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .filter(|name| !name.trim().is_empty())
                    .map(str::to_string)
                    .or_else(|| record.get("id").map(value_to_action_string))
                    .unwrap_or_else(|| crate::i18n::text(language, "活动", "Activity").into());
                let (status_label, status_order) = status(record, now);
                let status_label = crate::i18n::text(
                    language,
                    status_label,
                    match status_order {
                        0 => "Active",
                        1 => "Upcoming",
                        _ => "Ended",
                    },
                );
                let supported = activity_section_for_record(record).is_some();
                let period_label = period(record, language);
                let action_label = if supported {
                    crate::i18n::text(language, "查看详情", "View details")
                } else {
                    crate::i18n::text(language, "暂未支持详情", "Details not supported")
                };
                let status_element = div()
                    .id(format!("activity-directory-status-{index}"))
                    .w_full()
                    .text_xs()
                    .text_color(if status_order == 0 {
                        cx.theme().success
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(status_label);
                #[cfg(test)]
                let status_element = status_element.test_support();
                let title_element = div()
                    .id(format!("activity-directory-title-{index}"))
                    .w_full()
                    .min_w_0()
                    .truncate()
                    .font_medium()
                    .child(title.clone());
                #[cfg(test)]
                let title_element = title_element.test_support();
                let period_element = div()
                    .id(format!("activity-directory-period-{index}"))
                    .w_full()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(period_label.clone());
                #[cfg(test)]
                let period_element = period_element.test_support();
                let action_element = div()
                    .id(format!("activity-directory-action-{index}"))
                    .w_full()
                    .min_w_0()
                    .whitespace_normal()
                    .text_sm()
                    .text_color(if supported {
                        cx.theme().primary
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(action_label);
                #[cfg(test)]
                let action_element = action_element.test_support();
                let accessible_label =
                    format!("{title}, {status_label}, {period_label}, {action_label}");
                let record = record.clone();
                let mut card = Button::new(("activity-directory-open", index))
                    .accessibility_label(accessible_label)
                    .v_flex()
                    .items_start()
                    .w(px(300.))
                    .min_h(px(166.))
                    .gap_3()
                    .text_left()
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().group_box)
                    .p_4()
                    .disabled(!supported)
                    .when(!supported, |card| card.opacity(0.6))
                    .child(
                        div()
                            .v_flex()
                            .size_full()
                            .min_w_0()
                            .justify_between()
                            .items_start()
                            .gap_3()
                            .child(
                                div()
                                    .v_flex()
                                    .w_full()
                                    .min_w_0()
                                    .gap_2()
                                    .child(status_element)
                                    .child(title_element)
                                    .child(period_element),
                            )
                            .child(action_element),
                    );
                if supported {
                    card = card.on_click(cx.listener(move |this, _, window, cx| {
                        this.open_activity_record(&record, window, cx)
                    }));
                }
                card
            },
        ))
    }
}
