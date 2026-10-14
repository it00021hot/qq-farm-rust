use super::super::super::super::*;
use serde_json::Value;

use super::super::common::{is_true, picture, section_card, text};

pub(super) fn render(this: &FarmDesktop, snapshot: &Value, cx: &mut Context<FarmDesktop>) -> Div {
    let language = &this.preferences.language;
    let english = language == "en-US";
    let stories = snapshot.get("stories").and_then(Value::as_array).cloned().unwrap_or_default();
    let panel = section_card(this, "成长手记", "story", cx);
    if !this.activities.pet_diary_expanded_sections.contains("story") {
        return panel;
    }
    if stories.is_empty() {
        return panel;
    }
    panel.child(h_flex().flex_wrap().gap_3().children(stories.iter().enumerate().map(
        |(index, story)| {
            let order = story.get("order").cloned().unwrap_or(Value::Null);
            let order_label = text(Some(&order), "?");
            let unlocked = is_true(story.get("unlocked"));
            let claimed = is_true(story.get("claimed"));
            let photo = story.get("photo").and_then(Value::as_str).unwrap_or_default();
            let mut tile = div()
                .v_flex()
                .w(px(170.))
                .items_center()
                .gap_2()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .p_3()
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(if english {
                    format!("Journal {order_label}")
                } else {
                    format!("第 {order_label} 则")
                }));
            if photo.is_empty() {
                tile = tile.child(
                    div()
                        .w_full()
                        .h(px(84.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(language, "未解锁", "Locked")),
                );
            } else {
                tile = tile.child(picture(photo, 96., cx));
            }
            tile = tile.child(
                div()
                    .text_xs()
                    .text_color(if claimed {
                        cx.theme().success
                    } else if unlocked {
                        cx.theme().warning
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(if claimed {
                        crate::i18n::text(language, "已领取", "Claimed")
                    } else if unlocked {
                        crate::i18n::text(language, "可领取", "Available")
                    } else {
                        crate::i18n::text(language, "未解锁", "Locked")
                    }),
            );
            if unlocked && !claimed {
                tile = tile.child(
                    Button::new(("pet-diary-story-claim", index))
                        .label(crate::i18n::text(language, "领取", "Claim"))
                        .primary()
                        .loading(this.pet_diary_action_is_pending("story"))
                        .disabled(this.loading)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run_pet_diary_action(
                                "story",
                                serde_json::json!({"order": order}),
                                cx,
                            )
                        })),
                );
            }
            tile
        },
    )))
}
