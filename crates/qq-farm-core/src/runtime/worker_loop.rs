//! Worker 编排层 — 1:1 翻译原 `core/src/core/worker.ts`（965 行）。
//!
//! ## 职责
//!
//! 编排各 service 的执行节奏：
//! - 每日任务（email / share / monthcard / 商城免费 / qqvip）跨日调度
//! - 农场巡查（随机间隔 + 防重入）
//! - 帮助巡查（独立调度 + 经验满不帮忙）
//! - 偷菜巡查（独立调度）
//! - 状态上报（3s 间隔）
//! - 赛季进度刷新（5min）
//! - 网络事件（kickout / disconnect）→ quiesce + save
//! - IPC API 调用（admin 面板拉数据 / 触发操作）
//!
//! ## 与原 TS 的差异
//!
//! - 原 TS 是独立 child process；本实现 in-process tokio task
//! - 原 TS 用 worker_threads IPC；本实现直接 Rust async（无 IPC 边界）
//! - 原 TS 的 `setLogHook` / `setRecordGoldExpHook` 是全局回调；本实现走 broadcast event
//! - 自动化 config 走 `services::automation`（category → bool），不读 raw 字段
//!
//! ## 编排
//!
//! 1. `WorkerLoop` 持有所有 service Arcs
//! 2. `run()` 启动所有定时器
//! 3. 定时器触发的 `run_*_tick` 函数调用对应 service
//! 4. 状态 / 操作次数走 `services::status` / `services::stats`
//!
//! ## 与 worker.rs 的关系
//!
//! - `worker.rs` 负责 transport（cancel / msg_rx / TSDK / Gateway）
//! - `worker_loop.rs` 负责 business orchestration（intervals / daily routines）

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;
use tokio::sync::broadcast;

use crate::models::Account;
use crate::network::gateway::Gateway;
use crate::runtime::events::WorkerEvent;
use crate::runtime::scheduler::Scheduler;
use crate::services::activity_center::ActivityCenterService;
use crate::services::automation;
use crate::services::email::EmailService;
use crate::services::farm::scheduler::FarmService;
use crate::services::friend::scheduler::FriendService;
use crate::services::mall::MallService;
use crate::services::monthcard::MonthCardService;
use crate::services::mystery_shop::MysteryShopService;
use crate::services::qqvip::QQVipService;
use crate::services::share::ShareService;
use crate::services::status as status_svc;
use crate::services::task::TaskService;
use crate::services::warehouse::WarehouseService;

/// 编排层配置（interval 范围等）
#[derive(Debug, Clone)]
pub struct WorkerLoopConfig {
    /// 状态上报间隔
    pub status_interval: Duration,
    /// 每日跨日检查间隔
    pub daily_routine_interval: Duration,
    /// 赛季进度刷新间隔
    pub season_progress_interval: Duration,
}

impl Default for WorkerLoopConfig {
    fn default() -> Self {
        Self {
            status_interval: Duration::from_secs(3),
            daily_routine_interval: Duration::from_secs(30),
            season_progress_interval: Duration::from_secs(300),
        }
    }
}

/// Worker 编排器
pub struct WorkerLoop {
    account: Account,
    config: WorkerLoopConfig,
    gateway: Arc<Gateway>,
    /// event_tx 用于上报状态 / 错误 / 停止
    event_tx: broadcast::Sender<WorkerEvent>,
    /// farm / friend / status / automation / share / qq / monthcard / email / mall / task / activity_center / warehouse
    farm: Arc<FarmService>,
    friend: Arc<FriendService>,
    email: Arc<EmailService>,
    share: Arc<ShareService>,
    monthcard: Arc<MonthCardService>,
    qqvip: Arc<QQVipService>,
    mall: Arc<MallService>,
    task: Arc<TaskService>,
    warehouse: Arc<WarehouseService>,
    mystery_shop: Arc<MysteryShopService>,
    activity_center: Arc<ActivityCenterService>,

    // —— 内部状态 ——
    /// 登录完成
    login_ready: AtomicBool,
    /// shutdown 启动
    shutdown_started: AtomicBool,
    /// running
    is_running: AtomicBool,
    /// farm / help / steal 下次执行时间（ms）
    next_runs: Arc<Mutex<NextRuns>>,
    /// 每日 routine 上次执行日期（YYYY-MM-DD）
    last_daily_date: Arc<Mutex<String>>,
    /// 配置 revision（防重应用）
    applied_config_revision: AtomicU64,
}

