use gpui_kit::component::searchable_list::SearchableListItem;
use gpui_kit::component::select::{SearchableVec, SelectState};
use gpui_kit::component::IndexPath;
use gpui_kit::{App, Context, SharedString, Window};

use super::super::FarmDesktop;

#[derive(Clone)]
pub(in crate::desktop) struct StrategyOption {
    value: &'static str,
    label: String,
}

impl StrategyOption {
    fn new(value: &'static str, label: &str, language: &str) -> Self {
        Self { value, label: super::messages::settings_text(language, label) }
    }
}

impl SearchableListItem for StrategyOption {
    type Value = &'static str;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(Clone)]
pub(in crate::desktop) struct PreferredSeedOption {
    id: i64,
    label: String,
    disabled: bool,
}

impl PreferredSeedOption {
    fn automatic(language: &str) -> Self {
        Self {
            id: 0, label: super::messages::settings_text(language, "自动选择"), disabled: false
        }
    }

    fn seed(id: i64, label: String, disabled: bool) -> Self {
        Self { id, label, disabled }
    }
}

impl SearchableListItem for PreferredSeedOption {
    type Value = i64;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }

    fn disabled(&self) -> bool {
        self.disabled
    }
}

pub(in crate::desktop) type PlantingStrategySelect = SelectState<SearchableVec<StrategyOption>>;
pub(in crate::desktop) type BagFallbackSelect = SelectState<SearchableVec<StrategyOption>>;
pub(in crate::desktop) type PreferredSeedSelect = SelectState<SearchableVec<PreferredSeedOption>>;

pub(in crate::desktop) fn planting_strategy_options(language: &str) -> Vec<StrategyOption> {
    vec![
        StrategyOption::new("preferred", "指定种子", language),
        StrategyOption::new("level", "等级优先", language),
        StrategyOption::new("max_exp", "最高经验", language),
        StrategyOption::new("max_fert_exp", "化肥经验", language),
        StrategyOption::new("max_profit", "最高收益", language),
        StrategyOption::new("max_fert_profit", "化肥收益", language),
        StrategyOption::new("bag_priority", "背包优先", language),
    ]
}

pub(in crate::desktop) fn bag_fallback_options(language: &str) -> Vec<StrategyOption> {
    vec![
        StrategyOption::new("level", "等级优先", language),
        StrategyOption::new("max_exp", "最高经验", language),
        StrategyOption::new("max_fert_exp", "化肥经验", language),
        StrategyOption::new("max_profit", "最高收益", language),
        StrategyOption::new("max_fert_profit", "化肥收益", language),
        StrategyOption::new("preferred", "指定种子", language),
    ]
}

pub(in crate::desktop) fn preferred_seed_options(
    raw: &serde_json::Value,
    language: &str,
) -> Vec<PreferredSeedOption> {
    let mut options = vec![PreferredSeedOption::automatic(language)];
    options.extend(
        raw.get("seedOptions")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|seed| {
                let id = seed
                    .get("seedId")
                    .or_else(|| seed.get("seed_id"))
                    .and_then(serde_json::Value::as_i64)?;
                let name = seed
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| super::messages::settings_text(language, "未命名种子"));
                let level = seed
                    .get("requiredLevel")
                    .or_else(|| seed.get("required_level"))
                    .or_else(|| seed.get("landLevelNeed"))
                    .or_else(|| seed.get("land_level_need"))
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or_default();
                Some(PreferredSeedOption::seed(
                    id,
                    format!("Lv{level} {name}"),
                    seed.get("locked").and_then(serde_json::Value::as_bool).unwrap_or(false)
                        || seed
                            .get("soldOut")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false),
                ))
            }),
    );
    options
}

fn strategy_index(value: &str, options: &[StrategyOption]) -> IndexPath {
    let row = options.iter().position(|option| option.value == value).unwrap_or(0);
    IndexPath::new(row)
}

