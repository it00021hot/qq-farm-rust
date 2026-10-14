use super::super::super::*;

impl FarmDesktop {
    pub(super) fn append_pet_diary_action(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        if row.id.contains("/stories/") {
            let order = row.record.get("order").and_then(serde_json::Value::as_i64);
            let available = row.record.get("unlocked").and_then(serde_json::Value::as_bool)
                == Some(true)
                && row.record.get("claimed").and_then(serde_json::Value::as_bool) != Some(true);
            if let Some(order) = order {
                actions = actions.child(
                    Button::new(("pet-diary-story-claim", index))
                        .label(crate::i18n::text(
                            &self.preferences.language,
                            "领取手记奖励",
                            "Claim journal reward",
                        ))
                        .ghost()
                        .loading(self.pet_diary_action_is_pending("story"))
                        .disabled(self.loading || !available)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run_pet_diary_action(
                                "story",
                                serde_json::json!({"order": order}),
                                cx,
                            )
                        })),
                );
            }
        } else if row.id.contains("/charms/pool/") {
            let charm_id = row.record.get("id").and_then(serde_json::Value::as_i64);
            let available = self
                .page_raw
                .pointer("/petDiary/charms/canChoose")
                .and_then(serde_json::Value::as_bool)
                == Some(true);
            if let Some(charm_id) = charm_id {
                actions = actions.child(
                    Button::new(("pet-diary-equip-charm", index))
                        .label(crate::i18n::text(
                            &self.preferences.language,
                            "装备锦囊",
                            "Equip charm",
                        ))
                        .ghost()
                        .loading(self.pet_diary_action_is_pending("equipCharm"))
                        .disabled(self.loading || !available)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run_pet_diary_action(
                                "equipCharm",
                                serde_json::json!({"charmId": charm_id}),
                                cx,
                            )
                        })),
                );
            }
        } else if row.id.contains("/solarTerms/terms/") {
            let term_id = row
                .record
                .get("id")
                .or_else(|| row.record.get("termId"))
                .map(value_to_action_string)
                .unwrap_or_default();
            if pet_diary_solar_term_is_claimable(&row.record) && !term_id.is_empty() {
                actions = actions.child(
                    Button::new(format!("pet-diary-solar-claim-{term_id}"))
                        .label(crate::i18n::text(
                            &self.preferences.language,
                            "领取节令好礼",
                            "Claim seasonal gift",
                        ))
                        .ghost()
                        .loading(self.pet_diary_action_is_pending("solar"))
                        .disabled(self.loading)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run_pet_diary_action(
                                "solar",
                                serde_json::json!({"termId": term_id}),
                                cx,
                            )
                        })),
                );
            }
        } else if row.id.contains("/shop/") {
            let id = row
                .record
                .get("goodsId")
                .or_else(|| row.record.get("id"))
                .map(value_to_action_string)
                .unwrap_or_default();
            let exchangeable =
                row.record.get("exchangeable").and_then(serde_json::Value::as_bool) != Some(false);
            actions = actions.child(
                Button::new(("pet-diary-exchange", index))
                    .label(crate::i18n::text(&self.preferences.language, "兑换", "Exchange"))
                    .ghost()
                    .loading(self.pet_diary_action_is_pending("exchange"))
                    .disabled(self.loading || !exchangeable || id.is_empty())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.run_pet_diary_action(
                            "exchange",
                            serde_json::json!({"goodsId": id, "count": 1}),
                            cx,
                        )
                    })),
            );
        }

        actions
    }
}

fn pet_diary_solar_term_is_claimable(term: &serde_json::Value) -> bool {
    term.get("canClaim").and_then(serde_json::Value::as_bool) == Some(true)
}

#[cfg(test)]
mod tests {
    use super::pet_diary_solar_term_is_claimable;
    use serde_json::json;

    #[test]
    fn pet_diary_solar_action_only_appears_for_claimable_terms() {
        assert!(pet_diary_solar_term_is_claimable(&json!({"canClaim": true})));
        assert!(!pet_diary_solar_term_is_claimable(&json!({"canClaim": false})));
        assert!(!pet_diary_solar_term_is_claimable(&json!({"claimed": true})));
    }
}
