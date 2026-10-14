use super::super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn start_wx_auth_flow(&mut self, cx: &mut Context<Self>) {
        self.accounts.login_tab = "wx".into();
        self.accounts.platform = "wx".into();
        self.accounts.wx_generation = self.accounts.wx_generation.wrapping_add(1);
        let generation = self.accounts.wx_generation;
        self.accounts.wx_task_id = None;
        self.accounts.wx_session_id = None;
        self.accounts.wx_quick_profile = None;
        self.accounts.wx_mode = "local".into();
        self.accounts.wx_status = "正在检测本机微信…".into();
        self.accounts.wx_error = None;
        if let Some(path) = self.accounts.wx_qr_path.take() {
            let _ = std::fs::remove_file(path);
        }
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    let session = qq_farm_app::wx_login::create_quick_session(&app.wx_login)
                        .await
                        .map_err(|error| error.to_string())?;
                    let profile = qq_farm_app::wx_login::detect_quick_session(
                        &app.wx_login,
                        &session.session_id,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                    Ok::<_, String>((
                        session.session_id,
                        profile.port,
                        profile.authorize_uuid,
                        profile.nickname,
                        profile.headimgurl,
                    ))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.accounts.wx_generation || !this.accounts.form_open {
                    return;
                }
                match result {
                    Ok(Ok((session, port, uuid, nickname, avatar))) => {
                        this.accounts.wx_session_id = Some(session);
                        this.accounts.wx_quick_profile =
                            Some((port, uuid, nickname.clone(), avatar));
                        this.accounts.wx_status = if nickname.trim().is_empty() {
                            "本机微信已就绪，请点击授权".into()
                        } else {
                            format!("{} · 请在电脑微信中确认", nickname)
                        };
                        cx.notify();
                    }
                    Ok(Err(error)) => this.start_wx_qr_flow(
                        Some(format!("未检测到本机微信：{error}。请扫码登录")),
                        cx,
                    ),
                    Err(error) => this.start_wx_qr_flow(
                        Some(format!("未检测到本机微信：{error}。请扫码登录")),
                        cx,
                    ),
                }
            });
        })
        .detach();
    }
    pub(in crate::desktop) fn authorize_local_wechat(
        &mut self,
        x: i32,
        y: i32,
        cx: &mut Context<Self>,
    ) {
        let (Some(session_id), Some((port, authorize_uuid, _, _))) =
            (self.accounts.wx_session_id.clone(), self.accounts.wx_quick_profile.clone())
        else {
            self.start_wx_qr_flow(None, cx);
            return;
        };
        self.accounts.wx_status = "等待电脑微信确认…".into();
        self.accounts.wx_error = None;
        self.accounts.form_saving = true;
        let generation = self.accounts.wx_generation;
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    let redirect_url = qq_farm_app::wx_login::authorize_quick_session(
                        &app.wx_login,
                        &session_id,
                        port,
                        &authorize_uuid,
                        qq_farm_app::wx_login::LocalWechatPosition { x, y },
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                    let code = qq_farm_app::wx_login::confirm_quick_session(
                        &app.wx_login,
                        &session_id,
                        &redirect_url,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                    // The Vue quick-login IPC returns only code/openid, so a blank form name
                    // follows the app's generated account name instead of using this nickname.
                    Ok::<_, String>(code.code)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.accounts.wx_generation || !this.accounts.form_open {
                    return;
                }
                this.accounts.form_saving = false;
                match result {
                    Ok(Ok(code)) => this.submit_account_code(code, "wx".into(), None, true, cx),
                    Ok(Err(error)) => this.start_wx_qr_flow(
                        Some(local_wechat_authorize_error(&this.preferences.language, error)),
                        cx,
                    ),
                    Err(error) => this.start_wx_qr_flow(
                        Some(local_wechat_authorize_error(
                            &this.preferences.language,
                            error.to_string(),
                        )),
                        cx,
                    ),
                }
            });
        })
        .detach();
    }
}

fn local_wechat_authorize_error(language: &str, error: String) -> String {
    if error.trim().is_empty() {
        crate::i18n::text(
            language,
            "本机微信授权失败，请扫码登录",
            "WeChat authorization failed. Use QR code login.",
        )
        .into()
    } else {
        error
    }
}
