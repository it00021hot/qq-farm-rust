use super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.quitting {
            return;
        }
        if self.page == Page::Activities && self.page_section == "weather" {
            self.request_revision = self.request_revision.wrapping_add(1);
            self.activities.weather.invalidate_requests();
            self.error = None;
            self.load_activity_section("weather", cx);
            return;
        }
        if self.page == Page::Dashboard {
            self.dashboard.status_revision = self.dashboard.status_revision.wrapping_add(1);
        }
        if self.page == Page::Friends {
            self.friends.page = 0;
            self.friends.records_refresh_generation =
                self.friends.records_refresh_generation.wrapping_add(1);
            self.friends.records_refresh_loading = false;
            self.friends.interaction_items_refresh_generation =
                self.friends.interaction_items_refresh_generation.wrapping_add(1);
            self.friends.interaction_items_refresh_loading = false;
        }
        if self.page == Page::GameConfig {
            self.game_config.page = 0;
        }
        if self.test_mode {
            self.request_revision = self.request_revision.wrapping_add(1);
            self.loading = false;
            if self.farm.pets.activation_refresh_pending {
                self.farm.pets.activation_refresh_pending = false;
                self.farm.pets.activating = false;
            }
            self.error = None;
            cx.notify();
            return;
        }
        self.request_revision = self.request_revision.wrapping_add(1);
        let revision = self.request_revision;
        let page = self.page;
        let mall_slot_type = self.commerce.mall_slot_type;
        let page_section = self.page_section.clone();
        let account_id = self.selected_account.clone().unwrap_or_default();
        let cache_account_id = account_id.clone();
        let friend_detail_to_refresh = if page == Page::Friends {
            friends::detail_gid_for_refresh(
                self.friends.detail.as_ref(),
                self.friends.detail_loading_gid,
            )
        } else {
            None
        };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        self.loading = true;
        self.error = None;

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    pages::load_page_for_mall_slot(
                        &app,
                        page,
                        &account_id,
                        mall_slot_type,
                        &page_section,
                    )
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let result = match result {
                    Ok(result) => result,
                    Err(error) => {
                        Err(crate::i18n::background_task_error(&this.preferences.language, error))
                    }
                };
                this.apply_page_load_result(
                    revision,
                    page,
                    mall_slot_type,
                    cache_account_id,
                    friend_detail_to_refresh,
                    result,
                    cx,
                );
            });
        })
        .detach();
        cx.notify();
    }

    /// Apply a completed page request only while it still belongs to the active view generation.
    /// Keeping the commit path separate lets tests deliver an old response after a real account
    /// switch without depending on network timing.
    pub(in crate::desktop) fn apply_page_load_result(
        &mut self,
        revision: u64,
        page: Page,
        mall_slot_type: i32,
        cache_account_id: String,
        friend_detail_to_refresh: Option<i64>,
        result: Result<pages::PageData, String>,
        cx: &mut Context<Self>,
    ) {
        if self.request_revision != revision {
            return;
        }
        self.loading = false;
        if self.farm.lands.finish_operation_after_refresh {
            self.farm.lands.finish_operation_after_refresh = false;
            self.farm.lands.operating = false;
        }
        if self.farm.pets.activation_refresh_pending {
            self.farm.pets.activation_refresh_pending = false;
            self.farm.pets.activating = false;
        }
        match result {
            Ok(data) => {
                if page == Page::Mall && matches!(mall_slot_type, 1 | 4) {
                    self.commerce.store_catalog(
                        &cache_account_id,
                        mall_slot_type,
                        data.raw.clone(),
                    );
                }
                match self.page {
                    Page::Friends => {
                        self.apply_friends_snapshot(data.raw);
                        if self.page_section == "interactionItems" {
                            if let Some(handle) = self.window_handle {
                                let _ = handle.update(cx, |_, window, cx| {
                                    self.sync_friend_interaction_picker(window, cx);
                                });
                            }
                        }
                    }
                    Page::Personal => self.apply_personal_snapshot(data, cx),
                    _ => {
                        self.summary = data.summary;
                        self.rows = data.rows;
                        self.page_raw = data.raw;
                    }
                }
                if self.page == Page::GameConfig {
                    if let Some(handle) = self.window_handle {
                        let _ = handle.update(cx, |_, window, cx| {
                            self.sync_game_config_selectors(window, cx);
                        });
                    }
                }
                if self.page == Page::Activities {
                    self.reconcile_activity_selection();
                }
                if self.page == Page::Dashboard {
                    self.reconcile_dashboard_snapshot();
                }
                if self.page == Page::Settings {
                    self.apply_settings_snapshot(self.page_raw.clone(), cx);
                }
                if self.page == Page::Accounts {
                    self.reconcile_account_selection();
                }
                if self.page == Page::Analytics {
                    if let Some(handle) = self.window_handle {
                        let _ = handle.update(cx, |_, window, cx| {
                            self.sync_analytics_level(window, cx);
                            self.sync_plant_blacklist_selector(window, cx);
                        });
                    }
                }
                let lazy_activity_section = if self.page == Page::Activities {
                    match self.page_section.as_str() {
                        "petDiary" => Some("petDiary"),
                        "autumn" => Some("autumn"),
                        "weather" => Some("weather"),
                        _ => None,
                    }
                } else {
                    None
                };
                if let Some(section) = lazy_activity_section {
                    self.load_activity_section(section, cx);
                    return;
                }
                let directory = accounts::read_accounts(&self.app);
                self.accounts.records = directory.records;
                self.accounts.avatars = directory.avatars;
                if self.selected_account.as_ref().is_some_and(|id| {
                    !self.accounts.records.iter().any(|(candidate, _, _)| candidate == id)
                }) {
                    self.selected_account =
                        self.accounts.records.first().map(|(id, _, _)| id.clone());
                    self.refresh(cx);
                    return;
                }
            }
            Err(error) => self.error = Some(error),
        }
        if let Some(gid) = friend_detail_to_refresh {
            if self.page == Page::Friends
                && self.selected_account.as_deref() == Some(cache_account_id.as_str())
            {
                self.open_friend_detail(gid, cx);
            }
        }
        cx.notify();
    }
}
