use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn set_page(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page == Page::Mall && page != Page::Mall {
            self.commerce.clear_catalogs();
        }
        self.activities.view_revision = self.activities.view_revision.wrapping_add(1);
        self.activities.pending_actions.clear();
        self.activities.action_notice = None;
        self.activities.action_notice_generation =
            self.activities.action_notice_generation.wrapping_add(1);
        self.commerce.invalidate_purchase();
        self.invalidate_activity_clock();
        self.activities.weather.invalidate_requests();
        self.invalidate_farm_operations();
        self.invalidate_personal_lands_clock();
        self.invalidate_personal_bag_clock();
        self.invalidate_personal_interaction();
        self.invalidate_personal_pet_operations();
        self.invalidate_friend_operations();
        if page == Page::Mall {
            self.commerce.mall_slot_type = 1;
        } else if page == Page::MysteryShop {
            self.commerce.mall_slot_type = -1;
        }
        if self.page == Page::Analytics && self.page != page {
            self.cancel_plant_blacklist_save();
        }
        if self.page != page && (self.page == Page::GameConfig || page == Page::GameConfig) {
            self.game_config.reset_for_page_navigation();
        }
        self.friends.countdown_generation = self.friends.countdown_generation.wrapping_add(1);
        if self.page == Page::Settings && page != Page::Settings {
            self.settings.qq_bot_poll_generation =
                self.settings.qq_bot_poll_generation.wrapping_add(1);
        }
        self.dashboard.tick_generation = self.dashboard.tick_generation.wrapping_add(1);
        self.dashboard.bag_refresh_in_secs = 30;
        self.dashboard.bag_refresh_generation =
            self.dashboard.bag_refresh_generation.wrapping_add(1);
        self.dashboard.bag_refresh_pending = false;
        self.dashboard.status_revision = self.dashboard.status_revision.wrapping_add(1);
        self.page = page;
        self.accounts.column_picker_open = false;
        if page == Page::Activities {
            self.activities.autumn_logs_load_pending = false;
        }
        self.page_section = default_page_section(page).into();
        self.preferences.selected_page = page.id().into();
        self.persist_ui_preferences();
        self.rows.clear();
        self.page_raw = serde_json::Value::Null;
        self.activities.pet_diary_friend_gid.clear();
        if page == Page::GameConfig {
            if let Some(handle) = self.window_handle {
                let _ = handle.update(cx, |_, window, cx| {
                    self.sync_game_config_selectors(window, cx);
                });
            }
        }
        self.friends.detail = None;
        self.friends.detail_loading_gid = None;
        self.friends.detail_error = None;
        self.error = None;
        self.activities.pet_diary_battle_target = None;
        if let Some(handle) = self.window_handle {
            let _ = handle.update(cx, |_, window, cx| {
                self.sync_pet_diary_friend_selector(window, cx);
            });
        }
        self.activities.charity_donation_pending = false;
        if page == Page::Dashboard {
            self.start_dashboard_clock(cx);
        }
        if page == Page::Activities {
            self.start_activity_clock(cx);
        }
        if page == Page::Settings {
            if let Some(session_id) = self.settings.qq_bot_session_id.clone() {
                self.start_qq_bot_binding_polling(session_id, cx);
            }
        }
        self.refresh(cx);
    }
}
