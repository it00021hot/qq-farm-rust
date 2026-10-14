use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn append_game_config_row_actions(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.page == Page::GameConfig {
            let language = &self.preferences.language;
            let is_seed = row.section == "seeds";
            let id = if is_seed {
                row.record.get("seedId").or_else(|| row.record.get("id"))
            } else {
                row.record.get("id")
            }
            .and_then(serde_json::Value::as_i64);
            if let Some(id) = id {
                let record = row.record.clone();
                let edit_button = Button::new(("config-edit", index))
                    .label(crate::i18n::text(language, "编辑", "Edit"))
                    .primary()
                    .outline()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_game_config_form(Some(record.clone()), window, cx)
                    }));
                let delete_button = Button::new(("config-delete", index))
                    .label(crate::i18n::text(language, "删除", "Delete"))
                    .danger()
                    .outline();
                let delete_button = self.render_game_config_delete_confirmation(
                    if is_seed { "seed" } else { "item" },
                    id,
                    index,
                    delete_button,
                    cx,
                );
                actions = actions.child(edit_button).child(delete_button);
            }
        }
        actions
    }
}
