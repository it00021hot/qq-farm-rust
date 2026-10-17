//! 快乐不独享页渲染（bot `ShareRewardActivity.vue` 的 GPUI 紧凑移植）。

use super::super::*;

fn js_i64(value: Option<&serde_json::Value>) -> i64 {
    match value {
        Some(serde_json::Value::Number(number)) => number.as_i64().unwrap_or_default(),
        Some(serde_json::Value::String(text)) => text.trim().parse().unwrap_or_default(),
        _ => 0,
    }
}

fn rewards_text(rewards: Option<&serde_json::Value>) -> String {
    rewards
        .and_then(serde_json::Value::as_array)
        .map(|rewards| {
            rewards
                .iter()
                .map(|reward| {
                    format!(
                        "{} ×{}",
                        reward.get("name").and_then(serde_json::Value::as_str).unwrap_or(""),
                        js_i64(reward.get("count"))
                    )
                })
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default()
}

impl FarmDesktop {
    pub(in crate::desktop) fn render_share_reward(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let mut panel = div().v_flex().flex_1().min_h_0().gap_3();
        panel = panel.child(
            h_flex()
                .justify_between()
                .items_center()
                .child(div().text_lg().font_semibold().child(crate::i18n::text(
                    language,
                    "快乐不独享",
                    "Share & Reward",
                )))
                .child(
                    Button::new("share-reward-refresh")
                        .label(crate::i18n::text(language, "刷新", "Refresh"))
                        .outline()
                        .loading(self.share_reward.pending)
                        .disabled(self.selected_account.is_none() || self.share_reward.pending)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.share_reward_request(None, serde_json::json!({}), cx)
                        })),
                ),
        );
        if self.selected_account.is_none() {
            return panel.child(share_reward_notice(
                cx,
                crate::i18n::text(language, "请先选择账号", "Select an account first"),
            ));
        }
        if let Some(error) = &self.share_reward.error {
            return panel.child(share_reward_notice(cx, error));
        }
        let raw = &self.page_raw;
        if raw.is_null() {
            return panel.child(share_reward_notice(
                cx,
                crate::i18n::text(language, "暂无活动进度", "No activity data"),
            ));
        }
        let active = raw.get("active").and_then(serde_json::Value::as_bool) == Some(true);
        let can_share = raw.get("canShare").and_then(serde_json::Value::as_bool) == Some(true);
        let can_daily = raw.get("canClaimDaily").and_then(serde_json::Value::as_bool) == Some(true);
        let can_milestones =
            raw.get("canClaimMilestones").and_then(serde_json::Value::as_bool) == Some(true);
        let mut milestones = div().v_flex().gap_2();
        for (index, milestone) in raw
            .get("milestones")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let state = js_i64(milestone.get("state"));
            let threshold = js_i64(milestone.get("threshold"));
            let row = h_flex()
                .id(("share-reward-milestone", index))
                .aria_label(threshold.to_string())
                .items_center()
                .gap_2()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(if state == 2 { cx.theme().primary } else { cx.theme().border })
                .p_2()
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .child(js_i64(milestone.get("threshold")).to_string()),
                )
                .child(div().text_xs().child(rewards_text(milestone.get("rewards"))))
                .child(div().ml_auto().text_xs().child(if state == 2 {
                    crate::i18n::text(language, "可领取", "Claimable")
                } else {
                    crate::i18n::text(language, "未达成", "Locked")
                }));
            #[cfg(test)]
            let row = row.test_support();
            milestones = milestones.child(row);
        }
        let mut logs = div().v_flex().gap_1();
        if self.share_reward.logs.is_empty() {
            logs = logs.child(share_reward_notice(
                cx,
                crate::i18n::text(language, "暂无日志", "No logs yet"),
            ));
        }
        for (index, log) in self.share_reward.logs.iter().enumerate() {
            let name =
                log.pointer("/actor/name").and_then(serde_json::Value::as_str).unwrap_or("—");
            let score = js_i64(log.get("score"));
            logs = logs.child(
                div()
                    .id(("share-reward-log", index))
                    .text_xs()
                    .child(format!("{name} {}{score}", if score >= 0 { "+" } else { "" },)),
            );
        }
        panel
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().group_box)
                    .p_3()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div().font_semibold().child(
                                    raw.get("title")
                                        .and_then(serde_json::Value::as_str)
                                        .unwrap_or("快乐不独享")
                                        .to_string(),
                                ),
                            )
                            .child(div().text_xs().child(format!(
                                "{} {}",
                                crate::i18n::text(language, "快乐值", "Score"),
                                js_i64(raw.get("score"))
                            )))
                            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                                format!(
                                    "{}+{} · {}+{}",
                                    crate::i18n::text(language, "每日", "Daily"),
                                    js_i64(raw.get("dailyReward")),
                                    crate::i18n::text(language, "首次分享", "First share"),
                                    js_i64(raw.get("firstShareReward")),
                                ),
                            ))
                            .child(div().ml_auto().text_xs().child(if active {
                                crate::i18n::text(language, "进行中", "Open")
                            } else {
                                crate::i18n::text(language, "未开放或已结束", "Closed or ended")
                            })),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("share-reward-share")
                                    .label(crate::i18n::text(language, "分享", "Share"))
                                    .primary()
                                    .loading(self.share_reward.pending)
                                    .disabled(!can_share || self.share_reward.pending)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.share_reward_request(
                                            Some("share"),
                                            serde_json::json!({}),
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new("share-reward-daily")
                                    .label(crate::i18n::text(
                                        language,
                                        "领取每日奖励",
                                        "Claim daily",
                                    ))
                                    .secondary()
                                    .loading(self.share_reward.pending)
                                    .disabled(!can_daily || self.share_reward.pending)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.share_reward_request(
                                            Some("daily"),
                                            serde_json::json!({}),
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new("share-reward-milestones")
                                    .label(crate::i18n::text(
                                        language,
                                        "领取档位奖励",
                                        "Claim milestones",
                                    ))
                                    .secondary()
                                    .loading(self.share_reward.pending)
                                    .disabled(!can_milestones || self.share_reward.pending)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.share_reward_request(
                                            Some("milestones"),
                                            serde_json::json!({}),
                                            cx,
                                        )
                                    })),
                            ),
                    ),
            )
            .child(milestones)
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .p_3()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_sm().font_medium().child(crate::i18n::text(
                                language,
                                "互动日志",
                                "Logs",
                            )))
                            .child(
                                Button::new("share-reward-logs-host")
                                    .label(crate::i18n::text(language, "我收到的", "Received"))
                                    .when(self.share_reward.log_tab == 1, |button| button.primary())
                                    .when(self.share_reward.log_tab != 1, |button| {
                                        button.secondary()
                                    })
                                    .loading(self.share_reward.pending)
                                    .disabled(self.share_reward.pending)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.share_reward.log_tab = 1;
                                        this.share_reward_request(
                                            Some("logs"),
                                            serde_json::json!({ "tab": 1 }),
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new("share-reward-logs-guest")
                                    .label(crate::i18n::text(language, "我送出的", "Sent"))
                                    .when(self.share_reward.log_tab == 0, |button| button.primary())
                                    .when(self.share_reward.log_tab != 0, |button| {
                                        button.secondary()
                                    })
                                    .loading(self.share_reward.pending)
                                    .disabled(self.share_reward.pending)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.share_reward.log_tab = 0;
                                        this.share_reward_request(
                                            Some("logs"),
                                            serde_json::json!({ "tab": 0 }),
                                            cx,
                                        )
                                    })),
                            ),
                    )
                    .child(logs),
            )
    }
}

fn share_reward_notice(cx: &Context<FarmDesktop>, message: &str) -> Div {
    div().py_8().text_center().text_color(cx.theme().muted_foreground).child(message.to_string())
}
