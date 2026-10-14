use super::super::super::*;
use gpui_kit::component::switch::Switch;
use serde_json::Value;

use super::common::{is_true, item_line, section_card, text};
use super::friend_picker;

fn eligible_treasures(friend: &Value) -> Vec<Value> {
    friend
        .get("treasures")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|treasure| {
            number_status(treasure.get("status")) == 2
                && treasure.get("previews").and_then(Value::as_array).is_some_and(|previews| {
                    previews.iter().any(|preview| is_true(preview.get("canStart")))
                })
        })
        .cloned()
        .collect()
}

fn number_status(value: Option<&Value>) -> i64 {
    value
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
        .unwrap_or_default()
}

fn challenge_label(id: &str, language: &str) -> String {
    match (id, language == "en-US") {
        ("80101", false) => "初级（50 幸运星）".into(),
        ("80102", false) => "中级（150 幸运星）".into(),
        ("80103", false) => "高级（300 幸运星）".into(),
        ("80101", true) => "Beginner (50 lucky stars)".into(),
        ("80102", true) => "Intermediate (150 lucky stars)".into(),
        ("80103", true) => "Advanced (300 lucky stars)".into(),
        _ => id.to_string(),
    }
}

pub(super) fn render(this: &FarmDesktop, snapshot: &Value, cx: &mut Context<FarmDesktop>) -> Div {
    let language = &this.preferences.language;
    let english = language == "en-US";
    let hunt = snapshot.get("hunt").unwrap_or(&Value::Null);
    let gid = this.activities.pet_diary_friend_gid.trim().to_string();
    let battle_count = text(snapshot.get("battleCount"), "0");
    let battle_limit = text(snapshot.get("battleLimit"), "20");
    let skip_battle = is_true(snapshot.get("skipBattle"));
    let loaded_gid = text(this.page_raw.get("petDiaryFriendGid"), "");
    let loaded_friend = if loaded_gid == gid {
        this.page_raw.get("petDiaryFriend").unwrap_or(&Value::Null)
    } else {
        &Value::Null
    };
    let all_treasures =
        loaded_friend.get("treasures").and_then(Value::as_array).cloned().unwrap_or_default();
    let treasures = eligible_treasures(loaded_friend);
    let mut panel = section_card(this, "好友夺宝", "battle", cx);
    if !this.activities.pet_diary_expanded_sections.contains("battle") {
        return panel;
    }
    panel =
        panel.child(
            h_flex()
                .items_center()
                .flex_wrap()
                .gap_3()
                .child(div().w(px(240.)).child(friend_picker::render(
                    language,
                    &this.activities.pet_diary_friend_selector,
                )))
                .child(
                    Button::new("pet-diary-refresh-friends")
                        .label(crate::i18n::text(language, "刷新好友", "Refresh friends"))
                        .ghost()
                        .loading(this.activities.pet_diary_friends_loading)
                        .disabled(this.activities.pet_diary_friends_loading)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_pet_diary_friends(cx))),
                )
                .child(
                    Button::new("pet-diary-query-friend")
                        .label(crate::i18n::text(language, "查询好友宝藏", "Find friend treasure"))
                        .ghost()
                        .loading(this.activities.pet_diary_friend_loading)
                        .disabled(this.activities.pet_diary_friend_loading)
                        .on_click(cx.listener(|this, _, _, cx| this.load_pet_diary_friend(cx))),
                )
                .child(if english {
                    format!("Today's plunders {battle_count}/{battle_limit}")
                } else {
                    format!("今日夺宝 {battle_count}/{battle_limit}")
                }),
        );

    panel = panel.child(
        Switch::new("pet-diary-skip-battle")
            .label(crate::i18n::text(language, "跳过战斗动画", "Skip battle animation"))
            .checked(skip_battle)
            .disabled(this.loading || this.pet_diary_action_is_pending("skipBattle"))
            .on_change(cx.listener(move |this, skip, _, cx| {
                this.run_pet_diary_action("skipBattle", serde_json::json!({"skip": *skip}), cx)
            })),
    );

    if let Some(error) = this.activities.pet_diary_friend_error.as_ref() {
        let error = div()
            .id("pet-diary-friend-error")
            .aria_label(error.clone())
            .text_sm()
            .text_color(cx.theme().danger)
            .child(error.clone());
        #[cfg(test)]
        let error = error.test_support();
        panel = panel.child(error);
    }

    if !loaded_gid.is_empty() && loaded_gid == gid {
        if treasures.is_empty() {
            let empty = if all_treasures.is_empty() {
                crate::i18n::text(
                    language,
                    "该好友没有护送中的宝藏",
                    "This friend has no escorted treasure.",
                )
            } else {
                crate::i18n::text(
                    language,
                    "该好友暂无可开战宝藏",
                    "This friend has no treasure available to battle.",
                )
            };
            panel =
                panel.child(div().text_sm().text_color(cx.theme().muted_foreground).child(empty));
        } else {
            for (treasure_index, treasure) in treasures.iter().enumerate() {
                let treasure_id = text(treasure.get("id"), "");
                panel = panel.child(
                    div()
                        .v_flex()
                        .gap_2()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().group_box)
                        .p_3()
                        .child(item_line(
                            treasure.get("item").unwrap_or(&Value::Null),
                            24.,
                            language,
                            cx,
                        ))
                        .child(
                            h_flex().flex_wrap().gap_2().children(
                                treasure
                                    .get("previews")
                                    .and_then(Value::as_array)
                                    .into_iter()
                                    .flatten()
                                    .enumerate()
                                    .filter_map(|(preview_index, preview)| {
                                        let challenge_id = text(preview.get("challengeId"), "");
                                        if challenge_id.is_empty() {
                                            return None;
                                        }
                                        let available = is_true(preview.get("canStart"));
                                        let selected = this
                                            .activities
                                            .pet_diary_battle_target
                                            .as_ref()
                                            .is_some_and(
                                                |(
                                                    target_gid,
                                                    target_treasure,
                                                    target_challenge,
                                                )| {
                                                    target_gid == &gid
                                                        && target_treasure == &treasure_id
                                                        && target_challenge == &challenge_id
                                                },
                                            );
                                        let challenge_for_action = challenge_id.clone();
                                        let treasure_for_action = treasure_id.clone();
                                        let label = if english {
                                            format!(
                                                "{} (can plunder {})",
                                                challenge_label(&challenge_id, language),
                                                text(preview.get("plunderableCount"), "0")
                                            )
                                        } else {
                                            format!(
                                                "{}（可夺 {}）",
                                                challenge_label(&challenge_id, language),
                                                text(preview.get("plunderableCount"), "0")
                                            )
                                        };
                                        Some(
                                                Button::new(format!(
                                                    "pet-diary-challenge-{treasure_index}-{preview_index}"
                                                ))
                                            .label(if selected {
                                                format!(
                                                    "{} {label}",
                                                    crate::i18n::text(language, "已选", "Selected")
                                                )
                                            } else {
                                                label
                                            })
                                            .when(selected, |button| button.primary())
                                            .when(!selected, |button| button.ghost())
                                            .disabled(this.loading || !available)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.select_pet_diary_battle_target(
                                                    treasure_for_action.clone(),
                                                    challenge_for_action.clone(),
                                                    cx,
                                                )
                                            })),
                                        )
                                    }),
                            ),
                        ),
                );
            }
            if let Some((target_gid, treasure_id, challenge_id)) =
                this.activities.pet_diary_battle_target.as_ref()
            {
                if target_gid == &gid {
                    panel = panel.child(
                        div().text_sm().text_color(cx.theme().muted_foreground).child(if english {
                            format!(
                                "Selected treasure {treasure_id}, challenge {}",
                                challenge_label(challenge_id, language)
                            )
                        } else {
                            format!(
                                "已选宝藏 {treasure_id}，挑战书 {}",
                                challenge_label(challenge_id, language)
                            )
                        }),
                    );
                }
            }
            panel = panel.child(
                Button::new("pet-diary-start-battle")
                    .label(crate::i18n::text(language, "开始夺宝", "Start plunder"))
                    .primary()
                    .loading(this.pet_diary_action_is_pending("battle"))
                    .disabled(this.loading || !is_true(hunt.get("canPlunder")))
                    .on_click(cx.listener(|this, _, _, cx| this.start_pet_diary_battle(cx))),
            );
        }
    }

    if let Some(result) = this.page_raw.get("petDiaryBattleResult").filter(|value| !value.is_null())
    {
        let won = is_true(result.get("won"));
        let dice = if english {
            format!(
                "Dice · You {} : Opponent {}",
                text(result.get("attackerDice"), "0"),
                text(result.get("defenderDice"), "0")
            )
        } else {
            format!(
                "骰子 我方 {} : 对方 {}",
                text(result.get("attackerDice"), "0"),
                text(result.get("defenderDice"), "0")
            )
        };
        let mut result_panel = div()
            .v_flex()
            .gap_2()
            .rounded(cx.theme().radius)
            .bg(cx.theme().group_box)
            .p_3()
            .child(div().font_medium().child(if won {
                crate::i18n::text(language, "夺宝成功", "Plunder succeeded")
            } else {
                crate::i18n::text(language, "本次夺宝未获胜", "Plunder failed")
            }))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(dice));
        if is_true(result.get("isFake")) {
            result_panel = result_panel.child(
                div().text_sm().text_color(cx.theme().muted_foreground).child(crate::i18n::text(
                    language,
                    "触发移花接木：假宝藏",
                    "Misdirection triggered: fake treasure",
                )),
            );
        }
        if let Some(item) = result.get("plundered").filter(|item| !item.is_null()) {
            result_panel = result_panel.child(item_line(item, 22., language, cx));
        }
        if let Some(rewards) = result.get("rewards").and_then(Value::as_array) {
            result_panel = result_panel.child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .children(rewards.iter().map(|item| item_line(item, 22., language, cx))),
            );
        }
        panel = panel.child(result_panel);
    }
    panel
}
