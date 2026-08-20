//! 单账号 Worker。
//!
//! 每个账号对应一个 Worker，跑在独立 tokio task 里。
//! Worker 持有一个 [`Scheduler`] 和一个 [`Gateway`]，通过订阅 Notify 事件更新状态。
//! 登录成功后由 [`crate::runtime::worker_loop::WorkerLoop`] 编排农场 / 好友 / 每日任务。

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::models::AccountSession;
use crate::network::encryptor::Encryptor;
use crate::network::gateway::{Gateway, GatewayConfig};
use crate::proto::generated::gamepb::userpb::{DeviceInfo, ReportData};
use crate::runtime::events::WorkerEvent;
use crate::runtime::scheduler::Scheduler;
use crate::runtime::worker_handle::WorkerHandle;
use crate::runtime::worker_message::WorkerMessage;
use crate::services::wx_login::{WxAuthErrorKind, YybCredentials};

/// Worker 启动配置
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// 网关配置
    pub gateway: GatewayConfig,
    /// 状态上报间隔
    pub status_interval: Duration,
    /// TSDK wasm 路径
    pub tsdk_wasm_path: std::path::PathBuf,
    /// 数据目录
    pub data_dir: std::path::PathBuf,
}

/// Worker —— 单账号的运行时
pub struct Worker {
    account: AccountSession,
    config: WorkerConfig,
    scheduler: Scheduler,
    cancel: CancellationToken,
    msg_tx: mpsc::Sender<WorkerMessage>,
    msg_rx: Option<mpsc::Receiver<WorkerMessage>>,
    event_tx: tokio::sync::broadcast::Sender<WorkerEvent>,
}

impl Worker {
    /// 创建 Worker
    pub fn new(
        account: AccountSession,
        config: WorkerConfig,
        event_tx: tokio::sync::broadcast::Sender<WorkerEvent>,
    ) -> Self {
        let namespace = format!("worker:{}", account.id);
        let (msg_tx, msg_rx) = mpsc::channel(32);
        Self {
            account,
            config,
            scheduler: Scheduler::new(namespace),
            cancel: CancellationToken::new(),
            msg_tx,
            msg_rx: Some(msg_rx),
            event_tx,
        }
    }

    /// 拿到控制句柄（用于从外面发消息/取消）
    pub fn handle(&self) -> WorkerHandle {
        WorkerHandle {
            account_id: self.account.id.clone(),
            msg_tx: self.msg_tx.clone(),
            cancel: self.cancel.clone(),
        }
    }

    /// 取出消息接收端（只能调一次，由 `spawn` 内部使用）
    fn take_msg_rx(&mut self) -> mpsc::Receiver<WorkerMessage> {
        self.msg_rx.take().expect("msg_rx already taken")
    }

