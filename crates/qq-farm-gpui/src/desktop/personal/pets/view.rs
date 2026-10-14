use super::*;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::StatefulInteractiveElement;
use gpui_kit::TestSupportExt;

impl FarmDesktop {
    pub(in crate::desktop) fn render_personal_pets(&self, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        let info = self.page_raw.get("pets").unwrap_or(&serde_json::Value::Null);
        let dogs = info.get("dogs").and_then(serde_json::Value::as_array);
        let active_id =
            info.get("activeDogId").and_then(serde_json::Value::as_i64).unwrap_or_default();
        let active_name = dogs
            .into_iter()
            .flatten()
            .find(|dog| dog.get("id").and_then(serde_json::Value::as_i64) == Some(active_id))
            .and_then(|dog| dog.get("name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(crate::i18n::text(language, "无", "None"));
        let pending = self
            .page_raw
            .pointer("/petGiftStatus/pending")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or_default();
        div()
            .v_flex()
            .gap_4()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(div().font_semibold().child(crate::i18n::text(
                        language,
                        "我的宠物",
                        "My Pets",
                    )))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .when(pending > 0, |row| {
                                row.child(pet_tag(
                                    crate::i18n::text(
                                        language,
                                        &format!("待领礼包: {pending}"),
                                        &format!("Pending Gifts: {pending}"),
                                    )
                                    .to_string(),
                                    Some(cx.theme().warning),
                                    cx,
                                ))
                                .child(
                                    Button::new("claim-pet-gifts")
                                        .label(crate::i18n::text(language, "领取", "Claim"))
                                        .primary()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.run_personal_pet_action(PetAction::Claim, cx)
                                        })),
                                )
                            })
                            .child(
                                Button::new("pet-protect-logs")
                                    .label(crate::i18n::text(language, "守护记录", "Guard Logs"))
                                    .ghost()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.run_personal_pet_action(PetAction::Logs, cx)
                                    })),
                            )
                            .child(
                                Button::new("pet-refresh")
                                    .label(crate::i18n::text(language, "刷新", "Refresh"))
                                    .ghost()
                                    .on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.refresh_personal_pets(cx)
                                        }),
                                    ),
                            ),
                    ),
            )
            .when_some(self.farm.pets.notice.as_ref(), |panel, (message, success)| {
                panel.child(
                    div()
                        .text_sm()
                        .text_color(if *success { cx.theme().success } else { cx.theme().danger })
                        .child(message.clone()),
                )
            })
            .when(info.is_null(), |panel| {
                panel.child(
                    div()
                        .id("personal-pets-empty")
                        .py_8()
                        .text_center()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(language, "暂无宠物信息", "No pet info"))
                        .test_support(),
                )
            })
            .when(!info.is_null(), |panel| {
                let protect = duration_label(
                    info.get("protectDuration")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or_default(),
                    language,
                );
                let max_protect = duration_label(
                    info.get("maxProtectDuration")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or_default(),
                    language,
                );
                panel
                    .child(
                        h_flex()
                            .items_center()
                            .flex_wrap()
                            .gap_4()
                            .child(pet_summary(
                                "pet-protection-remaining",
                                crate::i18n::text(language, "守护剩余", "Guard Left"),
                                protect,
                                cx,
                            ))
                            .child(pet_summary(
                                "pet-protection-max",
                                crate::i18n::text(language, "守护上限", "Guard Max"),
                                max_protect,
                                cx,
                            ))
                            .child(pet_summary(
                                "pet-active-name",
                                crate::i18n::text(language, "当前宠物", "Active Pet"),
                                active_name.into(),
                                cx,
                            )),
                    )
                    .child({
                        let dog_list = dogs.into_iter().flatten().collect::<Vec<_>>();
                        let dogs_len = dog_list.len();
                        div()
                            .v_flex()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .px_3()
                            .children(dog_list.into_iter().enumerate().map(|(index, dog)| {
                                self.render_personal_pet_dog(dog, index + 1 == dogs_len, cx)
                            }))
                    })
                    .when(active_id > 0, |panel| {
                        panel.child(
                            h_flex().child(
                                Button::new("pet-withdraw")
                                    .label(crate::i18n::text(language, "收回", "Withdraw"))
                                    .warning()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.run_personal_pet_action(PetAction::Withdraw, cx)
                                    })),
                            ),
                        )
                    })
            })
            .child(self.render_personal_pet_food(cx))
    }

    fn render_personal_pet_dog(
        &self,
        dog: &serde_json::Value,
        last: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = dog.get("id").and_then(serde_json::Value::as_i64).unwrap_or_default();
        let language = &self.preferences.language;
        let name = dog
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(crate::i18n::text(language, "宠物", "Pet"));
        let active = dog.get("active").and_then(serde_json::Value::as_bool).unwrap_or(false);
        let owned = dog.get("owned").and_then(serde_json::Value::as_bool).unwrap_or(false);
        let activating = self.farm.pets.activating;
        let (activate, deploy) = dog_action_gates(dog);
        let rarity = dog.get("rarityLabel").and_then(serde_json::Value::as_str).unwrap_or("");
        let skill = dog
            .get("skillDescription")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string();
        h_flex()
            .id(format!("pet-dog-{id}"))
            .items_center()
            .justify_between()
            .gap_3()
            .py_3()
            .when(!last, |row| row.border_b_1().border_color(cx.theme().border))
            .child(
                div()
                    .v_flex()
                    .min_w_0()
                    .gap_1()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(div().font_medium().child(name.to_string()))
                            .child(pet_tag(
                                if active {
                                    crate::i18n::text(language, "看家中", "On Duty")
                                } else if owned {
                                    crate::i18n::text(language, "已获得", "Owned")
                                } else {
                                    crate::i18n::text(language, "未获得", "Locked")
                                }
                                .to_string(),
                                active.then(|| cx.theme().success),
                                cx,
                            ))
                            .when(activate && !active, |row| {
                                row.child(pet_tag(
                                    crate::i18n::text(language, "可激活", "Activatable")
                                        .to_string(),
                                    Some(cx.theme().warning),
                                    cx,
                                ))
                            })
                            .child(pet_tag(rarity.to_string(), None, cx)),
                    )
                    .child(pet_skill_description(id, &skill, cx)),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .when(activate, |row| {
                        row.child(
                            Button::new(format!("pet-activate-{id}"))
                                .label(crate::i18n::text(language, "激活", "Activate"))
                                .warning()
                                .loading(activating)
                                .disabled(activating)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.run_personal_pet_action(PetAction::Activate(id), cx)
                                })),
                        )
                    })
                    .when(deploy, |row| {
                        row.child(
                            Button::new(format!("pet-deploy-{id}"))
                                .label(crate::i18n::text(language, "上场", "Deploy"))
                                .primary()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.run_personal_pet_action(PetAction::Deploy(id), cx)
                                })),
                        )
                    }),
            )
            .test_support()
    }
}