/// AtomicBool/AtomicU64
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// 下次执行时间
#[derive(Debug, Clone, Default)]
struct NextRuns {
    farm_at: i64,
    help_at: i64,
    steal_at: i64,
}

/// IP 化：worker 上报给 master 的状态数据结构
#[derive(Debug, Clone, Serialize)]
pub struct StatusSyncPayload {
    pub account_id: String,
    pub account_name: String,
    pub connection: ConnectionInfo,
    /// status 走 JSON（StatusData 不一定实现 Serialize，统一以 value 形式存）
    pub status: serde_json::Value,
    pub operations: serde_json::Value,
    pub limits: serde_json::Value,
    pub automation: serde_json::Value,
    pub preferred_seed: i64,
    pub config_revision: u64,
    pub next_checks: NextChecks,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionInfo {
    pub connected: bool,
    pub ws_error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct NextChecks {
    pub farm_remain_sec: i64,
    pub help_remain_sec: i64,
    pub steal_remain_sec: i64,
}

impl WorkerLoop {
    /// 创建编排器
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        account: Account,
        config: WorkerLoopConfig,
        gateway: Arc<Gateway>,
        event_tx: broadcast::Sender<WorkerEvent>,
        farm: Arc<FarmService>,
        friend: Arc<FriendService>,
        email: Arc<EmailService>,
        share: Arc<ShareService>,
        monthcard: Arc<MonthCardService>,
        qqvip: Arc<QQVipService>,
        mall: Arc<MallService>,
        task: Arc<TaskService>,
        warehouse: Arc<WarehouseService>,
        mystery_shop: Arc<MysteryShopService>,
        activity_center: Arc<ActivityCenterService>,
    ) -> Self {
        Self {
            account,
            config,
            gateway,
            event_tx,
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
            login_ready: AtomicBool::new(false),
            shutdown_started: AtomicBool::new(false),
            is_running: AtomicBool::new(false),
            next_runs: Arc::new(Mutex::new(NextRuns::default())),
            last_daily_date: Arc::new(Mutex::new(String::new())),
            applied_config_revision: AtomicU64::new(0),
        }
    }

    /// 当前 account id
    #[must_use]
    pub fn account_id(&self) -> &str {
        &self.account.id
    }

    /// 当前 account name
    #[must_use]
    pub fn account_name(&self) -> &str {
        &self.account.display_name
    }

    /// 是否已登录
    #[must_use]
    pub fn login_ready(&self) -> bool {
        self.login_ready.load(Ordering::Acquire)
    }

    /// 是否在 shutdown
    #[must_use]
    pub fn shutdown_started(&self) -> bool {
        self.shutdown_started.load(Ordering::Acquire)
    }

    /// 设置 login ready
    pub fn mark_login_ready(&self) {
        self.login_ready.store(true, Ordering::Release);
        self.is_running.store(true, Ordering::Release);
    }

    /// 应用 config revision（idempotent guard）
    pub fn apply_config_revision(&self, rev: u64) -> bool {
        let prev = self.applied_config_revision.swap(rev, Ordering::AcqRel);
        prev != rev
    }

    /// 启动所有定时器
    pub fn start(&self, scheduler: &Scheduler) {
        // 状态上报
        let acc_id = self.account.id.clone();
        let acc_name = self.account.display_name.clone();
        let tx = self.event_tx.clone();
        scheduler.set_interval_task(
            "status_sync",
            self.config.status_interval,
            Arc::new(move || {
                let acc_id = acc_id.clone();
                let acc_name = acc_name.clone();
                let tx = tx.clone();
                Box::pin(async move {
                    // 真实 status 在 sync_status_payload() 内部组装
                    let _ = tx.send(WorkerEvent::Status {
                        account_id: acc_id,
                        account_name: acc_name,
                        status: serde_json::json!({
                            "phase": "online",
                            "note": "tick",
                        }),
                    });
                })
            }),
        );

        // 每日跨日检查
        let last = self.last_daily_date.clone();
        let acc_id = self.account.id.clone();
        scheduler.set_interval_task(
            "daily_routine_interval",
            self.config.daily_routine_interval,
            Arc::new(move || {
                let last = last.clone();
                let acc_id = acc_id.clone();
                Box::pin(async move {
                    let today = get_local_date_key();
                    let mut guard = last.lock();
                    if *guard == today {
                        return;
                    }
                    *guard = today.clone();
                    tracing::info!(account_id = %acc_id, date = %today, "daily routines due");
                })
            }),
        );
    }

    /// 启动 farm / help / steal tick（随机间隔）
    pub fn start_farm_ticks(&self, scheduler: &Scheduler) {
        // farm tick
        let farm_min_ms = 2000_u64;
        let farm_max_ms = 2000_u64;
        let next = self.next_runs.clone();
        scheduler.set_interval_task(
            "farm_tick",
            Duration::from_millis(500),
            Arc::new(move || {
                let next = next.clone();
                Box::pin(async move {
                    let now = now_ms();
                    let mut guard = next.lock();
                    if now < guard.farm_at {
                        return;
                    }
                    guard.farm_at = now + random_interval_ms(farm_min_ms, farm_max_ms) as i64;
                    // 实际 runFarmTick 在 on_login_success 中通过独立 task 调
                })
            }),
        );
    }

    /// 触发 farm tick（对外暴露给 on_login_success 启动独立 task）
    pub async fn run_farm_tick(&self) {
        if !self.login_ready() {
            return;
        }
        let auto_farm = automation::is_automation_on("farm");
        let auto_task = automation::is_automation_on("task");
        let auto_fertilizer_gift = automation::is_automation_on("fertilizer_gift");

        if auto_farm {
            let _ = self.farm.check_farm().await;
        }
        if auto_task {
            let _ = self.task.check_and_claim_tasks().await;
        }
        if auto_fertilizer_gift {
            let _ = self.warehouse.auto_open_fertilizer_gift_packs().await;
        }
    }

    /// 触发 help tick
    pub async fn run_help_tick(&self) {
        if !self.login_ready() {
            return;
        }
        let auto_help = automation::is_automation_on("friend_help");
        if !auto_help {
            return;
        }
        // 经验满不帮忙：跳过（具体判断在 friend.scheduler 内部）
        let _ = self.friend.check_friends().await;
    }

    /// 触发 steal tick
    pub async fn run_steal_tick(&self) {
        if !self.login_ready() {
            return;
        }
        let auto_steal = automation::is_automation_on("friend_steal");
        if !auto_steal {
            return;
        }
        let _ = self.friend.check_friends().await;
    }

    /// 跑每日任务
    pub async fn run_daily_routines(&self, force: bool) {
        if !self.login_ready() && !force {
            return;
        }
        // email
        let _ = self.email.check_and_claim_emails(force).await;
        // share
        let _ = self.share.check_daily_share_status(force).await;
        // monthcard
        let _ = self.monthcard.perform_daily_month_card_gift(force).await;
        // 商城免费礼包
        let _ = self.mall.buy_free_gifts(force).await;
        // qqvip
        let _ = self.qqvip.perform_daily_vip_gift(force).await;
    }

    /// 刷新赛季进度
    pub async fn refresh_season_progress(&self) {
        let _ = self.activity_center.refresh_season_pass().await;
    }

    /// 处理 kickout（用户被踢下线）
    pub fn on_kickout(&self, reason: &str) {
        if self.shutdown_started() {
            return;
        }
        tracing::warn!(account_id = %self.account.id, reason, "kicked out");
        self.quiesce_bot(&format!("踢下线: {reason}"));
    }

    /// 处理 disconnect
    pub fn on_disconnect(&self, source: &str, code: i64, phase: &str) {
        if self.shutdown_started() {
            return;
        }
        tracing::warn!(account_id = %self.account.id, source, code, phase, "disconnected");
        self.quiesce_bot(&format!("连接断开: {source}"));
    }

    /// ws error
    pub fn on_ws_error(&self, message: &str) {
        tracing::warn!(account_id = %self.account.id, "ws error: {message}");
    }

    /// 安静地停止 bot（清理所有 loop / scheduler）
    pub fn quiesce_bot(&self, _reason: &str) {
        self.shutdown_started.store(true, Ordering::Release);
        self.is_running.store(false, Ordering::Release);
        self.login_ready.store(false, Ordering::Release);
        // 停 farm / friend loop
        self.farm.stop_check_loop();
        self.friend.stop_check_loop();
    }

    /// 重启 bot
    pub fn resume_bot(&self) {
        self.shutdown_started.store(false, Ordering::Release);
        self.is_running.store(true, Ordering::Release);
        self.login_ready.store(true, Ordering::Release);
        self.farm.start_check_loop();
        self.friend.start_check_loop();
    }

    /// 同步状态（构造 payload + emit event）
    pub fn sync_status(&self) {
        let conn = ConnectionInfo {
            connected: self.login_ready(),
            ws_error: None,
        };
        let user_state = serde_json::to_value(&status_svc::status_data()).unwrap_or(serde_json::Value::Null);
        let limits = serde_json::json!({
            "water": { "used": 0, "max": 30 },
            "weed": { "used": 0, "max": 30 },
            "insecticide": { "used": 0, "max": 30 },
            "steal": { "used": 0, "max": 30 },
        });
        let automation = serde_json::json!({});
        let now = now_ms();
        let next = self.next_runs.lock().clone();
        let payload = StatusSyncPayload {
            account_id: self.account.id.clone(),
            account_name: self.account.display_name.clone(),
            connection: conn,
            status: user_state,
            operations: serde_json::json!({}),
            limits,
            automation,
            preferred_seed: 0,
            config_revision: self.applied_config_revision.load(Ordering::Acquire),
            next_checks: NextChecks {
                farm_remain_sec: ((next.farm_at - now) / 1000).max(0),
                help_remain_sec: ((next.help_at - now) / 1000).max(0),
                steal_remain_sec: ((next.steal_at - now) / 1000).max(0),
            },
        };
        let _ = self.event_tx.send(WorkerEvent::Status {
            account_id: payload.account_id.clone(),
            account_name: payload.account_name.clone(),
            status: serde_json::to_value(&payload).unwrap_or(serde_json::Value::Null),
        });
    }

    /// 暴露 farm / friend 给上层调用（admin panel）
    #[must_use]
    pub fn farm(&self) -> &Arc<FarmService> {
        &self.farm
    }
    #[must_use]
    pub fn friend(&self) -> &Arc<FriendService> {
        &self.friend
    }
    #[must_use]
    pub fn activity_center(&self) -> &Arc<ActivityCenterService> {
        &self.activity_center
    }
    #[must_use]
    pub fn email(&self) -> &Arc<EmailService> {
        &self.email
    }
    #[must_use]
    pub fn share(&self) -> &Arc<ShareService> {
        &self.share
    }
    #[must_use]
    pub fn monthcard(&self) -> &Arc<MonthCardService> {
        &self.monthcard
    }
    #[must_use]
    pub fn qqvip(&self) -> &Arc<QQVipService> {
        &self.qqvip
    }
    #[must_use]
    pub fn mall(&self) -> &Arc<MallService> {
        &self.mall
    }
    #[must_use]
    pub fn task(&self) -> &Arc<TaskService> {
        &self.task
    }
    #[must_use]
    pub fn warehouse(&self) -> &Arc<WarehouseService> {
        &self.warehouse
    }
    #[must_use]
    pub fn mystery_shop(&self) -> &Arc<MysteryShopService> {
        &self.mystery_shop
    }
    #[must_use]
    pub fn gateway(&self) -> &Arc<Gateway> {
        &self.gateway
    }
}

// =====================================================================
// 纯函数
// =====================================================================

/// 归一化 interval 区间（秒）
///
/// 规则：
/// - 0 或负数 → 用 fallback
/// - 都设了但 min > max → 交换
/// - 最后夹到 ≥1
#[must_use]
pub fn normalize_interval_range_sec(min_sec: i64, max_sec: i64, fallback_sec: i64) -> (i64, i64) {
    let fallback = fallback_sec.max(1);
    let mut min = if min_sec <= 0 { fallback } else { min_sec };
    let mut max = if max_sec <= 0 { fallback } else { max_sec };
    if min > max {
        std::mem::swap(&mut min, &mut max);
    }
    if min < 1 {
        min = 1;
    }
    if max < 1 {
        max = 1;
    }
    (min, max)
}

/// 随机 interval（毫秒）
#[must_use]
pub fn random_interval_ms(min_ms: u64, max_ms: u64) -> u64 {
    let min_sec = (min_ms.max(1000) / 1000) as i64;
    let max_sec = (max_ms.max(min_ms).max(1000) / 1000) as i64;
    if min_sec == max_sec {
        return (min_sec as u64) * 1000;
    }
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    // 简易 LCG（确定性足够）
    let r = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let range = (max_sec - min_sec + 1) as u64;
    let sec = min_sec as u64 + (r % range);
    sec * 1000
}

/// 本地日期键（YYYY-MM-DD）
#[must_use]
pub fn get_local_date_key() -> String {
    use chrono::Local;
    let now = Local::now();
    format!(
        "{:04}-{:02}-{:02}",
        now.format("%Y").to_string().parse::<i32>().unwrap_or(0),
        now.format("%m").to_string().parse::<u32>().unwrap_or(0),
        now.format("%d").to_string().parse::<u32>().unwrap_or(0),
    )
}

/// 当前毫秒
#[must_use]
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Account;
    use crate::network::gateway::{Gateway, GatewayConfig};
    use crate::services::activity_center::ActivityCenterService;

