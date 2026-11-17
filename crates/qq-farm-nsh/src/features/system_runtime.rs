use nsh_core::prelude::*;
use serde_json::{json, Value};

use crate::features::confirm;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, field, notice, ButtonVariant};

const PLATFORMS: &[(&str, &str)] = &[("qq", "settings.platformQq"), ("wx", "settings.platformWx")];

const SYSTEMS: &[&str] = &["Windows", "iOS", "Android"];

const ZONES: &[(&str, &str)] = &[
    ("Asia/Shanghai", "settings.zoneShanghai"),
    ("UTC", "settings.zoneUtc"),
    ("Asia/Hong_Kong", "settings.zoneHongKong"),
    ("Asia/Taipei", "settings.zoneTaipei"),
    ("Asia/Singapore", "settings.zoneSingapore"),
    ("Asia/Tokyo", "settings.zoneTokyo"),
    ("Asia/Seoul", "settings.zoneSeoul"),
    ("Europe/London", "settings.zoneLondon"),
    ("America/New_York", "settings.zoneNewYork"),
    ("America/Los_Angeles", "settings.zoneLosAngeles"),
];

#[derive(Clone, PartialEq, Eq)]
struct PresetLine {
    id: String,
    name: String,
    device: Value,
}

pub(crate) fn section() -> impl IntoElement {
    let draft = signal(None::<Value>);
    let presets = signal(Vec::<PresetLine>::new());
    let selected = signal(None::<String>);
    let pending = signal(false);
    let failure = signal(None::<String>);
    spawn(async move {
        let loaded = session::session()
            .runtime
            .spawn_blocking(|| {
                (qq_farm_app::admin::get_system_config(), qq_farm_app::admin::device_presets())
            })
            .await;
        match loaded {
            Ok((config, preset_value)) => {
                draft.set(Some(config));
                presets.set(preset_lines(&preset_value));
            }
            Err(error) => failure.set(Some(error.to_string())),
        }
    });
    view()
        .gap_3()
        .child(
            text(move || text_key("settings.runtime")).text_sm().font_weight(FontWeight::SEMIBOLD),
        )
        .child(text(move || text_key("settings.runtimeHint")).text_sm())
        .child(crate::ui::failure(move || {
            failure.get().map(SharedString::from).unwrap_or_default()
        }))
        .child(when(
            move || draft.get().is_none() && failure.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || draft.get().is_some(),
            move || runtime_form(draft, presets, selected, pending, failure),
        ))
}

fn runtime_form(
    draft: Signal<Option<Value>>,
    presets: Signal<Vec<PresetLine>>,
    selected: Signal<Option<String>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(when(
            move || !presets.get().is_empty(),
            move || {
                view().gap_2().child(text(move || text_key("settings.presets")).text_sm()).child(
                    view().flex_row().flex_wrap().gap_2().child(each(
                        move || presets.get(),
                        |preset: &PresetLine| preset.id.clone(),
                        move |preset, _| {
                            let line = preset.get_untracked();
                            let id = line.id.clone();
                            let selected_id = id.clone();
                            let name = line.name.clone();
                            let device = line.device.clone();
                            button(move || SharedString::from(name.clone()), ButtonVariant::Quiet)
                                .selected(move || {
                                    selected.get().as_deref() == Some(selected_id.as_str())
                                })
                                .disabled(move || pending.get())
                                .on_press(move || {
                                    apply_selected_preset(draft, selected, &id, &device)
                                })
                        },
                    )),
                )
            },
        ))
        .child(text_row("settings.server", &["serverUrl"], draft, pending))
        .child(choice_row(
            "settings.zone",
            ZONES,
            "/timeZone",
            "Asia/Shanghai",
            draft,
            pending,
            selected,
        ))
        .child(choice_row(
            "settings.platform",
            PLATFORMS,
            "/platform",
            "qq",
            draft,
            pending,
            selected,
        ))
        .child(system_row(draft, pending, selected))
        .child(text_row("settings.clientVersion", &["deviceInfo", "clientVersion"], draft, pending))
        .child(text_row("settings.wxClientVersion", &["wxClientVersion"], draft, pending))
        .child(text_row("settings.software", &["deviceInfo", "sysSoftware"], draft, pending))
        .child(text_row("settings.deviceId", &["deviceInfo", "deviceId"], draft, pending))
        .child(text_row("settings.memory", &["deviceInfo", "memory"], draft, pending))
        .child(text_row("settings.network", &["deviceInfo", "network"], draft, pending))
        .child(text_row("settings.userAgent", &["deviceInfo", "userAgent"], draft, pending))
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(
                    button(move || text_key("settings.resetRuntime"), ButtonVariant::Secondary)
                        .disabled(move || pending.get())
                        .on_press(move || reset_runtime(draft, selected, pending, failure)),
                )
                .child(
                    button(move || text_key("settings.saveRuntime"), ButtonVariant::Primary)
                        .disabled(move || pending.get() || draft.get().is_none())
                        .on_press(move || save_runtime(draft, pending, failure)),
                ),
        )
}

