use super::super::*;

use std::time::Duration;

impl FarmDesktop {
    pub(in crate::desktop) fn apply_qq_bot_binding_snapshot(
        &mut self,
        offline_reminder: serde_json::Value,
        binding_status: serde_json::Value,
    ) {
        self.settings.draft["offlineReminder"] = offline_reminder.clone();
        self.page_raw["offlineReminder"] = offline_reminder;
        self.page_raw["qqBotBinding"] = binding_status;
        self.rows = pages::page_data(Page::Settings, self.page_raw.clone()).rows;
    }

    pub(in crate::desktop) fn save_offline_reminder(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.settings.offline_test_loading {
            return;
        }
        let config = self.offline_reminder_snapshot(cx);
        self.settings.draft["offlineReminder"] = config.clone();
        self.page_raw["offlineReminder"] = config.clone();
        qq_farm_app::settings::set_offline_reminder(Some("local"), config);
        self.show_settings_notice("下线提醒已保存", "Offline alert saved", false, cx);
    }

    fn offline_reminder_snapshot(&self, cx: &Context<Self>) -> serde_json::Value {
        let mut config = self.settings.draft.get("offlineReminder").cloned().unwrap_or_default();
        if !config.is_object() {
            config = serde_json::json!({});
        }
        config["qqBot"]["appId"] =
            serde_json::json!(self.settings.qq_bot_app_id.read(cx).value().trim().to_string());
        config["qqBot"]["clientSecret"] =
            serde_json::json!(self.settings.qq_bot_secret.read(cx).value().trim().to_string());
        config["endpoint"] =
            serde_json::json!(self.settings.offline_endpoint.read(cx).value().trim().to_string());
        config["token"] =
            serde_json::json!(self.settings.offline_token.read(cx).value().trim().to_string());
        config["secret"] =
            serde_json::json!(self.settings.offline_secret.read(cx).value().trim().to_string());
        config
    }

