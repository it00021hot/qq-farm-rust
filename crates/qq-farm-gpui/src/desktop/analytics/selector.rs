use gpui_kit::component::combobox::ComboboxState;
use gpui_kit::component::searchable_list::SearchableListItem;
use gpui_kit::component::select::SearchableVec;
use gpui_kit::component::IndexPath;
use gpui_kit::{App, SharedString, Window};

use super::super::FarmDesktop;

#[derive(Clone)]
pub(crate) struct PlantBlacklistOption {
    seed_id: i64,
    label: String,
}

impl SearchableListItem for PlantBlacklistOption {
    type Value = i64;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.seed_id
    }
}

pub(crate) type PlantBlacklistSelector = ComboboxState<SearchableVec<PlantBlacklistOption>>;

pub(crate) fn plant_blacklist_options(
    raw: &serde_json::Value,
    language: &str,
) -> Vec<PlantBlacklistOption> {
    let english = language == "en-US";
    let mut options: Vec<PlantBlacklistOption> = Vec::new();
    let mut known = std::collections::HashMap::<i64, usize>::new();
    for row in raw.get("rankings").and_then(serde_json::Value::as_array).into_iter().flatten() {
        let Some(seed_id) = numeric_id(row.get("seedId")) else {
            continue;
        };
        if seed_id <= 0 {
            continue;
        }
        let label = ranking_option_label(row, seed_id, language);
        if let Some(index) = known.get(&seed_id).copied() {
            // JS Map.set replaces a duplicate value without changing its insertion order.
            options[index].label = label;
        } else {
            known.insert(seed_id, options.len());
            options.push(PlantBlacklistOption { seed_id, label });
        }
    }
    for seed_id in raw
        .get("plantBlacklist")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| numeric_id(Some(value)))
    {
        if seed_id > 0 && !known.contains_key(&seed_id) {
            known.insert(seed_id, options.len());
            options.push(PlantBlacklistOption {
                seed_id,
                label: if english {
                    format!("Seed {seed_id}")
                } else {
                    format!("种子 {seed_id}")
                },
            });
        }
    }
    options
}

fn ranking_option_label(row: &serde_json::Value, seed_id: i64, language: &str) -> String {
    let name = row
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| seed_id.to_string());
    let level = match row.get("level") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(value)) if value.is_empty() => None,
        Some(serde_json::Value::String(value)) => {
            if value.trim().parse::<f64>().is_ok_and(|number| number < 0.) {
                None
            } else {
                Some(value.clone())
            }
        }
        Some(serde_json::Value::Number(value)) => {
            if value.as_f64().is_some_and(|number| number < 0.) {
                None
            } else if let Some(integer) = value.as_i64() {
                Some(integer.to_string())
            } else if let Some(number) = value.as_f64().filter(|number| number.fract() == 0.) {
                Some(format!("{number:.0}"))
            } else {
                Some(value.to_string())
            }
        }
        Some(value) => Some(value.to_string()),
    }
    .unwrap_or_else(|| crate::i18n::text(language, "未知", "Unknown").to_string());
    format!("Lv{level} {name} ({seed_id})")
}

fn numeric_id(value: Option<&serde_json::Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
            .or_else(|| value.as_str()?.trim().parse::<i64>().ok())
    })
}

impl FarmDesktop {
    pub(in crate::desktop) fn sync_plant_blacklist_selector(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) {
        let options = plant_blacklist_options(&self.page_raw, &self.preferences.language);
        let selected = self
            .page_raw
            .get("plantBlacklist")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|value| numeric_id(Some(value)))
            .filter(|seed_id| *seed_id > 0)
            .collect::<Vec<_>>();
        let selected_indices = selected
            .iter()
            .filter_map(|selected| options.iter().position(|item| item.seed_id == *selected))
            .map(IndexPath::new)
            .collect::<Vec<_>>();
        self.analytics.blacklist_selector.update(cx, |selector, cx| {
            selector.set_items(SearchableVec::new(options), window, cx);
            selector.set_selected_indices(selected_indices, window, cx);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::plant_blacklist_options;
    use gpui_kit::component::searchable_list::SearchableListItem;

    #[test]
    fn blacklist_options_match_vue_map_and_label_semantics() {
        let options = plant_blacklist_options(
            &serde_json::json!({
                "rankings": [
                    {"seedId": 12, "name": "草莓", "level": 3},
                    {"seedId": 13, "name": "玉米", "level": null},
                    {"seedId": 12, "name": "重复项", "level": 8},
                    {"seedId": 14, "name": "", "level": ""},
                    {"seedId": 15, "level": -1},
                    {"seedId": 16, "level": "0"}
                ],
                "plantBlacklist": [13, 99]
            }),
            "zh-CN",
        );
        assert_eq!(options.len(), 6);
        assert_eq!(options[0].value(), &12);
        assert_eq!(options[0].title().as_ref(), "Lv8 重复项 (12)");
        assert_eq!(options[1].title().as_ref(), "Lv未知 玉米 (13)");
        assert_eq!(options[2].title().as_ref(), "Lv未知 14 (14)");
        assert_eq!(options[3].title().as_ref(), "Lv未知 15 (15)");
        assert_eq!(options[4].title().as_ref(), "Lv0 16 (16)");
        assert_eq!(options[5].value(), &99);
        assert_eq!(options[5].title().as_ref(), "种子 99");
    }

    #[test]
    fn blacklist_options_accept_numeric_strings_and_localize_unknown_ids() {
        let options = plant_blacklist_options(
            &serde_json::json!({
                "rankings": [{"seedId":"12", "name":"Wheat", "level":"3"}],
                "plantBlacklist": ["99"]
            }),
            "en-US",
        );
        assert_eq!(options[0].value(), &12);
        assert_eq!(options[0].title().as_ref(), "Lv3 Wheat (12)");
        assert_eq!(options[1].value(), &99);
        assert_eq!(options[1].title().as_ref(), "Seed 99");
    }
}
