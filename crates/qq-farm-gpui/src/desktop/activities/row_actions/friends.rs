use super::super::super::*;

impl FarmDesktop {
    pub(super) fn append_activity_friend_action(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        if let Some(gid) = row.record.get("gid").and_then(serde_json::Value::as_i64) {
            let value = gid.to_string();
            let selected = self.activities.qixi_selected_friend_gid == value;
            let label = crate::i18n::text(
                &self.preferences.language,
                if selected { "已选择" } else { "选择好友" },
                if selected { "Selected" } else { "Select friend" },
            );
            actions = actions.child(
                Button::new(("qixi-select-friend", index))
                    .label(label)
                    .when(selected, |button| button.primary())
                    .when(!selected, |button| button.ghost())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.activities.qixi_selected_friend_gid = value.clone();
                        cx.notify();
                    })),
            );
        }

        actions
    }
}
