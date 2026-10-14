use super::super::super::*;

impl FarmDesktop {
    pub(super) fn append_activity_directory_action(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        if activity_section_for_record(&row.record).is_some() {
            let record = row.record.clone();
            actions = actions.child(
                Button::new(("activity-open", index))
                    .label(crate::i18n::text(
                        &self.preferences.language,
                        "查看玩法",
                        "View activity",
                    ))
                    .ghost()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_activity_record(&record, window, cx)
                    })),
            );
        }

        actions
    }
}
