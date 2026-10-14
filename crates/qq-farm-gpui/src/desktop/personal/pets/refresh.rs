use super::super::super::*;

pub(super) fn merge_pet_snapshot(
    raw: &mut serde_json::Value,
    info: Option<serde_json::Value>,
    gift_status: Option<serde_json::Value>,
) {
    let mut errors = raw
        .get("sectionErrors")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(info) = info {
        raw["pets"] = info;
        errors.remove("pets");
    }
    if let Some(gift_status) = gift_status {
        raw["petGiftStatus"] = gift_status;
        errors.remove("petGiftStatus");
    }
    raw["sectionErrors"] = serde_json::Value::Object(errors);
}

impl FarmDesktop {
    pub(in crate::desktop) fn refresh_personal_pets(&mut self, cx: &mut Context<Self>) {
        if self.quitting || self.page != Page::Personal || self.page_section != "pets" {
            return;
        }
        if self.test_mode {
            if self.farm.pets.activation_refresh_pending {
                self.farm.pets.activation_refresh_pending = false;
                self.farm.pets.activating = false;
            }
            self.farm.pets.refreshing = false;
            cx.notify();
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };

        self.farm.pets.refresh_generation = self.farm.pets.refresh_generation.wrapping_add(1);
        let generation = self.farm.pets.refresh_generation;
        let revision = self.request_revision;
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let request_account_id = account_id.clone();
        self.farm.pets.refreshing = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    tokio::join!(
                        qq_farm_app::pets::pet_info(&app, &request_account_id),
                        qq_farm_app::pets::dog_skill_gifts_status(&app, &request_account_id)
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.pets.refresh_generation != generation {
                    return;
                }
                this.farm.pets.refreshing = false;
                if this.farm.pets.activation_refresh_pending {
                    this.farm.pets.activation_refresh_pending = false;
                    this.farm.pets.activating = false;
                }
                if revision != this.request_revision
                    || this.selected_account.as_deref() != Some(account_id.as_str())
                    || this.page != Page::Personal
                    || this.page_section != "pets"
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok((info, gifts)) => {
                        merge_pet_snapshot(&mut this.page_raw, info.ok(), gifts.ok());
                    }
                    Err(error) => {
                        let language = &this.preferences.language;
                        this.error = Some(crate::i18n::background_task_error(language, error));
                    }
                }
                if let Some(handle) = this.window_handle {
                    let _ = handle.update(cx, |_, window, cx| {
                        this.sync_personal_pet_food_inputs(window, cx);
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
    use super::merge_pet_snapshot;
    use serde_json::json;

    #[test]
    fn pet_refresh_updates_only_pet_sections_and_keeps_failed_data() {
        let mut raw = json!({
            "lands": {"lands": [{"id": 1}]},
            "pets": {"activeDogId": 1},
            "petGiftStatus": {"pending": 2},
            "sectionErrors": {"pets": "old", "petGiftStatus": "old", "lands": "keep"}
        });

        merge_pet_snapshot(&mut raw, Some(json!({"activeDogId": 3})), None);

        assert_eq!(raw["pets"]["activeDogId"], 3);
        assert_eq!(raw["petGiftStatus"]["pending"], 2);
        assert_eq!(raw["lands"]["lands"][0]["id"], 1);
        assert_eq!(raw["sectionErrors"]["petGiftStatus"], "old");
        assert_eq!(raw["sectionErrors"]["lands"], "keep");
        assert!(raw["sectionErrors"].get("pets").is_none());
    }
}