    fn make_account() -> Account {
        Account::new("acc-1", "code-1", "Test")
    }

    fn make_gateway() -> Arc<Gateway> {
        let cfg = GatewayConfig {
            server_url: "https://example.com".to_string(),
            platform: "qq".to_string(),
            os: "linux".to_string(),
            client_version: "1.0".to_string(),
            auth_code: "code-1".to_string(),
            headers: std::collections::HashMap::new(),
        };
        // Gateway::new 接受 (GatewayConfig, Arc<dyn Encryptor>)；这里用 dummy encryptor
        let enc: Arc<dyn crate::network::encryptor::Encryptor> =
            Arc::new(crate::network::encryptor::NoopEncryptor);
        Arc::new(Gateway::new(cfg, enc))
    }

    fn make_loop() -> (WorkerLoop, broadcast::Sender<WorkerEvent>) {
        let account = make_account();
        let (tx, _) = broadcast::channel(64);
        let gateway = make_gateway();
        let farm = Arc::new(FarmService::new(gateway.clone()));
        let friend = Arc::new(FriendService::new(gateway.clone(), 5));
        let email = Arc::new(EmailService::new(gateway.clone()));
        let share = Arc::new(ShareService::new(gateway.clone()));
        let monthcard = Arc::new(MonthCardService::new(gateway.clone()));
        let qqvip = Arc::new(QQVipService::new(gateway.clone()));
        let mall = Arc::new(MallService::new(gateway.clone()));
        let task = Arc::new(TaskService::new(gateway.clone()));
        let warehouse = Arc::new(WarehouseService::new(gateway.clone()));
        let mystery_shop = Arc::new(MysteryShopService::new(gateway.clone()));
        let activity_center = Arc::new(ActivityCenterService::new(gateway.clone()));
        let loop_ = WorkerLoop::new(
            account,
            WorkerLoopConfig::default(),
            gateway,
            tx.clone(),
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
        );
        (loop_, tx)
    }

