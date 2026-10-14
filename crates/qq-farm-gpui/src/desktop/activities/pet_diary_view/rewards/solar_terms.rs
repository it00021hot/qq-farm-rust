use super::super::super::super::*;
use serde_json::Value;

use super::super::common::{is_true, items_line, section_card, text};

pub(super) fn render(this: &FarmDesktop, snapshot: &Value, cx: &mut Context<FarmDesktop>) -> Div {
    let language = &this.preferences.language;
    let terms = snapshot
        .pointer("/solarTerms/terms")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if terms.is_empty() {
        return div();
    }
    let mut panel = section_card(this, "节令小礼", "solar", cx);
    if !this.activities.pet_diary_expanded_sections.contains("solar") {
        return panel;
    }
    panel = panel.child(h_flex().flex_wrap().gap_2().children(terms.iter().enumerate().map(
        |(index, term)| {
            let id = text(term.get("id"), "");
            let name = text(
                term.get("name"),
                &if language == "en-US" {
                    format!("Solar term {id}")
                } else {
                    format!("节令 {id}")
                },
            );
            let can_claim = is_true(term.get("canClaim"));
            let claimed = is_true(term.get("claimed"));
            h_flex()
                .items_center()
                .flex_wrap()
                .gap_2()
                .rounded(cx.theme().radius)
                .bg(cx.theme().group_box)
                .p_3()
                .child(div().text_sm().child(name))
                .child(items_line(term.get("rewards"), language, cx))
                .when(can_claim, |row| {
                    row.child(
                        Button::new(("pet-diary-solar-claim", index))
                            .label(crate::i18n::text(language, "领取", "Claim"))
                            .primary()
                            .loading(this.pet_diary_action_is_pending("solar"))
                            .disabled(this.loading)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_pet_diary_action(
                                    "solar",
                                    serde_json::json!({"termId": id}),
                                    cx,
                                )
                            })),
                    )
                })
                .when(!can_claim && claimed, |row| {
                    row.child(
                        div().text_xs().text_color(cx.theme().success).child(crate::i18n::text(
                            language,
                            "已领取",
                            "Claimed",
                        )),
                    )
                })
        },
    )));
    panel
}
