//! Runtime 引擎。
//!
//! 顶层入口，管理多个 Worker。
//!
//! ## 装配
//!
//! 1. 构造 `RuntimeState`（日志 / 账号日志 / config revision / 事件总线）
//! 2. 构造 `ReloginReminderService`（注入 engine 自己作为 `WorkerControls`）
//! 3. 构造 `RuntimeEngine`（持有上面两者 + 现有 worker 管理）
//!
//! ## 与原 TS 的差异
//!
//! - 原 TS 通过 fork 进程 + IPC 跑 worker；本实现是 in-process tokio task
//! - 原 TS 的 `worker-manager.ts` 在本实现里被简化（无 fork / 无 IPC）
//! - 原 TS 的 `data-provider.ts`（HTTP API 数据源）放到 `qq-farm-server` crate 实现
//!
//! 1:1 翻译自原 `core/src/runtime/runtime-engine.ts`（210 行）。

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::broadcast;

use crate::error::Result;
use crate::models::store::accounts as accounts_store;
use crate::models::AccountSession;
use crate::network::gateway::GatewayConfig;
use crate::runtime::events::WorkerEvent;
use crate::runtime::relogin_reminder::{
    AccountNoticeKind, NoopWorkerControls, ReloginReminderService, ReminderLogger, WorkerControls,
};
use crate::runtime::runtime_state::{AccountStoreLike, RuntimeEvent, RuntimeState, WorkerInfo};
use crate::runtime::worker::{Worker, WorkerConfig};
use crate::runtime::worker_handle::WorkerHandle;
use crate::services::qq_bot::QqBotService;
use crate::services::qrlogin::MiniProgramLoginSession;

/// 默认 operation keys（1:1 对齐原 TS `OPERATION_KEYS`）
pub const DEFAULT_OPERATION_KEYS: &[&str] = &[
    "harvest",
    "farming",
    "fertilize",
    "plant",
    "steal",
    "helpFarming",
    "taskClaim",
    "sell",
    "upgrade",
];

/// 引擎配置
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// 最多同时跑多少个 worker
    pub max_workers: usize,
    /// 状态上报间隔
    pub status_interval: Duration,
    /// TSDK wasm 路径
    pub tsdk_wasm_path: PathBuf,
    /// 数据根目录（每个 worker 一个子目录）
    pub data_root: PathBuf,
    /// 网关配置模板（每个 worker 的 `GatewayConfig` 由模板 + 账号 code 组合）
    pub gateway_template: GatewayConfigTemplate,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_workers: 16,
            status_interval: Duration::from_secs(3),
            tsdk_wasm_path: std::env::var("TSDK_WASM_PATH").map(PathBuf::from).unwrap_or_else(
                |_| crate::config::paths::get_resource_path(&["assets", "tsdk.wasm"]),
            ),
            data_root: crate::config::paths::get_data_dir(),
            gateway_template: GatewayConfigTemplate::default(),
        }
    }
}

/// 网关配置模板（不含 code）
#[derive(Debug, Clone, Default)]
pub struct GatewayConfigTemplate {
    pub server_url: String,
    pub platform: String,
    pub os: String,
    pub client_version: String,
    pub headers: std::collections::HashMap<String, String>,
}

/// Worker 摘要信息
#[derive(Debug, Clone)]
pub struct EngineWorkerInfo {
    pub account_id: String,
    pub account_name: String,
    pub running: bool,
}

/// Runtime 引擎
pub struct RuntimeEngine {
    config: EngineConfig,
    workers: Arc<RwLock<HashMap<String, WorkerHandle>>>,
    /// WorkerLoop 注册表（controller 用）
    worker_loops: Arc<RwLock<HashMap<String, Arc<crate::runtime::worker_loop::WorkerLoop>>>>,
    events: broadcast::Sender<WorkerEvent>,
    /// Runtime 状态（log / account_log / configRevision / 事件总线）
    runtime_state: Arc<RuntimeState>,
    /// 重登录提醒服务
    relogin_reminder: Arc<ReloginReminderService>,
    /// 微信应用宝授权掉线换码重连状态
    wx_reconnect: Arc<RwLock<WxReconnectState>>,
}

#[derive(Debug, Default)]
struct WxReconnectState {
    attempts: HashMap<String, u32>,
    inflight: HashSet<String>,
}

impl std::fmt::Debug for RuntimeEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeEngine")
            .field("worker_count", &self.workers.read().len())
            .field("max_workers", &self.config.max_workers)
            .finish_non_exhaustive()
    }
}

impl RuntimeEngine {
    /// 创建引擎（带 runtime state + relogin reminder 全套）。
    ///
    /// 完整装配：默认 AccountStoreLike 走 `models::store`，WorkerControls 走 self。
    /// 适合作为 server crate 的入口。
    #[must_use]
    pub fn assemble(config: EngineConfig) -> Self {
        let operation_keys: Vec<String> =
            DEFAULT_OPERATION_KEYS.iter().map(|s| s.to_string()).collect();
        let runtime_state =
            Arc::new(RuntimeState::new(Arc::new(StoreAccountStoreLike::default()), operation_keys));
        Self::assemble_with(config, runtime_state, None)
    }

    /// 创建引擎（注入已有的 RuntimeState + 可选 ReloginReminderService）。
    #[must_use]
    pub fn assemble_with(
        config: EngineConfig,
        runtime_state: Arc<RuntimeState>,
        relogin_reminder: Option<Arc<ReloginReminderService>>,
    ) -> Self {
        let (events, _) = broadcast::channel(4096);
        let workers: Arc<RwLock<HashMap<String, WorkerHandle>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let worker_loops: Arc<
            RwLock<HashMap<String, Arc<crate::runtime::worker_loop::WorkerLoop>>>,
        > = Arc::new(RwLock::new(HashMap::new()));
        let qq_bot = Arc::new(QqBotService::new());
        let relogin_reminder = relogin_reminder.unwrap_or_else(|| {
            // 没传就构造默认（无 worker controls 联动）
            let mp = Arc::new(MiniProgramLoginSession::new());
            Arc::new(ReloginReminderService::new(
                mp,
                qq_bot.clone(),
                Arc::new(NoopWorkerControls),
                Arc::new(StateLoggerAdapter::new(runtime_state.clone())),
            ))
        });
        Self {
            config,
            workers,
            worker_loops,
            events,
            runtime_state,
            relogin_reminder,
            wx_reconnect: Arc::new(RwLock::new(WxReconnectState::default())),
        }
    }

