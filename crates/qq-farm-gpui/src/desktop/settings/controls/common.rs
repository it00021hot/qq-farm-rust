use super::super::super::*;

use gpui_kit::component::switch::Switch;

impl FarmDesktop {
    pub(super) fn append_setting_switch(
        &self,
        mut controls: Div,
        path: &'static str,
        label: &'static str,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let checked = self
            .settings
            .draft
            .pointer(&json_pointer(path))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        controls = controls.child(
            Switch::new(format!("settings-switch-{path}"))
                .label(settings_text(&self.preferences.language, label))
                .checked(checked)
                .disabled(disabled)
                .on_change(cx.listener(move |this, checked, _, cx| {
                    this.set_setting_path_value(path, serde_json::json!(*checked), cx)
                })),
        );
        controls
    }

    pub(super) fn append_setting_checkbox(
        &self,
        mut controls: Div,
        path: &'static str,
        item: &'static str,
        label: &'static str,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let checked = self
            .settings
            .draft
            .pointer(&json_pointer(path))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|items| items.iter().any(|value| value.as_str() == Some(item)));
        controls = controls.child(
            Checkbox::new(format!("settings-check-{item}"))
                .label(settings_text(&self.preferences.language, label))
                .checked(checked)
                .disabled(disabled)
                .on_change(cx.listener(move |this, checked, _, cx| {
                    this.set_setting_array_contains(path, item, *checked, cx)
                })),
        );
        controls
    }

    pub(super) fn append_integer_setting(
        &self,
        controls: Div,
        path: &'static str,
        label: &'static str,
        minimum: Option<i64>,
        maximum: Option<i64>,
        step: i64,
        cx: &mut Context<Self>,
    ) -> Div {
        self.append_integer_setting_disabled(
            controls, path, label, minimum, maximum, step, false, cx,
        )
    }

    pub(super) fn append_integer_setting_disabled(
        &self,
        mut controls: Div,
        path: &'static str,
        label: &'static str,
        minimum: Option<i64>,
        maximum: Option<i64>,
        step: i64,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let value = self
            .settings
            .draft
            .pointer(&json_pointer(path))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or_default();
        controls = controls.child(
            h_flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(div().text_sm().child(settings_text(&self.preferences.language, label)))
                .child(
                    h_flex()
                        .items_center()
                        .gap_1()
                        .child(format!("{value}"))
                        .child(
                            Button::new(format!("settings-step-down-{path}"))
                                .label("−")
                                .ghost()
                                .disabled(
                                    disabled
                                        || self.loading
                                        || minimum.is_some_and(|minimum| value <= minimum),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.adjust_setting(path, -step, cx)
                                })),
                        )
                        .child(
                            Button::new(format!("settings-edit-{path}"))
                                .label(settings_text(&self.preferences.language, "编辑"))
                                .ghost()
                                .disabled(disabled || self.loading)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    let current = this
                                        .settings
                                        .draft
                                        .pointer(&json_pointer(path))
                                        .and_then(serde_json::Value::as_i64)
                                        .unwrap_or_default()
                                        .to_string();
                                    this.open_setting_edit(path.to_string(), current, window, cx)
                                })),
                        )
                        .child(
                            Button::new(format!("settings-step-up-{path}"))
                                .label("+")
                                .ghost()
                                .disabled(
                                    disabled
                                        || self.loading
                                        || maximum.is_some_and(|maximum| value >= maximum),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.adjust_setting(path, step, cx)
                                })),
                        ),
                ),
        );
        controls
    }

    pub(super) fn settings_section_title(
        &self,
        mut controls: Div,
        title: &'static str,
        cx: &Context<Self>,
    ) -> Div {
        controls = controls.child(
            div()
                .pt_3()
                .pb_1()
                .border_t_1()
                .border_color(cx.theme().border)
                .font_semibold()
                .child(settings_text(&self.preferences.language, title)),
        );
        controls
    }
}
