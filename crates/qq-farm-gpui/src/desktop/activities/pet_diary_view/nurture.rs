use super::super::super::*;
use serde_json::Value;

use super::common::{is_true, items_line, number, section_card};

pub(super) fn render(this: &FarmDesktop, snapshot: &Value, cx: &mut Context<FarmDesktop>) -> Div {
    let language = &this.preferences.language;
    let english = language == "en-US";
    let nurture = snapshot.get("nurture").unwrap_or(&Value::Null);
    let initialized = is_true(nurture.get("initialized"));
    let adult = is_true(nurture.get("adult"));
    let dog_granted = is_true(nurture.get("dogGranted"));
    let growth = number(nurture.get("growth"), 0);
    let adult_growth = number(nurture.get("adultGrowth"), 0);
    let feed_count = number(nurture.get("feedCount"), 0);
    let feed_limit = number(nurture.get("feedLimit"), 16);

    let mut panel = section_card(this, "萌宠养成", "nurture", cx);
    if !this.activities.pet_diary_expanded_sections.contains("nurture") {
        return panel;
    }
    if !initialized {
        panel = panel.child(
            Button::new("pet-diary-initialize")
                .label(crate::i18n::text(language, "领养比熊幼崽", "Adopt a bichon puppy"))
                .primary()
                .loading(this.pet_diary_action_is_pending("initialize"))
                .disabled(this.loading)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.run_pet_diary_action("initialize", serde_json::json!({}), cx)
                })),
        );
        return panel;
    }

    let ratio =
        if adult_growth > 0 { (growth as f32 / adult_growth as f32).clamp(0., 1.) } else { 1. };
    panel = panel
        .child(
            h_flex()
                .items_center()
                .flex_wrap()
                .gap_3()
                .child(
                    div().h(px(10.)).w(px(260.)).rounded_full().bg(cx.theme().group_box).child(
                        div().h_full().w(relative(ratio)).rounded_full().bg(cx.theme().primary),
                    ),
                )
                .child(if english {
                    format!(
                        "Growth {growth}/{adult_growth} ({})",
                        if adult { "Adult" } else { "Puppy" }
                    )
                } else {
                    format!(
                        "成长 {growth}/{adult_growth}（{}）",
                        if adult { "已成年" } else { "幼年期" }
                    )
                }),
        )
        .child(
            h_flex()
                .items_center()
                .flex_wrap()
                .gap_3()
                .child(
                    Button::new("pet-diary-feed")
                        .label(if english {
                            format!("Feed vitality cakes ({feed_count}/{feed_limit})")
                        } else {
                            format!("投喂元气糕（{feed_count}/{feed_limit}）")
                        })
                        .primary()
                        .loading(this.pet_diary_action_is_pending("feed"))
                        .disabled(this.loading || !is_true(nurture.get("canFeed")))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_pet_diary_action("feed", serde_json::json!({}), cx)
                        })),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(language, "消耗", "Cost")),
                )
                .child(items_line(nurture.get("feedCosts"), language, cx)),
        );
    if adult && !dog_granted {
        panel = panel.child(
            Button::new("pet-diary-claim-dog")
                .label(crate::i18n::text(language, "领取永久比熊", "Claim permanent bichon"))
                .primary()
                .loading(this.pet_diary_action_is_pending("claimDog"))
                .disabled(this.loading)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.run_pet_diary_action("claimDog", serde_json::json!({}), cx)
                })),
        );
    } else if dog_granted {
        panel = panel.child(div().text_sm().text_color(cx.theme().success).child(
            crate::i18n::text(language, "永久比熊已入队", "Permanent bichon added to your team"),
        ));
    }
    panel
}