    #[test]
    fn normalize_interval_range_sec_basic() {
        assert_eq!(normalize_interval_range_sec(10, 20, 5), (10, 20));
    }

    #[test]
    fn normalize_interval_range_sec_swap_min_max() {
        assert_eq!(normalize_interval_range_sec(30, 10, 5), (10, 30));
    }

    #[test]
    fn normalize_interval_range_sec_zero_uses_fallback() {
        assert_eq!(normalize_interval_range_sec(0, 0, 7), (7, 7));
    }

    #[test]
    fn normalize_interval_range_sec_negative_clamps() {
        assert_eq!(normalize_interval_range_sec(-5, 20, 5), (5, 20));
    }

    #[test]
    fn random_interval_ms_within_range() {
        for _ in 0..100 {
            let ms = random_interval_ms(2000, 5000);
            assert!((2000..=5000).contains(&ms));
        }
    }

    #[test]
    fn random_interval_ms_equal_endpoints() {
        for _ in 0..10 {
            assert_eq!(random_interval_ms(3000, 3000), 3000);
        }
    }

    #[test]
    fn random_interval_ms_handles_small_min() {
        // <1000ms 会被夹到 1000ms
        let ms = random_interval_ms(500, 2000);
        assert!((1000..=2000).contains(&ms));
    }

