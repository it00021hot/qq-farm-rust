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
        let _ = (minimum, maximum, step);
        // 框体输入 + 内嵌 − + 步进（对齐 Tauri NInputNumber）；值经 Change 事件回写 draft。
        let value = self
            .settings
            .draft
            .pointer(&json_pointer(path))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or_default();
        let number = div().id(format!("settings-number-{path}")).child(
            match self.settings.number_inputs.get(path).cloned() {
                Some(input) => gpui_kit::component::input::NumberInput::new(&input)
                    .w(px(200.))
                    .disabled(disabled || self.loading)
                    .into_any_element(),
                // 路径未注册的兜底：sync_setting_selectors 会在载入后注册全部路径
                None => div()
                    .w(px(200.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(value.to_string())
                    .into_any_element(),
            },
        );
        #[cfg(test)]
        let number = number.test_support();
        let control = number.into_any_element();
        controls = controls.child(
            h_flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(div().text_sm().child(settings_text(&self.preferences.language, label)))
                .child(control),
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
