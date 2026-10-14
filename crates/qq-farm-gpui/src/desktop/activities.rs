mod state;
pub(crate) use state::ActivitiesState;

mod actions;
mod autumn_view;
mod charity;
mod constellation;
mod controls;
mod directory;
mod feedback;
mod helpers;
mod loading;
mod pet_diary;
mod pet_diary_view;
mod qingmei_controls;
mod qingmei_view;
mod qixi_controls;
mod qixi_view;
mod rewards;
mod row_actions;
mod rules;
mod season_pass;
mod shop;
mod solar_terms;
mod weather;
#[cfg(any(test, feature = "ui-preview"))]
pub(crate) use weather::weather_fixture;

pub(crate) use directory::activity_section_for_record;
pub(crate) use helpers::{
    activity_action_enabled, activity_title_for_section, value_to_action_string,
};

pub(crate) fn star_sand_purchase_feedback(
    result: &serde_json::Value,
    goods: &serde_json::Value,
    count: i64,
    english: bool,
) -> (String, bool) {
    feedback::star_sand_purchase_feedback(
        result,
        goods.get("fallbackItem"),
        goods.get("id").unwrap_or(&serde_json::Value::Null),
        goods.get("fallbackGoodsName").and_then(serde_json::Value::as_str).unwrap_or_default(),
        count,
        english,
    )
}

pub(super) fn default_constellation_selection(raw: &serde_json::Value, selected: &str) -> String {
    constellation::selected_constellation_id(raw, selected)
}

pub(super) fn default_solar_term_selection(raw: &serde_json::Value, selected: &str) -> String {
    solar_terms::selected_solar_term_id(raw, selected)
}

impl super::FarmDesktop {
    pub(super) fn reconcile_activity_selection(&mut self) {
        self.activities.constellation_group_id = default_constellation_selection(
            &self.page_raw,
            &self.activities.constellation_group_id,
        );
        self.activities.solar_term_id =
            default_solar_term_selection(&self.page_raw, &self.activities.solar_term_id);
    }
}
