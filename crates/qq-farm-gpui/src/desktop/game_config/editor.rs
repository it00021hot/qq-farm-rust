use super::super::*;
use super::form;
use gpui_kit::component::WindowExt;

impl FarmDesktop {
    pub(in crate::desktop) fn open_game_config_form(
        &mut self,
        record: Option<serde_json::Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let kind = match self.page_section.as_str() {
            "seeds" => "seed",
            "fruits" => "fruit",
            _ => "item",
        };
        let record = record.unwrap_or(serde_json::Value::Null);
        let id = if kind == "seed" {
            record.get("seedId").or_else(|| record.get("seed_id")).or_else(|| record.get("id"))
        } else {
            record.get("id")
        }
        .map(value_to_action_string)
        .unwrap_or_default();
        let get = |keys: &[&str], fallback: &str| -> String {
            keys.iter()
                .find_map(|key| record.get(*key))
                .map(value_to_action_string)
                .unwrap_or_else(|| fallback.into())
        };
        let get_vue_or = |keys: &[&str], fallback: &str| -> String {
            keys.iter()
                .find_map(|key| record.get(*key).filter(|value| vue_truthy(value)))
                .map(value_to_action_string)
                .unwrap_or_else(|| fallback.into())
        };
        let item_type = get(&["type", "itemType"], if kind == "fruit" { "6" } else { "7" })
            .parse::<i64>()
            .unwrap_or(if kind == "fruit" { 6 } else { 7 });
        self.game_config.kind = kind.into();
        self.game_config.record = record.clone();
        self.game_config.form_error = None;
        self.game_config.plant_id = if kind == "fruit" {
            get(&["plantId", "plant_id"], "").parse::<i64>().ok()
        } else {
            None
        };
        self.game_config.id.update(cx, |state, cx| state.set_value(&id, window, cx));
        self.game_config
            .name
            .update(cx, |state, cx| state.set_value(get(&["name"], ""), window, cx));
        self.game_config.level.update(cx, |state, cx| {
            state.set_value(
                get_vue_or(
                    &["level", "requiredLevel", "landLevelNeed"],
                    if kind == "seed" { "1" } else { "0" },
                ),
                window,
                cx,
            )
        });
        self.game_config.desc.update(cx, |state, cx| {
            state.set_value(get_vue_or(&["desc", "description"], ""), window, cx)
        });
        self.game_config.asset.update(cx, |state, cx| {
            state.set_value(get_vue_or(&["assetName", "harvestAnimation"], ""), window, cx)
        });
        self.game_config.image_path.update(cx, |state, cx| state.set_value("", window, cx));
        self.game_config.extra_fields.iter().for_each(|(field, _)| {
            form::set_field(&self.game_config.extra_fields, field, "", window, cx)
        });
        self.game_config.extra_textareas.keys().for_each(|field| {
            form::set_textarea_field(&self.game_config.extra_textareas, field, "", window, cx)
        });
        for (field, keys, fallback) in [
            (
                "growPhases",
                &["growPhases", "grow_phases"][..],
                "种子:2400;发芽:2400;小叶子:2400;大叶子:2400;开花:2400;成熟:0;",
            ),
            ("seasons", &["seasons"][..], "1"),
            ("fruitCount", &["fruitCount", "harvestCount"][..], "200"),
            ("price", &["price"][..], "0"),
            ("priceId", &["priceId"][..], "0"),
            ("exp", &["exp"][..], "0"),
            ("size", &["size"][..], "0"),
            ("rarity", &["rarity"][..], "0"),
            ("maxCount", &["maxCount", "max_count"][..], "9999"),
            ("interactionType", &["interactionType", "interaction_type"][..], ""),
            ("canUse", &["canUse", "can_use"][..], "0"),
            ("effectDesc", &["effectDesc", "effect_desc"][..], ""),
        ] {
            let value = if field == "fruitCount" && kind == "fruit" {
                "200".to_string()
            } else if field == "growPhases" && kind == "seed" && !record.is_null() {
                // The seed editor starts new rows from its phase template but opens missing legacy
                // phase data as an empty field when editing, matching the Vue drawer.
                get_vue_or(keys, "")
            } else {
                get_vue_or(keys, fallback)
            };
            // growPhases and effectDesc are textareas; writing them through
            // the single-line map silently dropped the value.
            if self.game_config.extra_textareas.contains_key(field) {
                form::set_textarea_field(
                    &self.game_config.extra_textareas,
                    field,
                    &value,
                    window,
                    cx,
                );
            } else {
                form::set_field(&self.game_config.extra_fields, field, &value, window, cx);
            }
        }
        self.game_config.plants =
            qq_farm_app::config::list_plants().as_array().cloned().unwrap_or_default();
        self.sync_game_config_plant_selector(window, cx);
        self.game_config.item_types = qq_farm_app::config::item_types()
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| {
                Some((item.get("value")?.as_i64()?, item.get("label")?.as_str()?.to_string()))
            })
            .collect();
        let item_type_options =
            super::selectors::item_type_form_options(&self.game_config.item_types, item_type);
        self.game_config.item_type_form_selector.update(cx, |selector, cx| {
            selector.set_items(
                gpui_kit::component::select::SearchableVec::new(item_type_options),
                window,
                cx,
            );
            selector.set_selected_value(&item_type, window, cx);
        });
        self.game_config.form_open = true;
        let desktop = cx.weak_entity();
        window.open_sheet(cx, move |sheet, _, cx| {
            if let Some(desktop) = desktop.upgrade() {
                desktop.update(cx, |this, cx| this.render_game_config_form(sheet, cx))
            } else {
                sheet
            }
        });
        cx.notify();
    }

    pub(in crate::desktop) fn set_game_config_extra_field(
        &mut self,
        key: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        form::set_extra_field(
            &self.game_config.extra_fields,
            &self.game_config.extra_textareas,
            key,
            value,
            window,
            cx,
        );
        cx.notify();
    }

    pub(in crate::desktop) fn choose_game_config_image(&mut self, cx: &mut Context<Self>) {
        let language = self.preferences.language.clone();
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(
                crate::i18n::text(
                    &language,
                    "选择游戏配置图片",
                    "Choose a game configuration image",
                )
                .into(),
            ),
        });
        cx.spawn(async move |this, cx| match receiver.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.first() {
                    let path = path.to_string_lossy().to_string();
                    let supported = ["png", "jpg", "jpeg", "webp"]
                        .iter()
                        .any(|extension| path.to_lowercase().ends_with(&format!(".{extension}")));
                    let _ = this.update(cx, |this, cx| {
                        if supported {
                            let Some(window_handle) = this.window_handle else {
                                this.game_config.form_error = Some(
                                    crate::i18n::text(
                                        &language,
                                        "当前窗口不可用。",
                                        "The window is unavailable.",
                                    )
                                    .into(),
                                );
                                cx.notify();
                                return;
                            };
                            let _ = window_handle.update(cx, |_, window, cx| {
                                this.game_config
                                    .image_path
                                    .update(cx, |state, cx| state.set_value(&path, window, cx));
                            });
                        } else {
                            this.game_config.form_error = Some(
                                crate::i18n::text(
                                    &language,
                                    "图片格式仅支持 PNG、JPG 和 WebP。",
                                    "Supported image formats are PNG, JPG, and WebP.",
                                )
                                .into(),
                            );
                            cx.notify();
                        }
                    });
                }
            }
            Ok(Ok(None)) => {}
            Ok(Err(error)) => {
                let _ = this.update(cx, |this, cx| {
                    this.game_config.form_error = Some(if language == "en-US" {
                        format!("Could not open the file picker: {error}")
                    } else {
                        format!("打开文件选择器失败：{error}")
                    });
                    cx.notify();
                });
            }
            Err(error) => {
                let _ = this.update(cx, |this, cx| {
                    this.game_config.form_error = Some(if language == "en-US" {
                        format!("File picker ended: {error}")
                    } else {
                        format!("文件选择已结束：{error}")
                    });
                    cx.notify();
                });
            }
        })
        .detach();
    }
}

fn vue_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(value) => value.as_f64().is_some_and(|number| number != 0.0),
        serde_json::Value::String(value) => !value.is_empty(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => true,
    }
}
