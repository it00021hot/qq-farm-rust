use super::super::*;

pub(crate) fn json_pointer(path: &str) -> String {
    format!(
        "/{}",
        path.split('/')
            .map(|part| part.replace('~', "~0").replace('/', "~1"))
            .collect::<Vec<_>>()
            .join("/")
    )
}

fn setting_integer_bounds(path: &str) -> (i64, Option<i64>) {
    match path {
        "intervals/farmMin" | "intervals/farmMax" | "intervals/helpMin" | "intervals/helpMax"
        | "intervals/stealMin" | "intervals/stealMax" => (1, None),
        "plantDelaySeconds" | "stealDelaySeconds" => (0, Some(60)),
        "automation/fertilizer_smart_seconds" => (30, Some(3600)),
        "fertilizerBuyOrganicCount" | "fertilizerBuyNormalCount" => (1, Some(10_000)),
        "fertilizerBuyOrganicThresholdHours" | "fertilizerBuyNormalThresholdHours" => {
            (1, Some(990))
        }
        "autoAcceptFriendMinLevel" => (0, Some(200)),
        "autoAcceptHarvestStealHarvest" => (0, Some(9999)),
        "autoAcceptHarvestStealSteal" => (1, Some(9999)),
        _ => (0, None),
    }
}

fn normalized_time_input(value: &str) -> Option<String> {
    let (hours, minutes) = value.trim().split_once(':')?;
    if minutes.contains(':') || hours.is_empty() || minutes.is_empty() {
        return None;
    }
    let hours = hours.parse::<u8>().ok()?;
    let minutes = minutes.parse::<u8>().ok()?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    Some(format!("{hours:02}:{minutes:02}"))
}

impl FarmDesktop {
    pub(in crate::desktop) fn set_setting_draft_value(
        &mut self,
        key: &str,
        value: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        self.clear_settings_notice();
        self.settings.draft[key] = value.clone();
        self.page_raw[key] = value;
        self.rows = pages::page_data(Page::Settings, self.page_raw.clone()).rows;
        cx.notify();
    }

    pub(in crate::desktop) fn set_setting_path_value(
        &mut self,
        path: &str,
        value: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        self.clear_settings_notice();
        let pointer = json_pointer(path);
        let Some(target) = self.settings.draft.pointer_mut(&pointer) else { return };
        *target = value.clone();
        if let Some(target) = self.page_raw.pointer_mut(&pointer) {
            *target = value;
        }
        self.rows = pages::page_data(Page::Settings, self.page_raw.clone()).rows;
        cx.notify();
    }

    pub(in crate::desktop) fn set_setting_array_contains(
        &mut self,
        path: &str,
        item: &str,
        checked: bool,
        cx: &mut Context<Self>,
    ) {
        self.clear_settings_notice();
        let pointer = json_pointer(path);
        let mut values = self
            .settings
            .draft
            .pointer(&pointer)
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        values.retain(|value| value.as_str() != Some(item));
        if checked {
            values.push(serde_json::Value::String(item.to_string()));
        }
        self.set_setting_path_value(path, serde_json::Value::Array(values), cx);
    }
    pub(in crate::desktop) fn adjust_setting(
        &mut self,
        path: &str,
        delta: i64,
        cx: &mut Context<Self>,
    ) {
        let pointer = json_pointer(path);
        if let Some(value) =
            self.settings.draft.pointer(&pointer).and_then(serde_json::Value::as_i64)
        {
            *self.settings.draft.pointer_mut(&pointer).expect("setting pointer exists") =
                serde_json::json!(value.saturating_add(delta).max(0));
            if let Some(value) = self.page_raw.pointer_mut(&pointer) {
                *value = serde_json::json!(self
                    .settings
                    .draft
                    .pointer(&pointer)
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or_default());
            }
            cx.notify();
        }
    }

    pub(in crate::desktop) fn open_setting_edit(
        &mut self,
        path: String,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.error = None;
        self.settings.edit_value.update(cx, |state, cx| state.set_value(&value, window, cx));
        self.settings.edit_path = Some(path);
        cx.notify();
    }

