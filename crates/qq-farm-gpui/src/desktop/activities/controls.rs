use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn append_activity_controls(
        &self,
        mut controls: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        if let (Page::Activities, section) = (self.page, self.page_section.as_str()) {
            let actions: &[(&str, &str, &str)] = match section {
                "season" => &[],
                "constellation" => &[],
                "shop" => &[],
                "solarTerms" => &[],
                "qingmei" => &[],
                "qixi" => &[],
                "charity" => &[],
                "petDiary" => &[],
                "autumn" => &[],
                _ => &[],
            };
            let pet_diary = self.page_raw.get("petDiary").unwrap_or(&serde_json::Value::Null);
            let pet_initialized = pet_diary
                .pointer("/nurture/initialized")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            for (key, label, gate) in actions {
                let activity_active = match section {
                    "petDiary" => {
                        pet_diary.get("active").and_then(serde_json::Value::as_bool).unwrap_or(true)
                    }
                    _ => true,
                };
                let action_gate = match *key {
                    "pet-diary-initialize" => Some(!pet_initialized),
                    "pet-diary-feed" => {
                        pet_diary.pointer("/nurture/canFeed").and_then(serde_json::Value::as_bool)
                    }
                    "pet-diary-draw" => {
                        pet_diary.pointer("/hunt/canDraw").and_then(serde_json::Value::as_bool)
                    }
                    "pet-diary-open-treasure" => Some(
                        pet_diary
                            .pointer("/treasures")
                            .and_then(serde_json::Value::as_array)
                            .is_some_and(|treasures| {
                                treasures.iter().any(|treasure| {
                                    treasure.get("status").and_then(serde_json::Value::as_i64)
                                        == Some(3)
                                })
                            }),
                    ),
                    "pet-diary-compensation" => Some(
                        pet_diary
                            .get("compensationCount")
                            .and_then(|value| {
                                value.as_i64().or_else(|| value.as_str()?.parse().ok())
                            })
                            .unwrap_or_default()
                            > 0,
                    ),
                    "pet-diary-seeds" => {
                        pet_diary.pointer("/seeds/canClaim").and_then(serde_json::Value::as_bool)
                    }
                    "pet-diary-claim-dog" => Some(
                        pet_diary.pointer("/nurture/adult").and_then(serde_json::Value::as_bool)
                            == Some(true)
                            && pet_diary
                                .pointer("/nurture/dogGranted")
                                .and_then(serde_json::Value::as_bool)
                                != Some(true),
                    ),
                    _ => activity_action_enabled(&self.page_raw, gate),
                };
                let show_action = match *key {
                    "pet-diary-initialize" => !pet_initialized,
                    "pet-diary-feed" => pet_initialized,
                    "pet-diary-claim-dog" => {
                        pet_diary.pointer("/nurture/adult").and_then(serde_json::Value::as_bool)
                            == Some(true)
                            && pet_diary
                                .pointer("/nurture/dogGranted")
                                .and_then(serde_json::Value::as_bool)
                                != Some(true)
                    }
                    _ => true,
                };
                if !show_action {
                    continue;
                }
                controls = controls.child(
                    Button::new((*key).to_string())
                        .label(*label)
                        .ghost()
                        .disabled(
                            self.loading
                                || self.selected_account.is_none()
                                || !activity_active
                                || action_gate == Some(false),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let pet_action = match *key {
                                "pet-diary-initialize" => Some("initialize"),
                                "pet-diary-feed" => Some("feed"),
                                "pet-diary-draw" => Some("draw"),
                                "pet-diary-open-treasure" => Some("openTreasure"),
                                "pet-diary-compensation" => Some("compensation"),
                                "pet-diary-seeds" => Some("seeds"),
                                "pet-diary-claim-dog" => Some("claimDog"),
                                _ => None,
                            };
                            if let Some(action) = pet_action {
                                this.run_pet_diary_action(action, serde_json::json!({}), cx);
                            } else {
                                this.run_activity_action(key, cx);
                            }
                        })),
                );
            }
        }
        controls
    }
}
