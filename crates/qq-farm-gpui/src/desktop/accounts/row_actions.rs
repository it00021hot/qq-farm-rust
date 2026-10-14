use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn append_account_row_actions(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.page == Page::Accounts {
            if let Some(id) = row.record.get("id").and_then(serde_json::Value::as_str) {
                let id = id.to_string();
                let toggle_id = id.clone();
                let delete_id = id;
                let running =
                    row.record.get("running").and_then(serde_json::Value::as_bool).unwrap_or(false);
                let lifecycle_pending = self.accounts.lifecycle.is_pending(&toggle_id);
                actions = actions
                    .child(
                        Button::new(("account-toggle", index))
                            .label(crate::i18n::text(
                                &self.preferences.language,
                                if running { "停止" } else { "启动" },
                                if running { "Stop" } else { "Start" },
                            ))
                            .ghost()
                            .loading(lifecycle_pending)
                            .disabled(lifecycle_pending || self.loading)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_account_lifecycle(toggle_id.clone(), !running, cx)
                            })),
                    )
                    .child(
                        self.render_account_delete_confirmation(
                            delete_id,
                            index,
                            Button::new(("account-delete", index))
                                .label(crate::i18n::text(
                                    &self.preferences.language,
                                    "删除",
                                    "Delete",
                                ))
                                .ghost(),
                            cx,
                        ),
                    );
            }
        }
        actions
    }
}
