use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn set_page_section(
        &mut self,
        section: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.page == Page::Mall {
            let slot_type = match section {
                "svip" => 4,
                "mystery" => -1,
                _ => 1,
            };
            if self.page_section == section {
                return;
            }
            self.commerce.mall_slot_type = slot_type;
            self.page_section = section.into();
            let cached = if slot_type == -1 {
                None
            } else {
                self.selected_account
                    .as_deref()
                    .and_then(|account_id| self.commerce.cached_catalog(account_id, slot_type))
                    .cloned()
            };
            if let Some(snapshot) = cached {
                self.page_raw = snapshot;
                let mapped = pages::page_data(Page::Mall, self.page_raw.clone());
                self.summary = mapped.summary;
                self.rows = mapped.rows;
                self.loading = false;
                self.error = None;
                cx.notify();
                return;
            }
            self.page_raw = serde_json::Value::Null;
            self.summary.clear();
            self.rows.clear();
            self.refresh(cx);
            return;
        }
        let entering_section = self.page_section != section;
        if self.page == Page::Activities
            && matches!(section, "season" | "constellation" | "shop" | "solarTerms")
        {
            self.activities.stellar_section = section.into();
        }
        if self.page == Page::Personal && entering_section {
            self.invalidate_personal_lands_clock();
            self.invalidate_personal_bag_clock();
            self.invalidate_personal_interaction();
            self.invalidate_personal_pet_operations();
        }
        if self.page == Page::Friends && entering_section {
            self.invalidate_friend_operations();
        }
        if self.page == Page::Activities && entering_section {
            if self.page_section == "petDiary" && section != "petDiary" {
                self.activities.pet_diary_friend_loading = false;
                self.activities.pet_diary_friend_error = None;
                self.activities.pet_diary_friends_loading = false;
                self.activities.pet_diary_friend_request_generation =
                    self.activities.pet_diary_friend_request_generation.wrapping_add(1);
                self.activities.pet_diary_friends_request_generation =
                    self.activities.pet_diary_friends_request_generation.wrapping_add(1);
            }
            if self.page_section == "qixi" && section != "qixi" {
                self.activities.qixi_friends_loading = false;
                self.activities.qixi_friends_request_generation =
                    self.activities.qixi_friends_request_generation.wrapping_add(1);
            }
            if section == "directory" {
                self.activities.autumn_logs_load_pending = false;
            }
            if section == "weather" {
                // Vue destroys WeatherView when the user leaves the activity and creates a fresh
                // instance on re-entry. Drop its local scan/search/feedback state and stale
                // snapshot before starting the equivalent GPUI reload.
                self.reset_weather_view(window, cx);
            } else {
                self.activities.weather.invalidate_requests();
            }
            self.activities.charity_donation_pending = false;
        }
        self.page_section = section.into();
        if self.page == Page::Personal && entering_section {
            self.prepare_personal_section(section, window, cx);
            self.refresh(cx);
            return;
        }
        if self.page == Page::Personal && section == "lands" {
            self.start_personal_lands_clock(cx);
        }
        if self.page == Page::Personal && section == "bag" {
            self.start_personal_bag_clock(cx);
        }
        if self.page == Page::Friends {
            self.friends.page = 0;
        }
        if self.page == Page::GameConfig {
            if entering_section {
                self.game_config.reset_for_category_change(section);
                self.search.update(cx, |state, cx| state.set_value("", window, cx));
                self.sync_game_config_selectors(window, cx);
            } else {
                self.game_config.page = 0;
            }
        }
        if self.page == Page::Friends && entering_section {
            self.refresh(cx);
            return;
        }
        if self.page == Page::Activities && section == "directory" {
            self.start_activity_clock(cx);
        }
        if self.page == Page::Activities && entering_section {
            match section {
                "qixi" => {
                    self.activities.qixi_selected_friend_gid.clear();
                    self.activities.qixi_rules_expanded = true;
                    self.activities
                        .qixi_friend_search
                        .update(cx, |state, cx| state.set_value("", window, cx));
                    self.activities
                        .qixi_gift_count
                        .update(cx, |state, cx| state.set_value("1", window, cx));
                }
                "petDiary" => {
                    self.activities.pet_diary_notice = None;
                    self.activities.pet_diary_warning = None;
                    self.activities.pet_diary_friend_gid.clear();
                    self.sync_pet_diary_friend_selector(window, cx);
                    self.activities
                        .pet_diary_exchange_count
                        .update(cx, |state, cx| state.set_value("1", window, cx));
                }
                "charity" => {
                    self.activities.charity_rules_expanded = true;
                }
                _ => {}
            }
        }
        if section != "petDiary" {
            self.activities.pet_diary_battle_target = None;
        }
        let lazy_section = match section {
            "petDiary" => Some("petDiary"),
            "autumn" => Some("autumn"),
            "weather" => Some("weather"),
            _ => None,
        };
        if self.page == Page::Activities && lazy_section.is_some() {
            self.load_activity_section(lazy_section.unwrap_or("petDiary"), cx);
            return;
        }
        if self.page == Page::Activities && section == "qixi" {
            self.load_qixi_friends(false, cx);
            return;
        }
        cx.notify();
    }

    pub(in crate::desktop) fn reset_weather_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activities.weather.reset(window, cx);
        if let Some(raw) = self.page_raw.as_object_mut() {
            raw.remove("weather");
        }
    }
}
