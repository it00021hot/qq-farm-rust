//! 离线提醒 + 重登录监听。
//!
//! 1:1 翻译原 `core/src/runtime/relogin-reminder.ts`（268 行）。
//!
//! ## 职责
//!
//! - `getOfflineAutoDeleteMs` — 计算自动删除离线账号的延迟（用户级配置覆盖全局）
//! - `applyReloginCode` — 应用新 code（更新或新增账号 + 重启 worker）
//! - `startReloginWatcher` — 轮询登录码状态（`maxRounds = 120`，1s/轮）
//! - `triggerOfflineReminder` — 触发 QQ Bot 离线提醒与重登录二维码
//!
//! ## 与原 TS 的差异
//!
//! - 用 `tokio::sync::Mutex` 持有 `reloginWatchers`（跨 await）
//! - `WorkerControls` 通过 trait 注入（可 mock）
//! - `miniProgramLoginSession` / `QqBotService` 通过具体 service 引用
//! - 轮询协程用 tokio spawn（不阻塞调用方）
//! - 移除原 `getAccounts()` 直接调用，改为 `accounts::get_accounts()`

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::sleep;

use crate::models::store::accounts::{self, AccountRecord};
use crate::models::store::global_config::{self, NotificationProvider, OfflineReminder};
use crate::services::qq_bot::QqBotService;
use crate::services::qrlogin::{MiniProgramLoginSession, MpStatus, MpStatusResult};

/// watcher 轮询上限（原 TS `maxRounds = 120`）
pub const MAX_WATCHER_ROUNDS: u32 = 120;
/// watcher 单轮间隔
pub const WATCHER_INTERVAL_MS: u64 = 1000;

fn public_qr_image_url(content: &str) -> String {
    let encoded: String = url::form_urlencoded::byte_serialize(content.as_bytes()).collect();
    format!("https://quickchart.io/qr?size=300&margin=1&text={encoded}")
}

/// 重登录码载荷
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReloginCodePayload {
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub account_name: String,
    #[serde(default)]
    pub auth_code: String,
    #[serde(default)]
    pub uin: String,
}

/// 账号通知类型：下线、上线、应用宝授权二维码。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountNoticeKind {
    #[default]
    Offline,
    Online,
    YybQr,
}

/// 离线提醒载荷
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OfflineReminderPayload {
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub account_name: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub offline_ms: i64,
    #[serde(default)]
    pub kind: AccountNoticeKind,
}

/// worker 控制接口（依赖注入）。
///
/// 1:1 对应原 TS `resolveWorkerControls()` 返回的 `{ startWorker, restartWorker }`。
pub trait WorkerControls: Send + Sync {
    /// 启动 worker
    fn start_worker(&self, account: &AccountRecord) -> Option<()>;
    /// 重启 worker
    fn restart_worker(&self, account: &AccountRecord) -> Option<()>;
}

/// 简单 Noop 实现（单元测试 / 离线部署用）
#[derive(Debug, Default, Clone)]
pub struct NoopWorkerControls;

impl WorkerControls for NoopWorkerControls {
    fn start_worker(&self, _account: &AccountRecord) -> Option<()> {
        None
    }
    fn restart_worker(&self, _account: &AccountRecord) -> Option<()> {
        None
    }
}

/// 日志 / 账号日志回调（runtime_state 注入）
pub trait ReminderLogger: Send + Sync {
    fn log(&self, tag: &str, msg: &str, extra: Option<serde_json::Value>);
    fn add_account_log(
        &self,
        action: &str,
        msg: &str,
        account_id: Option<&str>,
        account_name: Option<&str>,
        extra: Option<serde_json::Value>,
    );
}

/// watcher 状态
#[derive(Debug, Clone)]
struct WatcherState {
    started_at: i64,
}

