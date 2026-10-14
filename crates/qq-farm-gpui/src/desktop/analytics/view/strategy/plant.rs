use super::super::super::super::*;
use super::metrics::{format_level, integer, season_count, value_text};
use serde_json::Value;

impl FarmDesktop {
    pub(super) fn render_current_strategy_plant(
        &self,
        row: &Value,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = &self.preferences.language;
        let english = language == "en-US";
        let name = row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(crate::i18n::text(language, "未知作物", "Unknown crop"))
            .to_string();
        let level = format_level(row.get("level"), language);
        let seasons = season_count(row.get("seasons"));
        let preview = self.render_analytics_plant_image(row, cx);
        h_flex()
            .id(format!(
                "analytics-current-plant-{}",
                integer(row.get("seedId")).unwrap_or_default()
            ))
            .items_center()
            .gap_3()
            .child(
                div()
                    .size(px(52.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .child(preview),
            )
            .child(
                div()
                    .v_flex()
                    .min_w_0()
                    .flex_1()
                    .gap_2()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(div().font_medium().child(name))
                            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                                if english {
                                    format!("Lv{level} · {seasons} season(s)")
                                } else {
                                    format!("Lv{level} · {seasons} 季")
                                },
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_3()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(if english {
                                format!("EXP/hour: {}", value_text(row, "expPerHour"))
                            } else {
                                format!("经验/时：{}", value_text(row, "expPerHour"))
                            })
                            .child(if english {
                                format!("Profit/hour: {}", value_text(row, "profitPerHour"))
                            } else {
                                format!("利润/时：{}", value_text(row, "profitPerHour"))
                            })
                            .child(if english {
                                format!(
                                    "Normal fertilizer EXP/hour: {}",
                                    value_text(row, "normalFertilizerExpPerHour")
                                )
                            } else {
                                format!(
                                    "普肥经验/时：{}",
                                    value_text(row, "normalFertilizerExpPerHour")
                                )
                            })
                            .child(if english {
                                format!(
                                    "Normal fertilizer profit/hour: {}",
                                    value_text(row, "normalFertilizerProfitPerHour")
                                )
                            } else {
                                format!(
                                    "普肥利润/时：{}",
                                    value_text(row, "normalFertilizerProfitPerHour")
                                )
                            }),
                    ),
            )
            .test_support()
    }

    pub(in crate::desktop::analytics::view) fn render_analytics_plant_image(
        &self,
        row: &Value,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let image = row.get("image").and_then(Value::as_str).unwrap_or_default();
        if image.is_empty() {
            return div()
                .size(px(42.))
                .flex()
                .items_center()
                .justify_center()
                .text_2xl()
                .child("🌱")
                .into_any_element();
        }
        let preview_row = DisplayRow {
            id: row.get("seedId").map(ToString::to_string).unwrap_or_default(),
            section: "rankings".into(),
            title: row.get("name").and_then(Value::as_str).unwrap_or_default().into(),
            detail: String::new(),
            state: String::new(),
            record: row.clone(),
        };
        render_asset_preview(&preview_row, cx).unwrap_or_else(|| {
            div()
                .size(px(42.))
                .flex()
                .items_center()
                .justify_center()
                .text_2xl()
                .child("🌱")
                .into_any_element()
        })
    }
}