fn text_row(
    label: &'static str,
    path: &'static [&'static str],
    draft: Signal<Option<Value>>,
    pending: Signal<bool>,
) -> View {
    view().gap_1().child(text(move || text_key(label)).text_sm()).child(field(
        move || read_path(draft, path),
        move || text_key(label).to_string(),
        move |text: &str| {
            if pending.get_untracked() {
                return;
            }
            write_path(draft, path, text);
        },
    ))
}

fn choice_row(
    label: &'static str,
    choices: &'static [(&str, &str)],
    pointer: &'static str,
    fallback: &'static str,
    draft: Signal<Option<Value>>,
    pending: Signal<bool>,
    selected: Signal<Option<String>>,
) -> View {
    view().gap_2().child(text(move || text_key(label)).text_sm()).child(
        view().flex_row().flex_wrap().gap_2().child(each(
            || choices.to_vec(),
            |choice: &(&str, &str)| choice.0.to_owned(),
            move |choice, _| {
                let (id, key) = choice.get_untracked();
                button(move || text_key(key), ButtonVariant::Quiet)
                    .selected(move || current_text(draft, pointer, fallback) == id)
                    .disabled(move || pending.get())
                    .on_press(move || {
                        write_pointer(draft, pointer, id);
                        if pointer == "/platform" {
                            selected.set(None);
                        }
                    })
            },
        )),
    )
}

fn system_row(
    draft: Signal<Option<Value>>,
    pending: Signal<bool>,
    selected: Signal<Option<String>>,
) -> View {
    view().gap_2().child(text(move || text_key("settings.os")).text_sm()).child(
        view().flex_row().flex_wrap().gap_2().child(each(
            || SYSTEMS.to_vec(),
            |system: &&str| (*system).to_owned(),
            move |system, _| {
                let system = system.get_untracked();
                button(move || SharedString::from(system), ButtonVariant::Quiet)
                    .selected(move || current_text(draft, "/os", "Windows") == system)
                    .disabled(move || pending.get())
                    .on_press(move || {
                        choose_os(draft, system);
                        selected.set(None);
                    })
            },
        )),
    )
}

fn apply_selected_preset(
    draft: Signal<Option<Value>>,
    selected: Signal<Option<String>>,
    id: &str,
    device: &Value,
) {
    let Some(mut config) = draft.get_untracked() else { return };
    apply_preset(&mut config, device);
    draft.set(Some(config));
    selected.set(Some(id.to_owned()));
}

fn save_runtime(
    draft: Signal<Option<Value>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
) {
    if pending.get_untracked() {
        return;
    }
    let Some(config) = draft.get_untracked() else { return };
    let body = save_body(&config);
    pending.set(true);
    failure.set(None);
    spawn(async move {
        let saved = session::session()
            .runtime
            .spawn_blocking(move || qq_farm_app::admin::set_system_config(body))
            .await
            .map_err(|error| error.to_string())
            .and_then(|saved| saved.map_err(|error| error.to_string()));
        match saved {
            Ok(config) => draft.set(Some(config)),
            Err(message) => failure.set(Some(message)),
        }
        pending.set(false);
    });
}

fn reset_runtime(
    draft: Signal<Option<Value>>,
    selected: Signal<Option<String>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
) {
    if pending.get_untracked() {
        return;
    }
    confirm::confirm(
        "settings.resetRuntime.title",
        "settings.resetRuntime.body",
        "reset",
        "settings.resetRuntime",
        move |accepted| {
            if !accepted {
                return;
            }
            pending.set(true);
            failure.set(None);
            spawn(async move {
                let reset = session::session()
                    .runtime
                    .spawn_blocking(qq_farm_app::admin::reset_system_config)
                    .await
                    .map_err(|error| error.to_string());
                match reset {
                    Ok(value) => {
                        let config = value.get("default").cloned().unwrap_or(Value::Null);
                        draft.set(Some(config));
                        selected.set(None);
                    }
                    Err(message) => failure.set(Some(message)),
                }
                pending.set(false);
            });
        },
    );
}

fn preset_lines(value: &Value) -> Vec<PresetLine> {
    value.as_array().map(|rows| rows.iter().filter_map(preset_line).collect()).unwrap_or_default()
}

fn preset_line(value: &Value) -> Option<PresetLine> {
    let id = value.get("id").and_then(Value::as_str).filter(|id| !id.is_empty())?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or(id);
    Some(PresetLine {
        id: id.to_owned(),
        name: if name.is_empty() { id.to_owned() } else { name.to_owned() },
        device: value.get("deviceInfo").cloned().unwrap_or_else(|| json!({})),
    })
}

