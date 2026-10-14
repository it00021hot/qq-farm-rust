use super::super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn start_wx_qr_flow(
        &mut self,
        hint: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.accounts.login_tab = "wx".into();
        self.accounts.platform = "wx".into();
        self.accounts.wx_generation = self.accounts.wx_generation.wrapping_add(1);
        let generation = self.accounts.wx_generation;
        self.accounts.wx_mode = "qr".into();
        self.accounts.wx_task_id = None;
        self.accounts.wx_session_id = None;
        self.accounts.wx_quick_profile = None;
        self.accounts.wx_status = "正在获取微信登录二维码…".into();
        self.accounts.wx_error = hint;
        if let Some(path) = self.accounts.wx_qr_path.take() {
            let _ = std::fs::remove_file(path);
        }
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::wx_login::create_task(&app.wx_login)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.accounts.wx_generation || !this.accounts.form_open {
                    return;
                }
                match result {
                    Ok(Ok(task)) => {
                        let path = std::env::temp_dir()
                            .join(format!("qq-farm-gpui-wx-{}.jpg", task.task_id));
                        match std::fs::write(&path, task.qr_jpeg) {
                            Ok(()) => {
                                this.accounts.wx_qr_path = Some(path);
                                this.accounts.wx_task_id = Some(task.task_id);
                                this.accounts.wx_status = "等待微信扫码".into();
                                this.poll_wx_login(generation, cx);
                                cx.notify();
                            }
                            Err(error) => {
                                this.accounts.wx_error =
                                    Some(format!("二维码图片写入失败：{error}"));
                                this.accounts.wx_status = "二维码加载失败".into();
                                cx.notify();
                            }
                        }
                    }
                    Ok(Err(error)) => {
                        this.accounts.wx_error = Some(error);
                        this.accounts.wx_status = "二维码获取失败".into();
                        cx.notify();
                    }
                    Err(error) => {
                        this.accounts.wx_error = Some(format!("二维码任务失败：{error}"));
                        this.accounts.wx_status = "二维码获取失败".into();
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn poll_wx_login(&mut self, generation: u64, cx: &mut Context<Self>) {
        let Some(task_id) = self.accounts.wx_task_id.clone() else { return };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(1200)).await;
            let app_for_poll = Arc::clone(&app);
            let task_for_poll = task_id.clone();
            let result = runtime
                .spawn(async move {
                    qq_farm_app::wx_login::poll_status(&app_for_poll.wx_login, &task_for_poll)
                        .await
                        .map(|status| status.status)
                        .map_err(|error| error.to_string())
                })
                .await;
            let status = match result {
                Ok(Ok(status)) => status,
                Ok(Err(error)) => {
                    let _ = this.update(cx, |this, cx| {
                        if generation == this.accounts.wx_generation {
                            this.accounts.wx_error = Some(error);
                            this.accounts.wx_status = "登录状态检查失败".into();
                            cx.notify();
                        }
                    });
                    break;
                }
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        if generation == this.accounts.wx_generation {
                            this.accounts.wx_error = Some(format!("登录状态检查任务失败：{error}"));
                            this.accounts.wx_status = "登录状态检查失败".into();
                            cx.notify();
                        }
                    });
                    break;
                }
            };
            if generation
                != this.read_with(cx, |this, _| this.accounts.wx_generation).unwrap_or_default()
            {
                break;
            }
            match status.as_str() {
                "waiting" => {
                    let _ = this.update(cx, |this, cx| {
                        if generation == this.accounts.wx_generation {
                            this.accounts.wx_status = "等待微信扫码".into();
                            cx.notify();
                        }
                    });
                }
                "scanned" => {
                    let _ = this.update(cx, |this, cx| {
                        if generation == this.accounts.wx_generation {
                            this.accounts.wx_status = "已扫码，请在手机上确认".into();
                            cx.notify();
                        }
                    });
                }
                "authorized" => {
                    let form = this.update(cx, |this, cx| {
                        if generation != this.accounts.wx_generation || !this.accounts.form_open {
                            return None;
                        }
                        this.accounts.wx_status = "已确认，正在建立登录会话…".into();
                        this.accounts.form_saving = true;
                        this.accounts.wx_error = None;
                        cx.notify();
                        Some((
                            this.accounts.name.read(cx).value().to_string().trim().to_string(),
                            this.accounts.remark.read(cx).value().to_string(),
                            this.accounts.editing_id.clone(),
                            this.accounts.platform.clone(),
                        ))
                    });
                    let Ok(Some((name, remark, id, platform))) = form else { break };
                    let editing = id.is_some();
                    let app_for_save = Arc::clone(&app);
                    let task_for_save = task_id.clone();
                    let result = runtime
                        .spawn(async move {
                            qq_farm_app::wx_login::confirm(&app_for_save.wx_login, &task_for_save)
                                .await
                                .map_err(|error| error.to_string())?;
                            let login = qq_farm_app::wx_login::issue_code(
                                &app_for_save.wx_login,
                                &task_for_save,
                            )
                            .await
                            .map_err(|error| error.to_string())?;
                            let display_name = if name.is_empty() {
                                login.nickname.clone().unwrap_or_default()
                            } else {
                                name
                            };
                            upsert_and_maybe_start_account(
                                &app_for_save,
                                qq_farm_app::accounts::UpsertAccountRequest {
                                    id,
                                    name: Some(display_name),
                                    remark: Some(remark),
                                    code: Some(login.code),
                                    platform: Some(platform),
                                    ..Default::default()
                                },
                                true,
                            )
                            .await
                        })
                        .await;
                    let _ = this.update(cx, |this, cx| {
                        if generation != this.accounts.wx_generation {
                            return;
                        }
                        this.accounts.form_saving = false;
                        match result {
                            Ok(Ok(start_error)) => {
                                this.show_account_success(
                                    if editing { "更新成功" } else { "添加成功" },
                                    if editing { "Update Success" } else { "Add Success" },
                                    cx,
                                );
                                this.close_account_form();
                                this.dismiss_account_sheet(cx);
                                this.accounts.selected.clear();
                                this.set_page(Page::Accounts, cx);
                                if let Some(error) = start_error {
                                    this.error = Some(super::super::localized_account_message(
                                        &this.preferences.language,
                                        &if editing {
                                            format!(
                                                "更新成功，自动启动失败，请手动重新登录：{error}"
                                            )
                                        } else {
                                            format!(
                                                "添加成功，自动启动失败，请手动点击启动：{error}"
                                            )
                                        },
                                    ));
                                    cx.notify();
                                }
                            }
                            Ok(Err(error)) => {
                                this.accounts.wx_error = Some(error.clone());
                                this.accounts.form_error = Some(error);
                                this.accounts.wx_status = "登录失败，请重试".into();
                                cx.notify();
                            }
                            Err(error) => {
                                let message = format!("登录任务失败：{error}");
                                this.accounts.wx_error = Some(message.clone());
                                this.accounts.form_error = Some(message);
                                this.accounts.wx_status = "登录失败，请重试".into();
                                cx.notify();
                            }
                        }
                    });
                    break;
                }
                "cancelled" | "expired" => {
                    let _ = this.update(cx, |this, cx| {
                        if generation == this.accounts.wx_generation {
                            this.accounts.wx_error = Some("二维码已失效，请重新获取".into());
                            this.accounts.wx_status = "登录二维码已失效".into();
                            cx.notify();
                        }
                    });
                    break;
                }
                _ => break,
            }
        })
        .detach();
    }
}
