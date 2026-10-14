use super::super::super::*;

impl FarmDesktop {
    pub(super) fn append_constellation_action(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        cx: &mut Context<Self>,
    ) -> Div {
        let group_id = row.record.get("id").map(value_to_action_string).unwrap_or_default();
        if group_id.is_empty() {
            return actions;
        }
        let selected = self.activities.constellation_group_id == group_id;
        let label = crate::i18n::text(
            &self.preferences.language,
            if selected { "已选择" } else { "选择星座" },
            if selected { "Selected" } else { "Select constellation" },
        );
        actions = actions.child(
            Button::new(format!("constellation-select-{group_id}"))
                .label(label)
                .when(selected, |button| button.primary())
                .when(!selected, |button| button.ghost())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.activities.constellation_group_id = group_id.clone();
                    cx.notify();
                })),
        );
        actions
    }
}
