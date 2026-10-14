use super::super::super::*;

impl FarmDesktop {
    pub(super) fn append_star_sand_action(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        let id = row
            .record
            .get("goodsId")
            .or_else(|| row.record.get("id"))
            .map(value_to_action_string)
            .unwrap_or_default();
        if !id.is_empty() {
            let name = row.title.clone();
            let fallback_goods = row.record.clone();
            let unavailable = row
                .record
                .get("owned")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
                || row.record.get("soldOut").and_then(serde_json::Value::as_bool).unwrap_or(false)
                || row.record.get("exchangeable").and_then(serde_json::Value::as_bool)
                    == Some(false)
                || activity_action_enabled(&self.page_raw, "exchange") == Some(false);
            actions = actions.child(
                Button::new(("star-sand-exchange", index))
                    .label(crate::i18n::text(&self.preferences.language, "兑换", "Exchange"))
                    .ghost()
                    .disabled(self.loading || unavailable)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_star_sand_confirmation(
                            id.clone(),
                            name.clone(),
                            fallback_goods.clone(),
                            window,
                            cx,
                        )
                    })),
            );
        }

        actions
    }
}
