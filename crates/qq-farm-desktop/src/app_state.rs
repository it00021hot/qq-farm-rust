//! 全局应用状态。

use std::sync::Arc;

use gpui::{AppContext, *};
use gpui_component::input::InputState;
use qq_farm_app::accounts::AclPolicy;
use qq_farm_app::AppContext as FarmAppContext;
use serde_json::Value;
use tokio::runtime::Handle;

/// 与 web menu 对齐的导航页。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavPage {
    Dashboard,
    Personal,
    Activity,
    GameMall,
    MysteryShop,
    Friends,
    Analytics,
    Settings,
    Config,
    Admin,
}

impl NavPage {
    pub const ALL: &'static [(NavPage, &'static str)] = &[
        (NavPage::Dashboard, "概览"),
        (NavPage::Personal, "个人"),
        (NavPage::Activity, "活动"),
        (NavPage::GameMall, "游戏商城"),
        (NavPage::MysteryShop, "神秘商人"),
        (NavPage::Friends, "好友"),
        (NavPage::Analytics, "分析"),
        (NavPage::Settings, "设置"),
        (NavPage::Config, "游戏配置"),
        (NavPage::Admin, "本机运维"),
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(p, _)| *p == self)
            .map(|(_, l)| *l)
            .unwrap_or("")
    }
}

pub struct AppState {
    pub app: Arc<FarmAppContext>,
    pub tokio: Handle,
    pub policy: AclPolicy,
    pub page: NavPage,
    pub account_id: String,
    pub accounts_json: Value,
    pub status_json: Value,
    pub logs_json: Value,
    pub lands_json: Value,
    pub bag_json: Value,
    pub gifts_json: Value,
    pub settings_json: Value,
    pub friends_json: Value,
    pub activity_json: Value,
    pub mall_json: Value,
    pub mystery_json: Value,
    pub analytics_json: Value,
    pub config_seeds: Value,
    pub admin_cards: Value,
    pub admin_users: Value,
    pub admin_system: Value,
    pub personal_tab: usize,
    pub settings_tab: usize,
    pub bag_category: usize,
    pub config_tab: usize,
    pub admin_tab: usize,
    pub friends_tab: usize,
    /// 0=输入 code，1=微信扫码（对齐 web AccountModal）
    pub add_login_tab: usize,
    /// qq | wx
    pub add_platform: String,
    pub wx_task_id: Option<String>,
    pub wx_status_text: String,
    pub wx_error: Option<String>,
    pub wx_loading: bool,
    pub wx_qr_image: Option<Arc<Image>>,
    pub wx_flow_version: u64,
    pub add_name_input: Entity<InputState>,
    pub add_code_input: Entity<InputState>,
    pub last_error: Option<String>,
    pub last_message: Option<String>,
    /// 0 success, 1 warning, 2 error
    pub toast_kind: u8,
    pub toast_epoch: u64,
    pub dark_theme: bool,
    pub activity_tab: usize,
    /// 0全部 1农场 2好友 3系统
    pub log_filter_module: usize,
    /// 账号管理：显示「新增/重新登录」面板
    pub show_add_account: bool,
    /// 重新登录时带入的备注名（对齐 web 同备注 upsert）
    pub relogin_name: Option<String>,
}

