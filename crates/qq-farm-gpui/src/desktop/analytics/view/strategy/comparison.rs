use super::super::super::super::*;
use super::metrics::{best_for, comparison_value_text, format_level};
use serde_json::Value;

impl FarmDesktop {
    pub(super) fn render_strategy_comparison_tile(
        &self,
        key: &str,
        label: &str,
        description: &str,
        metric: &str,
        unit: &'static str,
        current_strategy: &str,
        rankings: &[Value],
        reference_level: f64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = &self.preferences.language;
        let best = best_for(rankings, metric, reference_level);
        let is_current = key == current_strategy;
        div()
            .id(format!("analytics-strategy-{key}"))
            .v_flex()
            .gap_2()
            .w(px(220.))
            .min_h(px(168.))
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(if is_current { cx.theme().accent } else { cx.theme().border })
            .bg(cx.theme().background)
            .p_3()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_sm().font_medium().child(label.to_string()))
                    .when(is_current, |tile| {
                        tile.child(
                            div()
                                .rounded_full()
                                .bg(cx.theme().accent)
                                .px_2()
                                .py_1()
                                .text_xs()
                                .text_color(cx.theme().primary_foreground)
                                .child(crate::i18n::text(language, "当前策略", "Active")),
                        )
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(description.to_string()),
            )
            .when_some(best, |tile, row| {
                let title = row
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(crate::i18n::text(language, "未知作物", "Unknown crop"))
                    .to_string();
                let level = format_level(row.get("level"), language);
                let value =
                    if key == "level" { level.clone() } else { comparison_value_text(row, metric) };
                tile.child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(self.render_analytics_plant_image(row, cx))
                        .child(
                            div()
                                .v_flex()
                                .min_w_0()
                                .gap_1()
                                .child(div().truncate().font_medium().child(title))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("Lv{level}")),
                                ),
                        ),
                )
                .child(
                    div()
                        .h_flex()
                        .justify_between()
                        .items_center()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().group_box)
                        .px_2()
                        .py_1()
                        .child(div().text_xs().text_color(cx.theme().muted_foreground).child(unit))
                        .child(div().font_semibold().child(value)),
                )
            })
            .when(best.is_none(), |tile| {
                tile.child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(language, "暂无可种植作物", "No plantable crops")),
                )
            })
            .test_support()
    }
}