/// 重登录提醒服务
pub struct ReloginReminderService {
    mini_program_login: Arc<MiniProgramLoginSession>,
    qq_bot: Arc<QqBotService>,
    worker_controls: Arc<dyn WorkerControls>,
    logger: Arc<dyn ReminderLogger>,
    /// watcher 集合（key → startedAt）
    relogin_watchers: Arc<AsyncMutex<HashMap<String, WatcherState>>>,
}

impl std::fmt::Debug for ReloginReminderService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReloginReminderService").finish_non_exhaustive()
    }
}

impl ReloginReminderService {
    /// 创建服务
    #[must_use]
    pub fn new(
        mini_program_login: Arc<MiniProgramLoginSession>,
        qq_bot: Arc<QqBotService>,
        worker_controls: Arc<dyn WorkerControls>,
        logger: Arc<dyn ReminderLogger>,
    ) -> Self {
        Self {
            mini_program_login,
            qq_bot,
            worker_controls,
            logger,
            relogin_watchers: Arc::new(AsyncMutex::new(HashMap::new())),
        }
    }

    /// 计算自动删除离线账号的延迟（ms）。
    ///
    /// 0 表示不自动删除（用 `i64::MAX` 表示原 TS `Infinity`）。
    /// `username` 为空时取全局配置；否则先查 user 级，再回退到全局。
    #[must_use]
    pub fn get_offline_auto_delete_ms(&self, username: &str) -> i64 {
        let cfg = self.get_offline_reminder_config(username);
        let sec = cfg.offline_delete_sec.max(0);
        if sec == 0 {
            i64::MAX
        } else {
            sec.saturating_mul(1000)
        }
    }

    /// 获取生效的 OfflineReminder（user 级优先，回退全局）
    #[must_use]
    pub fn get_offline_reminder_config(&self, username: &str) -> OfflineReminder {
        if !username.is_empty() {
            if let Some(cfg) = global_config::get_user_offline_reminder(username) {
                return cfg;
            }
        }
        global_config::get_offline_reminder()
    }

    #[must_use]
    pub fn qq_bot(&self) -> Arc<QqBotService> {
        self.qq_bot.clone()
    }

    /// 应用新 code（更新或新增账号 + 重启/启动 worker）
    pub fn apply_relogin_code(&self, payload: ReloginCodePayload) {
        let code = payload.auth_code.trim();
        if code.is_empty() {
            return;
        }
        let account_id = payload.account_id.trim();
        let account_name = payload.account_name.trim();
        let uin = payload.uin.trim();

        let avatar = if !uin.is_empty() {
            format!("https://q1.qlogo.cn/g?b=qq&nk={uin}&s=640")
        } else {
            String::new()
        };

        let list = accounts::get_accounts();
        let found = if account_id.is_empty() {
            None
        } else {
            list.iter().find(|a| a.id == account_id).cloned()
        };

        if let Some(found_acc) = found {
            // 更新已有账号
            let new_acc = AccountRecord {
                code: code.to_string(),
                qq: if !uin.is_empty() { uin.to_string() } else { found_acc.qq.clone() },
                uin: if !uin.is_empty() { uin.to_string() } else { found_acc.uin.clone() },
                avatar: if !avatar.is_empty() { avatar } else { found_acc.avatar.clone() },
                ..found_acc.clone()
            };
            accounts::add_or_update_account(new_acc.clone());
            self.worker_controls.restart_worker(&new_acc);
            self.logger.add_account_log(
                "update",
                &format!("重登录成功，已更新账号: {}", found_acc.name),
                Some(&found_acc.id),
                Some(&found_acc.name),
                Some(serde_json::json!({ "reason": "relogin" })),
            );
            self.logger.log(
                "系统",
                &format!("重登录成功，账号已更新并重启: {}", found_acc.name),
                None,
            );
            return;
        }

        // 新增账号
        let name = if !account_name.is_empty() {
            account_name.to_string()
        } else if !uin.is_empty() {
            uin.to_string()
        } else {
            "重登录账号".to_string()
        };
        let new_acc = AccountRecord {
            id: String::new(), // 会被 add_or_update_account 分配 next_id
            name,
            code: code.to_string(),
            platform: "qq".to_string(),
            qq: uin.to_string(),
            uin: uin.to_string(),
            avatar,
            ..Default::default()
        };
        let new_acc = accounts::add_or_update_account(new_acc);
        self.worker_controls.start_worker(&new_acc);
        self.logger.add_account_log(
            "add",
            &format!("重登录成功，已新增账号: {}", new_acc.name),
            Some(&new_acc.id),
            Some(&new_acc.name),
            Some(serde_json::json!({ "reason": "relogin" })),
        );
        self.logger.log(
            "系统",
            &format!("重登录成功，已新增账号并启动: {}", new_acc.name),
            Some(serde_json::json!({
                "accountId": new_acc.id,
                "accountName": new_acc.name,
            })),
        );
    }

