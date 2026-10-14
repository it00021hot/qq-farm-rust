use super::super::super::*;

impl FarmDesktop {
    pub(super) fn append_autumn_action(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        let choice_id = row.record.get("id").map(value_to_action_string).unwrap_or_default();
        if !choice_id.is_empty() {
            let selected = self.activities.autumn_choice_id == choice_id;
            let label = crate::i18n::text(
                &self.preferences.language,
                if selected { "已选择" } else { "选择祈愿" },
                if selected { "Selected" } else { "Select wish" },
            );
            actions = actions.child(
                Button::new(("autumn-select-choice", index))
                    .label(label)
                    .when(selected, |button| button.primary())
                    .when(!selected, |button| button.ghost())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.activities.autumn_choice_id = choice_id.clone();
                        cx.notify();
                    })),
            );
        }

        actions
    }
}
