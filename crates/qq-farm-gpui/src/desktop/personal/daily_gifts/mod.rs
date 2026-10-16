use super::super::*;
use gpui_kit::Hsla;
use gpui_kit::TestSupportExt;

mod labels;
use labels::{
    gift_detail, gift_icon, gift_status, progress_label, task_progress_is_empty, value_i64,
    GiftStatusKind,
};

impl FarmDesktop {
    pub(in crate::desktop) fn render_personal_daily_gifts(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let language = &self.preferences.language;
        let gifts =
            self.page_raw.pointer("/dailyGifts/gifts").and_then(serde_json::Value::as_array);
        let growth = self.page_raw.pointer("/dailyGifts/growth");
        let growth_tasks =
            growth.and_then(|value| value.get("tasks")).and_then(serde_json::Value::as_array);
        let keyword = self.search.read(cx).value().trim().to_lowercase();
        let gifts_title = crate::i18n::text(language, "每日礼包 & 任务", "Daily Gifts & Tasks");
        let growth_title = crate::i18n::text(language, "成长任务", "Growth Tasks");

        div()
            .v_flex()
            .gap_5()
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().group_box)
                    .p_4()
                    .child(div().font_semibold().child(gifts_title))
                    .when(gifts.is_none(), |panel| {
                        panel.child(empty_state(
                            crate::i18n::text(language, "请登录账号后查看", "Log in to view"),
                            cx,
                        ))
                    })
                    .when_some(gifts, |panel, gifts| {
                        if gifts.is_empty() {
                            panel.child(empty_state(
                                crate::i18n::text(
                                    language,
                                    "暂无每日礼包与任务数据",
                                    "No daily gift data",
                                ),
                                cx,
                            ))
                        } else {
                            // 均匀网格（对齐 Tauri auto-fill minmax(200px,1fr)）：
                            // 列数按内容区宽度自适应，卡片等宽不再参差。
                            let rem = window.rem_size().as_f32();
                            let sidebar = if self.sidebar_collapsed {
                                64.
                            } else {
                                self.preferences.sidebar_width as f32
                            };
                            let available =
                                (window.bounds().size.width.as_f32() - sidebar - rem * 3.)
                                    .max(rem * 10.);
                            let columns = ((available / (rem * 14.)).floor() as usize).clamp(2, 6);
                            panel.child(
                                div()
                                    .id("daily-gifts-grid")
                                    .grid()
                                    .grid_cols(columns as u16)
                                    .gap_3()
                                    .children(gifts.iter().filter_map(|gift| {
                                        let key = gift.get("key")?.as_str().unwrap_or_default();
                                        let label = gift
                                            .get("label")
                                            .and_then(serde_json::Value::as_str)
                                            .unwrap_or(crate::i18n::text(language, "礼包", "Gift"));
                                        let status = gift_status(gift, language);
                                        let status_kind = GiftStatusKind::from_gift(gift);
                                        let detail = gift_detail(gift, language);
                                        let icon = gift_icon(key);
                                        let matches_keyword = keyword.is_empty()
                                            || label.to_lowercase().contains(&keyword)
                                            || status.to_lowercase().contains(&keyword)
                                            || detail.to_lowercase().contains(&keyword);
                                        matches_keyword.then(|| {
                                            div()
                                                .id(format!("daily-gift-{key}"))
                                                .v_flex()
                                                .w_full()
                                                .gap_2()
                                                .rounded(cx.theme().radius)
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .bg(cx.theme().background)
                                                .p_3()
                                                .child(
                                                    h_flex()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .w(px(32.))
                                                                .h(px(32.))
                                                                .rounded(cx.theme().radius)
                                                                .bg(cx.theme().group_box)
                                                                .child(div().text_sm().child(icon)),
                                                        )
                                                        .child(
                                                            div()
                                                                .v_flex()
                                                                .min_w_0()
                                                                .flex_1()
                                                                .gap_1()
                                                                .child(
                                                                    div()
                                                                        .truncate()
                                                                        .font_medium()
                                                                        .child(label.to_string()),
                                                                )
                                                                .child(h_flex().child(status_tag(
                                                                    format!(
                                                                        "daily-gift-status-{key}"
                                                                    ),
                                                                    status,
                                                                    status_kind.color(cx),
                                                                ))),
                                                        ),
                                                )
                                                .when(!detail.is_empty(), |card| {
                                                    card.child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(detail),
                                                    )
                                                })
                                                .test_support()
                                        })
                                    })),
                            )
                        }
                    }),
            )
            .child(
                div()
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
                            .justify_between()
                            .child(div().font_semibold().child(growth_title))
                            .when_some(growth, |header, growth| {
                                let completed = value_i64(growth.get("completedCount"));
                                let total = value_i64(growth.get("totalCount"));
                                let done = growth
                                    .get("doneToday")
                                    .and_then(serde_json::Value::as_bool)
                                    .unwrap_or(false);
                                let label = if done {
                                    crate::i18n::text(language, "今日已完成", "Done today")
                                        .to_string()
                                } else {
                                    progress_label(completed, total, language)
                                };
                                header.child(status_tag(
                                    "growth-summary-status".into(),
                                    label,
                                    if done { cx.theme().success } else { cx.theme().info },
                                ))
                            }),
                    )
                    .when(growth_tasks.is_none_or(Vec::is_empty), |panel| {
                        panel.child(empty_state(
                            crate::i18n::text(language, "暂无成长任务", "No growth tasks"),
                            cx,
                        ))
                    })
                    .when_some(growth_tasks, |panel, tasks| {
                        panel.children(tasks.iter().enumerate().filter_map(|(index, task)| {
                            let description = task
                                .get("desc")
                                .or_else(|| task.get("description"))
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or(crate::i18n::text(language, "任务", "Task"));
                            if !keyword.is_empty() && !description.to_lowercase().contains(&keyword)
                            {
                                return None;
                            }
                            let completed = task
                                .get("isCompleted")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false);
                            let current = value_i64(task.get("progress"));
                            let total = value_i64(task.get("totalProgress"));
                            let state = if completed {
                                crate::i18n::text(language, "已完成", "Completed").to_string()
                            } else {
                                crate::i18n::text(language, "进行中", "In progress").to_string()
                            };
                            let progress = if completed || task_progress_is_empty(task) {
                                state.clone()
                            } else {
                                progress_label(current, total, language)
                            };
                            let progress_element = div()
                                .id(format!("growth-task-progress-{index}"))
                                .aria_label(progress.clone())
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .whitespace_nowrap()
                                .child(progress);
                            #[cfg(test)]
                            let progress_element = progress_element.test_support();
                            let progress_element = progress_element.into_any_element();
                            let description_element = div()
                                .id(format!("growth-task-description-{index}"))
                                .min_w_0()
                                .flex_1()
                                .text_sm()
                                .truncate()
                                .child(description.to_string());
                            #[cfg(test)]
                            let description_element = description_element.test_support();
                            let description_element = description_element.into_any_element();
                            Some(
                                h_flex()
                                    .id(format!("growth-task-{index}"))
                                    .items_center()
                                    .justify_between()
                                    .gap_3()
                                    .rounded(cx.theme().radius)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .px_3()
                                    .py_2()
                                    .child(description_element)
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(status_tag(
                                                format!("growth-task-status-{index}"),
                                                state,
                                                if completed {
                                                    cx.theme().success
                                                } else {
                                                    cx.theme().info
                                                },
                                            ))
                                            .child(progress_element),
                                    )
                                    .test_support(),
                            )
                        }))
                    }),
            )
    }
}

fn empty_state(label: &'static str, cx: &Context<FarmDesktop>) -> Div {
    div()
        .w_full()
        .py_6()
        .text_center()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(label)
}

fn status_tag(id: String, label: String, color: Hsla) -> AnyElement {
    let tag = div()
        .id(id)
        .aria_label(label.clone())
        .rounded_full()
        .bg(color.opacity(0.16))
        .px_2()
        .py_1()
        .text_xs()
        .text_color(color)
        .child(label);
    #[cfg(test)]
    let tag = tag.test_support();
    tag.into_any_element()
}
