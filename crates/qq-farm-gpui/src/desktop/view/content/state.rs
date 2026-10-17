use super::super::super::*;

pub(super) struct ContentState {
    pub(super) milk_tea_view: bool,
    pub(super) share_reward_view: bool,
    pub(super) solar_calendar_view: bool,
    pub(super) personal_unavailable: bool,
    pub(super) personal_has_error: bool,
    pub(super) mall_catalog_view: bool,
    pub(super) mystery_shop_view: bool,
    pub(super) daily_gifts_view: bool,
    pub(super) personal_lands_view: bool,
    pub(super) personal_bag: bool,
    pub(super) personal_interaction_view: bool,
    pub(super) personal_illustrated_view: bool,
    pub(super) personal_pets_view: bool,
    pub(super) analytics_view: bool,
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
        let solar_calendar_view = desktop.page == Page::SolarCalendar;
        let milk_tea_view = desktop.page == Page::MilkTea;
        let share_reward_view = desktop.page == Page::ShareReward;
        let mall_catalog_view = desktop.page == Page::Mall && desktop.commerce.mall_slot_type != -1;
        let mystery_shop_view = desktop.page == Page::MysteryShop
            || (desktop.page == Page::Mall && desktop.commerce.mall_slot_type == -1);
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
        let analytics_view = desktop.page == Page::Analytics;
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
            milk_tea_view,
            share_reward_view,
            solar_calendar_view,
            personal_unavailable,
            personal_has_error,
            mall_catalog_view,
            mystery_shop_view,
            daily_gifts_view,
            personal_lands_view,
            personal_bag,
            personal_interaction_view,
            personal_illustrated_view,
            personal_pets_view,
            analytics_view,
            friend_section_error,
        }
    }
}
