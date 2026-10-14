use super::super::*;
use gpui_kit::component::searchable_list::SearchableListItem;
use gpui_kit::component::select::{SearchableVec, SelectState};
use gpui_kit::SharedString;

#[derive(Clone)]
pub(in crate::desktop) struct TimeZoneItem {
    value: &'static str,
    label: String,
}

impl SearchableListItem for TimeZoneItem {
    type Value = &'static str;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

pub(in crate::desktop) type TimeZoneSelect = SelectState<SearchableVec<TimeZoneItem>>;

pub(in crate::desktop) fn time_zone_items(language: &str) -> Vec<TimeZoneItem> {
    qq_farm_core::config::TIME_ZONE_OPTIONS
        .iter()
        .map(|option| TimeZoneItem {
            value: option.value,
            label: timezone_label(option.value, language),
        })
        .collect()
}

pub(in crate::desktop) fn time_zone_index(value: &str) -> usize {
    qq_farm_core::config::TIME_ZONE_OPTIONS
        .iter()
        .position(|option| option.value == value)
        .unwrap_or(0)
}

pub(in crate::desktop) fn timezone_label(value: &str, language: &str) -> String {
    if language != "en-US" {
        return qq_farm_core::config::TIME_ZONE_OPTIONS
            .iter()
            .find(|option| option.value == value)
            .map(|option| option.label)
            .unwrap_or(value)
            .to_string();
    }
    match value {
        "Asia/Shanghai" => "Beijing / Shanghai (UTC+8)".to_string(),
        "UTC" => "Coordinated Universal Time (UTC)".to_string(),
        "Asia/Hong_Kong" => "Hong Kong".to_string(),
        "Asia/Taipei" => "Taipei".to_string(),
        "Asia/Singapore" => "Singapore".to_string(),
        "Asia/Tokyo" => "Tokyo".to_string(),
        "Asia/Seoul" => "Seoul".to_string(),
        "Europe/London" => "London".to_string(),
        "America/New_York" => "New York".to_string(),
        "America/Los_Angeles" => "Los Angeles".to_string(),
        _ => "Unknown".to_string(),
    }
}

impl FarmDesktop {
    pub(in crate::desktop) fn apply_system_config_snapshot(&mut self, config: serde_json::Value) {
        self.settings.draft["systemConfig"] = config.clone();
        self.page_raw["systemConfig"] = config;
        self.rows = pages::page_data(Page::Settings, self.page_raw.clone()).rows;
    }

    pub(in crate::desktop) fn sync_system_config_inputs(&self, window: &mut Window, cx: &mut App) {
        let config = self.settings.draft.get("systemConfig").cloned().unwrap_or_default();
        let fields = [
            (&self.settings.system_server_url, "serverUrl"),
            (&self.settings.system_client_version, "deviceInfo/clientVersion"),
            (&self.settings.system_software, "deviceInfo/sysSoftware"),
            (&self.settings.system_device_id, "deviceInfo/deviceId"),
            (&self.settings.system_memory, "deviceInfo/memory"),
            (&self.settings.system_network, "deviceInfo/network"),
            (&self.settings.system_user_agent, "deviceInfo/userAgent"),
        ];
        for (input, path) in fields {
            let value = config
                .pointer(&json_pointer(path))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            input.update(cx, |state, cx| state.set_value(value, window, cx));
        }
    }

    pub(in crate::desktop) fn save_system_config(&mut self, cx: &mut Context<Self>) {
        let Some(mut config) = self.settings.draft.get("systemConfig").cloned() else { return };
        self.clear_settings_notice();
        self.error = None;
        if let Some(device_info) = config.get("deviceInfo") {
            let client_version = device_info.get("clientVersion").cloned();
            let os = device_info.get("os").cloned();
            if let Some(object) = config.as_object_mut() {
                if let Some(value) = client_version {
                    object.insert("clientVersion".into(), value);
                }
                if let Some(value) = os {
                    object.insert("os".into(), value);
                }
            }
        }
        match qq_farm_app::admin::set_system_config(config.clone()) {
            Ok(_) => {
                self.apply_system_config_snapshot(config);
                self.show_settings_notice(
                    "运行环境已保存并立即生效",
                    "Runtime saved and applied",
                    false,
                    cx,
                );
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }

    pub(in crate::desktop) fn reset_system_config(&mut self, cx: &mut Context<Self>) {
        self.settings.selected_device_preset = None;
        self.error = None;
        let result = qq_farm_app::admin::reset_system_config();
        let config = result.get("default").cloned().unwrap_or_else(|| {
            serde_json::json!(qq_farm_core::config::get_default_system_config())
        });
        self.apply_system_config_snapshot(config);
        if let Some(handle) = self.window_handle {
            let _ = handle.update(cx, |_, window, cx| self.sync_system_config_inputs(window, cx));
        }
        self.show_settings_notice("运行环境已重置为默认值", "Runtime reset to defaults", false, cx);
    }

    fn apply_device_preset(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(preset) = self
            .page_raw
            .get("devicePresets")
            .and_then(serde_json::Value::as_array)
            .and_then(|presets| {
                presets.iter().find(|preset| {
                    preset.get("id").and_then(serde_json::Value::as_str) == Some(id.as_str())
                })
            })
            .cloned()
        else {
            return;
        };
        let mut config = self.settings.draft.get("systemConfig").cloned().unwrap_or_default();
        let mut device_info = preset.get("deviceInfo").cloned().unwrap_or_default();
        let os = device_info
            .get("os")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Windows")
            .to_string();
        let client_version = device_info
            .get("clientVersion")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let input_values = [
            ("sysSoftware", &self.settings.system_software),
            ("deviceId", &self.settings.system_device_id),
            ("memory", &self.settings.system_memory),
            ("network", &self.settings.system_network),
            ("userAgent", &self.settings.system_user_agent),
        ]
        .into_iter()
        .map(|(key, input)| {
            let value = device_info
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            (input.clone(), value)
        })
        .collect::<Vec<_>>();
        if let Some(object) = device_info.as_object_mut() {
            object.entry("os").or_insert_with(|| serde_json::json!(os));
            object.entry("clientVersion").or_insert_with(|| serde_json::json!(client_version));
        }
        if let Some(object) = config.as_object_mut() {
            object.insert("os".into(), serde_json::json!(os));
            object.insert("clientVersion".into(), serde_json::json!(client_version));
            object.insert("deviceInfo".into(), device_info);
        } else {
            return;
        }
        self.settings.selected_device_preset = Some(id);
        self.set_setting_draft_value("systemConfig", config, cx);
        self.settings
            .system_client_version
            .update(cx, |state, cx| state.set_value(client_version, window, cx));
        for (input, value) in input_values {
            input.update(cx, |state, cx| state.set_value(value, window, cx));
        }
    }

    fn set_device_os(&mut self, os: &'static str, cx: &mut Context<Self>) {
        self.set_setting_path_value("systemConfig/os", serde_json::json!(os), cx);
        self.set_setting_path_value("systemConfig/deviceInfo/os", serde_json::json!(os), cx);
        self.settings.selected_device_preset = None;
    }
}

mod view;
