use super::super::super::*;
use super::super::rewards::reward_chip;

fn ordered_groups(raw: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut groups = raw
        .pointer("/constellation/groups")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    groups.sort_by_key(|group| {
        group
            .get("order")
            .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
            .unwrap_or(999)
    });
    groups
}

fn state_label(group: &serde_json::Value, language: &str) -> String {
    match group.get("visualState").and_then(serde_json::Value::as_str).unwrap_or_default() {
        "lit"
            if group.get("claimStatus").and_then(serde_json::Value::as_str)
                == Some("confirmed-no-claimable") =>
        {
            crate::i18n::text(language, "今日已领取", "Claimed today").into()
        }
        "lit" => crate::i18n::text(language, "已点亮", "Lit").into(),
        "locked" => crate::i18n::text(language, "未开启", "Locked").into(),
        "lightable" | "claimableUnknown" => {
            crate::i18n::text(language, "可点亮", "Lightable").into()
        }
        state => state.to_string(),
    }
}

impl FarmDesktop {
    pub(in crate::desktop) fn render_constellation_panel(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let constellation = self.page_raw.get("constellation").unwrap_or(&serde_json::Value::Null);
        let groups = ordered_groups(&self.page_raw);
        let selected_id = self.activities.constellation_group_id.clone();
        let selected = super::selected_constellation(&self.page_raw, &selected_id).cloned();
        let has_rules = constellation
            .get("rules")
            .is_some_and(|rules| !rules.is_null() && rules != &serde_json::json!([]));

        let details = if let Some(group) = selected {
            let id = group.get("id").map(value_to_action_string).unwrap_or_default();
            let name = group
                .get("name")
                .or_else(|| group.get("id"))
                .map(value_to_action_string)
                .unwrap_or_else(|| {
                    crate::i18n::text(language, "星座详情", "Constellation details").into()
                });
            let rewards = group
                .get("rewards")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let can_light = super::selected_constellation_is_lightable(
                &self.page_raw,
                &self.activities.constellation_group_id,
            );
            let mut card = div()
                .v_flex()
                .min_w_0()
                .flex_1()
                .gap_3()
                .rounded(cx.theme().radius_lg)
                .border_1()
                .border_color(cx.theme().border)
                .p_4()
                .child(div().text_lg().font_medium().child(name))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(state_label(&group, language)),
                );
            let reward_content = if rewards.is_empty() {
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::i18n::text(language, "暂无奖励", "No rewards"))
                    .into_any_element()
            } else {
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .children(rewards.iter().map(|reward| reward_chip(reward, cx)))
                    .into_any_element()
            };
            card = card.child(reward_content).child(
                Button::new("constellation-light-selected")
                    .label(crate::i18n::text(language, "点亮观星", "Light Constellation"))
                    .primary()
                    .loading(self.activity_action_is_pending("constellation"))
                    .disabled(
                        self.loading
                            || self.selected_account.is_none()
                            || !can_light
                            || id.is_empty(),
                    )
                    .on_click(
                        cx.listener(|this, _, _, cx| this.run_activity_action("constellation", cx)),
                    ),
            );
            card
        } else {
            div()
                .flex_1()
                .rounded(cx.theme().radius_lg)
                .border_1()
                .border_color(cx.theme().border)
                .p_4()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(crate::i18n::text(
                    language,
                    "当前没有可选择的星座",
                    "No constellation available to select",
                ))
        };

        let group_buttons = groups
            .iter()
            .enumerate()
            .map(|(index, group)| {
                let id = group.get("id").map(value_to_action_string).unwrap_or_default();
                let selected_group = id == selected_id;
                let label = group
                    .get("name")
                    .or_else(|| group.get("id"))
                    .map(value_to_action_string)
                    .unwrap_or_else(|| {
                        format!(
                            "{} {}",
                            crate::i18n::text(language, "星座", "Constellation"),
                            index + 1
                        )
                    });
                Button::new(format!("constellation-group-{id}"))
                    .label(label)
                    .when(selected_group, |button| button.primary())
                    .when(!selected_group, |button| button.ghost())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.activities.constellation_group_id = id.clone();
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect::<Vec<_>>();

        div()
            .v_flex()
            .gap_3()
            .when(has_rules, |panel| {
                panel.child(
                    h_flex().justify_end().child(
                        Button::new("constellation-rules-toggle")
                            .label(crate::i18n::text(language, "活动说明", "Event info"))
                            .ghost()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_activity_rules_dialog(window, cx);
                            })),
                    ),
                )
            })
            .when(groups.is_empty(), |panel| {
                panel.child(
                    div()
                        .py_10()
                        .text_center()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(
                            language,
                            "当前没有可显示的星座",
                            "No constellations to display",
                        )),
                )
            })
            .when(!groups.is_empty(), |panel| {
                panel.child(div().v_flex().gap_2().children(group_buttons).child(details))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::state_label;
    use serde_json::json;

    #[test]
    fn constellation_state_labels_follow_the_vue_locale() {
        assert_eq!(state_label(&json!({"visualState": "locked"}), "zh-CN"), "未开启");
        assert_eq!(state_label(&json!({"visualState": "locked"}), "en-US"), "Locked");
        assert_eq!(state_label(&json!({"visualState": "lightable"}), "zh-CN"), "可点亮");
        assert_eq!(state_label(&json!({"visualState": "lightable"}), "en-US"), "Lightable");
        assert_eq!(state_label(&json!({"visualState": "lit"}), "zh-CN"), "已点亮");
        assert_eq!(
            state_label(
                &json!({"visualState": "lit", "claimStatus": "confirmed-no-claimable"}),
                "en-US"
            ),
            "Claimed today"
        );
    }
}