    /// 用 `Arc<RuntimeEngine>` 构造一个 `EngineWorkerControls`，
    /// 供 `ReloginReminderService` 回调启动/重启 worker。
    #[must_use]
    pub fn worker_controls(self: &Arc<Self>) -> Arc<EngineWorkerControls> {
        Arc::new(EngineWorkerControls { engine: self.clone() })
    }

    /// 订阅 worker 事件
    pub fn subscribe_events(&self) -> broadcast::Receiver<WorkerEvent> {
        self.events.subscribe()
    }

    /// 订阅 runtime 事件（log / account_log / status / worker_log）
    pub fn subscribe_runtime_events(
        &self,
    ) -> broadcast::Receiver<crate::runtime::runtime_state::RuntimeEvent> {
        self.runtime_state.subscribe()
    }

    /// 获取 runtime state
    #[must_use]
    pub fn runtime_state(&self) -> Arc<RuntimeState> {
        self.runtime_state.clone()
    }

    /// 获取 relogin reminder
    #[must_use]
    pub fn relogin_reminder(&self) -> Arc<ReloginReminderService> {
        self.relogin_reminder.clone()
    }

    /// 获取 QQ 官方机器人通知服务。
    #[must_use]
    pub fn qq_bot(&self) -> Arc<QqBotService> {
        self.relogin_reminder.qq_bot()
    }

    /// 当前 worker 数
    #[must_use]
    pub fn worker_count(&self) -> usize {
        self.workers.read().len()
    }

    /// 列出所有 worker
    pub fn list_workers(&self) -> Vec<EngineWorkerInfo> {
        let workers = self.workers.read();
        let state_workers = self.runtime_state.workers.lock();
        workers
            .values()
            .filter(|h| !h.is_cancelled())
            .map(|h| {
                let account_name = state_workers
                    .get(&h.account_id)
                    .map(|w| w.account_name.clone())
                    .unwrap_or_else(|| h.account_id.clone());
                EngineWorkerInfo { account_id: h.account_id.clone(), account_name, running: true }
            })
            .collect()
    }

    /// 获取某账号的 WorkerLoop（controller 用）
    #[must_use]
    pub fn worker_loop(
        &self,
        account_id: &str,
    ) -> Option<Arc<crate::runtime::worker_loop::WorkerLoop>> {
        self.worker_loops.read().get(account_id).cloned()
    }

    /// 注册 WorkerLoop（worker.rs 在 spawn 完成后调用）
    pub fn register_worker_loop(
        &self,
        account_id: &str,
        worker_loop: Arc<crate::runtime::worker_loop::WorkerLoop>,
    ) {
        self.worker_loops.write().insert(account_id.to_string(), worker_loop);
    }

    /// 注销 WorkerLoop（worker 停止时调用）
    pub fn unregister_worker_loop(&self, account_id: &str) {
        self.worker_loops.write().remove(account_id);
    }

    /// 列出所有已注册的 WorkerLoop accountId
    pub fn registered_worker_loop_ids(&self) -> Vec<String> {
        self.worker_loops.read().keys().cloned().collect()
    }

    /// worker 任务是否还在注册表里（fork 语义：进程在即 running）
    #[must_use]
    pub fn has_worker(&self, account_id: &str) -> bool {
        self.workers.read().get(account_id).is_some_and(|h| !h.is_cancelled())
    }

    /// worker 任务已退出时摘掉注册，不 cancel（供 spawn 内部调用）
    pub fn release_worker(&self, account_id: &str) {
        self.workers.write().remove(account_id);
        self.worker_loops.write().remove(account_id);
        if let Some(w) = self.runtime_state.workers.lock().get_mut(account_id) {
            w.stopping = true;
        }
    }

    /// 对齐原 bot `getStatus`：worker 未启动或尚未上报也返回默认面板 status
    #[must_use]
    pub fn panel_status(&self, account_id: &str) -> serde_json::Value {
        let state = self.runtime_state();
        let default = serde_json::to_value(state.build_default_status(account_id))
            .unwrap_or_else(|_| serde_json::json!({}));
        let guard = state.workers.lock();
        let Some(w) = guard.get(account_id) else {
            return default;
        };
        let name = w.account_name.clone();
        let ws_error = panel_ws_error(&w.ws_error);
        let raw = w.status.clone();
        drop(guard);
        let Some(raw) = raw else {
            let mut v = default;
            if let Some(obj) = v.as_object_mut() {
                obj.insert("wsError".to_string(), ws_error);
            }
            return v;
        };
        let mut merged = default;
        if let (Some(base), Some(over)) = (merged.as_object_mut(), raw.as_object()) {
            for (k, v) in over {
                base.insert(k.clone(), v.clone());
            }
        }
        let mut normalized = state.normalize_status_for_panel(Some(&merged), account_id, &name);
        if let Some(obj) = normalized.as_object_mut() {
            obj.insert("wsError".to_string(), ws_error);
        }
        normalized
    }

