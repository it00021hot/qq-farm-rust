use crate::desktop::settings::{
    bag_fallback_options, offline_provider_options, planting_strategy_options, BagFallbackSelect,
    OfflineProviderSelect, PlantingStrategySelect, PreferredSeedSelect, StrategyPreview,
};
use crate::desktop::FarmDesktop;
use gpui_kit::component::input::InputState;
use gpui_kit::component::select::{SearchableVec, SelectState};
use gpui_kit::component::time_field::{HourCycle, TimeFieldEvent, TimeFieldState, TimePrecision};
use gpui_kit::component::IndexPath;
use gpui_kit::{AppContext as _, Context, Entity, Window};
use std::path::PathBuf;

pub(crate) struct SettingsState {
    pub(crate) qq_bot_app_id: Entity<InputState>,
    pub(crate) qq_bot_secret: Entity<InputState>,
    pub(crate) offline_endpoint: Entity<InputState>,
    pub(crate) offline_token: Entity<InputState>,
    pub(crate) offline_secret: Entity<InputState>,
    pub(in crate::desktop) offline_provider: Entity<OfflineProviderSelect>,
    pub(in crate::desktop) planting_strategy: Entity<PlantingStrategySelect>,
    pub(in crate::desktop) bag_fallback: Entity<BagFallbackSelect>,
    pub(in crate::desktop) preferred_seed: Entity<PreferredSeedSelect>,
    pub(crate) qq_bot_session_id: Option<String>,
    pub(crate) qq_bot_bind_loading: bool,
    pub(crate) offline_test_loading: bool,
    pub(crate) strategy_saving: bool,
    pub(crate) automation_saving: bool,
    pub(crate) qq_bot_poll_generation: u64,
    pub(crate) qq_bot_qr_path: Option<PathBuf>,
    pub(crate) qq_bot_status: Option<String>,
    pub(crate) save_notice: Option<String>,
    pub(crate) save_notice_warning: bool,
    pub(crate) save_notice_generation: u64,
    pub(crate) draft: serde_json::Value,
    pub(crate) strategy_preview: Option<StrategyPreview>,
    pub(crate) strategy_preview_generation: u64,
    pub(crate) edit_path: Option<String>,
    pub(crate) edit_value: Entity<InputState>,
    pub(crate) quiet_hours_start: Entity<TimeFieldState>,
    pub(crate) quiet_hours_end: Entity<TimeFieldState>,
    pub(crate) system_server_url: Entity<InputState>,
    pub(crate) system_client_version: Entity<InputState>,
    pub(crate) system_software: Entity<InputState>,
    pub(crate) system_device_id: Entity<InputState>,
    pub(crate) system_memory: Entity<InputState>,
    pub(crate) system_network: Entity<InputState>,
    pub(crate) system_user_agent: Entity<InputState>,
    pub(in crate::desktop) time_zone: Entity<super::system_config::TimeZoneSelect>,
    pub(crate) selected_device_preset: Option<String>,
    /// 各整数设置路径的 NumberInput 状态（框体输入 + 内嵌步进，回写 draft）
    pub(in crate::desktop) number_inputs: std::collections::HashMap<String, Entity<InputState>>,
}

/// 全部整数设置路径（对齐各 tab 的 append_integer_setting 调用）：
/// (json 路径, 默认值, 最小, 最大)
pub(in crate::desktop) const NUMBER_PATHS: &[(&str, i64, i64, i64)] = &[
    ("intervals/farmMin", 30, 1, 99_999),
    ("intervals/farmMax", 60, 1, 99_999),
    ("intervals/helpMin", 20, 1, 99_999),
    ("intervals/helpMax", 25, 1, 99_999),
    ("intervals/stealMin", 60, 1, 99_999),
    ("intervals/stealMax", 90, 1, 99_999),
    ("plantDelaySeconds", 0, 0, 60),
    ("stealDelaySeconds", 0, 0, 60),
    ("fertilizerBuy/organicCount", 1, 1, 10_000),
    ("fertilizerBuy/organicThresholdHours", 1, 1, 9_999),
    ("fertilizerBuy/normalCount", 1, 1, 10_000),
    ("fertilizerBuy/normalThresholdHours", 1, 1, 9_999),
    ("autoAcceptFriendMinLevel", 0, 0, 9_999),
    ("autoAcceptHarvestStealHarvest", 0, 0, 9_999),
    ("autoAcceptHarvestStealSteal", 0, 0, 9_999),
];