    /// 启动 worker（spawn 到当前 tokio runtime）。
    ///
    /// 如果传了 `engine`，会在 spawn 完成后构造 WorkerLoop 并注册到 engine，
    /// controller 就能通过 `engine.worker_loop(account_id)` 拿到实例。
    /// 退出时自动 `unregister_worker_loop`。
    pub fn spawn_with_engine(
        mut self,
        engine: Option<Arc<crate::runtime::engine::RuntimeEngine>>,
    ) -> WorkerHandle {
        let handle = self.handle();
        let event_tx = self.event_tx.clone();
        let account_id = self.account.id.clone();
        let account_name = self.account.display_name.clone();
        let cancel = self.cancel.clone();
        let config = self.config.clone();
        let scheduler = self.scheduler.clone();
        let msg_rx = self.take_msg_rx();
        let account = self.account;
        let has_wx_auth = account.has_wx_auth();

        let panic_account_id = account_id.clone();
        let panic_account_name = account_name.clone();
        let panic_event_tx = event_tx.clone();
        let panic_engine = engine.clone();

        crate::runtime::safe_spawn::spawn_logged_with_account(
            "worker",
            panic_account_id.clone(),
            async move {
            if let Some(eng) = &engine {
                crate::services::panel_log::register_with_runtime(
                    &account_id,
                    &account_name,
                    eng.runtime_state(),
                );
            } else {
                crate::services::panel_log::register(&account_id, &account_name, event_tx.clone());
            }

            let tsdk_data_dir = config.data_dir.join(account_id.as_str());
            let wasm_path = config.tsdk_wasm_path.clone();
            let data_dir_s = tsdk_data_dir.to_string_lossy().to_string();
            let tsdk = match tokio::task::spawn_blocking(move || {
                crate::crypto::tsdk::TsdkRuntime::load(&wasm_path, data_dir_s)
            })
            .await
            {
                Ok(Ok(rt)) => Arc::new(rt),
                Ok(Err(e)) => {
                    tracing::error!(account_id = %account_id, "TSDK 加载失败: {e}");
                    emit_login_log(&account_id, &format!("TSDK 加载失败: {e}"), true);
                    emit_terminal_stop(
                        &event_tx,
                        &account_id,
                        &account_name,
                        &format!("TSDK 加载失败: {e}"),
                        "tsdk_load",
                        false,
                    );
                    crate::services::panel_log::unregister(&account_id);
                    if let Some(eng) = &engine {
                        eng.release_worker(&account_id);
                    }
                    return;
                }
                Err(e) => {
                    tracing::error!(account_id = %account_id, "TSDK 加载任务失败: {e}");
                    emit_login_log(&account_id, &format!("TSDK 加载失败: {e}"), true);
                    emit_terminal_stop(
                        &event_tx,
                        &account_id,
                        &account_name,
                        &format!("TSDK 加载失败: {e}"),
                        "tsdk_load",
                        false,
                    );
                    crate::services::panel_log::unregister(&account_id);
                    if let Some(eng) = &engine {
                        eng.release_worker(&account_id);
                    }
                    return;
                }
            };
            let encryptor: Arc<dyn Encryptor> =
                Arc::new(crate::network::encryptor::TsdkEncryptor::new(tsdk.clone()));

            let mut config = config;
            if account.has_wx_auth() {
                emit_login_log(&account_id, "正在用应用宝授权换取新的登录码", false);
                match prepare_wx_gateway_code(&account).await {
                    Ok((code, creds)) => {
                        persist_wx_gateway_credentials(&account_id, &code, &creds);
                        config.gateway.auth_code = code;
                        emit_login_log(&account_id, "换码成功，正在连接网关", false);
                    }
                    Err(e) => {
                        tracing::warn!(account_id = %account_id, "应用宝换码失败: {e}");
                        let dead = e.kind == WxAuthErrorKind::CredentialsDead;
                        if dead {
                            crate::models::store::accounts::clear_wx_auth(&account_id);
                            crate::models::store::accounts::persist_global();
                        }
                        emit_login_log(
                            &account_id,
                            &format!("应用宝换码失败，请重新扫码: {e}"),
                            true,
                        );
                        let source = if dead { "wx_auth_failed" } else { "wx_mint_failed" };
                        emit_wx_failure_stop(
                            &event_tx,
                            &account_id,
                            &account_name,
                            &format!("应用宝授权已失效，请重新扫码: {e}"),
                            source,
                        );
                        if dead {
                            if let Some(eng) = &engine {
                                eng.notify_wx_auth_cleared(&account_id, &account_name);
                            }
                        }
                        crate::services::panel_log::unregister(&account_id);
                        if let Some(eng) = &engine {
                            eng.release_worker(&account_id);
                        }
                        return;
                    }
                }
            } else {
                emit_login_log(&account_id, "正在用已保存的登录码连接网关", false);
            }

            // 构造 Gateway
            let gateway = Arc::new(Gateway::new(config.gateway.clone(), encryptor));

            // 构造所有 service + WorkerLoop，注册到 engine
            if let Some(eng) = &engine {
                let farm =
                    Arc::new(crate::services::farm::scheduler::FarmService::new(gateway.clone()));
                let friend = Arc::new(crate::services::friend::scheduler::FriendService::new(
                    gateway.clone(),
                    5,
                ));
                let email = Arc::new(crate::services::email::EmailService::new(gateway.clone()));
                let share = Arc::new(crate::services::share::ShareService::new(gateway.clone()));
                let monthcard =
                    Arc::new(crate::services::monthcard::MonthCardService::new(gateway.clone()));
                let qqvip = Arc::new(crate::services::qqvip::QQVipService::new(gateway.clone()));
                let mall = Arc::new(crate::services::mall::MallService::new(gateway.clone()));
                let task = Arc::new(crate::services::task::TaskService::new(gateway.clone()));
                let warehouse =
                    Arc::new(crate::services::warehouse::WarehouseService::new(gateway.clone()));
                let mystery_shop = Arc::new(
                    crate::services::mystery_shop::MysteryShopService::new(gateway.clone()),
                );
                let activity_center = Arc::new(
                    crate::services::activity_center::ActivityCenterService::new(gateway.clone()),
                );

                let mut loop_cfg = crate::runtime::worker_loop::WorkerLoopConfig::default();
                loop_cfg.status_interval = config.status_interval;
                let rt = crate::config::get_runtime_config();
                loop_cfg.client_version = if rt.client_version.is_empty() {
                    config.gateway.client_version.clone()
                } else {
                    rt.client_version.clone()
                };
                if rt.heartbeat_interval_ms > 0 {
                    loop_cfg.heartbeat_interval =
                        Duration::from_millis(rt.heartbeat_interval_ms as u64);
                }
                let worker_loop = Arc::new(crate::runtime::worker_loop::WorkerLoop::new(
                    account.clone(),
                    loop_cfg,
                    gateway.clone(),
                    event_tx.clone(),
                    farm,
                    friend,
                    email,
                    share,
                    monthcard,
                    qqvip,
                    mall,
                    task,
                    warehouse,
                    mystery_shop,
                    activity_center,
                ));
                eng.register_worker_loop(&account_id, worker_loop.clone());
                tracing::info!(account_id = %account_id, "WorkerLoop 已注册到 engine");

                let plat = if account.platform.trim().is_empty() {
                    config.gateway.platform.clone()
                } else {
                    account.platform.clone()
                };
                if !plat.is_empty() {
                    crate::services::status::set_status_platform_for(&account_id, &plat);
                }

                // 心跳超时 = 终端断开，不再用旧 Code 重连
                let gw_for_hb = gateway.clone();
                worker_loop.on_heartbeat_timeout(move |_acc_id| {
                    gw_for_hb.force_disconnect_with_reason("heartbeat_timeout");
                });

                // === 1. WS 连接 + 登录；失败即退出（对齐 handleTerminalDisconnect） ===
                if let Err(e) = gateway.connect().await {
                    tracing::warn!(account_id = %account_id, "WS 连接失败: {e}");
                    let err_s = format!("WS 连接失败: {e}");
                    if parse_ws_http_code(&err_s) == Some(400) {
                        let _ = event_tx.send(WorkerEvent::Error {
                            account_id: account_id.clone(),
                            message: err_s.clone(),
                        });
                    }
                    emit_terminal_stop(
                        &event_tx,
                        &account_id,
                        &account_name,
                        &err_s,
                        "ws_connect",
                        has_wx_auth,
                    );
                    gateway.force_disconnect();
                    crate::services::panel_log::unregister(&account_id);
                    eng.release_worker(&account_id);
                    return;
                }
                emit_login_log(&account_id, "网关已连接，正在登录", false);
                tracing::info!(account_id = %account_id, "WS 已连接，开始登录");

                {
                    let mut notify_rx = gateway.subscribe_notify();
                    let wl = worker_loop.clone();
                    let gw = gateway.clone();
                    let tx = event_tx.clone();
                    let acc_id = account_id.clone();
                    let acc_name = account_name.clone();
                    let kick_wx_auth = has_wx_auth;
                    tokio::spawn(async move {
                        while let Some(ev) = notify_rx.recv().await {
                            match ev {
                                crate::network::notify::NotifyEvent::Kickout { reason, .. } => {
                                    let why = if reason.is_empty() {
                                        "未知".to_string()
                                    } else {
                                        reason
                                    };
                                    let _ = tx.send(WorkerEvent::Log {
                                        account_id: acc_id.clone(),
                                        account_name: acc_name.clone(),
                                        level: "info".to_string(),
                                        module: "system".to_string(),
                                        message: format!(
                                            "检测到踢下线，准备自动停止账号。原因: {why}"
                                        ),
                                    });
                                    wl.on_kickout(&why);
                                    emit_terminal_stop(
                                        &tx,
                                        &acc_id,
                                        &acc_name,
                                        &why,
                                        "kickout",
                                        kick_wx_auth,
                                    );
                                    gw.force_disconnect_with_reason("kickout");
                                    break;
                                }
                                crate::network::notify::NotifyEvent::ItemChanged {
                                    items, ..
                                } => {
                                    wl.apply_item_notify(&items);
                                }
                                crate::network::notify::NotifyEvent::BasicChanged {
                                    level,
                                    gold,
                                    exp,
                                    ..
                                } => {
                                    wl.apply_basic_notify(level, gold, exp);
                                }
                                crate::network::notify::NotifyEvent::LandsChanged {
                                    host_gid,
                                    changed_count,
                                    lands,
                                    ..
                                } => {
                                    wl.on_lands_notify(host_gid, changed_count, lands);
                                }
                                crate::network::notify::NotifyEvent::FriendApplications {
                                    applications,
                                } => {
                                    if !applications.is_empty() {
                                        let gids: Vec<i64> =
                                            applications.iter().map(|(g, _)| *g).collect();
                                        let names: Vec<String> =
                                            applications.iter().map(|(_, n)| n.clone()).collect();
                                        let friend = wl.friend().clone();
                                        tokio::spawn(async move {
                                            friend.accept_friend_applications(gids, &names).await;
                                        });
                                    }
                                }
                                crate::network::notify::NotifyEvent::Unknown { .. } => {}
                            }
                        }
                    });
                }

                let rt = crate::config::get_runtime_config();
                let di = &rt.device_info;
                let device_info = DeviceInfo {
                    client_version: if di.client_version.is_empty() {
                        config.gateway.client_version.clone()
                    } else {
                        di.client_version.clone()
                    },
                    sys_software: if di.sys_software.is_empty() {
                        "Windows".to_string()
                    } else {
                        di.sys_software.clone()
                    },
                    screen_width: 0,
                    ..Default::default()
                };
                let report_data = ReportData {
                    minigame_channel: "other-qq".to_string(),
                    minigame_platid: 2,
                    ..Default::default()
                };

                match gateway.login(&device_info, &report_data, &tsdk).await {
                    Ok(reply) => {
                        let login_msg = if let Some(basic) = &reply.basic {
                            let nick = if basic.name.is_empty() {
                                account_name.as_str()
                            } else {
                                basic.name.as_str()
                            };
                            format!("登录成功：{nick} Lv{}", basic.level)
                        } else {
                            "登录成功".to_string()
                        };
                        emit_login_log(&account_id, &login_msg, false);
                        if let Some(basic) = &reply.basic {
                            worker_loop.set_gid(basic.gid);
                            crate::services::status::update_status_from_login_for(
                                &account_id,
                                &serde_json::json!({
                                    "name": basic.name,
                                    "level": basic.level,
                                    "gold": basic.gold,
                                    "exp": basic.exp,
                                    "avatar": basic.avatar_url,
                                }),
                            );
                        }
                        // 对齐 sendLogin 成功后的顺序：ACE → 金豆/设置 → 心跳/状态 → onLoginSuccess
                        let ace = Arc::new(crate::services::ace::AceShared::new());
                        let sender = Arc::new(crate::services::ace::GatewayAceSender {
                            gateway: gateway.clone(),
                        });
                        ace.start(sender, tsdk.clone());
                        worker_loop.attach_ace(ace);

                        let extras = worker_loop.clone();
                        let gw_settings = gateway.clone();
                        tokio::spawn(async move {
                            extras.fetch_gold_bean_from_bag().await;
                            let _ = gw_settings.fetch_user_settings().await;
                        });

                        worker_loop.mark_login_ready();
                        worker_loop.sync_status();
                        worker_loop.start(&scheduler);
                        let _ = event_tx.send(WorkerEvent::Started {
                            account_id: account_id.clone(),
                            account_name: account_name.clone(),
                        });
                        let wl = worker_loop.clone();
                        let sched = scheduler.clone();
                        tokio::spawn(async move {
                            wl.on_login_success(&sched).await;
                        });
                    }
                    Err(e) => {
                        tracing::warn!(account_id = %account_id, "登录失败: {e}");
                        emit_login_log(&account_id, &format!("登录失败: {e}"), true);
                        emit_terminal_stop(
                            &event_tx,
                            &account_id,
                            &account_name,
                            &format!("登录失败: {e}"),
                            "login",
                            has_wx_auth,
                        );
                        gateway.force_disconnect();
                        crate::services::panel_log::unregister(&account_id);
                        eng.release_worker(&account_id);
                        return;
                    }
                }
            }

            let exit = run_worker_loop(
                account,
                config,
                scheduler,
                msg_rx,
                event_tx.clone(),
                gateway,
                cancel,
                engine.clone(),
            )
            .await;

            crate::services::panel_log::unregister(&account_id);
            if let Some(eng) = &engine {
                eng.release_worker(&account_id);
            }

            let _ = event_tx.send(WorkerEvent::Stopped { account_id, reason: exit.reason });
            },
            move |account_id, msg| {
                let _ = panic_event_tx.send(WorkerEvent::Stopped {
                    account_id: account_id.to_string(),
                    reason: format!("worker panicked: {msg}"),
                });
                let _ = panic_event_tx.send(WorkerEvent::Log {
                    account_id: account_id.to_string(),
                    account_name: panic_account_name,
                    level: "error".to_string(),
                    module: "system".to_string(),
                    message: format!("Worker 异常退出（已隔离，进程继续运行）: {msg}"),
                });
                crate::services::panel_log::unregister(account_id);
                if let Some(eng) = &panic_engine {
                    eng.release_worker(account_id);
                }
            },
        );

        handle
    }