    /// 把 WorkerEvent 灌进 runtime_state（status / 日志），供 HTTP 与 Socket.IO 共用
    pub fn spawn_event_bridge(self: &Arc<Self>) {
        let mut rx = self.subscribe_events();
        let state = self.runtime_state.clone();
        let engine = Arc::clone(self);
        crate::runtime::safe_spawn::spawn_logged("worker_event_bridge", async move {
            loop {
                let ev = match rx.recv().await {
                    Ok(ev) => ev,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "worker event bridge lagged");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                match ev {
                    WorkerEvent::Status { account_id, account_name, status } => {
                        let connected = status
                            .pointer("/connection/connected")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let nick = status
                            .pointer("/status/name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let avatar = status
                            .pointer("/status/avatar")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .or_else(|| {
                                status
                                    .pointer("/status/avatarUrl")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                            })
                            .unwrap_or("")
                            .to_string();
                        let now = crate::utils::time::now_ms();
                        let mut auto_delete: Option<(String, String, i64)> = None;
                        {
                            let mut workers = state.workers.lock();
                            if let Some(w) = workers.get_mut(&account_id) {
                                w.status = Some(status);
                                if !account_name.is_empty() {
                                    w.account_name = account_name.clone();
                                }
                                if connected {
                                    w.disconnected_since = None;
                                    w.auto_delete_triggered = false;
                                    engine.wx_reconnect.write().attempts.remove(&account_id);
                                } else if !w.stopping {
                                    if w.disconnected_since.is_none() {
                                        w.disconnected_since = Some(now);
                                    }
                                    let since = w.disconnected_since.unwrap_or(now);
                                    let username = crate::models::store::accounts::get_accounts()
                                        .into_iter()
                                        .find(|a| a.id == account_id)
                                        .map(|a| a.username)
                                        .unwrap_or_default();
                                    let auto_ms = engine
                                        .relogin_reminder()
                                        .get_offline_auto_delete_ms(&username);
                                    if !w.auto_delete_triggered
                                        && auto_ms != i64::MAX
                                        && now.saturating_sub(since) >= auto_ms
                                    {
                                        w.auto_delete_triggered = true;
                                        auto_delete = Some((
                                            account_id.clone(),
                                            w.account_name.clone(),
                                            now.saturating_sub(since),
                                        ));
                                    }
                                }
                            }
                        }
                        if !nick.is_empty() || !avatar.is_empty() {
                            if let Some(mut acc) = crate::models::store::accounts::get_accounts()
                                .into_iter()
                                .find(|a| a.id == account_id)
                            {
                                let mut dirty = false;
                                if !nick.is_empty() && acc.nick != nick {
                                    acc.nick = nick;
                                    dirty = true;
                                }
                                if !avatar.is_empty() && acc.avatar != avatar {
                                    acc.avatar = avatar;
                                    dirty = true;
                                }
                                if dirty {
                                    crate::models::store::accounts::add_or_update_account(acc);
                                    crate::models::store::accounts::persist_global();
                                }
                            }
                        }
                        if let Some((id, name, offline_ms)) = auto_delete {
                            let mins = offline_ms / 60_000;
                            state.log(
                                "系统",
                                &format!("账号 {name} 持续离线 {mins} 分钟，自动删除账号信息"),
                                Some(serde_json::json!({ "accountId": id })),
                            );
                            state.add_account_log(
                                "offline_delete",
                                &format!("账号 {name} 持续离线 {mins} 分钟，已自动删除"),
                                Some(&id),
                                Some(&name),
                                Some(serde_json::json!({ "reason": "offline_timeout", "offlineMs": offline_ms })),
                            );
                            engine.stop_worker(&id);
                            crate::models::store::accounts::delete_account(&id);
                            crate::models::store::accounts::persist_global();
                        }
                        let panel = engine.panel_status(&account_id);
                        let _ = state.events.send(RuntimeEvent::Status {
                            account_id,
                            account_name,
                            status: panel,
                        });
                    }
                    WorkerEvent::Error { account_id, message } => {
                        let code = parse_ws_http_code(&message).unwrap_or(0);
                        {
                            let mut workers = state.workers.lock();
                            if let Some(w) = workers.get_mut(&account_id) {
                                w.ws_error = Some(message.clone());
                            }
                        }
                        if code == 400 {
                            let has_wx = accounts_store::get_accounts()
                                .into_iter()
                                .find(|a| a.id == account_id)
                                .is_some_and(|a| a.has_wx_auth());
                            let msg = if has_wx {
                                "连接被拒绝，稍后将用应用宝授权重连".to_string()
                            } else {
                                "连接被拒绝，可能需要更新 Code".to_string()
                            };
                            state.log(
                                "系统",
                                &msg,
                                Some(serde_json::json!({ "accountId": account_id, "code": 400 })),
                            );
                            let name = state
                                .workers
                                .lock()
                                .get(&account_id)
                                .map(|w| w.account_name.clone())
                                .unwrap_or_else(|| account_id.clone());
                            let log_msg = if has_wx {
                                format!("账号 {name} 登录码失效，稍后将用应用宝授权重连")
                            } else {
                                format!("账号 {name} 登录失效，请更新 Code")
                            };
                            state.add_account_log(
                                "ws_400",
                                &log_msg,
                                Some(&account_id),
                                Some(&name),
                                None,
                            );
                        }
                    }
                    WorkerEvent::Log { account_id, account_name, level, module, message } => {
                        let tag = panel_log_tag(&level, &module);
                        let is_warn = level == "warn" || level == "error";
                        state.log(
                            tag,
                            &message,
                            Some(serde_json::json!({
                                "accountId": account_id,
                                "accountName": account_name,
                                "module": module,
                                "isWarn": is_warn,
                            })),
                        );
                    }
                    WorkerEvent::Started { account_id, account_name } => {
                        let username = accounts_store::get_accounts()
                            .into_iter()
                            .find(|a| a.id == account_id)
                            .map(|a| a.username)
                            .unwrap_or_default();
                        spawn_account_notice(
                            &engine,
                            &account_id,
                            &account_name,
                            &username,
                            "online",
                            AccountNoticeKind::Online,
                        );
                    }
                    WorkerEvent::Stopped { account_id, reason } => {
                        let (name, already) = {
                            let mut workers = state.workers.lock();
                            match workers.get_mut(&account_id) {
                                Some(w) => {
                                    let already = w.terminal_handled;
                                    w.terminal_handled = true;
                                    w.stopping = true;
                                    (w.account_name.clone(), already)
                                }
                                None => (String::new(), true),
                            }
                        };
                        let display =
                            if name.is_empty() { account_id.clone() } else { name.clone() };
                        let user_stop = reason == "主动取消";
                        let kicked = reason.contains("kickout");
                        let wx_auth_failed = reason.contains("wx_auth_failed");
                        let wx_mint_failed = reason.contains("wx_mint_failed");
                        let acc =
                            accounts_store::get_accounts().into_iter().find(|a| a.id == account_id);
                        let has_wx = acc.as_ref().is_some_and(|a| a.has_wx_auth());
                        if user_stop || wx_auth_failed {
                            engine.clear_wx_reconnect(&account_id);
                        }
                        let mut schedule_wx_reconnect: Option<u32> = None;
                        if !already && !user_stop {
                            if wx_auth_failed {
                                state.log(
                                    "系统",
                                    &format!("账号 {display} 应用宝授权已失效，请重新扫码"),
                                    Some(serde_json::json!({
                                        "accountId": account_id,
                                        "accountName": display,
                                        "reason": reason,
                                    })),
                                );
                                state.add_account_log(
                                    "wx_auth_failed",
                                    &format!("账号 {display} 应用宝授权已失效，请重新扫码"),
                                    Some(&account_id),
                                    Some(&display),
                                    Some(serde_json::json!({ "reason": reason })),
                                );
                                engine.emit_account_status(
                                    &account_id,
                                    &display,
                                    "error",
                                    "应用宝授权已失效，请重新扫码",
                                    false,
                                );
                                spawn_account_notice(
                                    &engine,
                                    &account_id,
                                    &display,
                                    acc.as_ref().map(|a| a.username.as_str()).unwrap_or(""),
                                    &reason,
                                    AccountNoticeKind::YybQr,
                                );
                            } else if wx_mint_failed {
                                state.log(
                                    "系统",
                                    &format!("账号 {display} 应用宝换码失败，请稍后重试或重新扫码"),
                                    Some(serde_json::json!({
                                        "accountId": account_id,
                                        "accountName": display,
                                        "reason": reason,
                                    })),
                                );
                                spawn_account_notice(
                                    &engine,
                                    &account_id,
                                    &display,
                                    acc.as_ref().map(|a| a.username.as_str()).unwrap_or(""),
                                    &reason,
                                    AccountNoticeKind::YybQr,
                                );
                            } else if should_attempt_wx_reconnect(user_stop, has_wx, &reason) {
                                match engine.plan_wx_reconnect(&account_id) {
                                    WxReconnectPlan::Spawn { attempt } => {
                                        schedule_wx_reconnect = Some(attempt);
                                        let wait = crate::constants::wx_reconnect_delay_zh(attempt);
                                        let max = crate::constants::WX_RECONNECT_MAX_ATTEMPTS;
                                        let msg = if kicked {
                                            format!(
                                                "账号 {display} 被踢下线，将在 {wait}后用应用宝授权重连（第 {attempt}/{max} 次）"
                                            )
                                        } else {
                                            format!(
                                                "账号 {display} 连接已断开，将在 {wait}后用应用宝授权重连（第 {attempt}/{max} 次）"
                                            )
                                        };
                                        state.log(
                                            "系统",
                                            &msg,
                                            Some(serde_json::json!({
                                                "accountId": account_id,
                                                "accountName": display,
                                                "reason": reason,
                                                "attempt": attempt,
                                            })),
                                        );
                                        state.add_account_log(
                                            "wx_reconnect",
                                            &msg,
                                            Some(&account_id),
                                            Some(&display),
                                            Some(serde_json::json!({ "reason": reason, "attempt": attempt })),
                                        );
                                        spawn_account_notice(
                                            &engine,
                                            &account_id,
                                            &display,
                                            acc.as_ref().map(|a| a.username.as_str()).unwrap_or(""),
                                            &reason,
                                            AccountNoticeKind::Offline,
                                        );
                                    }
                                    WxReconnectPlan::GiveUp => {
                                        state.log(
                                            "系统",
                                            &format!("账号 {display} 应用宝授权失效，请重新扫码"),
                                            Some(serde_json::json!({
                                                "accountId": account_id,
                                                "accountName": display,
                                                "reason": reason,
                                            })),
                                        );
                                        state.add_account_log(
                                            "disconnect_stop",
                                            &format!("账号 {display} 应用宝授权失效，请重新扫码"),
                                            Some(&account_id),
                                            Some(&display),
                                            Some(serde_json::json!({ "reason": reason })),
                                        );
                                        spawn_account_notice(
                                            &engine,
                                            &account_id,
                                            &display,
                                            acc.as_ref().map(|a| a.username.as_str()).unwrap_or(""),
                                            &reason,
                                            AccountNoticeKind::YybQr,
                                        );
                                    }
                                    WxReconnectPlan::Skip => {}
                                }
                            } else if kicked {
                                state.log(
                                    "系统",
                                    &format!("账号 {display} 被踢下线，已自动停止账号"),
                                    Some(serde_json::json!({
                                        "accountId": account_id,
                                        "accountName": display,
                                        "reason": reason,
                                    })),
                                );
                                state.add_account_log(
                                    "kickout_stop",
                                    &format!("账号 {display} 被踢下线，已自动停止"),
                                    Some(&account_id),
                                    Some(&display),
                                    Some(serde_json::json!({ "reason": reason })),
                                );
                                spawn_account_notice(
                                    &engine,
                                    &account_id,
                                    &display,
                                    acc.as_ref().map(|a| a.username.as_str()).unwrap_or(""),
                                    &reason,
                                    AccountNoticeKind::Offline,
                                );
                            } else {
                                state.log(
                                    "系统",
                                    &format!("账号 {display} 连接已断开，已停止运行并等待重新扫码"),
                                    Some(serde_json::json!({
                                        "accountId": account_id,
                                        "accountName": display,
                                        "reason": reason,
                                    })),
                                );
                                state.add_account_log(
                                    "disconnect_stop",
                                    &format!("账号 {display} 连接已断开，已停止运行并等待重新扫码"),
                                    Some(&account_id),
                                    Some(&display),
                                    Some(serde_json::json!({ "reason": reason })),
                                );
                                spawn_account_notice(
                                    &engine,
                                    &account_id,
                                    &display,
                                    acc.as_ref().map(|a| a.username.as_str()).unwrap_or(""),
                                    &reason,
                                    AccountNoticeKind::Offline,
                                );
                            }
                        }
                        state.workers.lock().remove(&account_id);
                        engine.release_worker(&account_id);
                        if let Some(attempt) = schedule_wx_reconnect {
                            let engine2 = engine.clone();
                            let reconnect_id = account_id.clone();
                            crate::runtime::safe_spawn::spawn_logged("wx_reconnect", async move {
                                let delay = crate::constants::wx_reconnect_delay_ms(attempt);
                                tokio::time::sleep(Duration::from_millis(delay)).await;
                                engine2.wx_reconnect.write().inflight.remove(&reconnect_id);
                                let Some(latest) = accounts_store::get_accounts()
                                    .into_iter()
                                    .find(|a| a.id == reconnect_id)
                                else {
                                    return;
                                };
                                if !latest.has_wx_auth() {
                                    return;
                                }
                                engine2.start_wx_authorized_account(&latest, attempt);
                            });
                        }
                        let panel = engine.panel_status(&account_id);
                        let _ = state.events.send(RuntimeEvent::Status {
                            account_id,
                            account_name: display,
                            status: panel,
                        });
                    }
                    _ => {}
                }
            }
        });
    }

    /// 启动一个 worker
    pub fn start_worker(self: &Arc<Self>, account: AccountSession) -> Result<()> {
        let max = self.config.max_workers;
        {
            let mut workers = self.workers.write();
            if let Some(h) = workers.get(&account.id) {
                if !h.is_cancelled() {
                    tracing::warn!(account_id = %account.id, "worker already running");
                    let name = account.display_name.clone();
                    self.runtime_state.log(
                        "系统",
                        &format!("账号 {name} 已在运行，跳过启动"),
                        Some(serde_json::json!({
                            "accountId": account.id,
                            "accountName": name,
                            "module": "system",
                            "event": "login",
                        })),
                    );
                    return Ok(());
                }
                workers.remove(&account.id);
            }
            if workers.len() >= max {
                return Err(crate::error::Error::Internal(format!("max workers reached ({max})")));
            }
        }

        let gateway_config = self.gateway_config_for(&account);
        tracing::info!(
            account_id = %account.id,
            platform = %gateway_config.platform,
            os = %gateway_config.os,
            ver = %gateway_config.client_version,
            "启动 worker"
        );

        let worker_config = WorkerConfig {
            gateway: gateway_config,
            status_interval: self.config.status_interval,
            tsdk_wasm_path: self.config.tsdk_wasm_path.clone(),
            data_dir: self.config.data_root.clone(),
        };

        let worker = Worker::new(account.clone(), worker_config, self.events.clone());
        let handle = worker.handle();
        self.workers.write().insert(handle.account_id.clone(), handle);

        // 同步 worker 状态到 runtime_state
        {
            let mut state_workers = self.runtime_state.workers.lock();
            state_workers.insert(
                account.id.clone(),
                WorkerInfo {
                    account_id: account.id.clone(),
                    account_name: account.display_name.clone(),
                    status: None,
                    ws_error: None,
                    stopping: false,
                    terminal_handled: false,
                    disconnected_since: None,
                    auto_delete_triggered: false,
                },
            );
        }
        worker.spawn_with_engine(Some(self.clone()));
        let start_extra = Some(serde_json::json!({
            "accountId": account.id,
            "accountName": account.display_name,
            "module": "system",
            "event": "login",
        }));
        self.runtime_state.log(
            "系统",
            &format!("开始启动账号: {}", account.display_name),
            start_extra.clone(),
        );
        self.runtime_state.add_account_log(
            "add",
            &format!("启动账号: {}", account.display_name),
            Some(&account.id),
            Some(&account.display_name),
            None,
        );
        Ok(())
    }

    /// 每个 worker 用账号自己的 platform/code 拼网关（对齐 TS `CONFIG.platform = platform || 'qq'`）。
    fn gateway_config_for(&self, account: &AccountSession) -> GatewayConfig {
        let rt = crate::config::get_runtime_config();
        let platform = if !account.platform.trim().is_empty() {
            account.platform.trim().to_string()
        } else if !rt.platform.trim().is_empty() {
            rt.platform.clone()
        } else if !self.config.gateway_template.platform.is_empty() {
            self.config.gateway_template.platform.clone()
        } else {
            "qq".to_string()
        };
        let os = if !rt.os.trim().is_empty() {
            rt.os.clone()
        } else if !self.config.gateway_template.os.is_empty() {
            self.config.gateway_template.os.clone()
        } else {
            "Windows".to_string()
        };
        let client_version = if !rt.client_version.trim().is_empty() {
            rt.client_version.clone()
        } else {
            self.config.gateway_template.client_version.clone()
        };
        let auth_code = if !account.code.trim().is_empty() {
            account.code.trim().to_string()
        } else {
            account.open_id.clone()
        };
        GatewayConfig {
            server_url: self.config.gateway_template.server_url.clone(),
            platform,
            os,
            client_version,
            auth_code,
            headers: self.config.gateway_template.headers.clone(),
        }
    }

    fn clear_wx_reconnect(&self, account_id: &str) {
        let mut g = self.wx_reconnect.write();
        g.attempts.remove(account_id);
        g.inflight.remove(account_id);
    }

    fn plan_wx_reconnect(&self, account_id: &str) -> WxReconnectPlan {
        let mut g = self.wx_reconnect.write();
        if g.inflight.contains(account_id) {
            return WxReconnectPlan::Skip;
        }
        let n = g.attempts.entry(account_id.to_string()).or_insert(0);
        *n = n.saturating_add(1);
        if *n > crate::constants::WX_RECONNECT_MAX_ATTEMPTS {
            g.inflight.remove(account_id);
            return WxReconnectPlan::GiveUp;
        }
        let attempt = *n;
        g.inflight.insert(account_id.to_string());
        WxReconnectPlan::Spawn { attempt }
    }

    /// 停止一个 worker
    pub fn stop_worker(&self, account_id: &str) {
        let handle = self.workers.write().remove(account_id);
        if let Some(h) = handle {
            h.cancel();
            tracing::info!(account_id, "worker stop requested");
        }
        // 注销 WorkerLoop
        self.worker_loops.write().remove(account_id);
        // 同步 worker 状态
        {
            let mut state_workers = self.runtime_state.workers.lock();
            if let Some(w) = state_workers.get_mut(account_id) {
                w.stopping = true;
            }
        }
    }

    /// 重启一个 worker
    pub fn restart_worker(self: &Arc<Self>, account: AccountSession) -> Result<()> {
        self.stop_worker(&account.id);
        self.start_worker(account)
    }

    /// 启动所有账号（原 TS `startAllAccounts`）
    pub fn start_all_accounts(self: &Arc<Self>) {
        let accounts = accounts_store::get_accounts();
        if accounts.is_empty() {
            self.runtime_state.log("系统", "未发现账号，请访问管理面板添加账号", None);
            return;
        }
        self.runtime_state.log(
            "系统",
            &format!("发现 {} 个账号，正在启动...", accounts.len()),
            None,
        );
        for acc in accounts {
            if acc.code.trim().is_empty() && !acc.has_wx_auth() {
                continue;
            }
            let name = acc.name.clone();
            let account = AccountSession::from_store(&acc);
            if let Err(e) = self.start_worker(account) {
                self.runtime_state.log("错误", &format!("启动账号 {name} 失败: {e}"), None);
            }
        }
    }

    /// 进程启动后：已授权微信账号延迟再连，结果写入运行日志。
    pub fn schedule_wx_authorized_start(self: &Arc<Self>) {
        let delay = crate::constants::WX_STARTUP_RECONNECT_DELAY_MS;
        let wait = crate::constants::wx_startup_reconnect_delay_zh();
        let accounts: Vec<_> =
            accounts_store::get_accounts().into_iter().filter(|a| a.has_wx_auth()).collect();
        if accounts.is_empty() {
            return;
        }
        let n = accounts.len();
        self.runtime_state.log(
            "系统",
            &format!("发现 {n} 个已授权微信账号，将在 {wait}后自动重连"),
            None,
        );
        let engine = self.clone();
        crate::runtime::safe_spawn::spawn_logged("wx_authorized_start", async move {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            for acc in accounts {
                let Some(latest) =
                    accounts_store::get_accounts().into_iter().find(|a| a.id == acc.id)
                else {
                    continue;
                };
                if !latest.has_wx_auth() {
                    continue;
                }
                engine.start_wx_authorized_account(&latest, 0);
            }
        });
    }

    /// 后台保活：每 30 分钟检查 accesstoken，剩余不足 45 分钟则续 token + buffer。
    pub fn spawn_wx_keepalive(self: &Arc<Self>) {
        let engine = self.clone();
        crate::runtime::safe_spawn::spawn_logged("wx_keepalive", async move {
            let interval = Duration::from_millis(crate::constants::WX_KEEPALIVE_INTERVAL_MS);
            loop {
                tokio::time::sleep(interval).await;
                engine.run_wx_keepalive_tick().await;
            }
        });
    }

    async fn run_wx_keepalive_tick(&self) {
        let svc = crate::services::wx_login::service::WxLoginService::new();
        let ahead = crate::constants::WX_KEEPALIVE_AHEAD_SECS;
        let accounts: Vec<_> = accounts_store::get_accounts()
            .into_iter()
            .filter(|a| a.can_refresh_wx_token())
            .collect();
        for acc in accounts {
            let creds = AccountSession::from_store(&acc).yyb_credentials();
            if !creds.token_due_for_refresh(ahead) {
                continue;
            }
            let account_id = acc.id.clone();
            let display =
                if acc.name.trim().is_empty() { account_id.clone() } else { acc.name.clone() };
            match svc.refresh_credentials_and_buffer(&creds).await {
                Ok(updated) => {
                    accounts_store::persist_yyb_credentials(
                        &account_id,
                        accounts_store::YybCredentialPatch {
                            wx_login_buffer: Some(updated.login_buffer.clone()),
                            wx_access_token: Some(updated.access_token.clone()),
                            wx_refresh_token: Some(updated.refresh_token.clone()),
                            wx_token_expires_at: Some(updated.expires_at),
                            wx_refresh_token_observed_at: Some(updated.refresh_token_observed_at),
                            ..Default::default()
                        },
                    );
                    accounts_store::persist_global();
                    tracing::info!(account_id = %account_id, "应用宝 token 保活成功");
                }
                Err(e) if e.kind == crate::services::wx_login::WxAuthErrorKind::CredentialsDead => {
                    tracing::warn!(account_id = %account_id, "应用宝保活失败，清授权: {e}");
                    accounts_store::clear_wx_auth(&account_id);
                    accounts_store::persist_global();
                    self.clear_wx_reconnect(&account_id);
                    self.stop_worker(&account_id);
                    self.runtime_state.log(
                        "系统",
                        &format!("账号 {display} 应用宝授权已失效，请重新扫码"),
                        Some(serde_json::json!({
                            "accountId": account_id,
                            "accountName": display,
                        })),
                    );
                    self.emit_account_status(
                        &account_id,
                        &display,
                        "error",
                        "应用宝授权已失效，请重新扫码",
                        false,
                    );
                }
                Err(e) => {
                    tracing::warn!(account_id = %account_id, "应用宝保活临时失败: {e}");
                }
            }
        }
    }

    /// 换码失败清授权后通知面板（wxAuthorized=false）。
    pub fn notify_wx_auth_cleared(&self, account_id: &str, account_name: &str) {
        self.emit_account_status(
            account_id,
            account_name,
            "error",
            "应用宝授权已失效，请重新扫码",
            false,
        );
        let panel = self.panel_status(account_id);
        let _ = self.runtime_state.events.send(RuntimeEvent::Status {
            account_id: account_id.to_string(),
            account_name: account_name.to_string(),
            status: panel,
        });
    }

    fn emit_account_status(
        &self,
        account_id: &str,
        account_name: &str,
        status: &str,
        detail: &str,
        wx_authorized: bool,
    ) {
        let _ = self.runtime_state.events.send(RuntimeEvent::AccountStatus {
            account_id: account_id.to_string(),
            account_name: account_name.to_string(),
            status: status.to_string(),
            detail: detail.to_string(),
            wx_authorized,
        });
    }

    fn start_wx_authorized_account(
        self: &Arc<Self>,
        acc: &crate::models::store::accounts::AccountRecord,
        attempt: u32,
    ) {
        if self.has_worker(&acc.id) {
            return;
        }
        let name = if acc.name.trim().is_empty() { acc.id.clone() } else { acc.name.clone() };
        let extra = Some(serde_json::json!({
            "accountId": acc.id,
            "accountName": name,
            "attempt": attempt,
        }));
        let start_msg = if attempt > 0 {
            format!("账号 {name} 开始重连（第 {attempt} 次）")
        } else {
            format!("账号 {name} 开始用应用宝授权自动重连")
        };
        self.runtime_state.log("系统", &start_msg, extra.clone());
        match self.start_worker(AccountSession::from_store(acc)) {
            Ok(()) => {
                self.runtime_state.log(
                    "系统",
                    &format!("账号 {name} 重连任务已启动，正在换码并连接网关"),
                    extra,
                );
            }
            Err(e) => {
                tracing::warn!(account_id = %acc.id, "应用宝授权重连启动失败: {e}");
                self.runtime_state.log("错误", &format!("账号 {name} 重连启动失败: {e}"), extra);
            }
        }
    }

    /// 停止所有账号（原 TS `stopAllAccounts`）
    pub fn stop_all_accounts(&self) {
        let ids: Vec<String> = self.workers.read().keys().cloned().collect();
        for id in ids {
            self.stop_worker(&id);
        }
    }

    /// 通知单个 worker 应用已持久化的配置（热更新，不停账号）
    pub fn reload_worker_config(&self, account_id: &str) {
        use crate::runtime::worker_message::WorkerMessage;
        if let Some(h) = self.workers.read().get(account_id).cloned() {
            if h.try_send(WorkerMessage::ReloadConfig).is_err() {
                if let Ok(rt) = tokio::runtime::Handle::try_current() {
                    rt.spawn(async move {
                        let _ = h.send(WorkerMessage::ReloadConfig).await;
                    });
                }
            }
        }
        if let Some(wl) = self.worker_loop(account_id) {
            wl.sync_status();
        }
    }

    /// 广播 ReloadConfig 消息到所有 worker
    pub fn broadcast_config(&self, _config: serde_json::Value) {
        use crate::runtime::worker_message::WorkerMessage;
        let workers = self.workers.read();
        for h in workers.values() {
            let _ = h.try_send(WorkerMessage::ReloadConfig);
        }
    }

    /// 广播 Disconnect 消息
    pub fn broadcast_disconnect(&self) {
        use crate::runtime::worker_message::WorkerMessage;
        let workers = self.workers.read();
        for h in workers.values() {
            let _ = h.try_send(WorkerMessage::Disconnect);
        }
    }

    /// 关闭所有 worker
    pub fn shutdown(&self) {
        let handles: Vec<_> = self.workers.write().drain().collect();
        for (_, h) in handles {
            h.cancel();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WxReconnectPlan {
    Skip,
    Spawn { attempt: u32 },
    GiveUp,
}

fn should_attempt_wx_reconnect(user_stop: bool, has_wx_auth: bool, reason: &str) -> bool {
    !user_stop
        && has_wx_auth
        && !reason.contains("wx_auth_failed")
        && !reason.contains("wx_mint_failed")
}

fn spawn_account_notice(
    engine: &Arc<RuntimeEngine>,
    account_id: &str,
    display: &str,
    username: &str,
    reason: &str,
    kind: crate::runtime::relogin_reminder::AccountNoticeKind,
) {
    let reminder = engine.relogin_reminder();
    let reason_clean = reason.strip_prefix("disconnect:").unwrap_or(reason);
    let reason = match kind {
        crate::runtime::relogin_reminder::AccountNoticeKind::Online => reason_clean.to_string(),
        _ => format!("disconnect:{reason_clean}"),
    };
    let payload = crate::runtime::relogin_reminder::OfflineReminderPayload {
        account_id: account_id.to_string(),
        account_name: display.to_string(),
        username: username.to_string(),
        reason,
        offline_ms: 0,
        kind,
    };
    crate::runtime::safe_spawn::spawn_logged("offline_reminder", async move {
        reminder.trigger_offline_reminder(payload).await;
    });
}

fn parse_ws_http_code(msg: &str) -> Option<i64> {
    let lower = msg.to_ascii_lowercase();
    let needle = "unexpected server response:";
    let idx = lower.find(needle)?;
    let rest = msg[idx + needle.len()..].trim();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok().filter(|c| *c > 0)
}

fn panel_ws_error(raw: &Option<String>) -> serde_json::Value {
    match raw {
        None => serde_json::Value::Null,
        Some(msg) => {
            let code = parse_ws_http_code(msg).unwrap_or(0);
            serde_json::json!({
                "code": code,
                "message": msg,
                "at": crate::utils::time::now_ms(),
            })
        }
    }
}

// =====================================================================
// WorkerControls impl（用于 relogin_reminder 回调 engine 自身）
// =====================================================================

/// Engine 自己作为 WorkerControls（让 ReloginReminderService 能调 start/restart）
pub struct EngineWorkerControls {
    engine: Arc<RuntimeEngine>,
}

impl WorkerControls for EngineWorkerControls {
    fn start_worker(&self, account: &crate::models::store::accounts::AccountRecord) -> Option<()> {
        let id = account.id.clone();
        let a = AccountSession::from_store(account);
        if let Err(e) = self.engine.start_worker(a) {
            tracing::warn!(account_id = %id, "start_worker failed: {e}");
            return None;
        }
        Some(())
    }

    fn restart_worker(
        &self,
        account: &crate::models::store::accounts::AccountRecord,
    ) -> Option<()> {
        let id = account.id.clone();
        let a = AccountSession::from_store(account);
        if let Err(e) = self.engine.restart_worker(a) {
            tracing::warn!(account_id = %id, "restart_worker failed: {e}");
            return None;
        }
        Some(())
    }
}

// =====================================================================
// AccountStoreLike impl（runtime_state 读 store）
// =====================================================================

/// 默认的 AccountStoreLike —— 直接走 `models::store`
#[derive(Debug, Default)]
pub struct StoreAccountStoreLike;

impl AccountStoreLike for StoreAccountStoreLike {
    fn get_config_snapshot(&self, account_id: &str) -> crate::models::AccountConfigSnapshot {
        crate::models::store::account_config::get_config_snapshot(Some(account_id))
    }
    fn get_automation(&self, account_id: &str) -> serde_json::Value {
        let auto = crate::models::store::account_config::get_automation(Some(account_id));
        serde_json::to_value(&auto).unwrap_or(serde_json::Value::Null)
    }
    fn get_preferred_seed(&self, account_id: &str) -> i64 {
        crate::models::store::account_config::get_preferred_seed(Some(account_id))
    }
}

// =====================================================================
// ReminderLogger impl（runtime_state 作为 logger）
// =====================================================================

/// 把 runtime_state 包成 ReminderLogger
pub struct StateLoggerAdapter {
    state: Arc<RuntimeState>,
}

impl StateLoggerAdapter {
    #[must_use]
    pub fn new(state: Arc<RuntimeState>) -> Self {
        Self { state }
    }
}

impl ReminderLogger for StateLoggerAdapter {
    fn log(&self, tag: &str, msg: &str, extra: Option<serde_json::Value>) {
        self.state.log(tag, msg, extra);
    }
    fn add_account_log(
        &self,
        action: &str,
        msg: &str,
        account_id: Option<&str>,
        account_name: Option<&str>,
        extra: Option<serde_json::Value>,
    ) {
        self.state.add_account_log(action, msg, account_id, account_name, extra);
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> Arc<RuntimeEngine> {
        Arc::new(RuntimeEngine::assemble(EngineConfig { max_workers: 4, ..Default::default() }))
    }

    #[test]
    fn default_operation_keys_count() {
        assert_eq!(DEFAULT_OPERATION_KEYS.len(), 9);
    }

    #[test]
    fn assemble_creates_default_state() {
        let engine = make_engine();
        assert_eq!(engine.worker_count(), 0);
        let state = engine.runtime_state();
        assert_eq!(state.config_revision() > 0, true);
    }

    #[test]
    fn assemble_with_explicit_state() {
        let state = Arc::new(RuntimeState::new(
            Arc::new(StoreAccountStoreLike),
            vec!["a".to_string(), "b".to_string()],
        ));
        let engine =
            Arc::new(RuntimeEngine::assemble_with(EngineConfig::default(), state.clone(), None));
        // 显式传入 state 仍然有效
        assert_eq!(engine.worker_count(), 0);
    }

    #[test]
    fn worker_controls_delegate_to_engine() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let engine = make_engine();
            let controls = engine.worker_controls();
            let acc = crate::models::store::accounts::AccountRecord {
                id: "test-acc".to_string(),
                name: "Test".to_string(),
                code: "code123".to_string(),
                platform: "qq".to_string(),
                uin: "u".to_string(),
                qq: "q".to_string(),
                ..Default::default()
            };
            controls.start_worker(&acc);
            assert_eq!(engine.worker_count(), 1);
            let listed = engine.list_workers();
            assert!(listed.iter().any(|w| w.account_id == "test-acc"));
        });
    }

    #[test]
    fn list_workers_empty() {
        let engine = make_engine();
        let listed = engine.list_workers();
        assert!(listed.is_empty());
    }

    #[test]
    fn gateway_config_uses_account_wx_platform_and_code() {
        let engine = make_engine();
        let mut acc = AccountSession::new("a1", "openid-fallback", "n");
        acc.platform = "wx".into();
        acc.code = "wx-one-time".into();
        let gw = engine.gateway_config_for(&acc);
        assert_eq!(gw.platform, "wx");
        assert_eq!(gw.auth_code, "wx-one-time");
    }

    #[test]
    fn start_worker_respects_max() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let engine = Arc::new(RuntimeEngine::assemble(EngineConfig {
                max_workers: 2,
                ..Default::default()
            }));
            for i in 0..2 {
                let acc = AccountSession::new(format!("a{i}"), format!("c{i}"), format!("n{i}"));
                engine.start_worker(acc).unwrap();
            }
            let acc = AccountSession::new("overflow", "c", "n");
            let r = engine.start_worker(acc);
            assert!(r.is_err());
        });
    }

    #[test]
    fn stop_worker_removes_from_map() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let engine = make_engine();
            let acc = AccountSession::new("a1", "c", "n");
            engine.start_worker(acc).unwrap();
            assert_eq!(engine.worker_count(), 1);
            engine.stop_worker("a1");
            assert_eq!(engine.worker_count(), 0);
        });
    }

    #[test]
    fn shutdown_clears_all() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let engine = make_engine();
            for i in 0..3 {
                let acc = AccountSession::new(format!("a{i}"), format!("c{i}"), format!("n{i}"));
                engine.start_worker(acc).unwrap();
            }
            assert_eq!(engine.worker_count(), 3);
            engine.shutdown();
            assert_eq!(engine.worker_count(), 0);
        });
    }

    #[test]
    #[serial_test::serial(engine)]
    fn start_all_accounts_handles_empty() {
        let engine = make_engine();
        // 没有任何账号 — 不应 panic
        // 先清空全局账号（避免被其他测试污染）
        crate::models::store::accounts::set_accounts_data(
            crate::models::store::accounts::AccountsData::default(),
        );
        engine.start_all_accounts();
        assert_eq!(engine.worker_count(), 0);
    }

    #[test]
    fn relogin_reminder_default_present() {
        let engine = make_engine();
        let svc = engine.relogin_reminder();
        // 即便没有显式传 relogin_reminder，assemble 也应保证一个可用实例
        let ms = svc.get_offline_auto_delete_ms("");
        // 默认 offline_delete_sec = 0 → i64::MAX
        assert_eq!(ms, i64::MAX);
    }

    #[test]
    fn subscribe_events_returns_receiver() {
        let engine = make_engine();
        let _rx = engine.subscribe_events();
        let _rx2 = engine.subscribe_runtime_events();
    }

    #[test]
    fn broadcast_config_no_panic_with_no_workers() {
        let engine = make_engine();
        engine.broadcast_config(serde_json::json!({}));
        engine.broadcast_disconnect();
    }

    #[test]
    fn engine_debug_impl_works() {
        let engine = make_engine();
        let s = format!("{engine:?}");
        assert!(s.contains("RuntimeEngine"));
    }

    #[test]
    fn should_attempt_wx_reconnect_skips_user_stop() {
        assert!(should_attempt_wx_reconnect(false, true, "disconnect:kickout"));
        assert!(!should_attempt_wx_reconnect(true, true, "disconnect:kickout"));
        assert!(!should_attempt_wx_reconnect(false, false, "disconnect:kickout"));
        assert!(!should_attempt_wx_reconnect(false, true, "disconnect:wx_auth_failed"));
        assert!(!should_attempt_wx_reconnect(false, true, "disconnect:wx_mint_failed"));
    }

    #[test]
    fn wx_reconnect_waits_match_attempt_and_startup() {
        assert_eq!(crate::constants::wx_reconnect_delay_ms(1), 3 * 60 * 1000);
        assert_eq!(crate::constants::wx_reconnect_delay_zh(1), "3 分钟");
        assert_eq!(crate::constants::wx_reconnect_delay_ms(2), 60 * 1000);
        assert_eq!(crate::constants::wx_reconnect_delay_ms(3), 60 * 1000);
        assert_eq!(crate::constants::wx_reconnect_delay_zh(2), "1 分钟");
        assert_eq!(crate::constants::WX_STARTUP_RECONNECT_DELAY_MS, 60 * 1000);
        assert_eq!(crate::constants::wx_startup_reconnect_delay_zh(), "1 分钟");
    }

    #[test]
    fn plan_wx_reconnect_caps_attempts_and_inflight() {
        let engine = make_engine();
        assert!(matches!(engine.plan_wx_reconnect("a1"), WxReconnectPlan::Spawn { attempt: 1 }));
        assert_eq!(engine.plan_wx_reconnect("a1"), WxReconnectPlan::Skip);
        engine.wx_reconnect.write().inflight.remove("a1");
        assert!(matches!(engine.plan_wx_reconnect("a1"), WxReconnectPlan::Spawn { attempt: 2 }));
        engine.wx_reconnect.write().inflight.remove("a1");
        assert!(matches!(engine.plan_wx_reconnect("a1"), WxReconnectPlan::Spawn { attempt: 3 }));
        engine.wx_reconnect.write().inflight.remove("a1");
        assert_eq!(engine.plan_wx_reconnect("a1"), WxReconnectPlan::GiveUp);
        engine.clear_wx_reconnect("a1");
        assert!(matches!(engine.plan_wx_reconnect("a1"), WxReconnectPlan::Spawn { attempt: 1 }));
    }

    #[test]
    fn store_account_store_like_get_automation() {
        let s = StoreAccountStoreLike;
        let _ = s.get_automation("any");
        let _ = s.get_preferred_seed("any");
    }

    #[test]
    fn state_logger_adapter_forwards_logs() {
        let state = Arc::new(RuntimeState::new(
            Arc::new(StoreAccountStoreLike),
            vec!["harvest".to_string()],
        ));
        let adapter = StateLoggerAdapter::new(state.clone());
        adapter.log("系统", "msg", None);
        adapter.add_account_log("a", "msg", Some("acc"), Some("n"), None);
        let logs = state.global_logs.lock();
        assert_eq!(logs.len(), 1);
        let acc_logs = state.account_logs.lock();
        assert_eq!(acc_logs.len(), 1);
    }
}

fn panel_log_tag<'a>(level: &str, module: &'a str) -> &'a str {
    if level == "error" {
        return "错误";
    }
    match module {
        "farm" => "农场",
        "friend" => "好友",
        "warehouse" => "仓库",
        "task" => "任务",
        "system" => "系统",
        other if !other.is_empty() => other,
        _ => "系统",
    }
}
