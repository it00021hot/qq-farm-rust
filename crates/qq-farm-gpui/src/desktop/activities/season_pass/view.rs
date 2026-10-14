use super::super::super::*;
use super::super::rewards::reward_chip;

fn value_text(value: Option<&serde_json::Value>) -> String {
    value
        .map(value_to_action_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "--".into())
}

fn number(value: Option<&serde_json::Value>) -> Option<f64> {
    value.and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_i64().map(|value| value as f64))
            .or_else(|| value.as_str()?.parse::<f64>().ok())
    })
}

fn display_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_991.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

fn pass_progress_display(progress: f64, progress_max: f64) -> (String, String) {
    let max_text =
        if progress_max == 0.0 { "--".to_string() } else { display_number(progress_max) };
    (display_number(progress), max_text)
}

impl FarmDesktop {
    pub(in crate::desktop) fn render_season_pass_panel(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let pass = self.page_raw.pointer("/season/pass").unwrap_or(&serde_json::Value::Null);

        let level = pass.get("level").or_else(|| pass.get("currentLevel"));
        let progress = number(pass.get("progress").or_else(|| pass.get("currentProgress")))
            .unwrap_or_default();
        let progress_max = number(pass.get("progressMax").or_else(|| pass.get("progressTarget")))
            .unwrap_or_default();
        let (progress_text, progress_max_text) = pass_progress_display(progress, progress_max);
        let ratio =
            if progress_max > 0. { (progress / progress_max).clamp(0., 1.) as f32 } else { 0. };
        let nodes =
            pass.get("nodes").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
        let has_claimable = super::has_claimable_reward(pass);
        let claim_gate = super::claim_action_enabled(&self.page_raw);
        let has_rules = pass
            .get("rules")
            .is_some_and(|rules| !rules.is_null() && rules != &serde_json::json!([]));

        let progress_card = div()
            .h_flex()
            .flex_wrap()
            .items_center()
            .gap_4()
            .rounded(cx.theme().radius_lg)
            .bg(cx.theme().primary.opacity(0.1))
            .p_4()
            .child(
                div()
                    .size(px(64.))
                    .v_flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(cx.theme().primary)
                    .text_color(cx.theme().background)
                    .child(div().text_2xl().font_semibold().child(value_text(level)))
                    .child(div().text_xs().child(crate::i18n::text(language, "等级", "Level"))),
            )
            .child(
                div()
                    .v_flex()
                    .min_w(px(200.))
                    .flex_1()
                    .gap_2()
                    .child(div().font_medium().child(crate::i18n::text(
                        language,
                        "游记积分",
                        "Travel Score",
                    )))
                    .child(format!("{progress_text} / {progress_max_text}"))
                    .child(div().h(px(8.)).w_full().rounded_full().bg(cx.theme().border).child(
                        div().h_full().w(relative(ratio)).rounded_full().bg(cx.theme().primary),
                    )),
            );

        let claim_all = Button::new("season-pass-claim")
            .label(if self.loading || self.activity_action_is_pending("battle-pass") {
                crate::i18n::text(language, "领取中…", "Claiming…")
            } else {
                crate::i18n::text(language, "一键领取", "Claim All")
            })
            .primary()
            .loading(self.activity_action_is_pending("battle-pass"))
            .disabled(
                self.loading || self.selected_account.is_none() || !claim_gate || !has_claimable,
            )
            .on_click(cx.listener(|this, _, _, cx| this.run_activity_action("battle-pass", cx)));

        let panel = div().v_flex().gap_4().child(progress_card).child(
            h_flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_3()
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                    crate::i18n::text(
                        language,
                        "收获游记果实可获得游记积分",
                        "Harvest travel fruits to earn travel score",
                    ),
                ))
                .when(has_rules, |row| {
                    row.child(
                        Button::new("season-rules-toggle")
                            .label(crate::i18n::text(language, "活动说明", "Event info"))
                            .ghost()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_activity_rules_dialog(window, cx);
                            })),
                    )
                }),
        );

        if nodes.is_empty() {
            return panel
                .child(
                    div()
                        .py_8()
                        .text_center()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(language, "暂无奖励节点", "No reward tiers")),
                )
                .child(h_flex().justify_center().child(claim_all));
        }

        let current_level = value_text(level);
        let node_rows = nodes
            .iter()
            .map(|node| {
                let node_level = node
                    .get("level")
                    .or_else(|| node.get("id"))
                    .map(value_to_action_string)
                    .unwrap_or_else(|| "--".into());
                let current = node.get("current").and_then(serde_json::Value::as_bool)
                    == Some(true)
                    || node_level == current_level;
                let claimed =
                    node.get("claimed").and_then(serde_json::Value::as_bool) == Some(true);
                let claimable =
                    node.get("claimable").and_then(serde_json::Value::as_bool) == Some(true);
                let rewards = node
                    .get("rewards")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let mut card =
                    h_flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .rounded(cx.theme().radius_lg)
                        .border_1()
                        .border_color(if current { cx.theme().primary } else { cx.theme().border })
                        .bg(cx.theme().group_box)
                        .p_3()
                        .when(claimed, |card| card.opacity(0.7))
                        .child(
                            h_flex()
                                .min_w_0()
                                .flex_1()
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .size(px(44.))
                                        .v_flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(cx.theme().radius)
                                        .bg(cx.theme().primary.opacity(0.12))
                                        .text_color(cx.theme().primary)
                                        .child(div().font_semibold().child(node_level))
                                        .child(
                                            div().text_xs().child(crate::i18n::text(
                                                language, "等级", "Level",
                                            )),
                                        ),
                                )
                                .child(h_flex().flex_wrap().gap_2().children(
                                    rewards.iter().map(|reward| reward_chip(reward, cx)),
                                )),
                        );
                let state = if claimed {
                    Some(crate::i18n::text(language, "已领取", "Claimed"))
                } else if claimable {
                    Some(crate::i18n::text(language, "可领取", "Claimable"))
                } else {
                    None
                };
                if let Some(state) = state {
                    card = card.child(
                        div()
                            .text_xs()
                            .text_color(if claimable {
                                cx.theme().success
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(state),
                    );
                }
                card.into_any_element()
            })
            .collect::<Vec<_>>();

        panel
            .child(div().v_flex().gap_2().children(node_rows))
            .child(h_flex().justify_center().child(claim_all))
    }
}

#[cfg(test)]
mod tests {
    use super::pass_progress_display;

    #[test]
    fn travel_progress_display_matches_vue_number_fallbacks() {
        assert_eq!(pass_progress_display(0.0, 0.0), ("0".into(), "--".into()));
        assert_eq!(pass_progress_display(25.0, 50.0), ("25".into(), "50".into()));
        assert_eq!(pass_progress_display(25.5, 0.0), ("25.5".into(), "--".into()));
        assert_eq!(pass_progress_display(0.0, 7.5), ("0".into(), "7.5".into()));
    }
}
