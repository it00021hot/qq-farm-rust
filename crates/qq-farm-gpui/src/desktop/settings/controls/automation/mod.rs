use super::super::super::*;

use gpui_kit::component::switch::Switch;

mod basic;
mod fertilizer;
mod friends;
mod mystery_shop;

impl FarmDesktop {
    pub(super) fn append_automation_controls(
        &self,
        mut controls: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.selected_account.is_none() {
            return controls.child(
                div()
                    .id("settings-select-account-automation")
                    .test_support()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().group_box)
                    .p_4()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::i18n::text(
                        &self.preferences.language,
                        "请选择账号",
                        "Select account",
                    )),
            );
        }

        let automation = div()
            .v_flex()
            .gap_2()
            .child(self.automation_basics(cx))
            .child(self.automation_fertilizer_controls(cx))
            .child(self.automation_friend_controls(cx))
            .child(self.automation_mystery_shop_controls(cx))
            .child(
                Button::new("save-automation-settings")
                    .label(settings_text(&self.preferences.language, "保存自动控制"))
                    .primary()
                    .loading(self.settings.automation_saving)
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| this.save_settings(cx))),
            );

        controls = controls.child(
            div()
                .v_flex()
                .gap_3()
                .rounded(cx.theme().radius_lg)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().group_box)
                .p_4()
                .child(automation),
        );
        controls
    }

    fn automation_value(&self) -> &serde_json::Value {
        self.settings.draft.get("automation").unwrap_or(&self.settings.draft)
    }

    fn automation_enabled(&self, key: &str) -> bool {
        self.automation_value().get(key).and_then(serde_json::Value::as_bool).unwrap_or(false)
    }

    fn setting_enabled(&self, key: &str) -> bool {
        self.settings.draft.get(key).and_then(serde_json::Value::as_bool).unwrap_or(false)
    }

    fn render_automation_switch(
        &self,
        key: &'static str,
        label: &'static str,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> Switch {
        let path = format!("automation/{key}");
        let checked = self.automation_enabled(key);
        Switch::new(format!("automation-switch-{key}"))
            .label(settings_text(&self.preferences.language, label))
            .checked(checked)
            .disabled(disabled || self.loading)
            .on_change(cx.listener(move |this, checked, _, cx| {
                this.set_setting_path_value(&path, serde_json::json!(*checked), cx)
            }))
    }
}