    /// 启动重登录监听（轮询 `queryStatus`，拿到 OK 后 `getAuthCode` + apply）
    ///
    /// 调用方需持有 `Arc<Self>`，因为轮询协程需要调用 `apply_relogin_code`。
    pub fn start_relogin_watcher(
        self: Arc<Self>,
        login_code: &str,
        account_id: &str,
        account_name: &str,
    ) {
        let code = login_code.trim();
        if code.is_empty() {
            return;
        }
        let account_id = account_id.to_string();
        let account_name = account_name.to_string();
        let key =
            format!("{}:{}", if account_id.is_empty() { "unknown" } else { &account_id }, code);

        // 同步抢占：避免重复启动
        let should_spawn = match self.relogin_watchers.try_lock() {
            Ok(mut guard) => {
                if guard.contains_key(&key) {
                    false
                } else {
                    guard.insert(key.clone(), WatcherState { started_at: now_ms() });
                    true
                }
            }
            Err(_) => true, // 锁竞争时不阻塞（与原 TS 异步一致）
        };
        if !should_spawn {
            return;
        }
        self.logger.log(
            "系统",
            &format!(
                "已启动重登录监听: {}",
                if !account_name.is_empty() {
                    &account_name
                } else if !account_id.is_empty() {
                    &account_id
                } else {
                    "未知账号"
                }
            ),
            Some(serde_json::json!({
                "accountId": account_id,
                "accountName": account_name,
            })),
        );

        // 异步轮询
        let account_id_bg = account_id.clone();
        let account_name_bg = account_name.clone();
        let acc_label = if !account_name_bg.is_empty() {
            account_name_bg.clone()
        } else if !account_id_bg.is_empty() {
            account_id_bg.clone()
        } else {
            "未知账号".to_string()
        };
        let mp = self.mini_program_login.clone();
        let watchers = self.relogin_watchers.clone();
        let logger = self.logger.clone();
        let code_owned = code.to_string();
        let svc = self.clone();
        tokio::spawn(async move {
            let payload = poll_relogin_status(mp, logger.clone(), &acc_label, &code_owned).await;
            // 不论结果如何都清理 watcher
            {
                let mut guard = watchers.lock().await;
                if let Some(state) = guard.remove(&key) {
                    let elapsed_ms = now_ms() - state.started_at;
                    logger.log(
                        "系统",
                        &format!("重登录监听结束: {acc_label} ({elapsed_ms}ms)"),
                        Some(serde_json::json!({
                            "accountId": account_id_bg,
                            "elapsedMs": elapsed_ms,
                        })),
                    );
                }
            }
            if let Some(mut payload) = payload {
                payload.account_id = account_id_bg;
                payload.account_name = account_name_bg;
                svc.apply_relogin_code(payload);
            }
        });
    }