    /// 启动 worker（不注册到 engine）
    pub fn spawn(self) -> WorkerHandle {
        self.spawn_with_engine(None)
    }
}

/// Worker 退出原因
struct WorkerExit {
    reason: String,
}

/// Worker 主循环
async fn run_worker_loop(
    account: AccountSession,
    config: WorkerConfig,
    scheduler: Scheduler,
    mut msg_rx: mpsc::Receiver<WorkerMessage>,
    event_tx: tokio::sync::broadcast::Sender<WorkerEvent>,
    gateway: Arc<Gateway>,
    cancel: CancellationToken,
    engine: Option<Arc<crate::runtime::engine::RuntimeEngine>>,
) -> WorkerExit {
    let _ = config;
    let watch_session = matches!(
        gateway.phase(),
        crate::network::gateway::ConnectionPhase::Login
            | crate::network::gateway::ConnectionPhase::Online
    );
    let session_gw = gateway.clone();

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                scheduler.shutdown();
                gateway.force_disconnect();
                return WorkerExit { reason: "主动取消".to_string() };
            }
            _ = async {
                if watch_session {
                    session_gw.wait_session_end().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                scheduler.shutdown();
                let source = session_gw.take_disconnect_reason();
                emit_disconnect_log(
                    &event_tx,
                    &account.id,
                    &account.display_name,
                    &source,
                    account.has_wx_auth(),
                );
                return WorkerExit {
                    reason: format!("disconnect:{source}"),
                };
            }
            msg = msg_rx.recv() => {
                match msg {
                    Some(WorkerMessage::Connect) => {
                        tracing::info!(account_id = %account.id, "忽略 Connect：不再使用旧 Code 重连");
                    }
                    Some(WorkerMessage::Disconnect) => {
                        tracing::info!(account_id = %account.id, "worker disconnect requested");
                        gateway.force_disconnect();
                    }
                    Some(WorkerMessage::ReloadConfig) => {
                        if let Some(eng) = &engine {
                            if let Some(wl) = eng.worker_loop(&account.id) {
                                let rev = eng.runtime_state().config_revision();
                                wl.apply_runtime_config(rev, &scheduler);
                            }
                        }
                    }
                    Some(WorkerMessage::Custom { tag, payload }) => {
                        tracing::debug!(account_id = %account.id, tag = %tag, ?payload, "custom msg");
                    }
                    None => {
                        scheduler.shutdown();
                        return WorkerExit { reason: "消息通道关闭".to_string() };
                    }
                }
            }
        }
    }
}

