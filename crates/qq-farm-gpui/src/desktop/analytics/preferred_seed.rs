use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn set_preferred_seed(&mut self, seed_id: i64, cx: &mut Context<Self>) {
        if self.page == Page::Settings {
            self.set_setting_draft_value("preferredSeedId", serde_json::json!(seed_id), cx);
            if self.settings.draft.get("plantingStrategy").and_then(serde_json::Value::as_str)
                == Some("bag_priority")
                && self
                    .settings
                    .draft
                    .get("bagSeedFallbackStrategy")
                    .and_then(serde_json::Value::as_str)
                    == Some("preferred")
            {
                self.reset_bag_seed_priority(cx);
            }
            return;
        }
        let Some(account_id) = self.selected_account.as_deref() else { return };
        match qq_farm_app::settings::save_settings(
            &self.app,
            account_id,
            serde_json::json!({"preferredSeedId": seed_id}),
        ) {
            Ok(_) => self.refresh(cx),
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }
}