    /// 触发账号通知（下线 / 上线 / 应用宝二维码）
    pub async fn trigger_offline_reminder(self: Arc<Self>, payload: OfflineReminderPayload) {
        let account_id = payload.account_id.trim().to_string();
        let account_name = payload.account_name.trim().to_string();
        let reason =
            if payload.reason.is_empty() { "unknown".to_string() } else { payload.reason.clone() };
        let kind_label = match payload.kind {
            AccountNoticeKind::Offline => "下线",
            AccountNoticeKind::Online => "上线",
            AccountNoticeKind::YybQr => "应用宝授权二维码",
        };

        self.logger.log(
            "系统",
            &format!(
                "触发{kind_label}通知: 账号={}, 原因={}",
                if !account_name.is_empty() { &account_name } else { &account_id },
                reason
            ),
            Some(serde_json::json!({
                "accountId": account_id,
                "accountName": account_name,
                "reason": reason,
                "kind": kind_label,
            })),
        );

        // 找用户名
        let mut username = payload.username.trim().to_string();
        if username.is_empty() && !account_id.is_empty() {
            let lookup = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                accounts::get_accounts()
                    .into_iter()
                    .find(|a| a.id == account_id)
                    .map(|a| a.username.clone())
            }));
            match lookup {
                Ok(Some(u)) => username = u.trim().to_string(),
                Ok(None) => {}
                Err(_) => {
                    self.logger.log("错误", "查找账号用户名失败: panic", None);
                }
            }
        }

        let cfg = self.get_offline_reminder_config(&username);
        if !cfg.is_configured() {
            tracing::debug!(account_id = %account_id, "未配置账号通知，跳过");
            return;
        }
        if cfg.provider == NotificationProvider::WechatBot {
            self.logger.log("错误", "微信机器人暂未实现", None);
            return;
        }
        let Some(send_config) = cfg.send_config() else {
            self.logger.log("错误", "QQ 通知未绑定", None);
            return;
        };

        let acc_label = if !account_name.is_empty() {
            account_name.clone()
        } else if !account_id.is_empty() {
            account_id.clone()
        } else {
            "未知账号".to_string()
        };
        let content = match payload.kind {
            AccountNoticeKind::Offline => format!("账号 {acc_label} 已下线"),
            AccountNoticeKind::Online => format!("账号 {acc_label} 已上线"),
            AccountNoticeKind::YybQr => {
                format!("账号 {acc_label} 应用宝授权失效，请扫描二维码重新登录")
            }
        };

        let mut qr_image_url = None;
        if payload.kind == AccountNoticeKind::YybQr {
            match self.mini_program_login.request_login_code().await {
                Ok(qr) => {
                    let login_code = qr.code.trim().to_string();
                    let qq_url = qr.url.trim().to_string();
                    if !qq_url.is_empty() {
                        qr_image_url = Some(public_qr_image_url(&qq_url));
                    }
                    if !login_code.is_empty() {
                        let svc = self.clone();
                        svc.start_relogin_watcher(&login_code, &account_id, &account_name);
                    }
                }
                Err(e) => {
                    self.logger.log("错误", &format!("获取重登录链接失败: {e}"), None);
                }
            }
        }

        let ret = self.qq_bot.send_text(&send_config, "", &content).await;
        if ret.ok {
            self.logger.log("系统", &format!("账号通知发送成功: {content}"), None);
            if let Some(image_url) = qr_image_url {
                let qr_result = self.qq_bot.send_qr_image(&send_config, &image_url).await;
                if qr_result.ok {
                    self.logger.log("系统", &format!("应用宝授权二维码发送成功: {acc_label}"), None);
                } else {
                    self.logger.log(
                        "错误",
                        &format!("应用宝授权二维码发送失败: {}", qr_result.msg),
                        None,
                    );
                }
            }
        } else {
            let msg = if ret.msg.is_empty() { "unknown" } else { ret.msg.as_str() };
            self.logger.log("错误", &format!("账号通知发送失败: {msg}"), None);
        }
    }

    /// 取出 watcher 数量（测试用）
    pub async fn watcher_count(&self) -> usize {
        self.relogin_watchers.lock().await.len()
    }
}

// =====================================================================
// 内部：polling 协程
// =====================================================================

