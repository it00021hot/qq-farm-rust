use super::super::super::*;

impl FarmDesktop {
    pub(in crate::desktop::activities) fn confirm_pet_diary_paid_refresh(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let paid_count = self
            .page_raw
            .pointer("/petDiary/charms/paidRefreshCount")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or_default();
        self.run_pet_diary_action(
            "refreshCharm",
            serde_json::json!({
                "payment": "tickets",
                "expectedPaidRefreshCount": paid_count,
            }),
            cx,
        );
    }
}
