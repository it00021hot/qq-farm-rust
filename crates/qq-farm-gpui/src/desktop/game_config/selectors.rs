use gpui_kit::component::searchable_list::SearchableListItem;
use gpui_kit::component::select::{SearchableVec, SelectState};
use gpui_kit::component::IndexPath;
use gpui_kit::{App, SharedString, Window};

use super::super::FarmDesktop;

#[derive(Clone)]
pub(crate) struct GameConfigFilterOption {
    value: Option<i64>,
    label: String,
}

impl SearchableListItem for GameConfigFilterOption {
    type Value = Option<i64>;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

pub(crate) type GameConfigFilterSelect = SelectState<SearchableVec<GameConfigFilterOption>>;

#[derive(Clone)]
pub(crate) struct GameConfigItemTypeOption {
    value: i64,
    label: String,
}

impl SearchableListItem for GameConfigItemTypeOption {
    type Value = i64;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

pub(crate) type GameConfigItemTypeSelect = SelectState<SearchableVec<GameConfigItemTypeOption>>;

#[derive(Clone)]
pub(crate) struct GameConfigPlantOption {
    value: i64,
    label: String,
}

impl SearchableListItem for GameConfigPlantOption {
    type Value = i64;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

pub(crate) type GameConfigPlantSelect = SelectState<SearchableVec<GameConfigPlantOption>>;

pub(crate) fn plant_form_options(
    plants: &[serde_json::Value],
    selected_value: Option<i64>,
    language: &str,
) -> Vec<GameConfigPlantOption> {
    let mut options = plants
        .iter()
        .filter_map(|plant| {
            let value = super::form::plant_id(plant)?;
            let name = plant.get("name").and_then(serde_json::Value::as_str)?;
            Some(GameConfigPlantOption {
                value,
                label: super::form::plant_option_label(
                    name,
                    value,
                    super::form::plant_seed_id(plant),
                    language,
                ),
            })
        })
        .collect::<Vec<_>>();
    if let Some(selected_value) =
        selected_value.filter(|selected| !options.iter().any(|option| option.value == *selected))
    {
        options.push(GameConfigPlantOption {
            value: selected_value,
            label: if language == "en-US" {
                format!("Plant #{selected_value} (not found in catalog)")
            } else {
                format!("植物 #{selected_value}（当前目录未找到）")
            },
        });
    }
    options
}

pub(crate) fn item_type_form_options(
    item_types: &[(i64, String)],
    selected_value: i64,
) -> Vec<GameConfigItemTypeOption> {
    let mut options = item_types
        .iter()
        .map(|(value, label)| GameConfigItemTypeOption {
            value: *value,
            label: format!("{value} - {label}"),
        })
        .collect::<Vec<_>>();
    if !item_types.iter().any(|(value, _)| *value == selected_value) {
        // Vue keeps the current model value even when the fetched option list no longer contains
        // it. Preserve that edit path so an existing catalog record can still be saved unchanged.
        options.push(GameConfigItemTypeOption {
            value: selected_value,
            label: selected_value.to_string(),
        });
    }
    options
}

pub(crate) fn season_options(language: &str) -> Vec<GameConfigFilterOption> {
    vec![
        option(None, text(language, "全部季数", "All seasons")),
        option(Some(1), text(language, "单季", "Single season")),
        option(Some(2), text(language, "双季", "Two seasons")),
    ]
}

pub(crate) fn rarity_options(language: &str) -> Vec<GameConfigFilterOption> {
    let mut options = vec![option(None, text(language, "全部品质", "All rarities"))];
    options.extend(
        [
            ("普通", "Common"),
            ("优秀", "Uncommon"),
            ("精良", "Fine"),
            ("稀有", "Rare"),
            ("史诗", "Epic"),
            ("传说", "Legendary"),
        ]
        .into_iter()
        .enumerate()
        .map(|(value, (chinese, english))| {
            option(Some(value as i64), text(language, chinese, english))
        }),
    );
    options
}

pub(crate) fn item_type_options(
    raw: &serde_json::Value,
    language: &str,
) -> Vec<GameConfigFilterOption> {
    let configured = raw
        .get("itemTypes")
        .and_then(serde_json::Value::as_array)
        .filter(|options| !options.is_empty())
        .cloned()
        .unwrap_or_else(|| {
            qq_farm_app::config::item_types().as_array().cloned().unwrap_or_default()
        });
    let mut options = vec![option(None, text(language, "全部类型", "All types"))];
    options.extend(configured.into_iter().filter_map(|entry| {
        let value = entry.get("value").and_then(serde_json::Value::as_i64)?;
        let label =
            entry.get("label").and_then(serde_json::Value::as_str).unwrap_or("Item").to_owned();
        Some(option(Some(value), label))
    }));
    options
}

fn option(value: Option<i64>, label: String) -> GameConfigFilterOption {
    GameConfigFilterOption { value, label }
}

fn text(language: &str, chinese: &str, english: &str) -> String {
    crate::i18n::text(language, chinese, english).to_owned()
}

pub(crate) fn selected_index(
    options: &[GameConfigFilterOption],
    value: Option<i64>,
) -> Option<IndexPath> {
    options.iter().position(|option| option.value == value).map(IndexPath::new)
}

impl FarmDesktop {
    pub(in crate::desktop) fn sync_game_config_plant_selector(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) {
        let options = plant_form_options(
            &self.game_config.plants,
            self.game_config.plant_id,
            &self.preferences.language,
        );
        self.game_config.plant_form_selector.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(options.clone()), window, cx);
            if let Some(selected) = self.game_config.plant_id {
                select.set_selected_value(&selected, window, cx);
            } else {
                select.set_selected_index(None, window, cx);
            }
        });
    }

