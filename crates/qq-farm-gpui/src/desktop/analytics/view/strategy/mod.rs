use super::super::super::*;
use gpui_kit::component::input::NumberInput;
use serde_json::Value;

mod comparison;
mod metrics;
mod plant;

use metrics::{
    best_for, comparison_description, comparison_label, comparison_unit, current_strategy_label,
    eligible, integer, reference_level, STRATEGIES,
};

impl FarmDesktop {
    pub(super) fn render_analytics_strategy(&self, cx: &mut Context<Self>) -> Div {
        let account_level = integer(self.page_raw.get("accountLevel")).unwrap_or(1).clamp(1, 200);
        let reference_level = reference_level(
            account_level,
            &self.analytics.level.read(cx).value(),
            self.analytics.level_is_user_override,
        );
        let rankings = self
            .page_raw
            .get("rankings")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let configuration = self.page_raw.get("configuration").unwrap_or(&Value::Null);
        let current_strategy = configuration
            .get("plantingStrategy")
            .or_else(|| configuration.get("strategy"))
            .and_then(Value::as_str)
            .unwrap_or("max_exp");
        let strategy_label = current_strategy_label(current_strategy, &self.preferences.language)
            .unwrap_or(current_strategy);
        let preferred_seed = configuration
            .get("preferredSeedId")
            .or_else(|| configuration.get("preferredSeed"))
            .and_then(|value| integer(Some(value)))
            .unwrap_or_default();
        let current_best = if current_strategy == "preferred" && preferred_seed > 0 {
            rankings.iter().find(|row| integer(row.get("seedId")) == Some(preferred_seed))
        } else {
            STRATEGIES
                .iter()
                .find(|(key, _, _, _)| *key == current_strategy)
                .and_then(|(_, _, metric, _)| best_for(rankings, metric, reference_level))
        };
        let available = rankings.iter().filter(|row| eligible(row, reference_level)).count();

        let current_card = div()
            .id("analytics-current-strategy")
            .v_flex()
            .gap_3()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_4()
            .child(
                div().font_medium().child(format!(
                    "{}: {strategy_label}",
                    crate::i18n::text(&self.preferences.language, "当前策略", "Current strategy")
                )),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::i18n::text(
                        &self.preferences.language,
                        "与自动化设置页种植策略同步",
                        "Synced with automation settings",
                    )),
            )
            .when(current_strategy == "bag_priority", |card| {
                card.child(
                    div()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().warning.opacity(0.12))
                        .px_3()
                        .py_2()
                        .text_sm()
                        .text_color(cx.theme().warning)
                        .child(crate::i18n::text(
                            &self.preferences.language,
                            "背包种子优先策略：优先使用背包中的种子，按背包优先级排序，用完后回退到商店购买。具体种植内容取决于背包中实际持有的种子。",
                            "Bag-first strategy: use bag seeds by priority, then fall back to shop purchases.",
                        )),
                )
            })
            .when(current_strategy != "bag_priority", |card| {
                card.child(match current_best {
                    Some(row) => self.render_current_strategy_plant(row, cx).into_any_element(),
                    None => div()
                        .py_3()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(&self.preferences.language, "暂无可种植作物", "No plantable crops"))
                        .into_any_element(),
                })
            })
            .test_support();

        let comparison = div()
            .id("analytics-strategy-comparison")
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
                    .gap_3()
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(div().font_semibold().child(crate::i18n::text(
                                &self.preferences.language,
                                "策略对比",
                                "Strategy comparison",
                            )))
                            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                                crate::i18n::text(
                                    &self.preferences.language,
                                    "各策略下可种植的最优作物",
                                    "Best plantable crop under each strategy",
                                ),
                            )),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                                crate::i18n::text(
                                    &self.preferences.language,
                                    "参考等级",
                                    "Reference level",
                                ),
                            ))
                            .child(NumberInput::new(&self.analytics.level).w(px(112.))),
                    ),
            )
            .child(div().flex().flex_wrap().gap_3().children(STRATEGIES.iter().map(
                |(key, _, metric, _)| {
                    self.render_strategy_comparison_tile(
                        key,
                        comparison_label(key, &self.preferences.language),
                        comparison_description(key, &self.preferences.language),
                        metric,
                        comparison_unit(key, &self.preferences.language),
                        current_strategy,
                        rankings,
                        reference_level,
                        cx,
                    )
                },
            )))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                if self.preferences.language == "en-US" {
                    format!(
                        "{available}/{} plantable · blue border marks current strategy",
                        rankings.len()
                    )
                } else {
                    format!(
                        "可种植 {available}/{} 种作物 · 蓝色边框为当前设置的种植策略",
                        rankings.len()
                    )
                },
            ))
            .test_support();

        div().v_flex().gap_4().child(current_card).child(comparison)
    }
}
