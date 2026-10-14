use super::super::super::*;

impl FarmDesktop {
    pub(super) fn append_charity_action(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        if row.record.get("target").is_some() || row.record.get("targetId").is_some() {
            let target = row
                .record
                .get("target")
                .or_else(|| row.record.get("targetId"))
                .map(value_to_action_string)
                .unwrap_or_default();
            let can_claim = row
                .record
                .get("canClaim")
                .or_else(|| row.record.get("claimable"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            actions = actions.child(
                Button::new(("charity-progress-claim", index))
                    .label(crate::i18n::text(
                        &self.preferences.language,
                        "领取阶段奖励",
                        "Claim milestone reward",
                    ))
                    .ghost()
                    .loading(self.activity_target_is_pending("charity-progress", &target))
                    .disabled(self.loading || !can_claim)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.run_activity_with_id("charity-progress", target.clone(), cx)
                    })),
            );
        }

        actions
    }
}
