use super::super::super::*;
use super::presentation::{reward_line, reward_line_plain};

fn milestone_target_label(target: &str, reached: bool, language: &str) -> String {
    if reached {
        return crate::i18n::text(language, "已达成", "Reached").to_owned();
    }
    format!(
        "{} {}",
        if target.is_empty() { "0" } else { target },
        crate::i18n::text(language, "爱心", "love")
    )
}

fn progress_claim_pending(this: &FarmDesktop) -> bool {
    this.activities.pending_actions.keys().any(|key| key.starts_with("charity-progress:"))
}

pub(super) fn seed_card(
    this: &FarmDesktop,
    charity: &serde_json::Value,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let enabled =
        charity.pointer("/actions/claimSeeds/enabled").and_then(serde_json::Value::as_bool)
            == Some(true);
    let claimed =
        charity.pointer("/seedReward/claimed").and_then(serde_json::Value::as_bool) == Some(true);
    let reward = charity.pointer("/seedReward/reward").unwrap_or(&serde_json::Value::Null);
    let pending = this.activity_action_is_pending("charity-seeds");
    let language = &this.preferences.language;
    let label = if pending {
        crate::i18n::text(language, "领取中…", "Claiming…")
    } else if claimed {
        crate::i18n::text(language, "种子已领取", "Seeds claimed")
    } else if enabled {
        crate::i18n::text(language, "领取种子", "Claim seeds")
    } else {
        crate::i18n::text(language, "暂不可领取", "Unavailable")
    };

    div()
        .v_flex()
        .flex_1()
        .min_w(px(220.))
        .gap_3()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .p_4()
        .child(div().font_medium().child(crate::i18n::text(
            language,
            "小红花种子",
            "Red flower seeds",
        )))
        .child(reward_line(
            reward,
            crate::i18n::text(language, "暂无奖励信息", "No reward info"),
            cx,
        ))
        .child(
            Button::new("charity-seeds")
                .label(label)
                .primary()
                .loading(pending)
                .disabled(this.loading || !enabled)
                .on_click(
                    cx.listener(|this, _, _, cx| this.run_activity_action("charity-seeds", cx)),
                ),
        )
}

pub(super) fn daily_gift_card(
    this: &FarmDesktop,
    charity: &serde_json::Value,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let enabled =
        charity.pointer("/actions/claimDailyGift/enabled").and_then(serde_json::Value::as_bool)
            == Some(true);
    let claimed =
        charity.pointer("/dailyGift/claimed").and_then(serde_json::Value::as_bool) == Some(true);
    let reward = charity.pointer("/dailyGift/reward").unwrap_or(&serde_json::Value::Null);
    let pending = this.activity_action_is_pending("charity");
    let language = &this.preferences.language;
    let label = if pending {
        crate::i18n::text(language, "领取中…", "Claiming…")
    } else if claimed {
        crate::i18n::text(language, "今日已领取", "Claimed today")
    } else if enabled {
        crate::i18n::text(language, "领取公益礼包", "Claim gift")
    } else {
        crate::i18n::text(language, "暂不可领取", "Unavailable")
    };
    let mut reward_row = reward_line_plain(
        reward,
        crate::i18n::text(language, "暂无奖励信息", "No reward info"),
        cx,
    );
    if claimed {
        let claimed_label = crate::i18n::text(language, "今日已领取", "Claimed today");
        let tag = div()
            .id("charity-gift-claimed")
            .aria_label(claimed_label)
            .text_xs()
            .text_color(cx.theme().success)
            .child(claimed_label);
        #[cfg(test)]
        let tag = tag.test_support();
        reward_row = reward_row.child(tag);
    }

    div()
        .v_flex()
        .flex_1()
        .min_w(px(220.))
        .gap_3()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .p_4()
        .child(div().font_medium().child(crate::i18n::text(
            language,
            "每日公益礼包",
            "Daily charity gift",
        )))
        .child(reward_row)
        .child(
            Button::new("charity-daily-gift")
                .label(label)
                .primary()
                .loading(pending)
                .disabled(this.loading || !enabled)
                .on_click(cx.listener(|this, _, _, cx| this.run_activity_action("charity", cx))),
        )
}

pub(super) fn progress_rewards(
    this: &FarmDesktop,
    charity: &serde_json::Value,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let language = &this.preferences.language;
    let progress_pending = progress_claim_pending(this);
    let rewards = charity
        .get("progressRewards")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut rows = Vec::with_capacity(rewards.len());
    for (index, reward) in rewards.into_iter().enumerate() {
        let target = reward
            .get("target")
            .or_else(|| reward.get("targetId"))
            .map(value_to_action_string)
            .unwrap_or_default();
        let can_claim = reward.get("claimSupported").and_then(serde_json::Value::as_bool)
            == Some(true)
            && reward.get("claimable").and_then(serde_json::Value::as_bool) == Some(true);
        let claimed = reward.get("claimed").and_then(serde_json::Value::as_bool) == Some(true);
        let reached = reward.get("reached").and_then(serde_json::Value::as_bool) == Some(true);
        let target_label = milestone_target_label(&target, reached, language);
        let item = reward.get("reward").cloned().unwrap_or(serde_json::Value::Null);
        let mut row = h_flex()
            .items_center()
            .justify_between()
            .gap_3()
            .rounded(cx.theme().radius)
            .bg(cx.theme().group_box)
            .p_3()
            .child(
                h_flex()
                    .items_center()
                    .min_w_0()
                    .flex_1()
                    .gap_3()
                    .child(div().text_sm().child(target_label))
                    .child(reward_line(
                        &item,
                        crate::i18n::text(language, "阶段奖励", "Milestone reward"),
                        cx,
                    )),
            );
        if can_claim {
            row = row.child(
                Button::new(("charity-progress-claim", index))
                    .label(if progress_pending {
                        crate::i18n::text(language, "领取中…", "Claiming…")
                    } else {
                        crate::i18n::text(language, "领取奖励", "Claim reward")
                    })
                    .primary()
                    .loading(progress_pending)
                    .disabled(this.loading || progress_pending)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.run_activity_with_id("charity-progress", target.clone(), cx)
                    })),
            );
        } else if claimed || reached {
            row = row.child(
                div()
                    .id(("charity-progress-state", index))
                    .test_support()
                    .text_xs()
                    .text_color(if claimed {
                        cx.theme().success
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(if claimed {
                        crate::i18n::text(language, "奖励已领取", "Reward claimed")
                    } else {
                        crate::i18n::text(language, "奖励已达成", "Reward unlocked")
                    }),
            );
        }
        rows.push(row.into_any_element());
    }

    if rows.is_empty() {
        rows.push(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(crate::i18n::text(language, "暂无奖励", "No rewards"))
                .into_any_element(),
        );
    }

    div()
        .v_flex()
        .gap_2()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .p_4()
        .child(div().font_medium().child(crate::i18n::text(
            language,
            "个人累计捐赠奖励",
            "Milestone rewards",
        )))
        .children(rows)
}

#[cfg(test)]
mod tests {
    use super::milestone_target_label;

    #[test]
    fn milestone_labels_match_vue_reached_and_target_copy() {
        assert_eq!(milestone_target_label("100", false, "zh-CN"), "100 爱心");
        assert_eq!(milestone_target_label("", false, "zh-CN"), "0 爱心");
        assert_eq!(milestone_target_label("100", true, "zh-CN"), "已达成");
        assert_eq!(milestone_target_label("100", false, "en-US"), "100 love");
        assert_eq!(milestone_target_label("100", true, "en-US"), "Reached");
    }
}