impl SettingsState {
    pub(crate) fn section_save_pending(&self, section: &str) -> bool {
        match section {
            "strategy" => self.strategy_saving,
            "automation" => self.automation_saving,
            _ => false,
        }
    }

    pub(crate) fn set_section_save_pending(&mut self, section: &str, pending: bool) {
        match section {
            "strategy" => self.strategy_saving = pending,
            "automation" => self.automation_saving = pending,
            _ => {}
        }
    }

    pub(in crate::desktop) fn new(
        language: &str,
        window: &mut Window,
        cx: &mut Context<FarmDesktop>,
    ) -> Self {
        let current_reminder = qq_farm_app::settings::offline_reminder_view(Some("local"));
        let credentials = current_reminder.get("qqBot").cloned().unwrap_or_default();
        let current_app_id =
            credentials.get("appId").and_then(serde_json::Value::as_str).unwrap_or("");
        let current_secret =
            credentials.get("clientSecret").and_then(serde_json::Value::as_str).unwrap_or("");
        let qq_bot_app_id = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("QQ Bot AppID");
            state.set_value(current_app_id, window, cx);
            state
        });
        let qq_bot_secret = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("QQ Bot AppSecret");
            state.set_value(current_secret, window, cx);
            state
        });
        let endpoint =
            current_reminder.get("endpoint").and_then(serde_json::Value::as_str).unwrap_or("");
        let token = current_reminder.get("token").and_then(serde_json::Value::as_str).unwrap_or("");
        let secret =
            current_reminder.get("secret").and_then(serde_json::Value::as_str).unwrap_or("");
        let offline_endpoint = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(crate::desktop::settings_text(
                language,
                "https://oapi.dingtalk.com/robot/send?access_token=…（与 Access Token 二选一）",
            ));
            state.set_value(endpoint, window, cx);
            state
        });
        let offline_token = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .placeholder(crate::desktop::settings_text(language, "与 Webhook 地址二选一"));
            state.set_value(token, window, cx);
            state
        });
        let offline_secret = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(crate::desktop::settings_text(
                language,
                "可选；安全设置选择加签时填写 SEC 开头密钥",
            ));
            state.set_value(secret, window, cx);
            state
        });
        let current_provider =
            current_reminder.get("provider").and_then(serde_json::Value::as_str).unwrap_or("none");
        let provider_options = offline_provider_options(language);
        let offline_provider = cx.new(|cx| {
            gpui_kit::component::select::SelectState::new(
                gpui_kit::component::select::SearchableVec::new(provider_options),
                Some(crate::desktop::settings::provider_index(current_provider)),
                window,
                cx,
            )
        });
        cx.subscribe(&offline_provider, |this, _, event, cx| {
            if let gpui_kit::component::select::SelectEvent::Confirm(Some(provider)) = event {
                this.set_setting_path_value(
                    "offlineReminder/provider",
                    serde_json::json!(*provider),
                    cx,
                );
            }
        })
        .detach();
        let planting_strategy = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(planting_strategy_options(language)),
                Some(IndexPath::new(2)),
                window,
                cx,
            )
        });
        cx.subscribe(&planting_strategy, |this, _, event, cx| {
            if let gpui_kit::component::select::SelectEvent::Confirm(Some(strategy)) = event {
                this.set_planting_strategy(strategy, cx);
            }
        })
        .detach();
        let fallback_options = bag_fallback_options(language);
        let bag_fallback = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(fallback_options),
                Some(IndexPath::new(5)),
                window,
                cx,
            )
        });
        cx.subscribe(&bag_fallback, |this, _, event, cx| {
            if let gpui_kit::component::select::SelectEvent::Confirm(Some(strategy)) = event {
                this.set_bag_seed_fallback_strategy(strategy, cx);
            }
        })
        .detach();
        let preferred_seed = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(crate::desktop::settings::preferred_seed_options(
                    &serde_json::Value::Null,
                    language,
                )),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
            .searchable(true)
        });
        cx.subscribe(&preferred_seed, |this, _, event, cx| {
            if let gpui_kit::component::select::SelectEvent::Confirm(Some(seed_id)) = event {
                this.set_preferred_seed(*seed_id, cx);
            }
        })
        .detach();
        let quiet_hours_start = cx.new(|cx| {
            TimeFieldState::new(window, cx)
                .precision(TimePrecision::Minute)
                .hour_cycle(HourCycle::H23)
        });
        cx.subscribe(&quiet_hours_start, |this, _, event, cx| {
            let TimeFieldEvent::Change(time) = event;
            this.set_setting_path_value(
                "friendQuietHours/start",
                serde_json::json!(time.format("%H:%M").to_string()),
                cx,
            );
        })
        .detach();
        let quiet_hours_end = cx.new(|cx| {
            TimeFieldState::new(window, cx)
                .precision(TimePrecision::Minute)
                .hour_cycle(HourCycle::H23)
        });
        cx.subscribe(&quiet_hours_end, |this, _, event, cx| {
            let TimeFieldEvent::Change(time) = event;
            this.set_setting_path_value(
                "friendQuietHours/end",
                serde_json::json!(time.format("%H:%M").to_string()),
                cx,
            );
        })
        .detach();
        let system_server_url = cx.new(|cx| InputState::new(window, cx).placeholder("wss://..."));
        let system_client_version = cx.new(|cx| InputState::new(window, cx));
        let system_software = cx.new(|cx| InputState::new(window, cx));
        let system_device_id = cx.new(|cx| InputState::new(window, cx));
        let system_memory = cx.new(|cx| InputState::new(window, cx));
        let system_network = cx.new(|cx| InputState::new(window, cx));
        let system_user_agent = cx.new(|cx| InputState::new(window, cx));
        let time_zone = cx.new(|cx| {
            super::system_config::TimeZoneSelect::new(
                gpui_kit::component::select::SearchableVec::new(Vec::new()),
                None,
                window,
                cx,
            )
        });
        cx.subscribe(&time_zone, |this, _, event, cx| {
            let gpui_kit::component::select::SelectEvent::Confirm(selected) = event;
            if let Some(value) = selected {
                this.set_setting_path_value("systemConfig/timeZone", serde_json::json!(value), cx);
            }
        })
        .detach();
        for (input, path) in [
            (&system_server_url, "systemConfig/serverUrl"),
            (&system_client_version, "systemConfig/deviceInfo/clientVersion"),
            (&system_software, "systemConfig/deviceInfo/sysSoftware"),
            (&system_device_id, "systemConfig/deviceInfo/deviceId"),
            (&system_memory, "systemConfig/deviceInfo/memory"),
            (&system_network, "systemConfig/deviceInfo/network"),
            (&system_user_agent, "systemConfig/deviceInfo/userAgent"),
        ] {
            cx.subscribe(input, move |this, input, event, cx| {
                if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                    let value = input.read(cx).value().to_string();
                    this.set_setting_path_value(path, serde_json::json!(value), cx);
                }
            })
            .detach();
        }
        // 整数设置输入框（NumberInput 框体 + 内嵌 − +，对齐 Tauri NInputNumber）：
        // Change 事件回写 draft；draft 载入后由 sync_setting_selectors 反向同步。
        let mut number_inputs = std::collections::HashMap::new();
        for (path, default, minimum, maximum) in NUMBER_PATHS {
            let state = cx.new(|cx| {
                let mut state = InputState::new(window, cx).placeholder(default.to_string());
                state.set_value(default.to_string(), window, cx);
                state.set_min(Some(*minimum as f64), window, cx);
                state.set_max(Some(*maximum as f64), window, cx);
                state
            });
            cx.subscribe(&state, move |this, input, event, cx| {
                if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                    let raw = input.read(cx).value().trim().to_string();
                    if let Ok(value) = raw.parse::<i64>() {
                        this.set_setting_path_value(path, serde_json::json!(value), cx);
                    }
                }
            })
            .detach();
            number_inputs.insert(path.to_string(), state);
        }
        Self {
            qq_bot_app_id,
            number_inputs,
            qq_bot_secret,
            offline_endpoint,
            offline_token,
            offline_secret,
            offline_provider,
            planting_strategy,
            bag_fallback,
            preferred_seed,
            qq_bot_session_id: None,
            qq_bot_bind_loading: false,
            offline_test_loading: false,
            strategy_saving: false,
            automation_saving: false,
            qq_bot_poll_generation: 0,
            qq_bot_qr_path: None,
            qq_bot_status: None,
            save_notice: None,
            save_notice_warning: false,
            save_notice_generation: 0,
            draft: serde_json::Value::Null,
            strategy_preview: None,
            strategy_preview_generation: 0,
            edit_path: None,
            edit_value: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(crate::desktop::settings_text(language, "设置值"))
            }),
            quiet_hours_start,
            quiet_hours_end,
            system_server_url,
            system_client_version,
            system_software,
            system_device_id,
            system_memory,
            system_network,
            system_user_agent,
            time_zone,
            selected_device_preset: None,
        }
    }
}
