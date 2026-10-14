use super::super::super::*;
use super::super::rewards::reward_chip;

fn text(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .map(value_to_action_string)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn claim_label(term: &serde_json::Value, pending: bool, language: &str) -> &'static str {
    if pending {
        crate::i18n::text(language, "领取中…", "Claiming…")
    } else if term.get("claimed").and_then(serde_json::Value::as_bool) == Some(true) {
        crate::i18n::text(language, "已领取", "Claimed")
    } else if term.get("locked").and_then(serde_json::Value::as_bool) == Some(true) {
        crate::i18n::text(language, "未开始", "Not started")
    } else if super::solar_term_claimable(term) {
        crate::i18n::text(language, "领取", "Claim")
    } else {
        crate::i18n::text(language, "暂不可领取", "Unavailable")
    }
}

impl FarmDesktop {
    pub(in crate::desktop) fn render_solar_terms_panel(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let solar = self.page_raw.get("solarTerms").unwrap_or(&serde_json::Value::Null);
        let terms =
            solar.get("terms").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
        if terms.is_empty() {
            return div()
                .v_flex()
                .items_center()
                .gap_2()
                .py_12()
                .text_color(cx.theme().muted_foreground)
                .child(crate::i18n::text(
                    language,
                    "当前没有可显示的节气",
                    "No solar terms to display",
                ));
        }

        let selected_id = self.activities.solar_term_id.clone();
        let selected = super::selected_solar_term(&self.page_raw, &selected_id).cloned();
        let mut term_buttons = Vec::with_capacity(terms.len());
        for (index, term) in terms.iter().enumerate() {
            let id = term.get("id").map(value_to_action_string).unwrap_or_default();
            let selected_term = id == selected_id;
            let label = text(term.get("name").or_else(|| term.get("id"))).unwrap_or_else(|| {
                format!("{} {}", crate::i18n::text(language, "节气", "Solar term"), index + 1)
            });
            let claimable = super::solar_term_claimable(term);
            term_buttons.push(
                Button::new(format!("solar-term-select-{id}"))
                    .label(if claimable { format!("{label}  •") } else { label })
                    .when(selected_term, |button| button.primary())
                    .when(!selected_term, |button| button.ghost())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.activities.solar_term_id = id.clone();
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }

        let detail = if let Some(term) = selected {
            let english = text(term.get("englishName"));
            let title = text(term.get("title").or_else(|| term.get("name"))).unwrap_or_else(|| {
                crate::i18n::text(language, "节气详情", "Solar term details").into()
            });
            let description =
                text(term.get("description")).or_else(|| text(solar.get("description")));
            let reward_title = text(term.get("rewardTitle"));
            let reward_description = text(term.get("rewardDescription"));
            let rewards = term
                .get("rewards")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let claimable = super::solar_term_claimable(&term)
                && activity_action_enabled(&self.page_raw, "claimSolar") != Some(false);
            let disabled = self.loading || self.selected_account.is_none() || !claimable;
            let id = term.get("id").map(value_to_action_string).unwrap_or_default();
            let mut card = div()
                .v_flex()
                .min_w_0()
                .flex_1()
                .gap_3()
                .rounded(cx.theme().radius_lg)
                .border_1()
                .border_color(cx.theme().border)
                .p_4();
            if let Some(english) = english {
                card = card
                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child(english));
            }
            card = card.child(div().text_2xl().font_semibold().child(title));
            if let Some(description) = description {
                card = card.child(
                    div()
                        .whitespace_normal()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(description),
                );
            }
            if let Some(reward_title) = reward_title {
                card = card.child(div().font_medium().child(reward_title));
            }
            if let Some(reward_description) = reward_description {
                card = card.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(reward_description),
                );
            }
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
                Button::new("solar-claim-selected")
                    .label(claim_label(
                        &term,
                        self.loading || self.activity_target_is_pending("solar-term", &id),
                        language,
                    ))
                    .primary()
                    .loading(self.activity_target_is_pending("solar-term", &id))
                    .disabled(disabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.run_activity_with_id("solar-term", id.clone(), cx)
                    })),
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
                    "选择一个节气查看详情",
                    "Select a solar term to view its details",
                ))
        };

        div().v_flex().gap_4().child(
            h_flex()
                .items_start()
                .gap_4()
                .child(
                    div()
                        .v_flex()
                        .w(px(140.))
                        .max_h(px(520.))
                        .gap_2()
                        .overflow_y_scrollbar()
                        .children(term_buttons),
                )
                .child(detail),
        )
    }
}
