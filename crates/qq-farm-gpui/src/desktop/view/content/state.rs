use super::super::super::*;

pub(super) struct ContentState {
    pub(super) personal_unavailable: bool,
    pub(super) personal_has_error: bool,
    pub(super) activity_directory: bool,
    pub(super) activity_shop_view: bool,
    pub(super) mall_catalog_view: bool,
    pub(super) mystery_shop_view: bool,
    pub(super) weather_view: bool,
    pub(super) qixi_view: bool,
    pub(super) qingmei_view: bool,
    pub(super) constellation_view: bool,
    pub(super) solar_terms_view: bool,
    pub(super) season_view: bool,
    pub(super) daily_gifts_view: bool,
    pub(super) personal_lands_view: bool,
    pub(super) personal_bag: bool,
    pub(super) personal_interaction_view: bool,
    pub(super) personal_illustrated_view: bool,
    pub(super) personal_pets_view: bool,
    pub(super) charity_view: bool,
    pub(super) analytics_view: bool,
    pub(super) pet_diary_view: bool,
    pub(super) pet_diary_error: bool,
    pub(super) weather_error: bool,
    pub(super) autumn_view: bool,
    pub(super) friend_section_error: Option<String>,
}

impl ContentState {
    pub(super) fn new(desktop: &FarmDesktop) -> Self {
        let personal_unavailable = desktop.personal_page_unavailable();
        let personal_has_error = desktop.page == Page::Personal
            && desktop
                .page_raw
                .get("sectionErrors")
                .and_then(|errors| errors.get(&desktop.page_section))
                .is_some();
        let personal_ready = !personal_unavailable && !personal_has_error;
        let activity_directory =
            desktop.page == Page::Activities && desktop.page_section == "directory";
        let activity_shop_view = desktop.page == Page::Activities
            && desktop.page_section == "shop"
            && desktop.page_raw.get("shop").is_some();
        let mall_catalog_view = desktop.page == Page::Mall && desktop.commerce.mall_slot_type != -1;
        let mystery_shop_view = desktop.page == Page::MysteryShop
            || (desktop.page == Page::Mall && desktop.commerce.mall_slot_type == -1);
        let weather_view = desktop.page == Page::Activities && desktop.page_section == "weather";
        let qixi_view = desktop.page == Page::Activities
            && desktop.page_section == "qixi"
            && desktop.page_raw.get("qixi").is_some();
        let qingmei_view = desktop.page == Page::Activities
            && desktop.page_section == "qingmei"
            && (desktop.page_raw.get("qingMei").is_some()
                || desktop.page_raw.get("greenPlum").is_some()
                || desktop.page_raw.get("qingmei").is_some());
        let constellation_view = desktop.page == Page::Activities
            && desktop.page_section == "constellation"
            && desktop.page_raw.get("constellation").is_some();
        let solar_terms_view = desktop.page == Page::Activities
            && desktop.page_section == "solarTerms"
            && desktop.page_raw.get("solarTerms").is_some();
        let season_view = desktop.page == Page::Activities
            && desktop.page_section == "season"
            && desktop.page_raw.get("season").is_some();
        let daily_gifts_view = personal_ready
            && desktop.page == Page::Personal
            && desktop.page_section == "dailyGifts";
        let personal_lands_view =
            personal_ready && desktop.page == Page::Personal && desktop.page_section == "lands";
        let personal_bag =
            personal_ready && desktop.page == Page::Personal && desktop.page_section == "bag";
        let personal_interaction_view = personal_ready
            && desktop.page == Page::Personal
            && desktop.page_section == "interactionItems";
        let personal_illustrated_view = personal_ready
            && desktop.page == Page::Personal
            && desktop.page_section == "illustrated";
        let personal_pets_view =
            personal_ready && desktop.page == Page::Personal && desktop.page_section == "pets";
        let charity_view = desktop.page == Page::Activities && desktop.page_section == "charity";
        let analytics_view = desktop.page == Page::Analytics;
        let pet_diary_view = desktop.page == Page::Activities && desktop.page_section == "petDiary";
        let pet_diary_error = pet_diary_view
            && desktop.error.is_some()
            && desktop.page_raw.get("petDiary").is_none_or(serde_json::Value::is_null);
        let weather_error = weather_view
            && desktop.error.is_some()
            && desktop.page_raw.pointer("/weather/snapshot").is_none_or(serde_json::Value::is_null);
        let autumn_view = desktop.page == Page::Activities
            && desktop.page_section == "autumn"
            && desktop.page_raw.get("autumn").is_some();
        let friend_section_error = if desktop.page == Page::Friends {
            let key = match desktop.page_section.as_str() {
                "records" => Some("recordsError"),
                "interactionItems" => Some("interactionItemsError"),
                _ => None,
            };
            key.and_then(|key| desktop.page_raw.get(key))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        } else {
            None
        };

        Self {
            personal_unavailable,
            personal_has_error,
            activity_directory,
            activity_shop_view,
            mall_catalog_view,
            mystery_shop_view,
            weather_view,
            qixi_view,
            qingmei_view,
            constellation_view,
            solar_terms_view,
            season_view,
            daily_gifts_view,
            personal_lands_view,
            personal_bag,
            personal_interaction_view,
            personal_illustrated_view,
            personal_pets_view,
            charity_view,
            analytics_view,
            pet_diary_view,
            pet_diary_error,
            weather_error,
            autumn_view,
            friend_section_error,
        }
    }
}