fn parse_ws_http_code(msg: &str) -> Option<i64> {
    let lower = msg.to_ascii_lowercase();
    let needle = "unexpected server response:";
    let idx = lower.find(needle)?;
    let rest = msg[idx + needle.len()..].trim();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok().filter(|c| *c > 0)
}

fn disconnect_reason_label(source: &str) -> &str {
    match source {
        "heartbeat_timeout" => "心跳超时",
        "kickout" => "被踢下线",
        "ws_close" => "连接关闭",
        "" => "未知原因",
        other => other,
    }
}

fn emit_login_log(account_id: &str, msg: &str, warn: bool) {
    if warn {
        crate::services::panel_log::log_warn(
            account_id,
            "系统",
            msg,
            crate::constants::PanelEvent::Login,
            Some(serde_json::json!({ "module": "system" })),
        );
    } else {
        crate::services::panel_log::log(
            account_id,
            "系统",
            msg,
            crate::constants::PanelEvent::Login,
            Some(serde_json::json!({ "module": "system" })),
        );
    }
}

fn emit_disconnect_log(
    event_tx: &tokio::sync::broadcast::Sender<WorkerEvent>,
    account_id: &str,
    account_name: &str,
    source: &str,
    has_wx_auth: bool,
) {
    let reason = disconnect_reason_label(source);
    let wx_reconnect = has_wx_auth;
    let message = if wx_reconnect {
        format!("连接已断开，将自动使用应用宝授权重连（{reason}）")
    } else {
        format!("连接已断开，不再使用旧 Code 重连（{reason}）")
    };
    let _ = event_tx.send(WorkerEvent::Log {
        account_id: account_id.to_string(),
        account_name: account_name.to_string(),
        level: "info".to_string(),
        module: "system".to_string(),
        message,
    });
}

