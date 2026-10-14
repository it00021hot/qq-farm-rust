use super::*;
use gpui_kit::component::select::Select;

impl FarmDesktop {
    pub(in crate::desktop) fn render_system_config_view(&self, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        let config = self.settings.draft.get("systemConfig").cloned().unwrap_or_default();
        let selected_platform =
            config.get("platform").and_then(serde_json::Value::as_str).unwrap_or("qq");
        let selected_os = config.get("os").and_then(serde_json::Value::as_str).unwrap_or("Windows");
        let presets = self
            .page_raw
            .get("devicePresets")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut preset_buttons = h_flex().flex_wrap().gap_2();
        for preset in presets {
            let id = preset.get("id").and_then(serde_json::Value::as_str).unwrap_or_default();
            let name = preset.get("name").and_then(serde_json::Value::as_str).unwrap_or(id);
            let description =
                preset.get("description").and_then(serde_json::Value::as_str).unwrap_or_default();
            let selected = self.settings.selected_device_preset.as_deref() == Some(id);
            let id_owned = id.to_string();
            preset_buttons = preset_buttons.child(
                Button::new(format!("system-device-preset-{id}"))
                    .label(name)
                    .tooltip(description)
                    .when(selected, |button| button.primary())
                    .when(!selected, |button| button.ghost())
                    .disabled(self.loading)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.apply_device_preset(id_owned.clone(), window, cx)
                    })),
            );
        }

        let mut platform_buttons = h_flex().flex_wrap().gap_2();
        for (id, label) in [("qq", "QQ"), ("wx", "微信")] {
            let selected = selected_platform == id;
            platform_buttons = platform_buttons.child(
                Button::new(format!("system-platform-{id}"))
                    .label(crate::i18n::text(
                        language,
                        label,
                        if id == "qq" { "QQ" } else { "WeChat" },
                    ))
                    .when(selected, |button| button.primary())
                    .when(!selected, |button| button.ghost())
                    .disabled(self.loading)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_setting_path_value(
                            "systemConfig/platform",
                            serde_json::json!(id),
                            cx,
                        );
                        this.settings.selected_device_preset = None;
                    })),
            );
        }

        let mut os_buttons = h_flex().flex_wrap().gap_2();
        for (id, label) in [("Windows", "Windows"), ("iOS", "iOS"), ("Android", "Android")] {
            let selected = selected_os == id;
            os_buttons = os_buttons.child(
                Button::new(format!("system-os-{id}"))
                    .label(label)
                    .when(selected, |button| button.primary())
                    .when(!selected, |button| button.ghost())
                    .disabled(self.loading)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_device_os(id, cx))),
            );
        }

        let text_field = |id: &'static str, label: &'static str, input: &Entity<InputState>| {
            div()
                .v_flex()
                .gap_1()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(language, label, system_config_label_en(id))),
                )
                .child(Input::new(input).id(id).w_full().disabled(self.loading))
        };

        div()
            .v_flex()
            .gap_4()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_4()
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(div().font_semibold().child(crate::i18n::text(
                        language,
                        "运行环境",
                        "Runtime environment",
                    )))
                    .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                        crate::i18n::text(
                            language,
                            "修改连接网关、登录平台和设备指纹；保存后应用到后续连接。",
                            "Configure the gateway, login platform, and device profile used by new connections.",
                        ),
                    )),
            )
            .when(!self.page_raw.get("devicePresets").and_then(serde_json::Value::as_array).is_none_or(Vec::is_empty), |panel| {
                panel
                    .child(div().text_sm().font_medium().child(crate::i18n::text(language, "设备预设", "Device presets")))
                    .child(preset_buttons)
            })
            .child(text_field("system-server-url", "服务器地址", &self.settings.system_server_url))
            .child(
                div().v_flex().gap_2()
                    .child(div().text_sm().font_medium().child(crate::i18n::text(language, "时区", "Time zone")))
                    .child(
                        Select::new(&self.settings.time_zone)
                            .id("system-timezone-select")
                            .w(px(320.))
                            .accessibility_label(crate::i18n::text(language, "时区", "Time zone"))
                            .disabled(self.loading),
                    ),
            )
            .child(
                div().v_flex().gap_2()
                    .child(div().text_sm().font_medium().child(crate::i18n::text(language, "平台", "Platform")))
                    .child(platform_buttons),
            )
            .child(
                div().v_flex().gap_2()
                    .child(div().text_sm().font_medium().child(crate::i18n::text(language, "操作系统", "Operating system")))
                    .child(os_buttons),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap_3()
                    .child(text_field("system-client-version", "客户端版本", &self.settings.system_client_version))
                    .child(text_field("system-software", "系统软件", &self.settings.system_software))
                    .child(text_field("system-device-id", "设备 ID", &self.settings.system_device_id))
                    .child(text_field("system-memory", "内存 (MB)", &self.settings.system_memory))
                    .child(text_field("system-network", "网络类型", &self.settings.system_network)),
            )
            .child(text_field("system-user-agent", "User-Agent", &self.settings.system_user_agent))
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .pt_3()
                    .child(
                        Button::new("reset-system-config")
                            .label(crate::i18n::text(language, "重置为默认", "Reset to default"))
                            .ghost()
                            .disabled(self.loading)
                            .on_click(cx.listener(|this, _, _, cx| this.reset_system_config(cx))),
                    )
                    .child(
                        Button::new("save-system-config")
                            .label(crate::i18n::text(language, "保存运行环境", "Save runtime environment"))
                            .primary()
                            .disabled(self.loading)
                            .on_click(cx.listener(|this, _, _, cx| this.save_system_config(cx))),
                    ),
            )
    }
}

fn system_config_label_en(id: &str) -> &'static str {
    match id {
        "system-server-url" => "Gateway URL",
        "system-client-version" => "Client version",
        "system-software" => "System software",
        "system-device-id" => "Device ID",
        "system-memory" => "Memory",
        "system-network" => "Network type",
        "system-user-agent" => "User-Agent",
        _ => "Setting",
    }
}