impl AppState {
    pub fn new(
        app: Arc<FarmAppContext>,
        tokio: Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let accounts = qq_farm_app::accounts::list_accounts_enriched(&app, None);
        let account_id = accounts
            .get("accounts")
            .and_then(|a| a.as_array())
            .and_then(|arr| arr.first())
            .and_then(|a| a.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let add_name_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("备注名，如：大号")
        });
        let add_code_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("粘贴登录 code / 链接")
        });
        let mut state = Self {
            app,
            tokio,
            policy: AclPolicy::LocalOwner,
            page: NavPage::Dashboard,
            account_id: account_id.clone(),
            accounts_json: accounts,
            status_json: Value::Null,
            logs_json: Value::Null,
            lands_json: Value::Null,
            bag_json: Value::Null,
            gifts_json: Value::Null,
            settings_json: Value::Null,
            friends_json: Value::Null,
            activity_json: Value::Null,
            mall_json: Value::Null,
            mystery_json: Value::Null,
            analytics_json: Value::Null,
            config_seeds: Value::Null,
            admin_cards: Value::Null,
            admin_users: Value::Null,
            admin_system: Value::Null,
            personal_tab: 0,
            settings_tab: 0,
            bag_category: 0,
            config_tab: 0,
            admin_tab: 0,
            friends_tab: 0,
            add_login_tab: 1,
            add_platform: "wx".into(),
            wx_task_id: None,
            wx_status_text: String::new(),
            wx_error: None,
            wx_loading: false,
            wx_qr_image: None,
            wx_flow_version: 0,
            add_name_input,
            add_code_input,
            last_error: None,
            last_message: None,
            toast_kind: 0,
            toast_epoch: 0,
            dark_theme: false,
            activity_tab: 0,
            log_filter_module: 0,
            show_add_account: false,
            relogin_name: None,
        };
        state.refresh_sync();
        state
    }

    pub fn bridge_parts(&self) -> (Arc<FarmAppContext>, String, Handle) {
        (self.app.clone(), self.account_id.clone(), self.tokio.clone())
    }

    pub fn set_page(&mut self, page: NavPage, cx: &mut Context<Self>) {
        self.page = page;
        self.refresh_sync();
        self.refresh_async(cx);
        if page == NavPage::Settings && self.settings_tab == 0 && self.add_login_tab == 1 {
            if self.show_add_account
                && self.wx_qr_image.is_none()
                && !self.wx_loading
            {
                self.start_wx_login(cx);
            }
        }
        cx.notify();
    }

    pub fn set_account(&mut self, id: String, cx: &mut Context<Self>) {
        self.account_id = id;
        self.refresh_sync();
        self.refresh_async(cx);
        cx.notify();
    }

    pub fn refresh_accounts(&mut self) {
        self.accounts_json = qq_farm_app::accounts::list_accounts_enriched(&self.app, None);
    }

    pub fn refresh_sync(&mut self) {
        self.refresh_accounts();
        if !self.account_id.is_empty() {
            self.status_json =
                qq_farm_app::farm::panel_status_with_progress(&self.app, &self.account_id);
            self.logs_json =
                qq_farm_app::farm::engine_global_logs(&self.app, Some(&self.account_id), 200);
            self.settings_json =
                qq_farm_app::settings::settings_panel(&self.account_id, "local");
            self.analytics_json = qq_farm_app::farm::analytics(Some("exp"));
            self.config_seeds = qq_farm_app::config::list_seeds();
            self.admin_cards = qq_farm_app::admin::list_cards();
            self.admin_users = qq_farm_app::admin::list_users();
            self.admin_system = qq_farm_app::admin::get_system_config();
        }
    }

    pub fn refresh_async(&mut self, cx: &mut Context<Self>) {
        if self.account_id.is_empty() {
            return;
        }
        let app = self.app.clone();
        let id = self.account_id.clone();
        let handle = self.tokio.clone();
        cx.spawn(async move |this, cx| {
            let lands = handle
                .spawn({
                    let app = app.clone();
                    let id = id.clone();
                    async move { qq_farm_app::farm::lands(&app, &id).await.map_err(|e| e.to_string()) }
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
            let bag = handle
                .spawn({
                    let app = app.clone();
                    let id = id.clone();
                    async move { qq_farm_app::farm::bag(&app, &id).await.map_err(|e| e.to_string()) }
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
            let gifts = handle
                .spawn({
                    let app = app.clone();
                    let id = id.clone();
                    async move {
                        qq_farm_app::farm::daily_gift_overview(&app, &id)
                            .await
                            .map_err(|e| e.to_string())
                    }
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
            let friends = handle
                .spawn({
                    let app = app.clone();
                    let id = id.clone();
                    async move {
                        qq_farm_app::friend::list_friends(&app, &id, false)
                            .await
                            .map_err(|e| e.to_string())
                    }
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
            let activity = handle
                .spawn({
                    let app = app.clone();
                    let id = id.clone();
                    async move {
                        qq_farm_app::activity::snapshot(&app, &id)
                            .await
                            .map_err(|e| e.to_string())
                    }
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
            let mall = handle
                .spawn({
                    let app = app.clone();
                    let id = id.clone();
                    async move {
                        qq_farm_app::commerce::mall_catalog(&app, &id, None, None)
                            .await
                            .map_err(|e| e.to_string())
                    }
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
            let mystery = handle
                .spawn({
                    let app = app.clone();
                    let id = id.clone();
                    async move {
                        qq_farm_app::commerce::mystery_shop(&app, &id)
                            .await
                            .map_err(|e| e.to_string())
                    }
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()));

            this.update(cx, |state, cx| {
                if let Ok(v) = lands {
                    state.lands_json = v;
                }
                if let Ok(v) = bag {
                    state.bag_json = v;
                }
                if let Ok(v) = gifts {
                    state.gifts_json = v;
                }
                if let Ok(v) = friends {
                    state.friends_json = v;
                }
                if let Ok(v) = activity {
                    state.activity_json = v;
                }
                if let Ok(v) = mall {
                    state.mall_json = v;
                }
                if let Ok(v) = mystery {
                    state.mystery_json = v;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn account_labels(&self) -> Vec<(String, String, bool)> {
        self.accounts_json
            .get("accounts")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|a| {
                let id = a.get("id")?.as_str()?.to_string();
                let nick = a.get("nick").and_then(|v| v.as_str()).unwrap_or("");
                let name = a
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("未命名");
                let label = if nick.is_empty() {
                    name.to_string()
                } else if nick == name {
                    nick.to_string()
                } else {
                    format!("{name} · {nick}")
                };
                let running = a.get("running").and_then(|v| v.as_bool()).unwrap_or(false);
                Some((id, label, running))
            })
            .collect()
    }

    pub fn clear_toast(&mut self) {
        self.last_error = None;
        self.last_message = None;
    }

    pub fn flash_success(&mut self, msg: impl Into<String>, cx: &mut Context<Self>) {
        self.push_toast(0, msg.into(), cx);
    }

    pub fn flash_warning(&mut self, msg: impl Into<String>, cx: &mut Context<Self>) {
        self.push_toast(1, msg.into(), cx);
    }

    pub fn flash_error(&mut self, msg: impl Into<String>, cx: &mut Context<Self>) {
        let msg = crate::views::humanize_error(&msg.into());
        if crate::views::is_soft_business_message(&msg) {
            self.push_toast(1, msg, cx);
        } else {
            self.push_toast(2, msg, cx);
        }
    }

    fn push_toast(&mut self, kind: u8, msg: String, cx: &mut Context<Self>) {
        self.toast_kind = kind;
        self.toast_epoch = self.toast_epoch.wrapping_add(1);
        let epoch = self.toast_epoch;
        if kind == 2 {
            self.last_error = Some(msg);
            self.last_message = None;
        } else {
            self.last_message = Some(msg);
            self.last_error = None;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(4))
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.toast_epoch == epoch {
                    s.clear_toast();
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// 打开新增/重新登录面板（默认微信扫码）。
    pub fn open_add_account(
        &mut self,
        remark: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_add_account = true;
        self.add_login_tab = 1;
        self.add_platform = "wx".into();
        self.relogin_name = remark.clone();
        let name = remark.unwrap_or_default();
        self.add_name_input.update(cx, |input, cx| {
            input.set_value(name, window, cx);
        });
        self.start_wx_login(cx);
        cx.notify();
    }

    /// 关闭新增面板并取消扫码。
    pub fn close_add_account(&mut self, cx: &mut Context<Self>) {
        self.show_add_account = false;
        self.relogin_name = None;
        self.reset_wx_login();
        cx.notify();
    }

    /// 取消当前微信扫码任务。
    pub fn reset_wx_login(&mut self) {
        self.wx_flow_version = self.wx_flow_version.wrapping_add(1);
        if let Some(task_id) = self.wx_task_id.take() {
            qq_farm_app::wx_login::destroy_task(&self.app.wx_login, &task_id);
        }
        self.wx_status_text.clear();
        self.wx_error = None;
        self.wx_loading = false;
        self.wx_qr_image = None;
    }

    /// 启动微信扫码（create → 展示 QR → 轮询 → confirm → code → upsert）。
    pub fn start_wx_login(&mut self, cx: &mut Context<Self>) {
        self.reset_wx_login();
        self.wx_loading = true;
        self.wx_status_text = "正在获取二维码…".into();
        self.add_platform = "wx".into();
        let flow = self.wx_flow_version;
        let app = self.app.clone();
        let handle = self.tokio.clone();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let created = handle
                .spawn({
                    let hub = app.wx_login.clone();
                    async move { qq_farm_app::wx_login::create_task(&hub).await }
                })
                .await
                .unwrap_or_else(|e| Err(qq_farm_app::AppError::Internal(e.to_string())));

            let created = match created {
                Ok(v) => v,
                Err(e) => {
                    let _ = this.update(cx, |s, cx| {
                        if s.wx_flow_version != flow {
                            return;
                        }
                        s.wx_loading = false;
                        s.wx_error = Some(e.to_string());
                        s.wx_status_text.clear();
                        cx.notify();
                    });
                    return;
                }
            };

            let render_img = Arc::new(Image::from_bytes(
                ImageFormat::Jpeg,
                created.qr_jpeg.clone(),
            ));
            let task_id = created.task_id.clone();
            let _ = this.update(cx, |s, cx| {
                if s.wx_flow_version != flow {
                    qq_farm_app::wx_login::destroy_task(&s.app.wx_login, &task_id);
                    return;
                }
                s.wx_task_id = Some(task_id.clone());
                s.wx_qr_image = Some(render_img);
                s.wx_loading = false;
                s.wx_status_text = "等待微信扫码".into();
                s.wx_error = None;
                cx.notify();
            });

            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1200))
                    .await;

                let still_active = this
                    .update(cx, |s, _| {
                        s.wx_flow_version == flow
                            && s.wx_task_id.as_deref() == Some(task_id.as_str())
                    })
                    .unwrap_or(false);
                if !still_active {
                    break;
                }

                let status = handle
                    .spawn({
                        let hub = app.wx_login.clone();
                        let task_id = task_id.clone();
                        async move { qq_farm_app::wx_login::poll_status(&hub, &task_id).await }
                    })
                    .await
                    .unwrap_or_else(|e| Err(qq_farm_app::AppError::Internal(e.to_string())));

                let status = match status {
                    Ok(v) => v,
                    Err(e) => {
                        let _ = this.update(cx, |s, cx| {
                            if s.wx_flow_version == flow {
                                s.wx_error = Some(e.to_string());
                                cx.notify();
                            }
                        });
                        break;
                    }
                };

                match status.status.as_str() {
                    "waiting" => {
                        let _ = this.update(cx, |s, cx| {
                            if s.wx_flow_version == flow {
                                s.wx_status_text = "等待微信扫码".into();
                                cx.notify();
                            }
                        });
                    }
                    "scanned" => {
                        let _ = this.update(cx, |s, cx| {
                            if s.wx_flow_version == flow {
                                s.wx_status_text = "已扫码，请在手机上确认".into();
                                cx.notify();
                            }
                        });
                    }
                    "authorized" => {
                        let _ = this.update(cx, |s, cx| {
                            if s.wx_flow_version == flow {
                                s.wx_status_text = "正在建立登录会话…".into();
                                s.wx_loading = true;
                                cx.notify();
                            }
                        });

                        if let Err(e) = handle
                            .spawn({
                                let hub = app.wx_login.clone();
                                let task_id = task_id.clone();
                                async move { qq_farm_app::wx_login::confirm(&hub, &task_id).await }
                            })
                            .await
                            .unwrap_or_else(|e| Err(qq_farm_app::AppError::Internal(e.to_string())))
                        {
                            let _ = this.update(cx, |s, cx| {
                                if s.wx_flow_version == flow {
                                    s.wx_loading = false;
                                    s.wx_error = Some(e.to_string());
                                    cx.notify();
                                }
                            });
                            break;
                        }

                        let code_res = handle
                            .spawn({
                                let hub = app.wx_login.clone();
                                let task_id = task_id.clone();
                                async move {
                                    qq_farm_app::wx_login::issue_code(&hub, &task_id).await
                                }
                            })
                            .await
                            .unwrap_or_else(|e| Err(qq_farm_app::AppError::Internal(e.to_string())));

                        match code_res {
                            Ok(code_res) => {
                                let name = this
                                    .update(cx, |s, cx| {
                                        s.add_name_input.read(cx).value().to_string()
                                    })
                                    .unwrap_or_default();
                                let name = if name.trim().is_empty() {
                                    format!("微信{}", chrono::Local::now().format("%H%M%S"))
                                } else {
                                    name.trim().to_string()
                                };
                                let _ = this.update(cx, |s, cx| {
                                    if s.wx_flow_version != flow {
                                        return;
                                    }
                                    let req = qq_farm_app::accounts::UpsertAccountRequest {
                                        name: Some(name),
                                        code: Some(code_res.code),
                                        platform: Some("wx".into()),
                                        username: Some("local".into()),
                                        ..Default::default()
                                    };
                                    match qq_farm_app::accounts::upsert_account(
                                        &s.app, &s.policy, req,
                                    ) {
                                        Ok(v) => {
                                            s.accounts_json = v.clone();
                                            if let Some(id) = v
                                                .get("accounts")
                                                .and_then(|a| a.as_array())
                                                .and_then(|arr| arr.last())
                                                .and_then(|a| a.get("id"))
                                                .and_then(|x| x.as_str())
                                            {
                                                s.account_id = id.to_string();
                                            }
                                            s.wx_task_id = None;
                                            s.wx_qr_image = None;
                                            s.wx_loading = false;
                                            s.wx_status_text =
                                                "登录成功，账号已添加并启动".into();
                                            s.wx_error = None;
                                            s.show_add_account = false;
                                            s.relogin_name = None;
                                            s.flash_success("微信扫码登录成功", cx);
                                            s.refresh_async(cx);
                                        }
                                        Err(e) => {
                                            s.wx_loading = false;
                                            s.wx_error = Some(e.to_string());
                                        }
                                    }
                                    cx.notify();
                                });
                            }
                            Err(e) => {
                                let _ = this.update(cx, |s, cx| {
                                    if s.wx_flow_version == flow {
                                        s.wx_loading = false;
                                        s.wx_error = Some(e.to_string());
                                        cx.notify();
                                    }
                                });
                            }
                        }
                        break;
                    }
                    "cancelled" | "expired" | "failed" => {
                        let _ = this.update(cx, |s, cx| {
                            if s.wx_flow_version == flow {
                                s.wx_error = Some("二维码已失效，请重新获取".into());
                                s.wx_loading = false;
                                s.wx_task_id = None;
                                cx.notify();
                            }
                        });
                        break;
                    }
                    _ => {}
                }
            }
        })
        .detach();
    }
}