    pub(in crate::desktop) fn save_setting_edit(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.settings.edit_path.clone() else { return };
        let pointer = json_pointer(&path);
        let value = self.settings.edit_value.read(cx).value().to_string();
        let Some(current) = self.settings.draft.pointer(&pointer).cloned() else {
            self.error = Some(
                crate::i18n::text(
                    &self.preferences.language,
                    "无法更新此设置项。",
                    "This setting could not be updated.",
                )
                .into(),
            );
            cx.notify();
            return;
        };
        let next = if current.as_i64().is_some() {
            match value.trim().parse::<i64>() {
                Ok(number) => {
                    let (minimum, maximum) = setting_integer_bounds(&path);
                    if number < minimum || maximum.is_some_and(|maximum| number > maximum) {
                        self.error = Some(if let Some(maximum) = maximum {
                            crate::i18n::text(
                                &self.preferences.language,
                                &format!("请输入 {minimum} 到 {maximum} 之间的整数。"),
                                &format!("Enter an integer between {minimum} and {maximum}."),
                            )
                            .into()
                        } else {
                            crate::i18n::text(
                                &self.preferences.language,
                                &format!("请输入不小于 {minimum} 的整数。"),
                                &format!("Enter an integer of at least {minimum}."),
                            )
                            .into()
                        });
                        cx.notify();
                        return;
                    }
                    serde_json::json!(number)
                }
                Err(_) => {
                    self.error = Some(
                        crate::i18n::text(
                            &self.preferences.language,
                            "请输入有效整数。",
                            "Enter a valid integer.",
                        )
                        .into(),
                    );
                    cx.notify();
                    return;
                }
            }
        } else if current.as_f64().is_some() {
            match value.trim().parse::<f64>() {
                Ok(number) if number.is_finite() => serde_json::json!(number),
                _ => {
                    self.error = Some(
                        crate::i18n::text(
                            &self.preferences.language,
                            "请输入有效数字。",
                            "Enter a valid number.",
                        )
                        .into(),
                    );
                    cx.notify();
                    return;
                }
            }
        } else if matches!(path.as_str(), "friendQuietHours/start" | "friendQuietHours/end") {
            let Some(time) = normalized_time_input(&value) else {
                self.error = Some(
                    crate::i18n::text(
                        &self.preferences.language,
                        "请输入有效时间（00:00–23:59）。",
                        "Enter a valid time between 00:00 and 23:59.",
                    )
                    .into(),
                );
                cx.notify();
                return;
            };
            serde_json::Value::String(time)
        } else {
            serde_json::Value::String(value)
        };
        self.error = None;
        if let Some(target) = self.settings.draft.pointer_mut(&pointer) {
            *target = next.clone();
        }
        if let Some(target) = self.page_raw.pointer_mut(&pointer) {
            *target = next;
        }
        self.settings.edit_path = None;
        self.rows = pages::page_data(Page::Settings, self.page_raw.clone()).rows;
        cx.notify();
    }

    pub(in crate::desktop) fn toggle_setting(&mut self, path: &str, cx: &mut Context<Self>) {
        let pointer = json_pointer(path);
        if let Some(value) =
            self.settings.draft.pointer(&pointer).and_then(serde_json::Value::as_bool)
        {
            *self.settings.draft.pointer_mut(&pointer).expect("setting pointer exists") =
                serde_json::Value::Bool(!value);
            *self.page_raw.pointer_mut(&pointer).expect("setting pointer exists") =
                serde_json::Value::Bool(!value);
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{normalized_time_input, setting_integer_bounds};

    #[test]
    fn editable_settings_keep_vue_numeric_limits() {
        assert_eq!(setting_integer_bounds("intervals/farmMin"), (1, None));
        assert_eq!(setting_integer_bounds("plantDelaySeconds"), (0, Some(60)));
        assert_eq!(setting_integer_bounds("fertilizerBuyNormalCount"), (1, Some(10_000)));
        assert_eq!(setting_integer_bounds("autoAcceptFriendMinLevel"), (0, Some(200)));
    }

    #[test]
    fn quiet_hours_time_edit_is_normalized_and_rejects_out_of_range_values() {
        assert_eq!(normalized_time_input("9:5"), Some("09:05".into()));
        assert_eq!(normalized_time_input("23:59"), Some("23:59".into()));
        assert_eq!(normalized_time_input("24:00"), None);
        assert_eq!(normalized_time_input("09:60"), None);
        assert_eq!(normalized_time_input("09"), None);
    }
}