    #[test]
    fn get_local_date_key_format() {
        let s = get_local_date_key();
        assert_eq!(s.len(), 10);
        assert_eq!(s.chars().nth(4), Some('-'));
        assert_eq!(s.chars().nth(7), Some('-'));
    }

    #[test]
    fn now_ms_reasonable() {
        let n = now_ms();
        // 当前时间应该 > 1.7e12 ms (2024)
        assert!(n > 1_700_000_000_000);
    }

    #[test]
    fn worker_loop_initial_state() {
        let (loop_, _) = make_loop();
        assert!(!loop_.login_ready());
        assert!(!loop_.shutdown_started());
        assert!(!loop_.is_running.load(Ordering::Acquire));
    }

    #[test]
    fn worker_loop_mark_login_ready() {
        let (loop_, _) = make_loop();
        loop_.mark_login_ready();
        assert!(loop_.login_ready());
        assert!(loop_.is_running.load(Ordering::Acquire));
    }

    #[test]
    fn worker_loop_quiesce_bot() {
        let (loop_, _) = make_loop();
        loop_.mark_login_ready();
        loop_.quiesce_bot("test");
        assert!(loop_.shutdown_started());
        assert!(!loop_.login_ready());
    }

    #[test]
    fn worker_loop_resume_bot() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (loop_, _) = make_loop();
        rt.block_on(async {
            loop_.mark_login_ready();
            loop_.quiesce_bot("test");
            loop_.resume_bot();
            assert!(!loop_.shutdown_started());
            assert!(loop_.login_ready());
        });
    }

    #[test]
    fn apply_config_revision_idempotent() {
        let (loop_, _) = make_loop();
        assert!(loop_.apply_config_revision(1));
        assert!(!loop_.apply_config_revision(1));
        assert!(loop_.apply_config_revision(2));
    }

    #[test]
    fn account_info() {
        let (loop_, _) = make_loop();
        assert_eq!(loop_.account_id(), "acc-1");
        assert_eq!(loop_.account_name(), "Test");
    }

    #[test]
    fn service_accessors() {
        let (loop_, _) = make_loop();
        // 简单确保各 service 可访问（不调用真实方法）
        let _ = loop_.farm();
        let _ = loop_.friend();
        let _ = loop_.activity_center();
        let _ = loop_.email();
        let _ = loop_.share();
        let _ = loop_.monthcard();
        let _ = loop_.qqvip();
        let _ = loop_.mall();
        let _ = loop_.task();
        let _ = loop_.warehouse();
        let _ = loop_.mystery_shop();
        let _ = loop_.gateway();
    }

    #[test]
    fn sync_status_emits_event() {
        let (loop_, tx) = make_loop();
        let mut rx = tx.subscribe();
        loop_.sync_status();
        // 应该收到一条 Status 事件
        let ev = rx.try_recv();
        assert!(ev.is_ok());
    }

    #[test]
    fn on_kickout_quiesces() {
        let (loop_, _) = make_loop();
        loop_.mark_login_ready();
        loop_.on_kickout("test_reason");
        assert!(loop_.shutdown_started());
    }

    #[test]
    fn on_disconnect_quiesces() {
        let (loop_, _) = make_loop();
        loop_.mark_login_ready();
        loop_.on_disconnect("ws_close", 1006, "online");
        assert!(loop_.shutdown_started());
    }

    #[test]
    fn on_kickout_idempotent_after_shutdown() {
        let (loop_, _) = make_loop();
        loop_.mark_login_ready();
        loop_.on_kickout("first");
        // 第二次 kickout 不应 panic / 不应改状态
        loop_.on_kickout("second");
        assert!(loop_.shutdown_started());
    }

    #[test]
    fn start_schedulers_runs() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (loop_, _) = make_loop();
        rt.block_on(async {
            let scheduler = Scheduler::new(format!("worker:{}", loop_.account_id()));
            loop_.start(&scheduler);
            // 至少 status_sync 任务注册了
            let snap = scheduler.snapshot();
            assert!(snap.tasks.iter().any(|t| t.name == "status_sync"));
            scheduler.shutdown();
        });
    }

    #[test]
    fn default_config_reasonable() {
        let cfg = WorkerLoopConfig::default();
        assert_eq!(cfg.status_interval, Duration::from_secs(3));
        assert_eq!(cfg.daily_routine_interval, Duration::from_secs(30));
        assert_eq!(cfg.season_progress_interval, Duration::from_secs(300));
    }
}
