use super::super::super::*;
use serde_json::Value;

use super::common::{is_true, item_line, items_line, number, section_card, text, timestamp};

fn treasure_status(status: i64, language: &str) -> String {
    match status {
        1 => crate::i18n::text(language, "待护送", "Awaiting escort").to_string(),
        2 => crate::i18n::text(language, "护送中", "Escorting").to_string(),
        3 => crate::i18n::text(language, "待领取", "Ready to claim").to_string(),
        4 => crate::i18n::text(language, "已领取", "Claimed").to_string(),
        other => crate::i18n::text(language, &format!("状态 {other}"), &format!("Status {other}"))
            .to_string(),
    }
}

pub(super) fn render(this: &FarmDesktop, snapshot: &Value, cx: &mut Context<FarmDesktop>) -> Div {
    let language = &this.preferences.language;
    let english = language == "en-US";
    let hunt = snapshot.get("hunt").unwrap_or(&Value::Null);
    let count = number(hunt.get("count"), 0);
    let limit = number(hunt.get("limit"), 10);
    let compensation = number(snapshot.get("compensationCount"), 0);
    let treasures =
        snapshot.get("treasures").and_then(Value::as_array).cloned().unwrap_or_default();
    let claimable = treasures.iter().any(|treasure| number(treasure.get("status"), 0) == 3);

    let mut panel = section_card(this, "寻宝与护送", "hunt", cx);
    if !this.activities.pet_diary_expanded_sections.contains("hunt") {
        return panel;
    }
    panel = panel
        .child(
            h_flex()
                .items_center()
                .flex_wrap()
                .gap_3()
                .child(
                    Button::new("pet-diary-draw")
                        .label(if english {
                            format!("Treasure hunt ({count}/{limit})")
                        } else {
                            format!("寻宝（{count}/{limit}）")
                        })
                        .primary()
                        .loading(this.pet_diary_action_is_pending("draw"))
                        .disabled(this.loading || !is_true(hunt.get("canDraw")))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_pet_diary_action("draw", serde_json::json!({}), cx)
                        })),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(language, "消耗", "Cost")),
                )
                .child(items_line(hunt.get("costs"), language, cx))
                .child(if english {
                    format!(
                        "Treasures {} · Lucky stars {}",
                        text(hunt.get("total"), "0"),
                        text(hunt.get("luckyStarTotal"), "0")
                    )
                } else {
                    format!(
                        "累计宝藏 {} · 幸运星 {}",
                        text(hunt.get("total"), "0"),
                        text(hunt.get("luckyStarTotal"), "0")
                    )
                }),
        )
        .child(
            h_flex()
                .items_center()
                .flex_wrap()
                .gap_2()
                .child(
                    Button::new("pet-diary-open-treasure")
                        .label(crate::i18n::text(
                            language,
                            "开启完成的宝藏",
                            "Open completed treasure",
                        ))
                        .ghost()
                        .loading(this.pet_diary_action_is_pending("openTreasure"))
                        .disabled(this.loading || !claimable)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_pet_diary_action("openTreasure", serde_json::json!({}), cx)
                        })),
                )
                .child(
                    Button::new("pet-diary-compensation")
                        .label(if english {
                            format!("Claim plunder compensation ({compensation})")
                        } else {
                            format!("领取夺宝补偿（{compensation}）")
                        })
                        .ghost()
                        .loading(this.pet_diary_action_is_pending("compensation"))
                        .disabled(this.loading || compensation <= 0)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_pet_diary_action("compensation", serde_json::json!({}), cx)
                        })),
                ),
        );

    if treasures.is_empty() {
        panel = panel.child(div().text_sm().text_color(cx.theme().muted_foreground).child(
            crate::i18n::text(
                language,
                "暂无宝藏，寻宝后可开启护送",
                "No treasure yet. Start a hunt to begin an escort.",
            ),
        ));
    } else {
        for (index, treasure) in treasures.iter().enumerate() {
            let status = number(treasure.get("status"), 0);
            let item = treasure.get("item").unwrap_or(&Value::Null);
            let time = timestamp(treasure.get("endTime"));
            panel = panel.child(
                h_flex()
                    .items_center()
                    .flex_wrap()
                    .gap_3()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().group_box)
                    .p_3()
                    .child(
                        div()
                            .rounded_full()
                            .bg(if status == 3 {
                                cx.theme().success
                            } else {
                                cx.theme().background
                            })
                            .px_2()
                            .py_1()
                            .text_xs()
                            .child(treasure_status(status, language)),
                    )
                    .child(item_line(item, 24., language, cx))
                    .child(if english {
                        format!(
                            "Protected {}/{} · Plundered {}/{}",
                            text(treasure.get("protectedCount"), "0"),
                            text(treasure.get("originalCount"), "0"),
                            text(treasure.get("plunderCount"), "0"),
                            text(treasure.get("maxPlunderCount"), "0")
                        )
                    } else {
                        format!(
                            "保护 {}/{} · 被夺 {}/{}",
                            text(treasure.get("protectedCount"), "0"),
                            text(treasure.get("originalCount"), "0"),
                            text(treasure.get("plunderCount"), "0"),
                            text(treasure.get("maxPlunderCount"), "0")
                        )
                    })
                    .when_some(time, |row, time| {
                        row.child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                            if english {
                                format!("Arrives {time}")
                            } else {
                                format!("{time} 到达")
                            },
                        ))
                    })
                    .id(format!("pet-diary-treasure-{index}")),
            );
        }
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::treasure_status;

    #[test]
    fn unknown_treasure_statuses_keep_the_vue_status_number_label() {
        assert_eq!(treasure_status(3, "zh-CN"), "待领取");
        assert_eq!(treasure_status(9, "zh-CN"), "状态 9");
        assert_eq!(treasure_status(9, "en-US"), "Status 9");
    }
}
