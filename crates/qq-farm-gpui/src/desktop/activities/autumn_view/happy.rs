use super::super::super::*;

pub(super) fn render(
    desktop: &FarmDesktop,
    activity: &serde_json::Value,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let language = &desktop.preferences.language;
    let english = language == "en-US";
    let mut panel = div().v_flex().gap_4();
    let score = super::shared::number(activity.get("score")).unwrap_or_default();
    let claimed = super::shared::number(activity.get("claimedCount")).unwrap_or_default();
    let claim_limit = super::shared::number(activity.get("claimLimit")).unwrap_or_default();
    let pool_claimed = super::shared::number(activity.get("poolClaimedCount")).unwrap_or_default();
    let pool_limit = super::shared::number(activity.get("poolClaimLimit")).unwrap_or_default();
    let score_label = if english {
        format!("Current happiness {score}")
    } else {
        format!("当前快乐值 {score}")
    };
    let claims_label = if english {
        format!("Today's claims {claimed}/{claim_limit}")
    } else {
        format!("今日领取 {claimed}/{claim_limit}")
    };
    let pool_label = if english {
        format!("Community pool {pool_claimed}/{pool_limit}")
    } else {
        format!("池子 {pool_claimed}/{pool_limit}")
    };
    let mut summary = h_flex()
        .flex_wrap()
        .gap_2()
        .child(super::shared::status_chip_tinted(
            score_label,
            cx.theme().primary,
            cx.theme().primary.opacity(0.12),
            cx,
        ))
        .child(super::shared::status_chip(claims_label, cx))
        .child(super::shared::status_chip(pool_label, cx));
    if activity.get("firstShareAwarded").and_then(serde_json::Value::as_bool) == Some(true) {
        summary = summary.child(super::shared::status_chip_tinted(
            crate::i18n::text(language, "首次分享奖励已领", "First-share reward claimed"),
            cx.theme().success,
            cx.theme().success.opacity(0.16),
            cx,
        ));
    }
    panel = panel.child(summary);

    let share_reward = super::shared::number(activity.get("firstShareReward")).unwrap_or_default();
    let share_label =
        if activity.get("firstShareAwarded").and_then(serde_json::Value::as_bool) == Some(true) {
            crate::i18n::text(language, "分享（+快乐值）", "Share (+happiness)").to_owned()
        } else if english {
            format!("Share for first reward +{share_reward}")
        } else {
            format!("分享领首次奖励 +{share_reward}")
        };
    panel = panel.child(
        h_flex()
            .flex_wrap()
            .gap_2()
            .child(action_button(
                desktop,
                cx,
                "autumn-happy-daily",
                crate::i18n::text(language, "领取每日快乐值", "Claim daily happiness"),
                activity.get("canClaimDaily").and_then(serde_json::Value::as_bool) == Some(true),
                true,
            ))
            .child(action_button(
                desktop,
                cx,
                "autumn-happy-share",
                &share_label,
                activity.get("canShare").and_then(serde_json::Value::as_bool) == Some(true),
                false,
            ))
            .child(action_button(
                desktop,
                cx,
                "autumn-happy-milestones",
                crate::i18n::text(language, "领取全部档位奖励", "Claim all milestone rewards"),
                activity.get("canClaimMilestones").and_then(serde_json::Value::as_bool)
                    == Some(true),
                false,
            )),
    );

    let mut milestones = h_flex().flex_wrap().gap_2();
    let mut has_milestones = false;
    for (index, milestone) in activity
        .get("milestones")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        has_milestones = true;
        let threshold = super::shared::number(milestone.get("threshold")).unwrap_or_default();
        let state = super::shared::number(milestone.get("state")).unwrap_or_default();
        let status = match state {
            2 => crate::i18n::text(language, "可领取", "Claimable"),
            3 => crate::i18n::text(language, "已领取", "Claimed"),
            _ => crate::i18n::text(language, "未达成", "Not reached"),
        };
        let (status_fg, status_bg) = match state {
            2 => (cx.theme().success, cx.theme().success.opacity(0.16)),
            3 => (cx.theme().primary, cx.theme().primary.opacity(0.12)),
            _ => (cx.theme().muted_foreground, cx.theme().background),
        };
        let mut rewards = h_flex().flex_wrap().gap_2();
        for reward in
            milestone.get("rewards").and_then(serde_json::Value::as_array).into_iter().flatten()
        {
            let count = super::shared::number(reward.get("count")).unwrap_or_default();
            rewards = rewards.child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .child(super::shared::item_image(reward, 28., cx))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("×{count}")),
                    ),
            );
        }
        let milestone_status = div()
            .id(("autumn-milestone-status", index))
            .aria_label(status)
            .rounded(cx.theme().radius)
            .bg(status_bg)
            .px_2()
            .py_1()
            .text_xs()
            .text_color(status_fg)
            .child(status);
        #[cfg(test)]
        let milestone_status = milestone_status.test_support();
        milestones = milestones.child(
            super::shared::surface(cx)
                .when(state == 2, |card| card.bg(cx.theme().success.opacity(0.08)))
                .child(div().text_sm().font_medium().child(if english {
                    format!("{threshold} happiness")
                } else {
                    format!("{threshold} 快乐值")
                }))
                .child(rewards)
                .child(milestone_status),
        );
    }
    if has_milestones {
        panel = panel
            .child(div().text_sm().font_medium().child(crate::i18n::text(
                language,
                "快乐值档位",
                "Happiness milestones",
            )))
            .child(milestones);
    }

    let mine_action = "autumn-happy-logs-mine";
    let others_action = "autumn-happy-logs-others";
    panel = panel.child(
        h_flex()
            .gap_2()
            .child(log_tab_button(
                desktop,
                cx,
                0,
                crate::i18n::text(language, "领取我的", "Available to claim"),
                mine_action,
            ))
            .child(log_tab_button(
                desktop,
                cx,
                1,
                crate::i18n::text(language, "我领取的", "Claimed by me"),
                others_action,
            )),
    );
    let logs = activity
        .pointer("/logs/logs")
        .and_then(serde_json::Value::as_array)
        .or_else(|| activity.get("logs").and_then(serde_json::Value::as_array));
    let logs_pending = desktop.activity_action_is_pending("autumn-happy-logs-mine")
        || desktop.activity_action_is_pending("autumn-happy-logs-others");
    if desktop.loading || logs_pending {
        panel = panel.child(div().text_sm().text_color(cx.theme().muted_foreground).child(
            crate::i18n::text(language, "正在加载互动记录…", "Loading interaction history…"),
        ));
    } else if let Some(logs) = logs.filter(|logs| !logs.is_empty()) {
        let mut rows = div().v_flex().gap_1();
        for (index, row) in logs.iter().enumerate() {
            let seq = super::shared::number(row.get("seq")).unwrap_or(index as i64);
            let actor = row
                .pointer("/actor/name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    if english {
                        format!("Player #{seq}")
                    } else {
                        format!("玩家#{seq}")
                    }
                });
            let score = super::shared::number(row.get("score")).unwrap_or_default();
            let timestamp = super::shared::number(row.get("createdAt")).unwrap_or_default();
            rows = rows.child(
                h_flex()
                    .justify_between()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().background)
                    .px_3()
                    .py_2()
                    .text_xs()
                    .child(actor)
                    .child(div().text_color(cx.theme().muted_foreground).child(if english {
                        format!(
                            "Happiness {}{} · {}",
                            if score > 0 { "+" } else { "" },
                            score,
                            super::shared::format_time(Some(&serde_json::json!(timestamp * 1000)))
                        )
                    } else {
                        format!(
                            "快乐值 {}{} · {}",
                            if score > 0 { "+" } else { "" },
                            score,
                            super::shared::format_time(Some(&serde_json::json!(timestamp * 1000)))
                        )
                    })),
            );
        }
        panel = panel.child(rows);
    } else {
        panel = panel.child(
            div()
                .py_4()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(crate::i18n::text(language, "暂无互动记录", "No interaction history")),
        );
    }
    panel
}

