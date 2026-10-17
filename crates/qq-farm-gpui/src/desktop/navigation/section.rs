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
        if self.page == Page::MilkTea && entering_section {
            self.milk_tea.craft_selected.clear();
            self.page_section = section.into();
            self.fetch_milk_tea_page(cx);
            return;
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
        cx.notify();
    }
}
