use super::super::super::super::*;
use serde_json::Value;

use super::super::common::{is_true, item_line, section_card};

pub(super) fn render(this: &FarmDesktop, snapshot: &Value, cx: &mut Context<FarmDesktop>) -> Div {
    let language = &this.preferences.language;
    let english = language == "en-US";
    let seeds = snapshot.get("seeds").unwrap_or(&Value::Null);
    let days = seeds.get("days").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut panel = section_card(this, "种子赠礼", "seeds", cx);
    if !this.activities.pet_diary_expanded_sections.contains("seeds") {
        return panel;
    }
    panel = panel.child(h_flex().flex_wrap().gap_3().children(days.iter().map(|day| {
        let day_number = day.get("day").and_then(Value::as_i64).unwrap_or_default();
        let claimed = is_true(day.get("claimed"));
        let claimable = is_true(day.get("claimable"));
        let rewards = day.get("rewards").and_then(Value::as_array).cloned().unwrap_or_default();
        div()
            .v_flex()
            .w(px(150.))
            .items_center()
            .gap_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .p_3()
            .child(if english {
                format!("Day {day_number}")
            } else {
                format!("第 {day_number} 天")
            })
            .children(rewards.iter().map(|reward| item_line(reward, 22., language, cx)))
            .child(
                div()
                    .text_xs()
                    .text_color(if claimed {
                        cx.theme().success
                    } else if claimable {
                        cx.theme().warning
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(if claimed {
                        crate::i18n::text(language, "已领取", "Claimed")
                    } else if claimable {
                        crate::i18n::text(language, "可领取", "Available")
                    } else {
                        crate::i18n::text(language, "未解锁", "Locked")
                    }),
            )
    })));
    panel.child(
        Button::new("pet-diary-seeds")
            .label(crate::i18n::text(language, "一键领取可领种子", "Claim all available seeds"))
            .primary()
            .loading(this.pet_diary_action_is_pending("seeds"))
            .disabled(this.loading || !is_true(seeds.get("canClaim")))
            .on_click(cx.listener(|this, _, _, cx| {
                this.run_pet_diary_action("seeds", serde_json::json!({}), cx)
            })),
    )
}