fn action_button(
    desktop: &FarmDesktop,
    cx: &mut Context<FarmDesktop>,
    action: &'static str,
    label: &str,
    enabled: bool,
    primary: bool,
) -> Button {
    Button::new(action.to_string())
        .label(label.to_string())
        .when(primary, |button| button.primary())
        .when(!primary, |button| button.ghost())
        .loading(desktop.activity_action_is_pending(action))
        .disabled(desktop.loading || desktop.selected_account.is_none() || !enabled)
        .on_click(cx.listener(move |this, _, _, cx| this.run_activity_action(action, cx)))
}

fn log_tab_button(
    desktop: &FarmDesktop,
    cx: &mut Context<FarmDesktop>,
    tab: i64,
    label: &str,
    action: &'static str,
) -> Button {
    Button::new(format!("autumn-log-tab-{tab}"))
        .label(label)
        .when(desktop.activities.autumn_log_tab == tab, |button| button.primary())
        .when(desktop.activities.autumn_log_tab != tab, |button| button.ghost())
        .loading(desktop.activity_action_is_pending(action))
        .disabled(desktop.loading || desktop.selected_account.is_none())
        .on_click(cx.listener(move |this, _, _, cx| {
            this.activities.autumn_log_tab = tab;
            this.run_activity_action(action, cx);
        }))
}
