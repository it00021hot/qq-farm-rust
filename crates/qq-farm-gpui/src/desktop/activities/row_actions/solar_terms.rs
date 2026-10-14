use super::super::super::*;

impl FarmDesktop {
    pub(super) fn append_solar_term_action(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        let id = row.record.get("id").or_else(|| row.record.get("termId"));
        let id = id.map(value_to_action_string).unwrap_or_default();
        if !id.is_empty() {
            let selected = self.activities.solar_term_id == id;
            let label = crate::i18n::text(
                &self.preferences.language,
                if selected { "当前节气" } else { "查看节气" },
                if selected { "Current term" } else { "View term" },
            );
            actions = actions.child(
                Button::new(("solar-select", index))
                    .label(label)
                    .when(selected, |button| button.primary())
                    .when(!selected, |button| button.ghost())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.activities.solar_term_id = id.clone();
                        cx.notify();
                    })),
            );
        }

        actions
    }
}
