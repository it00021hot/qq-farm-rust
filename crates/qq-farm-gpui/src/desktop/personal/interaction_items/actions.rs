use super::super::super::*;
use super::picker::parse_land_ids;

impl FarmDesktop {
    pub(in crate::desktop) fn run_interaction_item(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.farm.interaction.using {
            return;
        }
        let Some(item_id) = self.farm.interaction.picker.read(cx).selected_value().copied() else {
            return;
        };
        let land_ids = parse_land_ids(&self.farm.interaction.land_ids.read(cx).value());
        if land_ids.is_empty() {
            self.set_personal_interaction_notice(
                crate::i18n::text(&self.preferences.language, "例如： 1,2,3", "e.g. 1,2,3").into(),
                false,
                cx,
            );
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let generation = self.farm.interaction.generation;
        self.farm.interaction.using = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let worker_account_id = account_id.clone();
            let result = runtime
                .spawn(async move {
                    qq_farm_app::friend::farm_interaction_items_use(
                        &app,
                        &worker_account_id,
                        item_id,
                        land_ids,
                    )
                    .await
                    .map_err(|e| e.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.interaction.generation != generation {
                    return;
                }
                this.farm.interaction.using = false;
                if this.selected_account.as_deref() != Some(account_id.as_str())
                    || this.page != Page::Personal
                    || this.page_section != "interactionItems"
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(data)) => {
                        let message = data
                            .get("message")
                            .and_then(serde_json::Value::as_str)
                            .filter(|message| !message.trim().is_empty())
                            .unwrap_or(crate::i18n::text(
                                &this.preferences.language,
                                "批量使用完成",
                                "Batch use done",
                            ));
                        this.set_personal_interaction_notice(message.into(), true, cx);
                        this.refresh_personal_interaction_items(cx);
                    }
                    Ok(Err(error)) => {
                        this.set_personal_interaction_notice(error, false, cx);
                    }
                    Err(error) => {
                        this.set_personal_interaction_notice(
                            if this.preferences.language == "en-US" {
                                format!("Background task failed: {error}")
                            } else {
                                format!("后台任务已结束：{error}")
                            },
                            false,
                            cx,
                        );
                    }
                }
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn invalidate_personal_interaction(&mut self) {
        self.farm.interaction.generation = self.farm.interaction.generation.wrapping_add(1);
        self.farm.interaction.using = false;
        self.farm.interaction.refresh_generation =
            self.farm.interaction.refresh_generation.wrapping_add(1);
        self.farm.interaction.refreshing = false;
        self.farm.interaction.notice = None;
        self.farm.interaction.notice_generation =
            self.farm.interaction.notice_generation.wrapping_add(1);
    }

    fn set_personal_interaction_notice(
        &mut self,
        message: String,
        success: bool,
        cx: &mut Context<Self>,
    ) {
        self.farm.interaction.notice_generation =
            self.farm.interaction.notice_generation.wrapping_add(1);
        let generation = self.farm.interaction.notice_generation;
        self.farm.interaction.notice = Some((message, success));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.interaction.notice_generation == generation {
                    this.farm.interaction.notice = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