    pub(in crate::desktop) fn test_offline_reminder(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.settings.offline_test_loading {
            return;
        }
        self.clear_settings_notice();
        let config = self.offline_reminder_snapshot(cx);
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        self.settings.offline_test_loading = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::settings::test_offline_reminder(&app, Some("local"), config).await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let result = match result {
                    Ok(Ok(value)) => Ok(value),
                    Ok(Err(error)) => Err(error.to_string()),
                    Err(error) => {
                        Err(crate::i18n::background_task_error(&this.preferences.language, error))
                    }
                };
                this.complete_offline_reminder_test(result, cx);
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn complete_offline_reminder_test(
        &mut self,
        result: Result<serde_json::Value, String>,
        cx: &mut Context<Self>,
    ) {
        self.settings.offline_test_loading = false;
        match result {
            Ok(value) if value.get("ok").and_then(serde_json::Value::as_bool) == Some(true) => {
                self.show_settings_notice("测试消息发送成功", "Test message sent", false, cx);
                self.error = None;
                cx.notify();
            }
            Ok(value) => {
                let detail = value
                    .get("msg")
                    .and_then(serde_json::Value::as_str)
                    .filter(|msg| !msg.trim().is_empty())
                    .unwrap_or("unknown");
                self.error = Some(format!(
                    "{}: {detail}",
                    crate::i18n::text(&self.preferences.language, "测试失败", "Test failed")
                ));
                cx.notify();
            }
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }

    pub(in crate::desktop) fn start_qq_bot_binding(&mut self, cx: &mut Context<Self>) {
        if qq_bot_binding_in_flight(
            self.settings.qq_bot_session_id.is_some(),
            self.settings.qq_bot_bind_loading,
        ) {
            return;
        }
        let app_id = self.settings.qq_bot_app_id.read(cx).value().trim().to_string();
        let secret = self.settings.qq_bot_secret.read(cx).value().trim().to_string();
        if app_id.is_empty() || secret.is_empty() {
            self.error = Some(
                crate::i18n::text(
                    &self.preferences.language,
                    "请先填写 AppID 和 AppSecret。",
                    "Enter AppID and AppSecret first.",
                )
                .into(),
            );
            cx.notify();
            return;
        }
        let config = self.offline_reminder_snapshot(cx);
        self.settings.qq_bot_bind_loading = true;
        qq_farm_app::settings::set_offline_reminder(Some("local"), config.clone());
        self.settings.draft["offlineReminder"] = config.clone();
        self.page_raw["offlineReminder"] = config;
        self.settings.qq_bot_poll_generation = self.settings.qq_bot_poll_generation.wrapping_add(1);
        self.settings.qq_bot_session_id = None;
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        self.settings.qq_bot_qr_path = None;
        self.settings.qq_bot_status = Some("正在创建 QQ Bot 绑定二维码…".into());
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { qq_farm_app::qq_bot_bind::start_qq_bot_bind(&app, "local") })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.settings.qq_bot_bind_loading = false;
                match result {
                    Ok(Ok(start)) => {
                        this.settings.qq_bot_session_id = Some(start.session_id.clone());
                        let path = start.qr_data_url.split_once(',').and_then(|(_, encoded)| {
                            use base64::Engine as _;
                            let bytes =
                                base64::engine::general_purpose::STANDARD.decode(encoded).ok()?;
                            let path = std::env::temp_dir()
                                .join(format!("qq-farm-gpui-qqbot-{}.png", start.session_id));
                            std::fs::write(&path, bytes).ok()?;
                            Some(path)
                        });
                        this.settings.qq_bot_qr_path = path;
                        this.settings.qq_bot_status = Some("请使用 QQ 扫描二维码完成绑定。".into());
                        this.start_qq_bot_binding_polling(start.session_id, cx);
                    }
                    Ok(Err(error)) => this.error = Some(error.to_string()),
                    Err(error) => {
                        this.error = Some(crate::i18n::error_with_detail(
                            &this.preferences.language,
                            "绑定任务失败",
                            "Binding failed",
                            error,
                        ))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn start_qq_bot_binding_polling(
        &mut self,
        session_id: String,
        cx: &mut Context<Self>,
    ) {
        self.settings.qq_bot_poll_generation = self.settings.qq_bot_poll_generation.wrapping_add(1);
        let generation = self.settings.qq_bot_poll_generation;
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let session_id_check = session_id.clone();
        cx.spawn(async move |this, cx| loop {
            let app = Arc::clone(&app);
            let session = session_id.clone();
            let result = runtime
                .spawn(async move {
                    let poll = qq_farm_app::qq_bot_bind::poll_qq_bot_bind(&app, &session);
                    if let Some(binding) = poll.binding.clone() {
                        qq_farm_app::qq_bot_bind::save_qq_bot_binding("local", binding);
                    }
                    poll
                })
                .await;
            let keep_polling = this.update(cx, |this, cx| {
                if generation != this.settings.qq_bot_poll_generation
                    || this.settings.qq_bot_session_id.as_deref() != Some(session_id_check.as_str())
                {
                    return false;
                }
                match result {
                    Ok(poll) => match poll.status {
                        qq_farm_core::services::qq_bot::BindPollStatus::Pending => {
                            this.settings.qq_bot_status = Some("等待 QQ 扫码并确认绑定…".into());
                        }
                        qq_farm_core::services::qq_bot::BindPollStatus::Bound => {
                            this.settings.qq_bot_status = Some("QQ Bot 已绑定。".into());
                            this.settings.qq_bot_session_id = None;
                            this.settings.qq_bot_qr_path = None;
                            this.apply_qq_bot_binding_snapshot(
                                qq_farm_app::settings::offline_reminder_view(Some("local")),
                                qq_farm_app::qq_bot_bind::qq_bot_bind_status(Some("local")),
                            );
                            cx.notify();
                            return false;
                        }
                        qq_farm_core::services::qq_bot::BindPollStatus::Expired => {
                            this.settings.qq_bot_status =
                                Some("二维码已过期，请重新发起绑定。".into());
                            this.settings.qq_bot_session_id = None;
                            this.settings.qq_bot_qr_path = None;
                            cx.notify();
                            return false;
                        }
                    },
                    Err(error) => {
                        this.error = Some(crate::i18n::error_with_detail(
                            &this.preferences.language,
                            "绑定状态查询失败，将自动重试",
                            "Could not check binding status; retrying automatically",
                            error,
                        ));
                    }
                }
                cx.notify();
                true
            });
            if !matches!(keep_polling, Ok(true)) {
                break;
            }
            cx.background_executor().timer(Duration::from_secs(2)).await;
        })
        .detach();
    }

    pub(in crate::desktop) fn unbind_qq_bot(&mut self, cx: &mut Context<Self>) {
        self.settings.qq_bot_poll_generation = self.settings.qq_bot_poll_generation.wrapping_add(1);
        self.settings.qq_bot_session_id = None;
        self.settings.qq_bot_bind_loading = false;
        self.settings.qq_bot_qr_path = None;
        qq_farm_app::qq_bot_bind::unbind_qq_bot("local");
        self.settings.qq_bot_status = Some("QQ Bot 已解除绑定。".into());
        self.apply_qq_bot_binding_snapshot(
            qq_farm_app::settings::offline_reminder_view(Some("local")),
            qq_farm_app::qq_bot_bind::qq_bot_bind_status(Some("local")),
        );
        cx.notify();
    }

    pub(in crate::desktop) fn open_bot_invite(&mut self) {
        let Some(url) =
            self.page_raw.pointer("/qqBotBinding/botInviteUrl").and_then(serde_json::Value::as_str)
        else {
            return;
        };
        if let Err(error) = Self::open_external_url(url) {
            self.error = Some(crate::i18n::error_with_detail(
                &self.preferences.language,
                "无法打开链接",
                "Could not open the link",
                error,
            ));
        }
    }

    pub(in crate::desktop) fn open_external_url(url: &str) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        let mut command = std::process::Command::new("open");
        #[cfg(target_os = "windows")]
        let mut command = std::process::Command::new("rundll32");
        #[cfg(target_os = "windows")]
        command.arg("url.dll,FileProtocolHandler");
        #[cfg(target_os = "linux")]
        let mut command = std::process::Command::new("xdg-open");
        command.arg(url).spawn().map(|_| ()).map_err(|error| error.to_string())
    }
}

fn qq_bot_binding_in_flight(has_session: bool, request_pending: bool) -> bool {
    has_session || request_pending
}

#[cfg(test)]
mod tests {
    use super::qq_bot_binding_in_flight;

    #[test]
    fn qq_bot_binding_rejects_a_second_start_while_the_request_or_session_is_active() {
        assert!(!qq_bot_binding_in_flight(false, false));
        assert!(qq_bot_binding_in_flight(false, true));
        assert!(qq_bot_binding_in_flight(true, false));
        assert!(qq_bot_binding_in_flight(true, true));
    }
}
