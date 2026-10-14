mod donation;
mod presentation;
mod rewards;
mod summary;

use super::super::*;

fn rule_lines(charity: &serde_json::Value) -> Vec<String> {
    fn strings(value: Option<&serde_json::Value>) -> Vec<String> {
        value
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item.as_str().map(str::trim))
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect()
    }

    let rules = charity.get("rules").unwrap_or(&serde_json::Value::Null);
    let mut lines = strings(rules.get("paragraphs"));
    lines.extend(strings(rules.pointer("/tips/txt")));
    lines.extend(strings(rules.get("lines")));
    lines
}

fn rule_title(charity: &serde_json::Value, language: &str) -> String {
    charity
        .pointer("/rules/title")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| crate::i18n::text(language, "活动说明", "Event info").into())
}

fn rules_card(
    this: &FarmDesktop,
    charity: &serde_json::Value,
    cx: &mut Context<FarmDesktop>,
) -> Option<Div> {
    let lines = rule_lines(charity);
    if lines.is_empty() {
        return None;
    }
    let language = &this.preferences.language;
    let title = rule_title(charity, language);
    let expanded = this.activities.charity_rules_expanded;
    let mut card = div()
        .v_flex()
        .gap_3()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .p_4()
        .child(
            h_flex().child(
                Button::new("charity-rules-toggle")
                    .label(format!("{} {title}", if expanded { "⌄" } else { "›" }))
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.activities.charity_rules_expanded =
                            !this.activities.charity_rules_expanded;
                        cx.notify();
                    })),
            ),
        );
    if expanded {
        let body =
            div().id("charity-rules-body").v_flex().gap_2().children(lines.iter().map(|line| {
                div().text_sm().text_color(cx.theme().muted_foreground).child(line.clone())
            }));
        #[cfg(test)]
        let body = body.test_support();
        card = card.child(body);
    }
    Some(card)
}

pub(super) fn charity_donation_enabled(raw: &serde_json::Value) -> bool {
    raw.pointer("/charity/actions/donateLove/enabled").and_then(serde_json::Value::as_bool)
        == Some(true)
}

impl FarmDesktop {
    pub(in crate::desktop) fn render_charity_panel(&self, cx: &mut Context<Self>) -> Div {
        let charity = self.page_raw.get("charity").unwrap_or(&serde_json::Value::Null);
        let language = &self.preferences.language;
        let mut panel = div()
            .v_flex()
            .gap_3()
            .child(summary::metrics(charity, language, cx))
            .child(summary::global_progress(charity, language, cx))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_3()
                    .child(rewards::seed_card(self, charity, cx))
                    .child(donation::card(
                        self,
                        charity,
                        charity_donation_enabled(&self.page_raw),
                        cx,
                    ))
                    .child(rewards::daily_gift_card(self, charity, cx)),
            )
            .child(rewards::progress_rewards(self, charity, cx));
        if let Some(rules) = rules_card(self, charity, cx) {
            panel = panel.child(rules);
        }
        panel
    }
}

#[cfg(test)]
mod tests {
    use super::charity_donation_enabled;
    use serde_json::json;

    #[test]
    fn charity_donation_uses_the_backend_gate() {
        assert!(charity_donation_enabled(&json!({
            "charity": {"actions": {"donateLove": {"enabled": true}}}
        })));
        assert!(!charity_donation_enabled(&json!({
            "charity": {"actions": {"donateLove": {"enabled": false}}}
        })));
        assert!(!charity_donation_enabled(&json!({"charity": {}})));
    }
}
