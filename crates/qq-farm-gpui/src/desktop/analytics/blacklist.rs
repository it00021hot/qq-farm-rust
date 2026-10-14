use super::super::*;

fn save_plant_blacklist(account_id: &str, seed_ids: Vec<i64>) -> Result<Vec<i64>, String> {
    if account_id.is_empty() {
        return Err("Missing account id".into());
    }
    Ok(qq_farm_app::farm::set_plant_blacklist(account_id, seed_ids))
}

impl FarmDesktop {
    pub(in crate::desktop) fn cancel_plant_blacklist_save(&mut self) {
        self.analytics.blacklist_save_generation =
            self.analytics.blacklist_save_generation.wrapping_add(1);
        self.analytics.blacklist_save_pending = false;
        self.analytics.blacklist_feedback_generation =
            self.analytics.blacklist_feedback_generation.wrapping_add(1);
        self.analytics.blacklist_feedback = None;
        self.analytics.blacklist_feedback_is_error = false;
    }

    fn show_plant_blacklist_feedback(
        &mut self,
        message: String,
        is_error: bool,
        cx: &mut Context<Self>,
    ) {
        self.analytics.blacklist_feedback_generation =
            self.analytics.blacklist_feedback_generation.wrapping_add(1);
        let generation = self.analytics.blacklist_feedback_generation;
        self.analytics.blacklist_feedback = Some(message);
        self.analytics.blacklist_feedback_is_error = is_error;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(3)).await;
            let _ = this.update(cx, |this, cx| {
                if this.analytics.blacklist_feedback_generation == generation {
                    this.analytics.blacklist_feedback = None;
                    this.analytics.blacklist_feedback_is_error = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn finish_plant_blacklist_save(
        &mut self,
        account_id: &str,
        generation: u64,
        result: Result<Vec<i64>, String>,
        cx: &mut Context<Self>,
    ) {
        if self.analytics.blacklist_save_generation != generation {
            return;
        }
        self.analytics.blacklist_save_pending = false;
        if self.page != Page::Analytics || self.selected_account.as_deref() != Some(account_id) {
            cx.notify();
            return;
        }

        match result {
            Ok(seed_ids) => {
                self.page_raw["plantBlacklist"] = serde_json::json!(seed_ids);
                self.rows = pages::page_data(Page::Analytics, self.page_raw.clone()).rows;
                let message = crate::i18n::text(
                    &self.preferences.language,
                    "黑名单已更新",
                    "Blacklist updated",
                )
                .to_string();
                self.show_plant_blacklist_feedback(message, false, cx);
            }
            Err(error) => {
                let message = crate::i18n::error_with_detail(
                    &self.preferences.language,
                    "黑名单更新失败",
                    "Failed to update blacklist",
                    error,
                );
                self.show_plant_blacklist_feedback(message, true, cx);
            }
        }

        if let Some(handle) = self.window_handle {
            let _ = handle.update(cx, |_, window, cx| {
                self.sync_plant_blacklist_selector(window, cx);
            });
        }
        cx.notify();
    }

    pub(in crate::desktop) fn replace_plant_blacklist(
        &mut self,
        seed_ids: Vec<i64>,
        cx: &mut Context<Self>,
    ) {
        if self.analytics.blacklist_save_pending {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let mut seen = HashSet::new();
        let seed_ids = seed_ids
            .into_iter()
            .filter(|seed_id| *seed_id > 0 && seen.insert(*seed_id))
            .collect::<Vec<_>>();
        let account_id_guard = account_id.clone();
        let runtime = Arc::clone(&self.tokio_runtime);
        self.analytics.blacklist_save_pending = true;
        self.analytics.blacklist_save_generation =
            self.analytics.blacklist_save_generation.wrapping_add(1);
        let save_generation = self.analytics.blacklist_save_generation;
        self.analytics.blacklist_feedback_generation =
            self.analytics.blacklist_feedback_generation.wrapping_add(1);
        self.analytics.blacklist_feedback = None;
        self.analytics.blacklist_feedback_is_error = false;
        cx.notify();

        if self.test_mode {
            // Keep the test scheduler deterministic; production saves run on the shared Tokio runtime.
            let result = save_plant_blacklist(&account_id_guard, seed_ids);
            self.finish_plant_blacklist_save(&account_id_guard, save_generation, result, cx);
            return;
        }

        let language = self.preferences.language.clone();
        cx.spawn(async move |this, cx| {
            let result =
                runtime.spawn(async move { save_plant_blacklist(&account_id, seed_ids) }).await;
            let result = match result {
                Ok(result) => result,
                Err(error) => Err(crate::i18n::background_task_error(&language, error)),
            };
            let _ = this.update(cx, |this, cx| {
                this.finish_plant_blacklist_save(&account_id_guard, save_generation, result, cx);
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn change_plant_blacklist(
        &mut self,
        seed_id: i64,
        add: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut seed_ids = self
            .page_raw
            .get("plantBlacklist")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
            .collect::<Vec<_>>();
        if add {
            if !seed_ids.contains(&seed_id) {
                seed_ids.push(seed_id);
            }
        } else {
            seed_ids.retain(|existing| *existing != seed_id);
        }
        self.replace_plant_blacklist(seed_ids, cx);
        self.sync_plant_blacklist_selector(window, cx);
    }
    pub(in crate::desktop) fn set_all_plant_blacklist(
        &mut self,
        add_all: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut seed_ids = if add_all {
            self.page_raw
                .get("plantBlacklist")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if add_all {
            for seed_id in self
                .page_raw
                .pointer("/rankings")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|row| {
                    row.get("seedId").and_then(|value| {
                        value.as_i64().or_else(|| value.as_str()?.trim().parse::<i64>().ok())
                    })
                })
            {
                if !seed_ids.contains(&seed_id) {
                    seed_ids.push(seed_id);
                }
            }
        }
        self.replace_plant_blacklist(seed_ids, cx);
        self.sync_plant_blacklist_selector(window, cx);
    }
}
