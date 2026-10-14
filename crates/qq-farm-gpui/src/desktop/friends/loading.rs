use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn refresh_friend_records(&mut self, cx: &mut Context<Self>) {
        if self.friends.records_refresh_loading || self.page != Page::Friends {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        self.friends.records_refresh_generation =
            self.friends.records_refresh_generation.wrapping_add(1);
        let generation = self.friends.records_refresh_generation;
        let revision = self.request_revision;
        let account_id_check = account_id.clone();
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        self.friends.records_refresh_loading = true;
        cx.notify();
        if self.test_mode {
            self.friends.records_refresh_loading = false;
            cx.notify();
            return;
        }

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::friend::interact_records(&app, &account_id)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.friends.records_refresh_generation != generation {
                    return;
                }
                this.friends.records_refresh_loading = false;
                if this.request_revision != revision
                    || this.page != Page::Friends
                    || this.page_section != "records"
                    || this.selected_account.as_deref() != Some(account_id_check.as_str())
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(records)) => {
                        this.page_raw["records"] = records;
                        this.page_raw["recordsError"] = serde_json::Value::Null;
                    }
                    Ok(Err(error)) => {
                        this.page_raw["records"] = serde_json::json!([]);
                        this.page_raw["recordsError"] = serde_json::json!(error);
                    }
                    Err(error) => {
                        this.page_raw["records"] = serde_json::json!([]);
                        this.page_raw["recordsError"] =
                            serde_json::json!(crate::i18n::background_task_error(
                                &this.preferences.language,
                                error.to_string(),
                            ));
                    }
                }
                this.rows = pages::page_data(Page::Friends, this.page_raw.clone()).rows;
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn refresh_friend_interaction_items(&mut self, cx: &mut Context<Self>) {
        if self.page != Page::Friends {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        self.friends.interaction_items_refresh_generation =
            self.friends.interaction_items_refresh_generation.wrapping_add(1);
        let generation = self.friends.interaction_items_refresh_generation;
        let revision = self.request_revision;
        let account_id_check = account_id.clone();
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        self.friends.interaction_items_refresh_loading = true;
        cx.notify();
        if self.test_mode {
            self.friends.interaction_items_refresh_loading = false;
            cx.notify();
            return;
        }

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::friend::friend_interaction_items(&app, &account_id)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.friends.interaction_items_refresh_generation != generation {
                    return;
                }
                this.friends.interaction_items_refresh_loading = false;
                if this.request_revision != revision
                    || this.page != Page::Friends
                    || this.page_section != "interactionItems"
                    || this.selected_account.as_deref() != Some(account_id_check.as_str())
                {
                    cx.notify();
                    return;
                }
                if let Ok(Ok(items)) = result {
                    this.page_raw["interactionItems"] = items;
                    this.page_raw["interactionItemsError"] = serde_json::Value::Null;
                    let available = this
                        .page_raw
                        .pointer("/interactionItems/items")
                        .and_then(serde_json::Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|item| {
                            item.get("itemId").or_else(|| item.get("id")).and_then(|value| {
                                value.as_i64().or_else(|| value.as_str()?.trim().parse().ok())
                            })
                        })
                        .collect::<Vec<_>>();
                    if !this
                        .friends
                        .selected_interaction_item_id
                        .is_some_and(|id| available.contains(&id))
                    {
                        this.friends.selected_interaction_item_id = available.first().copied();
                    }
                    this.rows = pages::page_data(Page::Friends, this.page_raw.clone()).rows;
                    if let Some(handle) = this.window_handle {
                        let _ = handle.update(cx, |_, window, cx| {
                            this.sync_friend_interaction_picker(window, cx);
                        });
                    }
                }
                // Vue keeps the previous item list when a manual refresh fails.
                cx.notify();
            });
        })
        .detach();
    }
}