fn emit_terminal_stop(
    event_tx: &tokio::sync::broadcast::Sender<WorkerEvent>,
    account_id: &str,
    account_name: &str,
    _detail: &str,
    source: &str,
    has_wx_auth: bool,
) {
    emit_disconnect_log(event_tx, account_id, account_name, source, has_wx_auth);
    let _ = event_tx.send(WorkerEvent::Stopped {
        account_id: account_id.to_string(),
        reason: format!("disconnect:{source}"),
    });
}

fn emit_wx_failure_stop(
    event_tx: &tokio::sync::broadcast::Sender<WorkerEvent>,
    account_id: &str,
    account_name: &str,
    message: &str,
    source: &str,
) {
    let _ = event_tx.send(WorkerEvent::Log {
        account_id: account_id.to_string(),
        account_name: account_name.to_string(),
        level: "warn".to_string(),
        module: "system".to_string(),
        message: message.to_string(),
    });
    let _ = event_tx.send(WorkerEvent::Stopped {
        account_id: account_id.to_string(),
        reason: format!("disconnect:{source}"),
    });
}

async fn prepare_wx_gateway_code(
    account: &AccountSession,
) -> Result<(String, YybCredentials), crate::services::wx_login::WxAuthError> {
    let svc = crate::services::wx_login::service::WxLoginService::new();
    svc.mint_gateway_code(&account.yyb_credentials(), crate::constants::WX_MINI_APP_ID).await
}

