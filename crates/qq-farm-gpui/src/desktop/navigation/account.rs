use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn set_account(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_account.as_deref() == Some(id.as_str()) {
            return;
        }
        self.cancel_plant_blacklist_save();
        self.invalidate_solar_calendar();
        self.invalidate_milk_tea();
        self.invalidate_share_reward();
        self.commerce.invalidate_purchase();
        self.invalidate_farm_operations();
        self.invalidate_personal_lands_clock();
        self.invalidate_personal_bag_clock();
        self.invalidate_personal_interaction();
        self.invalidate_personal_pet_operations();
        self.invalidate_friend_operations();
        self.page_raw = serde_json::Value::Null;
        self.rows.clear();
        self.friends.countdown_generation = self.friends.countdown_generation.wrapping_add(1);
        self.dashboard.coupon_baseline = None;
        self.dashboard.bag_refresh_in_secs = 30;
        self.dashboard.bag_refresh_generation =
            self.dashboard.bag_refresh_generation.wrapping_add(1);
        self.dashboard.bag_refresh_pending = false;
        let strategy_level = qq_farm_app::farm::panel_status_with_progress(&self.app, &id).level;
        self.apply_reference_level(strategy_level, window, cx);
        self.farm.pets.food_inputs.clear();
        self.selected_account = Some(id);
        self.friends.detail = None;
        self.friends.detail_loading_gid = None;
        self.friends.detail_error = None;
        self.preferences.selected_account = self.selected_account.clone();
        self.persist_ui_preferences();
        self.refresh(cx);
    }
}
