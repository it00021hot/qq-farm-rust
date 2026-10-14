use super::super::super::*;

use super::shared::{item_image, item_label, number};

fn stage_status(stage: &serde_json::Value, language: &str) -> &'static str {
    if stage.get("claimable").and_then(serde_json::Value::as_bool) == Some(true) {
        crate::i18n::text(language, "待领取", "Claimable")
    } else if stage.get("claimed").and_then(serde_json::Value::as_bool) == Some(true) {
        crate::i18n::text(language, "已完成", "Done")
    } else if stage.get("current").and_then(serde_json::Value::as_bool) == Some(true) {
        crate::i18n::text(language, "建设中", "In progress")
    } else if stage.get("completed").and_then(serde_json::Value::as_bool) == Some(true) {
        crate::i18n::text(language, "已完成", "Done")
    } else {
        crate::i18n::text(language, "未达成", "Locked")
    }
}

pub(super) fn render(
    desktop: &FarmDesktop,
    activity: &serde_json::Value,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let language = desktop.preferences.language.as_str();
    let bridge = activity.get("bridge").unwrap_or(&serde_json::Value::Null);
    let stages = bridge.get("stages").and_then(serde_json::Value::as_array);
    let current = number(bridge.get("currentStage")).unwrap_or_default();
    let can_claim =
        activity.pointer("/actions/bridge/enabled").and_then(serde_json::Value::as_bool)
            == Some(true)
            && bridge.get("claimable").and_then(serde_json::Value::as_bool) == Some(true);
    let claim_pending = desktop.activity_action_is_pending("qixi-bridge");
    let mut panel = super::shared::panel(cx).child(
        h_flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::i18n::text(language, "鹊桥进度", "Bridge progress")),
                    )
                    .child(
                        div().font_medium().child(
                            crate::i18n::text(
                                language,
                                &format!("当前第 {current} 阶段"),
                                &format!("Stage {current}"),
                            )
                            .to_string(),
                        ),
                    ),
            )
            .child(
                Button::new("qixi-bridge-claim")
                    .label(if claim_pending {
                        crate::i18n::text(language, "领取中…", "Claiming…")
                    } else if can_claim {
                        crate::i18n::text(language, "领取鹊桥奖励", "Claim bridge reward")
                    } else {
                        crate::i18n::text(language, "暂无奖励", "No reward yet")
                    })
                    .primary()
                    .loading(claim_pending)
                    .disabled(claim_pending || desktop.selected_account.is_none() || !can_claim)
                    .on_click(
                        cx.listener(|this, _, _, cx| this.run_activity_action("qixi-bridge", cx)),
                    ),
            ),
    );

    if let Some(stages) = stages {
        let mut progress = h_flex().gap_1();
        for stage in stages {
            let complete = stage.get("completed").and_then(serde_json::Value::as_bool)
                == Some(true)
                || stage.get("claimed").and_then(serde_json::Value::as_bool) == Some(true);
            let current_stage =
                stage.get("current").and_then(serde_json::Value::as_bool) == Some(true);
            progress = progress.child(div().h(px(6.)).flex_1().rounded(px(3.)).bg(if complete {
                cx.theme().primary
            } else if current_stage {
                cx.theme().primary.opacity(0.45)
            } else {
                cx.theme().border
            }));
        }
        panel = panel.child(progress);
        if stages.is_empty() {
            panel = panel.child(
                div()
                    .py_6()
                    .text_center()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::i18n::text(language, "暂无鹊桥阶段", "No bridge stages yet")),
            );
        } else {
            let mut cards = div().grid().grid_cols(3).gap_3();
            for (index, stage) in stages.iter().enumerate() {
                let stage_number = number(stage.get("stage")).unwrap_or_default();
                let current =
                    stage.get("current").and_then(serde_json::Value::as_bool) == Some(true);
                let status = stage_status(stage, language);
                let claimable =
                    stage.get("claimable").and_then(serde_json::Value::as_bool) == Some(true);
                let (status_bg, status_fg) = if claimable {
                    (cx.theme().danger.opacity(0.12), cx.theme().danger)
                } else if current {
                    (cx.theme().warning.opacity(0.14), cx.theme().warning)
                } else {
                    (cx.theme().group_box, cx.theme().muted_foreground)
                };
                let cost = stage.get("cost").unwrap_or(&serde_json::Value::Null);
                let mut rewards = div().v_flex().gap_2();
                for reward in
                    stage.get("rewards").and_then(serde_json::Value::as_array).into_iter().flatten()
                {
                    let count = super::shared::display_count(reward.get("count"));
                    let name = item_label(reward, "", "", language);
                    rewards = rewards.child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(item_image(reward, 28., language, cx))
                            .child(div().flex_1().min_w_0().text_sm().child(name))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("×{count}")),
                            ),
                    );
                }
                let cost_count = super::shared::display_count(cost.get("count"));
                cards = cards.child(
                    div()
                        .v_flex()
                        .gap_3()
                        .rounded(cx.theme().radius_lg)
                        .border_1()
                        .border_color(if current { cx.theme().primary } else { cx.theme().border })
                        .bg(cx.theme().background)
                        .p_3()
                        .child(
                            h_flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div().text_sm().font_medium().child(
                                        crate::i18n::text(
                                            language,
                                            &format!("第 {stage_number} 阶段"),
                                            &format!("Stage {stage_number}"),
                                        )
                                        .to_string(),
                                    ),
                                )
                                .child({
                                    let status_element = div()
                                        .id(("qixi-stage-status", index))
                                        .aria_label(status)
                                        .rounded(cx.theme().radius)
                                        .bg(status_bg)
                                        .px_2()
                                        .py_1()
                                        .text_xs()
                                        .text_color(status_fg)
                                        .child(status);
                                    #[cfg(test)]
                                    let status_element = status_element.test_support();
                                    status_element
                                }),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(item_image(cost, 24., language, cx))
                                .child(
                                    div()
                                        .flex_1()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(
                                            crate::i18n::text(
                                                language,
                                                &format!(
                                                    "需要 {}",
                                                    item_label(
                                                        cost,
                                                        "鹊羽",
                                                        "Magpie feather",
                                                        language
                                                    )
                                                ),
                                                &format!(
                                                    "Requires {}",
                                                    item_label(
                                                        cost,
                                                        "鹊羽",
                                                        "Magpie feather",
                                                        language
                                                    )
                                                ),
                                            )
                                            .to_string(),
                                        ),
                                )
                                .child(div().text_sm().font_medium().child(cost_count)),
                        )
                        .child(rewards),
                );
            }
            panel = panel.child(cards);
        }
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::stage_status;
    use serde_json::json;

    #[test]
    fn qixi_stage_labels_follow_vue_status_precedence() {
        assert_eq!(
            stage_status(&json!({"claimable": true, "current": true}), "en-US"),
            "Claimable"
        );
        assert_eq!(stage_status(&json!({"claimed": true, "current": true}), "en-US"), "Done");
        assert_eq!(
            stage_status(&json!({"current": true, "completed": true}), "en-US"),
            "In progress"
        );
        assert_eq!(stage_status(&json!({"claimable": true}), "zh-CN"), "待领取");
        assert_eq!(stage_status(&json!({"completed": true}), "zh-CN"), "已完成");
        assert_eq!(stage_status(&json!({}), "zh-CN"), "未达成");
    }
}