fn pet_skill_description(id: i64, skill: &str, cx: &App) -> impl IntoElement {
    let tooltip = skill.to_string();
    div()
        .id(format!("pet-skill-{id}"))
        .aria_label(skill.to_string())
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(pet_skill_preview(skill))
        .when(!skill.is_empty(), |description| {
            description.tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        })
        .test_support()
}

fn pet_skill_preview(skill: &str) -> String {
    format!("{}…", skill.chars().take(12).collect::<String>())
}

fn pet_summary(id: &'static str, label: &'static str, value: String, cx: &App) -> impl IntoElement {
    h_flex()
        .id(id)
        .items_center()
        .gap_2()
        .flex_none()
        .child(div().text_sm().text_color(cx.theme().muted_foreground).child(label))
        .child(div().font_medium().child(value))
        .test_support()
}

pub(super) fn pet_tag(label: String, color: Option<Hsla>, cx: &App) -> impl IntoElement {
    let mut tag = div().rounded_full().px_2().py_1().text_xs();
    match color {
        Some(color) => tag = tag.bg(color.opacity(0.16)).text_color(color),
        None => tag = tag.bg(cx.theme().muted),
    }
    tag.child(label)
}

#[cfg(test)]
mod tests {
    #[test]
    fn pet_skill_preview_truncates_after_twelve_characters_and_keeps_ellipsis() {
        assert_eq!(super::pet_skill_preview("1234567890ABCDEFGHI"), "1234567890AB…");
        assert_eq!(super::pet_skill_preview("技能说明"), "技能说明…");
        assert_eq!(super::pet_skill_preview(""), "…");
    }
}
