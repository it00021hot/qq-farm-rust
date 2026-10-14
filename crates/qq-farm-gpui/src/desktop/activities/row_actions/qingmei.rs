use super::super::super::*;

impl FarmDesktop {
    pub(super) fn append_qingmei_action(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        if let Some(uid) = row.record.get("uid").and_then(serde_json::Value::as_str) {
            let language = &self.preferences.language;
            let uid = uid.to_string();
            let stock = row
                .record
                .get("count")
                .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
                .unwrap_or_default();
            let selected = self.activities.qingmei_selected_ingredients.get(&uid).copied();
            let select_uid = uid.clone();
            actions = actions.child(
                Button::new(("qingmei-ingredient-select", index))
                    .label(if let Some(count) = selected {
                        format!("{} ×{count}", crate::i18n::text(language, "已选", "Selected"))
                    } else {
                        crate::i18n::text(language, "选择原料", "Select ingredient").into()
                    })
                    .when(selected.is_some(), |button| button.primary())
                    .when(selected.is_none(), |button| button.ghost())
                    .disabled(self.loading || stock <= 0)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_qingmei_ingredient(select_uid.clone(), stock, cx)
                    })),
            );
            if selected.is_some() {
                let down_uid = uid.clone();
                let up_uid = uid;
                actions = actions
                    .child(
                        Button::new(("qingmei-ingredient-down", index))
                            .label("−")
                            .ghost()
                            .disabled(self.loading || selected == Some(1))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.adjust_qingmei_ingredient(down_uid.clone(), stock, -1, cx)
                            })),
                    )
                    .child(
                        Button::new(("qingmei-ingredient-up", index))
                            .label("+")
                            .ghost()
                            .disabled(self.loading || selected == Some(stock))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.adjust_qingmei_ingredient(up_uid.clone(), stock, 1, cx)
                            })),
                    );
            }
        }

        actions
    }
}