fn read_path(draft: Signal<Option<Value>>, path: &[&str]) -> String {
    let pointer = json_pointer(path);
    draft
        .get()
        .and_then(|value| value.pointer(&pointer).and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default()
}

fn current_text(draft: Signal<Option<Value>>, pointer: &str, fallback: &str) -> String {
    draft
        .get()
        .and_then(|value| value.pointer(pointer).and_then(Value::as_str).map(str::to_owned))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn write_path(draft: Signal<Option<Value>>, path: &[&str], text: &str) {
    let Some(mut config) = draft.get_untracked() else { return };
    put_string(&mut config, path, text);
    draft.set(Some(config));
}

fn write_pointer(draft: Signal<Option<Value>>, pointer: &str, text: &str) {
    let path = pointer.trim_start_matches('/').split('/').filter(|part| !part.is_empty());
    let owned = path.map(str::to_owned).collect::<Vec<_>>();
    let refs = owned.iter().map(String::as_str).collect::<Vec<_>>();
    write_path(draft, &refs, text);
}

fn choose_os(draft: Signal<Option<Value>>, os: &str) {
    write_pointer(draft, "/os", os);
    write_pointer(draft, "/deviceInfo/os", os);
}

fn apply_preset(config: &mut Value, device: &Value) {
    let os = device.get("os").and_then(Value::as_str).unwrap_or("Windows").to_owned();
    let version = device.get("clientVersion").and_then(Value::as_str).unwrap_or("").to_owned();
    let mut device = device.clone();
    if let Some(object) = device.as_object_mut() {
        object.insert("os".into(), json!(os.clone()));
        object.insert("clientVersion".into(), json!(version.clone()));
    }
    let Some(object) = config.as_object_mut() else { return };
    object.insert("os".into(), json!(os));
    object.insert("clientVersion".into(), json!(version));
    object.insert("deviceInfo".into(), device);
}

fn save_body(config: &Value) -> Value {
    let mut config = config.clone();
    let version = config.pointer("/deviceInfo/clientVersion").cloned();
    let os = config.pointer("/deviceInfo/os").cloned();
    let Some(object) = config.as_object_mut() else { return config };
    if let Some(version) = version {
        object.insert("clientVersion".into(), version);
    }
    if let Some(os) = os {
        object.insert("os".into(), os);
    }
    config
}

fn put_string(value: &mut Value, path: &[&str], text: &str) {
    let Some((key, rest)) = path.split_first() else { return };
    let Some(object) = value.as_object_mut() else { return };
    if rest.is_empty() {
        object.insert((*key).to_owned(), Value::String(text.to_owned()));
        return;
    }
    let child = object.entry((*key).to_owned()).or_insert_with(|| json!({}));
    if !child.is_object() {
        *child = json!({});
    }
    put_string(child, rest, text);
}

fn json_pointer(path: &[&str]) -> String {
    format!("/{}", path.join("/"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{apply_preset, preset_lines, save_body};

    #[test]
    fn preset_copies_device_version_and_os() {
        let mut config = json!({
            "serverUrl": "wss://old.example/ws",
            "os": "Windows",
            "clientVersion": "old",
            "deviceInfo": { "os": "Windows", "clientVersion": "old", "deviceId": "OLD" }
        });
        let presets = preset_lines(&json!([
            { "id": "", "name": "drop" },
            {
                "id": "iphone",
                "name": "",
                "deviceInfo": {
                    "os": "iOS",
                    "clientVersion": "ios-version",
                    "deviceId": "iPhone-test",
                    "userAgent": "iPhone Agent"
                }
            }
        ]));
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].name, "iphone");
        apply_preset(&mut config, &presets[0].device);
        assert_eq!(config["os"], "iOS");
        assert_eq!(config["clientVersion"], "ios-version");
        assert_eq!(config["deviceInfo"]["deviceId"], "iPhone-test");
        assert_eq!(config["deviceInfo"]["userAgent"], "iPhone Agent");
        assert_eq!(config["serverUrl"], "wss://old.example/ws");
        let saved = save_body(&config);
        assert_eq!(saved["clientVersion"], "ios-version");
        assert_eq!(saved["os"], "iOS");
    }

    #[test]
    fn save_keeps_root_when_device_fields_are_missing() {
        let saved = save_body(&json!({ "clientVersion": "kept", "os": "Windows" }));
        assert_eq!(saved["clientVersion"], "kept");
        assert_eq!(saved["os"], "Windows");
        let mut empty = json!([]);
        apply_preset(&mut empty, &json!({ "os": "iOS" }));
        assert!(empty.as_array().is_some());
    }
}
