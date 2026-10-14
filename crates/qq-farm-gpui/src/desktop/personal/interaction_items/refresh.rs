use super::super::super::*;

fn merge_interaction_items(raw: &mut serde_json::Value, result: Result<serde_json::Value, String>) {
    let Ok(items) = result else { return };
    raw["interactionItems"] = items;
    if let Some(errors) = raw.get_mut("sectionErrors").and_then(serde_json::Value::as_object_mut) {
        errors.remove("interactionItems");
    }
}

impl FarmDesktop {
    pub(in crate::desktop) fn refresh_personal_interaction_items(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.quitting || self.page != Page::Personal || self.page_section != "interactionItems" {
            return;
        }
        if self.farm.interaction.refreshing {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };

        self.farm.interaction.refresh_generation =
            self.farm.interaction.refresh_generation.wrapping_add(1);
        let generation = self.farm.interaction.refresh_generation;
        let revision = self.request_revision;
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let request_account_id = account_id.clone();
        self.farm.interaction.refreshing = true;
        cx.notify();
        if self.test_mode {
            self.farm.interaction.refreshing = false;
            cx.notify();
            return;
        }

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::friend::farm_interaction_items(&app, &request_account_id)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.interaction.refresh_generation != generation {
                    return;
                }
                this.farm.interaction.refreshing = false;
                if revision != this.request_revision
                    || this.selected_account.as_deref() != Some(account_id.as_str())
                    || this.page != Page::Personal
                    || this.page_section != "interactionItems"
                {
                    cx.notify();
                    return;
                }

                match result {
                    Ok(Ok(items)) => merge_interaction_items(&mut this.page_raw, Ok(items)),
                    Ok(Err(error)) => {
                        merge_interaction_items(&mut this.page_raw, Err(error));
                    }
                    Err(error) => {
                        merge_interaction_items(&mut this.page_raw, Err(error.to_string()));
                    }
                }
                if let Some(handle) = this.window_handle {
                    let _ = handle.update(cx, |_, window, cx| {
                        this.sync_personal_interaction_picker(window, cx);
                    });
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::merge_interaction_items;
    use serde_json::json;

    #[test]
    fn interaction_refresh_replaces_only_its_section_and_keeps_old_data_on_failure() {
        let mut raw = json!({
            "lands": {"lands": [{"id": 1}]},
            "interactionItems": {"items": [{"itemId": 4}]},
            "sectionErrors": {"interactionItems": "old", "lands": "keep"}
        });

        merge_interaction_items(&mut raw, Err("offline".into()));
        assert_eq!(raw["interactionItems"]["items"][0]["itemId"], 4);

        merge_interaction_items(&mut raw, Ok(json!({"items": [{"itemId": 8}]})));
        assert_eq!(raw["interactionItems"]["items"][0]["itemId"], 8);
        assert_eq!(raw["lands"]["lands"][0]["id"], 1);
        assert_eq!(raw["sectionErrors"]["lands"], "keep");
        assert!(raw["sectionErrors"].get("interactionItems").is_none());
    }
}
