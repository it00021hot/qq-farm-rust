use super::super::*;
use gpui_kit::component::WindowExt;

impl FarmDesktop {
    pub(in crate::desktop) fn open_account_form(
        &mut self,
        record: Option<serde_json::Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.accounts.wx_generation = self.accounts.wx_generation.wrapping_add(1);
        if let Some(path) = self.accounts.wx_qr_path.take() {
            let _ = std::fs::remove_file(path);
        }
        let id = record.as_ref().and_then(|record| record.get("id")).and_then(|value| {
            value.as_str().map(str::to_string).or_else(|| value.as_i64().map(|v| v.to_string()))
        });
        let name = record
            .as_ref()
            .and_then(|record| record.get("name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let remark = record
            .as_ref()
            .and_then(|record| record.get("remark"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(name);
        let code = record
            .as_ref()
            .and_then(|record| record.get("code"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        self.accounts.name.update(cx, |state, cx| state.set_value(name, window, cx));
        self.accounts.code.update(cx, |state, cx| state.set_value(code, window, cx));
        self.accounts.remark.update(cx, |state, cx| state.set_value(remark, window, cx));
        self.accounts.editing_id = id;
        self.accounts.platform = record
            .as_ref()
            .and_then(|record| record.get("platform"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("qq")
            .to_string();
        self.accounts.login_tab = "code".into();
        self.accounts.form_error = None;
        self.accounts.form_saving = false;
        self.accounts.wx_mode = "local".into();
        self.accounts.wx_status.clear();
        self.accounts.wx_error = None;
        self.accounts.wx_task_id = None;
        self.accounts.wx_session_id = None;
        self.accounts.wx_quick_profile = None;
        self.accounts.form_open = true;
        let desktop = cx.weak_entity();
        window.open_sheet(cx, move |sheet, _, cx| {
            if let Some(desktop) = desktop.upgrade() {
                desktop.update(cx, |this, cx| this.render_account_sheet(sheet, cx))
            } else {
                sheet
            }
        });
        self.accounts.name.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    pub(in crate::desktop) fn dismiss_account_sheet(&self, cx: &mut Context<Self>) {
        if let Some(handle) = self.window_handle {
            let _ = handle.update(cx, |_, window, cx| window.close_sheet(cx));
        }
    }

    pub(in crate::desktop) fn close_account_form(&mut self) {
        self.accounts.wx_generation = self.accounts.wx_generation.wrapping_add(1);
        if let Some(path) = self.accounts.wx_qr_path.take() {
            let _ = std::fs::remove_file(path);
        }
        self.accounts.form_open = false;
        self.accounts.form_saving = false;
        self.accounts.form_error = None;
        self.accounts.wx_task_id = None;
        self.accounts.wx_session_id = None;
        self.accounts.wx_quick_profile = None;
    }

    pub(in crate::desktop) fn save_account(&mut self, cx: &mut Context<Self>) {
        if self.accounts.form_saving {
            return;
        }
        if self.accounts.login_tab == "wx" {
            self.accounts.form_error = Some("请完成微信授权后保存账号".into());
            cx.notify();
            return;
        }
        let raw_code = self.accounts.code.read(cx).value().to_string();
        let raw_code = raw_code.trim();
        if raw_code.is_empty() {
            self.accounts.form_error = Some("请输入登录 code 或粘贴完整登录链接".into());
            cx.notify();
            return;
        }
        let (platform, _) = parse_account_login_input(raw_code, &self.accounts.platform);
        self.submit_account_code(raw_code.to_string(), platform, None, false, cx);
    }

    pub(in crate::desktop) fn submit_account_code(
        &mut self,
        code: String,
        platform: String,
        nickname: Option<String>,
        ensure_running: bool,
        cx: &mut Context<Self>,
    ) {
        if self.accounts.form_saving {
            return;
        }
        let mut name = self.accounts.name.read(cx).value().to_string().trim().to_string();
        if name.is_empty() {
            name = nickname.unwrap_or_default().trim().to_string();
        }
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let id = self.accounts.editing_id.clone();
        let remark = self.accounts.remark.read(cx).value().to_string();
        let generation = self.accounts.wx_generation;
        let request = qq_farm_app::accounts::UpsertAccountRequest {
            id: id.clone(),
            name: Some(name),
            remark: Some(remark),
            code: Some(code),
            platform: Some(platform),
            ..Default::default()
        };
        self.accounts.form_saving = true;
        self.accounts.form_error = None;
        self.accounts.wx_status = "正在保存账号…".into();
        cx.notify();

        // Keep the GPUI test scheduler deterministic: the isolated test context has no external
        // login worker, so complete the local-owner facade call inline in test mode.
        #[cfg(test)]
        if self.test_mode {
            let result =
                runtime.block_on(upsert_and_maybe_start_account(&app, request, ensure_running));
            if generation == self.accounts.wx_generation {
                self.finish_account_submission(id, result, cx);
            }
            return;
        }

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    upsert_and_maybe_start_account(&app, request, ensure_running).await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.accounts.wx_generation {
                    return;
                }
                match result {
                    Ok(result) => this.finish_account_submission(id, result, cx),
                    Err(error) => {
                        this.accounts.form_saving = false;
                        this.accounts.form_error = Some(format!("账号保存任务失败：{error}"));
                        this.accounts.wx_status = "保存账号失败".into();
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn finish_account_submission(
        &mut self,
        id: Option<String>,
        result: Result<Option<String>, String>,
        cx: &mut Context<Self>,
    ) {
        self.accounts.form_saving = false;
        match result {
            Ok(start_error) => {
                self.show_account_success(
                    if id.is_some() { "更新成功" } else { "添加成功" },
                    if id.is_some() { "Update Success" } else { "Add Success" },
                    cx,
                );
                self.close_account_form();
                self.dismiss_account_sheet(cx);
                self.accounts.selected.clear();
                self.set_page(Page::Accounts, cx);
                if let Some(error) = start_error {
                    self.error = Some(super::localized_account_message(
                        &self.preferences.language,
                        &if id.is_some() {
                            format!("更新成功，自动启动失败，请手动重新登录：{error}")
                        } else {
                            format!("添加成功，自动启动失败，请手动点击启动：{error}")
                        },
                    ));
                    cx.notify();
                }
            }
            Err(error) => {
                self.accounts.form_error = Some(error);
                self.accounts.wx_status = "保存账号失败".into();
                cx.notify();
            }
        }
    }

    pub(in crate::desktop) fn confirm_delete_account(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let ids = self.accounts.pending_delete.take().into_iter().collect();
        self.delete_accounts(ids, cx);
    }

    pub(in crate::desktop) fn delete_accounts(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        if self.loading || ids.is_empty() {
            return;
        }
        // The UI test harness uses an isolated local-owner app context; execute this synchronous
        // facade call inline there so Tokio worker wakeups do not race the deterministic GPUI loop.
        #[cfg(test)]
        if self.test_mode {
            let result = delete_accounts_with_app(&self.app, ids);
            self.complete_account_deletion(result, cx);
            return;
        }

        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        self.loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { Ok::<_, String>(delete_accounts_with_app(&app, ids)) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(Ok((deleted_count, failures))) => {
                        this.complete_account_deletion((deleted_count, failures), cx);
                    }
                    Ok(Err(error)) => {
                        this.error = Some(error);
                        cx.notify();
                    }
                    Err(error) => {
                        this.error = Some(crate::i18n::background_task_error(
                            &this.preferences.language,
                            error,
                        ));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn complete_account_deletion(
        &mut self,
        (deleted_count, failures): (usize, Vec<String>),
        cx: &mut Context<Self>,
    ) {
        self.accounts.selected.clear();
        if failures.is_empty() && deleted_count > 0 {
            self.show_account_success("删除成功", "Delete Success", cx);
        }
        self.refresh(cx);
        if !failures.is_empty() {
            self.error = Some(super::localized_account_message(
                &self.preferences.language,
                &format!("部分账号未删除：{}", failures.join("；")),
            ));
            cx.notify();
        }
    }
}

fn delete_accounts_with_app(
    app: &qq_farm_app::session::AppContext,
    ids: Vec<String>,
) -> (usize, Vec<String>) {
    let policy = qq_farm_app::accounts::AclPolicy::LocalOwner;
    let mut failures = Vec::new();
    let mut deleted_count = 0;
    for id in ids {
        match qq_farm_app::accounts::delete_account(app, &policy, &id) {
            Ok(()) => deleted_count += 1,
            Err(error) => failures.push(format!("{id}: {error}")),
        }
    }
    (deleted_count, failures)
}
