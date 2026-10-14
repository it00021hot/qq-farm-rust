use super::super::super::*;

impl FarmDesktop {
    pub(super) fn append_pet_diary_friend_action(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        if row.id.contains("/treasures/") {
            let treasure_id = row.record.get("id").map(value_to_action_string).unwrap_or_default();
            if !treasure_id.is_empty() {
                for (preview_index, preview) in row
                    .record
                    .get("previews")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    let challenge_id =
                        preview.get("challengeId").map(value_to_action_string).unwrap_or_default();
                    let can_start = preview
                        .get("canStart")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false);
                    if !challenge_id.is_empty() {
                        let target_treasure_id = treasure_id.clone();
                        let target_challenge_id = challenge_id.clone();
                        let selected =
                            self.activities.pet_diary_battle_target.as_ref().is_some_and(
                                |(_, selected_treasure, selected_challenge)| {
                                    selected_treasure == &treasure_id
                                        && selected_challenge == &challenge_id
                                },
                            );
                        let label = if selected {
                            crate::i18n::text(
                                &self.preferences.language,
                                "已选挑战书",
                                "Selected challenge card",
                            )
                        } else {
                            crate::i18n::text(
                                &self.preferences.language,
                                "选择挑战书",
                                "Select challenge card",
                            )
                        };
                        actions = actions.child(
                            Button::new(format!("pet-diary-challenge-{index}-{preview_index}"))
                                .label(format!("{label} {challenge_id}"))
                                .when(selected, |button| button.primary())
                                .when(!selected, |button| button.ghost())
                                .disabled(self.loading || !can_start)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_pet_diary_battle_target(
                                        target_treasure_id.clone(),
                                        target_challenge_id.clone(),
                                        cx,
                                    )
                                })),
                        );
                    }
                }
            }
        }

        actions
    }
}