fn persist_wx_gateway_credentials(account_id: &str, code: &str, creds: &YybCredentials) {
    use crate::models::store::accounts::{self, YybCredentialPatch};
    accounts::persist_yyb_credentials(
        account_id,
        YybCredentialPatch {
            code: Some(code.to_string()),
            wx_login_buffer: Some(creds.login_buffer.clone()),
            wx_access_token: Some(creds.access_token.clone()),
            wx_refresh_token: Some(creds.refresh_token.clone()),
            wx_token_expires_at: Some(creds.expires_at),
            wx_refresh_token_observed_at: Some(creds.refresh_token_observed_at),
            ..Default::default()
        },
    );
    accounts::persist_global();
}

#[cfg(test)]
mod tests {
    use super::disconnect_reason_label;

    #[test]
    fn disconnect_reason_uses_chinese_labels() {
        assert_eq!(disconnect_reason_label("heartbeat_timeout"), "心跳超时");
        assert_eq!(disconnect_reason_label("kickout"), "被踢下线");
        assert_eq!(disconnect_reason_label("ws_close"), "连接关闭");
        assert_eq!(disconnect_reason_label(""), "未知原因");
        assert_eq!(disconnect_reason_label("自定义原因"), "自定义原因");
    }
}

// Worker 持有 `Arc<Gateway>`；网关本身是具体类型而非 trait object。
