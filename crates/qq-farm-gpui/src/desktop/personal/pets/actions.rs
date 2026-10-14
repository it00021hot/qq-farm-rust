use super::*;

impl PetAction {
    async fn execute(
        self,
        app: &AppContext,
        account_id: &str,
    ) -> qq_farm_app::AppResult<serde_json::Value> {
        match self {
            Self::Activate(id) => qq_farm_app::pets::pet_activate(app, account_id, id).await,
            Self::Deploy(id) => qq_farm_app::pets::pet_deploy(app, account_id, id).await,
            Self::Withdraw => qq_farm_app::pets::pet_withdraw(app, account_id).await,
            Self::Food(id, count) => {
                qq_farm_app::pets::pet_food_use(app, account_id, id, count, 0).await
            }
            Self::Claim => qq_farm_app::pets::dog_skill_gifts_claim(app, account_id).await,
            Self::Logs => qq_farm_app::pets::pet_protect_logs(app, account_id).await,
        }
    }

    fn success_message(self, result: &serde_json::Value, language: &str) -> String {
        match self {
            Self::Activate(_) => {
                crate::i18n::text(language, "宠物激活成功", "Pet activated").into()
            }
            Self::Deploy(_) => crate::i18n::text(language, "宠物上场成功", "Pet deployed").into(),
            Self::Withdraw => crate::i18n::text(language, "宠物已收回", "Pet withdrawn").into(),
            Self::Food(..) => crate::i18n::text(language, "喂食成功", "Fed").into(),
            Self::Claim => crate::i18n::text(
                language,
                &format!(
                    "已领取礼包: {}",
                    result.get("claimed").and_then(serde_json::Value::as_i64).unwrap_or_default()
                ),
                &format!(
                    "Gifts claimed: {}",
                    result.get("claimed").and_then(serde_json::Value::as_i64).unwrap_or_default()
                ),
            )
            .into(),
            Self::Logs => String::new(),
        }
    }
}

impl FarmDesktop {
    pub(super) fn run_personal_pet_action(&mut self, action: PetAction, cx: &mut Context<Self>) {
        let activation = matches!(action, PetAction::Activate(_));
        if activation && self.farm.pets.activating {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let generation = self.farm.pets.operation_generation;
        if activation {
            self.farm.pets.activating = true;
            cx.notify();
        }
        cx.spawn(async move |this, cx| {
            let worker_account_id = account_id.clone();
            let result = runtime
                .spawn(async move {
                    action
                        .execute(&app, &worker_account_id)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.pets.operation_generation != generation {
                    return;
                }
                if this.selected_account.as_deref() != Some(account_id.as_str())
                    || this.page != Page::Personal
                    || this.page_section != "pets"
                {
                    cx.notify();
                    return;
                }
                if activation {
                    match &result {
                        Ok(Ok(_)) => this.farm.pets.activation_refresh_pending = true,
                        _ => this.farm.pets.activating = false,
                    }
                }
                match result {
                    Ok(Ok(data)) if matches!(action, PetAction::Logs) => {
                        this.farm.pets.logs = data
                            .get("logs")
                            .and_then(serde_json::Value::as_array)
                            .cloned()
                            .unwrap_or_default();
                        if let Some(handle) = this.window_handle {
                            let _ = handle.update(cx, |_, window, cx| {
                                this.show_personal_pet_logs(window, cx)
                            });
                        }
                        cx.notify();
                    }
                    Ok(Ok(data)) => {
                        this.set_personal_pet_notice(
                            action.success_message(&data, &this.preferences.language),
                            true,
                            cx,
                        );
                        this.refresh_personal_pets(cx);
                    }
                    Ok(Err(error)) => this.set_personal_pet_notice(error, false, cx),
                    Err(error) => this.set_personal_pet_notice(
                        if this.preferences.language == "en-US" {
                            format!("Background task failed: {error}")
                        } else {
                            format!("后台任务已结束：{error}")
                        },
                        false,
                        cx,
                    ),
                }
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn invalidate_personal_pet_operations(&mut self) {
        self.farm.pets.operation_generation = self.farm.pets.operation_generation.wrapping_add(1);
        self.farm.pets.refresh_generation = self.farm.pets.refresh_generation.wrapping_add(1);
        self.farm.pets.refreshing = false;
        self.farm.pets.activating = false;
        self.farm.pets.activation_refresh_pending = false;
        self.farm.pets.notice = None;
        self.farm.pets.notice_generation = self.farm.pets.notice_generation.wrapping_add(1);
    }

    fn set_personal_pet_notice(&mut self, message: String, success: bool, cx: &mut Context<Self>) {
        self.farm.pets.notice_generation = self.farm.pets.notice_generation.wrapping_add(1);
        let generation = self.farm.pets.notice_generation;
        self.farm.pets.notice = Some((message, success));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.pets.notice_generation == generation {
                    this.farm.pets.notice = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