    pub(in crate::desktop) fn sync_game_config_selectors(&self, window: &mut Window, cx: &mut App) {
        let seasons = season_options(&self.preferences.language);
        self.game_config.season_selector.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(seasons.clone()), window, cx);
            select.set_selected_index(
                selected_index(&seasons, self.game_config.seasons),
                window,
                cx,
            );
        });

        let rarities = rarity_options(&self.preferences.language);
        self.game_config.rarity_selector.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(rarities.clone()), window, cx);
            select.set_selected_index(
                selected_index(&rarities, self.game_config.rarity),
                window,
                cx,
            );
        });

        let item_types = item_type_options(&self.page_raw, &self.preferences.language);
        self.game_config.item_type_selector.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(item_types.clone()), window, cx);
            select.set_selected_index(
                selected_index(&item_types, self.game_config.item_type),
                window,
                cx,
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{item_type_form_options, plant_form_options};
    use gpui_kit::component::searchable_list::SearchableListItem;
    use serde_json::json;

    #[test]
    fn item_type_editor_labels_match_vue_and_keep_unknown_existing_values() {
        let options = item_type_form_options(&[(7, "化肥".into())], 99);
        assert_eq!(options[0].title().as_ref(), "7 - 化肥");
        assert_eq!(options[0].value(), &7);
        assert_eq!(options[1].title().as_ref(), "99");
        assert_eq!(options[1].value(), &99);
    }

    #[test]
    fn fruit_plant_select_matches_vue_labels_and_preserves_an_unknown_edit_value() {
        let plants = vec![json!({
            "plantId": 1001,
            "seedId": 2001,
            "name": "草莓"
        })];
        let options = plant_form_options(&plants, None, "zh-CN");
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].title().as_ref(), "草莓 (种子2001 / 植物1001)");
        assert_eq!(options[0].value(), &1001);

        let options = plant_form_options(&plants, Some(999), "en-US");
        assert_eq!(options.len(), 2);
        assert_eq!(options[1].title().as_ref(), "Plant #999 (not found in catalog)");
        assert_eq!(options[1].value(), &999);
    }
}