impl FarmDesktop {
    pub(in crate::desktop) fn update_setting_selector_language(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let language = &self.preferences.language;
        let provider = self
            .settings
            .draft
            .pointer("/offlineReminder/provider")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("none");
        let provider_options = super::offline_provider_options(language);
        self.settings.offline_provider.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(provider_options), window, cx);
            select.set_selected_index(Some(super::provider_index(provider)), window, cx);
        });

        let time_zone = self
            .settings
            .draft
            .pointer("/systemConfig/timeZone")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(qq_farm_core::config::DEFAULT_TIME_ZONE);
        self.settings.time_zone.update(cx, |select, cx| {
            select.set_items(
                SearchableVec::new(super::system_config::time_zone_items(language)),
                window,
                cx,
            );
            select.set_selected_index(
                Some(IndexPath::new(super::system_config::time_zone_index(time_zone))),
                window,
                cx,
            );
        });

        let strategy_options = planting_strategy_options(language);
        let strategy = self
            .settings
            .draft
            .get("plantingStrategy")
            .or_else(|| self.settings.draft.get("strategy"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("max_exp");
        self.settings.planting_strategy.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(strategy_options.clone()), window, cx);
            select.set_selected_index(
                Some(strategy_index(strategy, &strategy_options)),
                window,
                cx,
            );
        });

        let fallback_options = bag_fallback_options(language);
        let fallback = self
            .settings
            .draft
            .get("bagSeedFallbackStrategy")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("preferred");
        self.settings.bag_fallback.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(fallback_options.clone()), window, cx);
            select.set_selected_index(
                Some(strategy_index(fallback, &fallback_options)),
                window,
                cx,
            );
        });

        let seed_options = preferred_seed_options(&self.page_raw, language);
        let selected_seed = self
            .settings
            .draft
            .get("preferredSeedId")
            .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
            .unwrap_or_default();
        let selected_index =
            seed_options.iter().position(|option| option.id == selected_seed).map(IndexPath::new);
        self.settings.preferred_seed.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(seed_options), window, cx);
            select.set_selected_index(selected_index, window, cx);
        });
    }

    pub(in crate::desktop) fn sync_setting_selectors(&self, window: &mut Window, cx: &mut App) {
        let raw = &self.page_raw;
        let provider = self
            .settings
            .draft
            .pointer("/offlineReminder/provider")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("none");
        let provider_options =
            super::provider::offline_provider_options(&self.preferences.language);
        self.settings.offline_provider.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(provider_options), window, cx);
            select.set_selected_index(Some(super::provider::provider_index(provider)), window, cx);
        });

        let strategy_options = planting_strategy_options(&self.preferences.language);
        let strategy = self
            .settings
            .draft
            .get("plantingStrategy")
            .or_else(|| self.settings.draft.get("strategy"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("max_exp");
        self.settings.planting_strategy.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(strategy_options.clone()), window, cx);
            select.set_selected_index(
                Some(strategy_index(strategy, &strategy_options)),
                window,
                cx,
            );
        });

        let fallback_options = bag_fallback_options(&self.preferences.language);
        let fallback = self
            .settings
            .draft
            .get("bagSeedFallbackStrategy")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("preferred");
        self.settings.bag_fallback.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(fallback_options.clone()), window, cx);
            select.set_selected_index(
                Some(strategy_index(fallback, &fallback_options)),
                window,
                cx,
            );
        });

        let seed_options = preferred_seed_options(raw, &self.preferences.language);
        let selected_seed = self
            .settings
            .draft
            .get("preferredSeedId")
            .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
            .unwrap_or_default();
        let selected_index =
            seed_options.iter().position(|option| option.id == selected_seed).map(IndexPath::new);
        self.settings.preferred_seed.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(seed_options), window, cx);
            select.set_selected_index(selected_index, window, cx);
        });

        for (field, path) in [
            (&self.settings.quiet_hours_start, "/friendQuietHours/start"),
            (&self.settings.quiet_hours_end, "/friendQuietHours/end"),
        ] {
            let time = self
                .settings
                .draft
                .pointer(path)
                .and_then(serde_json::Value::as_str)
                .and_then(|value| chrono::NaiveTime::parse_from_str(value, "%H:%M").ok())
                .unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(0, 0, 0).expect("midnight"));
            field.update(cx, |state, cx| state.set_time(time, window, cx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{bag_fallback_options, planting_strategy_options, preferred_seed_options};
    use gpui_kit::component::searchable_list::SearchableListItem;

    #[test]
    fn planting_strategy_options_include_every_vue_strategy() {
        let values = planting_strategy_options("zh-CN")
            .iter()
            .map(|option| *option.value())
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            [
                "preferred",
                "level",
                "max_exp",
                "max_fert_exp",
                "max_profit",
                "max_fert_profit",
                "bag_priority"
            ]
        );
        assert_eq!(
            bag_fallback_options("zh-CN").iter().map(|option| *option.value()).collect::<Vec<_>>(),
            ["level", "max_exp", "max_fert_exp", "max_profit", "max_fert_profit", "preferred"]
        );
    }

    #[test]
    fn preferred_seed_options_normalize_catalog_fields_and_disable_unavailable_seeds() {
        let options = preferred_seed_options(
            &serde_json::json!({
                "seedOptions": [
                    {"seed_id": 101, "name": "番茄", "land_level_need": 4},
                    {"seedId": 102, "name": "锁定种子", "requiredLevel": 8, "locked": true},
                    {"seedId": 103, "name": "售罄种子", "requiredLevel": 9, "soldOut": true}
                ]
            }),
            "zh-CN",
        );
        assert_eq!(options.len(), 4);
        assert_eq!(options[0].value(), &0);
        assert_eq!(options[0].title().as_ref(), "自动选择");
        assert_eq!(options[1].value(), &101);
        assert_eq!(options[1].title().as_ref(), "Lv4 番茄");
        assert!(!options[1].disabled());
        assert!(options[2].disabled());
        assert!(options[3].disabled());
    }

    #[test]
    fn selector_titles_follow_locale_and_localize_missing_seed_names() {
        assert_eq!(planting_strategy_options("en-US")[0].title().as_ref(), "Preferred seed");
        let options =
            preferred_seed_options(&serde_json::json!({"seedOptions": [{"seedId": 7}]}), "en-US");
        assert_eq!(options[0].title().as_ref(), "Automatic selection");
        assert_eq!(options[1].title().as_ref(), "Lv0 Unnamed seed");
    }
}
