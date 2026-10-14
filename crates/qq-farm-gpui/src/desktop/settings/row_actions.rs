use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn append_settings_row_actions(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.page == Page::Settings {
            if let Some(enabled) = self
                .settings
                .draft
                .pointer(&json_pointer(&row.id))
                .and_then(serde_json::Value::as_bool)
            {
                let path = row.id.clone();
                actions = actions.child(
                    Button::new(("setting-toggle", index))
                        .label(settings_text(
                            &self.preferences.language,
                            if enabled { "已开启" } else { "已关闭" },
                        ))
                        .ghost()
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.toggle_setting(&path, cx)),
                        ),
                );
            } else if row.record.as_i64().is_some() {
                let path = row.id.clone();
                actions = actions
                    .child(Button::new(("setting-minus", index)).label("−").ghost().on_click(
                        cx.listener({
                            let path = path.clone();
                            move |this, _, _, cx| this.adjust_setting(&path, -1, cx)
                        }),
                    ))
                    .child(Button::new(("setting-plus", index)).label("+1").ghost().on_click(
                        cx.listener({
                            let path = path.clone();
                            move |this, _, _, cx| this.adjust_setting(&path, 1, cx)
                        }),
                    ))
                    .child(
                        Button::new(("setting-edit-number", index))
                            .label(settings_text(&self.preferences.language, "编辑"))
                            .ghost()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                let value = this
                                    .settings
                                    .draft
                                    .pointer(&json_pointer(&path))
                                    .and_then(serde_json::Value::as_i64)
                                    .unwrap_or_default()
                                    .to_string();
                                this.open_setting_edit(path.clone(), value, window, cx)
                            })),
                    );
            } else if let Some(value) = row.record.as_str() {
                let path = row.id.clone();
                let value = value.to_string();
                actions = actions.child(
                    Button::new(("setting-edit", index))
                        .label(settings_text(&self.preferences.language, "编辑"))
                        .ghost()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_setting_edit(path.clone(), value.clone(), window, cx)
                        })),
                );
            }
        }
        actions
    }
}
