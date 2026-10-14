use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn set_planting_strategy(
        &mut self,
        strategy: &'static str,
        cx: &mut Context<Self>,
    ) {
        if self.page == Page::Settings {
            self.settings.draft["plantingStrategy"] = serde_json::json!(strategy);
            self.page_raw["plantingStrategy"] = serde_json::json!(strategy);
            self.rows = pages::page_data(Page::Settings, self.page_raw.clone()).rows;
            self.refresh_strategy_preview(cx);
            cx.notify();
            if strategy == "bag_priority"
                && self.page_raw.get("bagSeeds").is_none_or(serde_json::Value::is_null)
            {
                if let Some(account_id) = self.selected_account.clone() {
                    let account_id_check = account_id.clone();
                    let app = Arc::clone(&self.app);
                    let runtime = Arc::clone(&self.tokio_runtime);
                    cx.spawn(async move |this, cx| {
                        let result =
                            runtime
                                .spawn(async move {
                                    qq_farm_app::farm::bag_seeds(&app, &account_id).await
                                })
                                .await;
                        let _ = this.update(cx, |this, cx| {
                            if this.page != Page::Settings
                                || this.selected_account.as_deref()
                                    != Some(account_id_check.as_str())
                                || this
                                    .settings
                                    .draft
                                    .get("plantingStrategy")
                                    .and_then(serde_json::Value::as_str)
                                    != Some("bag_priority")
                            {
                                return;
                            }
                            match result {
                                Ok(Ok(seeds)) => {
                                    this.page_raw["bagSeeds"] = seeds;
                                    if let Some(raw) = this.page_raw.as_object_mut() {
                                        raw.remove("bagSeedsError");
                                    }
                                }
                                Ok(Err(error)) => {
                                    this.page_raw["bagSeeds"] = serde_json::Value::Null;
                                    this.page_raw["bagSeedsError"] =
                                        serde_json::json!(error.to_string());
                                }
                                Err(error) => {
                                    this.page_raw["bagSeeds"] = serde_json::Value::Null;
                                    this.page_raw["bagSeedsError"] =
                                        serde_json::json!(crate::i18n::error_with_detail(
                                            &this.preferences.language,
                                            "读取背包种子失败",
                                            "Could not read inventory seeds",
                                            error,
                                        ));
                                }
                            }
                            cx.notify();
                        });
                    })
                    .detach();
                }
            }
            return;
        }
        let Some(account_id) = self.selected_account.as_deref() else { return };
        match qq_farm_app::settings::save_settings(
            &self.app,
            account_id,
            serde_json::json!({"plantingStrategy": strategy}),
        ) {
            Ok(_) => self.refresh(cx),
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }
}
