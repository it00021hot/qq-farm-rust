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
        self.activities.view_revision = self.activities.view_revision.wrapping_add(1);
        self.activities.pending_actions.clear();
        self.commerce.invalidate_purchase();
        self.invalidate_farm_operations();
        self.invalidate_personal_lands_clock();
        self.invalidate_personal_bag_clock();
        self.invalidate_personal_interaction();
        self.invalidate_personal_pet_operations();
        self.invalidate_friend_operations();
        self.activities.qingmei_selected_ingredients.clear();
        self.activities.qixi_selected_friend_gid.clear();
        self.activities.autumn_choice_id.clear();
        self.activities.autumn_log_tab = 1;
        self.activities.autumn_logs_load_pending = false;
        self.activities.weather.reset(window, cx);
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
        if self.page == Page::Activities {
            self.page_section = "directory".into();
            self.start_activity_clock(cx);
        }
        self.friends.detail = None;
        self.friends.detail_loading_gid = None;
        self.friends.detail_error = None;
        self.activities.pet_diary_battle_target = None;
        self.activities.pet_diary_notice = None;
        self.activities.pet_diary_warning = None;
        self.activities.pet_diary_friend_loading = false;
        self.activities.pet_diary_friend_error = None;
        self.activities.pet_diary_friends_loading = false;
        self.activities.pet_diary_friend_request_generation =
            self.activities.pet_diary_friend_request_generation.wrapping_add(1);
        self.activities.pet_diary_friends_request_generation =
            self.activities.pet_diary_friends_request_generation.wrapping_add(1);
        self.activities.qixi_friends_loading = false;
        self.activities.qixi_friends_request_generation =
            self.activities.qixi_friends_request_generation.wrapping_add(1);
        self.activities.pet_diary_friend_gid.clear();
        self.sync_pet_diary_friend_selector(window, cx);
        self.activities.charity_donation_pending = false;
        self.preferences.selected_account = self.selected_account.clone();
        self.persist_ui_preferences();
        self.refresh(cx);
    }
}