/// 轮询登录码状态直到 OK / Used / 超时。
///
/// 成功时返回构造好的 `ReloginCodePayload`（由 caller 调 `apply_relogin_code`）。
async fn poll_relogin_status(
    mp: Arc<MiniProgramLoginSession>,
    logger: Arc<dyn ReminderLogger>,
    acc_label: &str,
    code: &str,
) -> Option<ReloginCodePayload> {
    for _ in 0..MAX_WATCHER_ROUNDS {
        let status: MpStatusResult = match mp.query_status(code).await {
            Ok(s) => s,
            Err(_) => {
                sleep(Duration::from_millis(WATCHER_INTERVAL_MS)).await;
                continue;
            }
        };
        if status.status == MpStatus::Wait {
            sleep(Duration::from_millis(WATCHER_INTERVAL_MS)).await;
            continue;
        }
        if status.status == MpStatus::Used {
            logger.log("系统", &format!("重登录二维码已失效: {acc_label}"), None);
            return None;
        }
        if status.status == MpStatus::OK {
            let ticket = status.ticket.as_deref().unwrap_or("").trim();
            if ticket.is_empty() {
                logger.log("错误", "重登录监听失败: ticket 为空", None);
                return None;
            }
            let auth_code = match mp.get_auth_code(ticket, "1112386029").await {
                Ok(c) => c,
                Err(_) => {
                    logger.log("错误", "重登录监听失败: 未获取到新 code", None);
                    return None;
                }
            };
            if auth_code.is_empty() {
                logger.log("错误", "重登录监听失败: 未获取到新 code", None);
                return None;
            }
            return Some(ReloginCodePayload {
                account_id: String::new(), // 由 caller 注入
                account_name: String::new(),
                auth_code,
                uin: status.uin.unwrap_or_default(),
            });
        }
        // 其它状态（Error 等）继续轮询
        sleep(Duration::from_millis(WATCHER_INTERVAL_MS)).await;
    }
    logger.log("系统", &format!("重登录监听超时: {acc_label}"), None);
    None
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 隔离会落盘的 global_config 写操作，避免覆盖真实 `store.json`。
    struct TempFarmData {
        prev: Option<String>,
        dir: PathBuf,
    }

    impl TempFarmData {
        fn enter() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "qq-farm-relogin-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ));
            let _ = std::fs::create_dir_all(&dir);
            let prev = std::env::var("FARM_DATA_DIR").ok();
            std::env::set_var("FARM_DATA_DIR", &dir);
            Self { prev, dir }
        }
    }

    impl Drop for TempFarmData {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
            match &self.prev {
                Some(v) => std::env::set_var("FARM_DATA_DIR", v),
                None => std::env::remove_var("FARM_DATA_DIR"),
            }
        }
    }

    /// 测试用 logger
    #[derive(Default)]
    struct TestLogger {
        logs: Mutex<Vec<(String, String)>>,
        account_logs: Mutex<Vec<(String, String)>>,
    }
    impl ReminderLogger for TestLogger {
        fn log(&self, tag: &str, msg: &str, _extra: Option<serde_json::Value>) {
            self.logs.lock().push((tag.to_string(), msg.to_string()));
        }
        fn add_account_log(
            &self,
            action: &str,
            msg: &str,
            _account_id: Option<&str>,
            _account_name: Option<&str>,
            _extra: Option<serde_json::Value>,
        ) {
            self.account_logs.lock().push((action.to_string(), msg.to_string()));
        }
    }

    /// 计数 worker controls
    #[derive(Default)]
    struct CountingControls {
        starts: AtomicUsize,
        restarts: AtomicUsize,
    }
    impl WorkerControls for CountingControls {
        fn start_worker(&self, _account: &AccountRecord) -> Option<()> {
            self.starts.fetch_add(1, Ordering::SeqCst);
            Some(())
        }
        fn restart_worker(&self, _account: &AccountRecord) -> Option<()> {
            self.restarts.fetch_add(1, Ordering::SeqCst);
            Some(())
        }
    }

    /// 构造一个 service（测试辅助）
    fn make_service() -> (Arc<ReloginReminderService>, Arc<TestLogger>, Arc<CountingControls>) {
        let mp = Arc::new(MiniProgramLoginSession::new());
        let qq_bot = Arc::new(QqBotService::new());
        let controls = Arc::new(CountingControls::default());
        let logger = Arc::new(TestLogger::default());
        let svc = ReloginReminderService::new(
            mp,
            qq_bot,
            controls.clone() as Arc<dyn WorkerControls>,
            logger.clone() as Arc<dyn ReminderLogger>,
        );
        (Arc::new(svc), logger, controls)
    }

    #[test]
    fn get_offline_auto_delete_ms_default_infinity() {
        let (svc, _, _) = make_service();
        // 全局默认 offline_delete_sec = 0 → i64::MAX
        let ms = svc.get_offline_auto_delete_ms("");
        assert_eq!(ms, i64::MAX);
    }

    #[test]
    #[serial_test::serial(relogin)]
    #[serial_test::serial(farm_data_dir)]
    fn get_offline_auto_delete_ms_user_override() {
        let _dir = TempFarmData::enter();
        // 先重置全局（避免被其他测试污染）
        global_config::set_offline_reminder(OfflineReminder::default());
        let (svc, _, _) = make_service();
        global_config::set_offline_reminder(OfflineReminder {
            offline_delete_sec: 60,
            ..Default::default()
        });
        let ms = svc.get_offline_auto_delete_ms("any");
        assert_eq!(ms, 60_000);
    }

    #[test]
    #[serial_test::serial(relogin)]
    #[serial_test::serial(farm_data_dir)]
    fn get_offline_reminder_config_falls_back_to_global() {
        let _dir = TempFarmData::enter();
        global_config::set_offline_reminder(OfflineReminder::default());
        let (svc, _, _) = make_service();
        global_config::set_offline_reminder(OfflineReminder {
            title: "T".to_string(),
            msg: "M".to_string(),
            ..Default::default()
        });
        let cfg = svc.get_offline_reminder_config("nobody");
        assert_eq!(cfg.title, "T");
    }

    #[test]
    #[serial_test::serial(relogin)]
    #[serial_test::serial(farm_data_dir)]
    fn get_offline_reminder_config_user_priority() {
        let _dir = TempFarmData::enter();
        global_config::set_offline_reminder(OfflineReminder::default());
        global_config::delete_user_offline_reminder("alice");
        let (svc, _, _) = make_service();
        global_config::set_offline_reminder(OfflineReminder {
            title: "global".to_string(),
            ..Default::default()
        });
        global_config::set_user_offline_reminder(
            "alice",
            OfflineReminder { title: "user".to_string(), ..Default::default() },
        );
        let cfg = svc.get_offline_reminder_config("alice");
        assert_eq!(cfg.title, "user");
    }

    #[test]
    fn apply_relogin_code_empty_code_noop() {
        let (svc, logger, controls) = make_service();
        svc.apply_relogin_code(ReloginCodePayload {
            auth_code: "".to_string(),
            ..Default::default()
        });
        assert_eq!(logger.logs.lock().len(), 0);
        assert_eq!(controls.starts.load(Ordering::SeqCst), 0);
        assert_eq!(controls.restarts.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn apply_relogin_code_whitespace_code_noop() {
        let (svc, logger, controls) = make_service();
        svc.apply_relogin_code(ReloginCodePayload {
            auth_code: "   ".to_string(),
            ..Default::default()
        });
        assert_eq!(logger.logs.lock().len(), 0);
        assert_eq!(controls.starts.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn start_relogin_watcher_empty_code_noop() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let (svc, logger, _) = make_service();
        rt.block_on(async {
            let svc2 = svc.clone();
            svc2.start_relogin_watcher("", "acc1", "测试");
            tokio::time::sleep(Duration::from_millis(50)).await;
            assert_eq!(logger.logs.lock().len(), 0);
            assert_eq!(svc.watcher_count().await, 0);
        });
    }

    #[test]
    fn start_relogin_watcher_logs_started() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let (svc, logger, _) = make_service();
        rt.block_on(async {
            svc.start_relogin_watcher("code-xyz", "acc1", "测试账号");
            // 让 spawn 起飞
            tokio::time::sleep(Duration::from_millis(50)).await;
            // 至少有 "已启动重登录监听" 一条
            let logs = logger.logs.lock().clone();
            assert!(logs.iter().any(|(_, m)| m.contains("已启动重登录监听")));
        });
    }

    #[test]
    #[serial_test::serial(relogin)]
    #[serial_test::serial(farm_data_dir)]
    fn offline_reminder_incomplete_config_no_push() {
        let _dir = TempFarmData::enter();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        global_config::set_offline_reminder(OfflineReminder::default());
        let (svc, logger, _) = make_service();
        // 留默认空配置
        rt.block_on(async {
            svc.trigger_offline_reminder(OfflineReminderPayload {
                account_id: "a1".to_string(),
                account_name: "n".to_string(),
                reason: "ws_error".to_string(),
                ..Default::default()
            })
            .await;
            // 不完整，应该不发
            let logs = logger.logs.lock().clone();
            assert!(logs.iter().any(|(_, m)| m.contains("触发下线通知") || m.contains("触发下线提醒")));
            assert!(!logs.iter().any(|(_, m)| m.contains("下线提醒配置: provider=")));
        });
    }

    #[test]
    #[serial_test::serial(relogin)]
    #[serial_test::serial(farm_data_dir)]
    fn offline_reminder_factory_default_skips_without_error() {
        let _dir = TempFarmData::enter();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        global_config::set_offline_reminder(global_config::default_offline_reminder());
        let (svc, logger, _) = make_service();
        rt.block_on(async {
            svc.trigger_offline_reminder(OfflineReminderPayload {
                account_id: "a1".to_string(),
                account_name: "大号".to_string(),
                reason: "kickout".to_string(),
                ..Default::default()
            })
            .await;
            let logs = logger.logs.lock().clone();
            assert!(logs.iter().any(|(_, m)| m.contains("触发下线通知") || m.contains("触发下线提醒")));
            assert!(!logs.iter().any(|(_, m)| m.contains("下线提醒配置不完整")));
            assert!(!logs.iter().any(|(_, m)| m.contains("下线提醒配置: provider=")));
        });
    }

    #[test]
    fn offline_reminder_payload_default_reason_is_unknown() {
        let payload = OfflineReminderPayload::default();
        assert_eq!(payload.reason, "");
        assert_eq!(payload.kind, AccountNoticeKind::Offline);
        let qr = public_qr_image_url("https://example.com/login?a=1");
        assert!(qr.starts_with("https://quickchart.io/qr?"));
        assert!(qr.contains("https%3A%2F%2Fexample.com%2Flogin%3Fa%3D1"));
    }

    #[test]
    fn relogin_code_payload_default() {
        let p = ReloginCodePayload::default();
        assert_eq!(p.account_id, "");
        assert_eq!(p.auth_code, "");
        assert_eq!(p.uin, "");
    }

    #[test]
    fn noop_worker_controls_returns_none() {
        let c = NoopWorkerControls;
        let acc = AccountRecord {
            id: "a".to_string(),
            name: "n".to_string(),
            code: "c".to_string(),
            platform: "qq".to_string(),
            uin: "u".to_string(),
            qq: "q".to_string(),
            ..Default::default()
        };
        assert!(c.start_worker(&acc).is_none());
        assert!(c.restart_worker(&acc).is_none());
    }

    #[test]
    fn watcher_cap_constant() {
        assert_eq!(MAX_WATCHER_ROUNDS, 120);
        assert_eq!(WATCHER_INTERVAL_MS, 1000);
    }
}
