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

use crate::models::AccountSession;
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
    /// 心跳间隔（原 TS `CONFIG.heartbeatInterval`，默认 25s）
    pub heartbeat_interval: Duration,
    /// 心跳超时（30s 无响应则强制重连）
    pub heartbeat_timeout: Duration,
    /// 客户端版本（用于 HeartbeatRequest.client_version）
    pub client_version: String,
}

impl Default for WorkerLoopConfig {
    fn default() -> Self {
        Self {
            status_interval: Duration::from_secs(3),
            daily_routine_interval: Duration::from_secs(30),
            heartbeat_interval: Duration::from_millis(crate::constants::HEARTBEAT_INTERVAL_MS),
            heartbeat_timeout: Duration::from_millis(crate::constants::HEARTBEAT_SILENCE_MS),
            client_version: crate::config::DEFAULT_CLIENT_VERSION.to_string(),
        }
    }
}

/// Worker 编排器
pub struct WorkerLoop {
    account: AccountSession,
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
    /// 当前登录的 GID（0 = 未登录）
    gid: Arc<Mutex<i64>>,
    /// 上次 heartbeat 响应时间（ms）
    last_heartbeat_response: Arc<Mutex<i64>>,
    /// heartbeat miss 计数
    heartbeat_miss_count: Arc<Mutex<u32>>,
    /// 心跳超时回调
    on_heartbeat_timeout: Arc<Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>>,
    farm_tick_running: AtomicBool,
    help_tick_running: AtomicBool,
    steal_tick_running: AtomicBool,
    /// 对齐 TS `runUnifiedTick`：farm/help/steal 串行，避免并发打满网关
    unified_tick_running: AtomicBool,
    /// 对齐 TS `unifiedSchedulerRunning`
    unified_scheduler_running: AtomicBool,
    /// 对齐 TS `lastPushTime`（土地推送 500ms 去抖）
    last_lands_push_at: AtomicI64,
    harvest_sell_running: Arc<AtomicBool>,
    harvest_sell_pending: Arc<AtomicBool>,
    /// 上次已应用的施肥模式（用于配置保存后立即施肥）
    last_fertilizer_mode: Mutex<crate::models::types::FertilizerMode>,
    /// 点券 / 金豆豆（对齐 TS userState.coupon / goldBean）
    coupon: Mutex<i64>,
    gold_bean: Mutex<i64>,
    ace: Mutex<Option<Arc<crate::services::ace::AceShared>>>,
    /// worker 启动时刻（对齐 TS `process.uptime()`）
    started_at: std::time::Instant,
}

/// 心跳 miss 阈值
const MAX_HEARTBEAT_MISS: u32 = 1;

/// AtomicBool/AtomicU64
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};

/// 下次执行时间
#[derive(Debug, Clone, Default)]
struct NextRuns {
    farm_at: i64,
    help_at: i64,
    steal_at: i64,
}

fn heartbeat_silence_exceeded(now: i64, last_hb: i64, last_rx: i64, silence_ms: i64) -> bool {
    let last = last_hb.max(last_rx);
    last > 0 && now.saturating_sub(last) > silence_ms
}

struct FlagGuard<'a>(&'a AtomicBool);

impl Drop for FlagGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct NextSlotGuard<'a> {
    flag: &'a AtomicBool,
    next: &'a Mutex<NextRuns>,
    kind: &'static str,
    min_ms: u64,
    max_ms: u64,
}

impl Drop for NextSlotGuard<'_> {
    fn drop(&mut self) {
        let at = now_ms() + random_interval_ms(self.min_ms, self.max_ms) as i64;
        {
            let mut g = self.next.lock();
            match self.kind {
                "farm" => g.farm_at = at,
                "help" => g.help_at = at,
                "steal" => g.steal_at = at,
                _ => {}
            }
        }
        self.flag.store(false, Ordering::Release);
    }
}

/// IP 化：worker 上报给 master 的状态数据结构
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub connected: bool,
    pub ws_error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
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
        account: AccountSession,
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
        farm.api().set_operation_limits_callback(Arc::new({
            let friend = friend.clone();
            move |limits| friend.update_operation_limits(&limits)
        }));
        friend.api().set_operation_limits_callback(Arc::new({
            let friend = friend.clone();
            move |limits| friend.update_operation_limits(&limits)
        }));
        friend.api().set_bad_gate(
            Arc::new({
                let friend = friend.clone();
                move || friend.is_bad_operation_limit_reached()
            }),
            Arc::new({
                let friend = friend.clone();
                move || friend.get_remaining_bad_operation_times()
            }),
            Arc::new({
                let friend = friend.clone();
                move |method| {
                    let _ = friend.mark_bad_operation_limit_reached(method);
                }
            }),
        );
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
            gid: Arc::new(Mutex::new(0)),
            last_heartbeat_response: Arc::new(Mutex::new(crate::utils::time::now_ms())),
            heartbeat_miss_count: Arc::new(Mutex::new(0)),
            on_heartbeat_timeout: Arc::new(Mutex::new(None)),
            farm_tick_running: AtomicBool::new(false),
            help_tick_running: AtomicBool::new(false),
            steal_tick_running: AtomicBool::new(false),
            unified_tick_running: AtomicBool::new(false),
            unified_scheduler_running: AtomicBool::new(false),
            last_lands_push_at: AtomicI64::new(0),
            harvest_sell_running: Arc::new(AtomicBool::new(false)),
            harvest_sell_pending: Arc::new(AtomicBool::new(false)),
            last_fertilizer_mode: Mutex::new(crate::models::types::FertilizerMode::None),
            coupon: Mutex::new(0),
            gold_bean: Mutex::new(0),
            ace: Mutex::new(None),
            started_at: std::time::Instant::now(),
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

    /// 设置登录完成后的 GID（启动 heartbeat 任务时使用）
    pub fn set_gid(&self, gid: i64) {
        *self.gid.lock() = gid;
        *self.last_heartbeat_response.lock() = crate::utils::time::now_ms();
    }

    /// 注册心跳超时回调
    pub fn on_heartbeat_timeout<F>(&self, cb: F)
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        *self.on_heartbeat_timeout.lock() = Some(Box::new(cb));
    }

    /// 设置 login ready
    pub fn mark_login_ready(&self) {
        self.login_ready.store(true, Ordering::Release);
        self.is_running.store(true, Ordering::Release);
    }

    fn auto_on(&self, category: &str) -> bool {
        automation::is_automation_on_for(&self.account.id, category)
    }

    fn interval_range_ms(&self, kind: &str) -> (u64, u64) {
        let i = crate::models::store::account_config::get_intervals(Some(&self.account.id));
        let (min_sec, max_sec) = match kind {
            "help" => (i.help_min, i.help_max),
            "steal" => (i.steal_min, i.steal_max),
            _ => (i.farm_min, i.farm_max),
        };
        let min_ms = (min_sec.max(1) as u64).saturating_mul(1000);
        let max_ms = (max_sec.max(1) as u64).saturating_mul(1000);
        (min_ms, max_ms.max(min_ms))
    }

    /// 应用 config revision（idempotent guard）
    pub fn apply_config_revision(&self, rev: u64) -> bool {
        let prev = self.applied_config_revision.swap(rev, Ordering::AcqRel);
        prev != rev
    }

    /// 对齐 TS `applyRuntimeConfig`：revision + 重置统一调度 + 施肥模式变更立即补肥
    pub fn apply_runtime_config(self: &Arc<Self>, rev: u64, scheduler: &Scheduler) {
        self.apply_config_revision(rev);
        let auto = crate::models::store::account_config::get_automation(Some(&self.account.id));
        let next_mode = auto.fertilizer;
        let prev_mode = *self.last_fertilizer_mode.lock();
        *self.last_fertilizer_mode.lock() = next_mode;

        if self.login_ready() {
            self.reset_unified_schedule();
            let intervals =
                crate::models::store::account_config::get_intervals(Some(&self.account.id));
            self.farm.set_check_interval(Duration::from_secs(intervals.farm.max(1) as u64));
            if self.unified_scheduler_running.load(Ordering::Acquire) {
                self.schedule_unified_next_tick(scheduler);
            }
            self.start_fertilizer_buy_timer(scheduler);

            // 对齐 bot：施肥模式变更且目标为 both/organic/smart 时，600ms 后立即有机补肥
            if prev_mode != next_mode
                && matches!(
                    next_mode,
                    crate::models::types::FertilizerMode::Both
                        | crate::models::types::FertilizerMode::Organic
                        | crate::models::types::FertilizerMode::Smart
                )
            {
                let this = Arc::clone(self);
                scheduler.set_timeout_task(
                    "fertilizer_immediate_after_save",
                    Duration::from_millis(600),
                    Arc::new(move || {
                        let this = Arc::clone(&this);
                        Box::pin(async move {
                            if !this.login_ready() {
                                return;
                            }
                            let gid = *this.gid.lock();
                            let planting = this.farm.planting();
                            let _ = planting
                                .lock()
                                .await
                                .fertilize_by_config_ex(
                                    &[],
                                    gid,
                                    &this.account.id,
                                    crate::services::farm::planting::FertilizeOptions {
                                        skip_normal: true,
                                        ..Default::default()
                                    },
                                )
                                .await;
                        })
                    }),
                );
            }
        }
        self.sync_status();
    }

    /// 登录成功后的编排：邀请码、礼包、收获自动出售、启动 tick / 跨日 / 放虫放草
    pub async fn on_login_success(self: &Arc<Self>, scheduler: &Scheduler) {
        self.mark_login_ready();
        let gid = *self.gid.lock();
        self.farm.set_host_gid(gid);
        self.friend.set_host_gid(gid);
        self.farm.set_account_id(&self.account.id);
        self.task.set_account_id(&self.account.id);
        self.warehouse.set_account_id(&self.account.id);
        self.friend.set_account_id(&self.account.id);
        self.activity_center.set_account_id(&self.account.id);
        self.activity_center.set_warehouse(self.warehouse.clone());
        self.farm.set_external_scheduler(true);
        self.friend.set_external_scheduler(true);
        *self.last_fertilizer_mode.lock() =
            crate::models::store::account_config::get_automation(Some(&self.account.id)).fertilizer;

        let mut harvest_rx = self.farm.subscribe();
        let warehouse = self.warehouse.clone();
        let harvest_sell_running = self.harvest_sell_running.clone();
        let harvest_sell_pending = self.harvest_sell_pending.clone();
        let harvest_account_id = self.account.id.clone();
        tokio::spawn(async move {
            loop {
                match harvest_rx.recv().await {
                    Ok(crate::services::farm::scheduler::FarmEvent::Harvested { .. }) => {
                        if !crate::services::automation::is_automation_on_for(
                            &harvest_account_id,
                            "sell",
                        ) {
                            continue;
                        }
                        if harvest_sell_running.swap(true, Ordering::AcqRel) {
                            harvest_sell_pending.store(true, Ordering::Release);
                            continue;
                        }
                        loop {
                            tokio::time::sleep(Duration::from_millis(800)).await;
                            let _ = warehouse.sell_all_fruits().await;
                            if !harvest_sell_pending.swap(false, Ordering::AcqRel) {
                                harvest_sell_running.store(false, Ordering::Release);
                                break;
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        // 对齐 worker.ts onLoginSuccess：先背包点券/金豆 → 统计基线 → 邀请码 → 礼包
        if let Ok(bag) = self.warehouse.get_bag().await {
            let items = crate::services::warehouse::get_bag_items(&bag);
            let coupon = items.iter().find(|i| i.id == 1002).map(|i| i.count).unwrap_or(0);
            let gold_bean = items.iter().find(|i| i.id == 1005).map(|i| i.count).unwrap_or(0);
            *self.coupon.lock() = coupon.max(0);
            if gold_bean > 0 {
                *self.gold_bean.lock() = gold_bean;
            }
            let st = status_svc::status_data_for(&self.account.id);
            crate::services::stats::init_stats_with_persistence(
                &self.account.id,
                st.gold,
                st.exp,
                coupon.max(0),
            );
            crate::services::stats::reset_session_gains_for(&self.account.id);
        }

        let invite = crate::services::invite::InviteService::new(self.gateway.clone());
        let _ = invite.process_invite_codes().await;

        if self.auto_on("fertilizer_gift") {
            let _ = self.warehouse.auto_open_fertilizer_gift_packs().await;
        }

        let this = Arc::clone(self);
        scheduler.set_timeout_task(
            "bad_startup_once",
            Duration::from_secs(10),
            Arc::new(move || {
                let this = this.clone();
                Box::pin(async move {
                    if let Err(e) = this.friend.run_bad_once_on_startup(&this.account.id).await {
                        tracing::warn!(
                            account_id = %this.account.id,
                            error = %e,
                            "启动时放虫放草执行失败"
                        );
                    }
                })
            }),
        );

        self.start_farm_ticks(scheduler);
        {
            *self.last_daily_date.lock() = get_local_date_key();
            let this = Arc::clone(self);
            tokio::spawn(async move {
                this.run_daily_routines(true).await;
            });
        }
        self.start_fertilizer_buy_timer(scheduler);
        {
            let this = Arc::clone(self);
            scheduler.set_timeout_task(
                "friend_check_bootstrap_applications",
                Duration::from_secs(3),
                Arc::new(move || {
                    let this = this.clone();
                    Box::pin(async move {
                        this.friend.check_and_accept_applications().await;
                    })
                }),
            );
        }
        self.sync_status();
    }

    /// 对齐 fetchGoldBeanFromBag：登录后立刻拉一次背包金豆
    pub async fn fetch_gold_bean_from_bag(&self) {
        let Ok(bag) = self.warehouse.get_bag().await else {
            return;
        };
        let items = crate::services::warehouse::get_bag_items(&bag);
        for it in items {
            if it.id == 1005 && it.count > 0 {
                *self.gold_bean.lock() = it.count;
                tracing::info!(count = it.count, "金豆豆数量");
                break;
            }
        }
    }

    /// 对齐 network.ts ItemNotify
    pub fn apply_item_notify(&self, items: &[crate::network::notify::ItemChgLite]) {
        let account_id = &self.account.id;
        for chg in items {
            match chg.id {
                1101 => {
                    let mut st = status_svc::status_data_for(account_id);
                    if chg.count > 0 {
                        st.exp = chg.count;
                    } else if chg.delta != 0 {
                        st.exp = (st.exp + chg.delta).max(0);
                    }
                    status_svc::update_status_level_for(account_id, st.level, Some(st.exp));
                }
                1 | 1001 => {
                    let mut gold = status_svc::status_data_for(account_id).gold;
                    if chg.count > 0 {
                        gold = chg.count;
                    } else if chg.delta != 0 {
                        gold = (gold + chg.delta).max(0);
                    }
                    status_svc::update_status_gold_for(account_id, gold);
                }
                1002 => {
                    let mut coupon = *self.coupon.lock();
                    if chg.count > 0 {
                        coupon = chg.count;
                    } else if chg.delta != 0 {
                        coupon = (coupon + chg.delta).max(0);
                    }
                    *self.coupon.lock() = coupon;
                }
                1005 => {
                    let mut bean = *self.gold_bean.lock();
                    if chg.count > 0 {
                        bean = chg.count;
                    } else if chg.delta != 0 {
                        bean = (bean + chg.delta).max(0);
                    }
                    *self.gold_bean.lock() = bean;
                }
                _ => {}
            }
        }
    }

    /// 对齐 network.ts BasicNotify
    pub fn apply_basic_notify(&self, level: Option<i64>, gold: Option<i64>, exp: Option<i64>) {
        let account_id = &self.account.id;
        let st = status_svc::status_data_for(account_id);
        let old_level = st.level;
        let mut next_level = st.level;
        let mut next_exp = st.exp;
        if let Some(lv) = level {
            if lv > 0 {
                next_level = lv;
            }
        }
        if let Some(e) = exp {
            if e >= 0 {
                next_exp = e;
            }
        }
        if next_level != st.level || next_exp != st.exp {
            status_svc::update_status_level_for(account_id, next_level, Some(next_exp));
        }
        if let Some(g) = gold {
            if g >= 0 {
                status_svc::update_status_gold_for(account_id, g);
            }
        }
        if next_level != old_level {
            crate::services::stats::record_operation_for(account_id, "levelUp", 1);
        }
    }

    /// 启动所有定时器
    pub fn start(self: &Arc<Self>, scheduler: &Scheduler) {
        let this = Arc::clone(self);
        scheduler.set_interval_task(
            "status_sync",
            self.config.status_interval,
            Arc::new(move || {
                let this = this.clone();
                Box::pin(async move {
                    this.sync_status();
                })
            }),
        );

        // 每日跨日检查
        let this = Arc::clone(self);
        scheduler.set_interval_task(
            "daily_routine_interval",
            self.config.daily_routine_interval,
            Arc::new(move || {
                let this = this.clone();
                Box::pin(async move {
                    let today = get_local_date_key();
                    {
                        let mut guard = this.last_daily_date.lock();
                        if *guard == today {
                            return;
                        }
                        *guard = today.clone();
                    }
                    tracing::info!(account_id = %this.account.id, date = %today, "daily routines due");
                    this.run_daily_routines(false).await;
                })
            }),
        );

        // 心跳：每 25s 发 HeartbeatRequest，30s 无响应则触发重连回调
        let gateway_for_hb = self.gateway.clone();
        let acc_id_hb = self.account.id.clone();
        let last_hb_resp = self.last_heartbeat_response.clone();
        let hb_miss = self.heartbeat_miss_count.clone();
        let hb_interval = self.config.heartbeat_interval;
        let hb_timeout = self.config.heartbeat_timeout;
        let on_hb_timeout = self.on_heartbeat_timeout.clone();
        let gid = self.gid.clone();
        let client_version = self.config.client_version.clone();
        scheduler.set_interval_task(
            "heartbeat_interval",
            hb_interval,
            Arc::new(move || {
                let gateway = gateway_for_hb.clone();
                let acc_id = acc_id_hb.clone();
                let last_resp = last_hb_resp.clone();
                let miss = hb_miss.clone();
                let on_timeout = on_hb_timeout.clone();
                let gid_lock = gid.clone();
                let cv_for_req = client_version.clone();
                Box::pin(async move {
                    // 对齐 network.ts：phase !== 'online' || !gid 则跳过
                    if gateway.phase() != crate::network::gateway::ConnectionPhase::Online {
                        return;
                    }
                    let now = crate::utils::time::now_ms();
                    let last_hb = *last_resp.lock();
                    let last_rx = gateway.last_rx_ms();
                    let last = last_hb.max(last_rx);
                    let elapsed = now - last;
                    // 杀号看入站帧 / 心跳成功，不看 pending（超时 cancel 会把 pending 打成 0）。
                    if heartbeat_silence_exceeded(
                        now,
                        last_hb,
                        last_rx,
                        hb_timeout.as_millis() as i64,
                    ) {
                        let miss_n = {
                            let mut g = miss.lock();
                            *g += 1;
                            *g
                        };
                        tracing::warn!(
                            account_id = %acc_id,
                            elapsed_ms = elapsed,
                            pending = gateway.pending_count(),
                            "心跳超时 ({}s 无响应)",
                            elapsed / 1000
                        );
                        crate::services::panel_log::log(
                            &acc_id,
                            "心跳",
                            format!("连接可能已断开 ({}s 无响应)", elapsed / 1000),
                            crate::constants::PanelEvent::HeartbeatTimeout,
                            Some(serde_json::json!({
                                "module": "heartbeat",
                                "isWarn": true,
                                "elapsedMs": elapsed,
                            })),
                        );
                        if miss_n >= MAX_HEARTBEAT_MISS {
                            tracing::error!(account_id = %acc_id, "心跳 miss 达到上限，触发重连");
                            crate::services::panel_log::log(
                                &acc_id,
                                "心跳",
                                "心跳超时，账号将停止运行...",
                                crate::constants::PanelEvent::HeartbeatTimeout,
                                Some(serde_json::json!({
                                    "module": "heartbeat",
                                    "isWarn": true
                                })),
                            );
                            if let Some(cb) = on_timeout.lock().as_ref() {
                                cb(acc_id.clone());
                            }
                            return;
                        }
                    }

                    let current_gid = *gid_lock.lock();
                    if current_gid == 0 {
                        return;
                    }
                    if gateway.has_pending_method("Heartbeat") {
                        tracing::debug!(
                            account_id = %acc_id,
                            "skip Heartbeat: already in flight"
                        );
                        return;
                    }
                    // 对齐 network.ts：sendMsgAsync(...).then(...).catch(() => {}) —— 发完即返回，不阻塞 interval
                    let gateway = gateway.clone();
                    let last_resp = last_resp.clone();
                    let miss = miss.clone();
                    let acc_id = acc_id.clone();
                    let cv_for_req = cv_for_req.clone();
                    tokio::spawn(async move {
                        match gateway.heartbeat(current_gid, &cv_for_req).await {
                            Ok(_reply) => {
                                *last_resp.lock() = crate::utils::time::now_ms();
                                *miss.lock() = 0;
                            }
                            Err(e) => {
                                tracing::debug!(
                                    account_id = %acc_id,
                                    error = %e,
                                    "Heartbeat RPC 超时（忙时常见，不等于掉线）"
                                );
                            }
                        }
                    });
                })
            }),
        );
    }

    /// 对齐 TS `resetUnifiedSchedule`：首次执行推迟到随机间隔之后，而不是立刻打满
    fn reset_unified_schedule(&self) {
        let now = now_ms();
        let (farm_min, farm_max) = self.interval_range_ms("farm");
        let (help_min, help_max) = self.interval_range_ms("help");
        let (steal_min, steal_max) = self.interval_range_ms("steal");
        let mut next = self.next_runs.lock();
        next.farm_at = now + random_interval_ms(farm_min, farm_max) as i64;
        next.help_at = now + random_interval_ms(help_min, help_max) as i64;
        next.steal_at = now + random_interval_ms(steal_min, steal_max) as i64;
    }

    /// 启动统一 farm / help / steal（对齐 TS `startUnifiedScheduler` + `scheduleUnifiedNextTick`）
    pub fn start_farm_ticks(self: &Arc<Self>, scheduler: &Scheduler) {
        if self.unified_scheduler_running.swap(true, Ordering::AcqRel) {
            return;
        }
        self.reset_unified_schedule();
        self.schedule_unified_next_tick(scheduler);
    }

    /// 停止统一调度（对齐 TS `stopUnifiedScheduler`）
    pub fn stop_farm_ticks(&self, scheduler: &Scheduler) {
        self.unified_scheduler_running.store(false, Ordering::Release);
        self.farm_tick_running.store(false, Ordering::Release);
        self.help_tick_running.store(false, Ordering::Release);
        self.steal_tick_running.store(false, Ordering::Release);
        self.unified_tick_running.store(false, Ordering::Release);
        scheduler.clear("unified_next_tick");
    }

    /// 对齐 TS `scheduleUnifiedNextTick`：按下次到期时间 setTimeout，最低 1s
    fn schedule_unified_next_tick(self: &Arc<Self>, scheduler: &Scheduler) {
        if !self.unified_scheduler_running.load(Ordering::Acquire) {
            return;
        }
        if !self.login_ready() {
            return;
        }
        scheduler.clear("unified_next_tick");
        let now = now_ms();
        let next_at = {
            let g = self.next_runs.lock();
            let farm = if g.farm_at > 0 { g.farm_at } else { now + 1000 };
            let help = if g.help_at > 0 { g.help_at } else { now + 1000 };
            let steal = if g.steal_at > 0 { g.steal_at } else { now + 1000 };
            farm.min(help).min(steal)
        };
        let delay_ms = (next_at - now).max(1000) as u64;
        let this = Arc::clone(self);
        let sched = scheduler.clone();
        scheduler.set_timeout_task(
            "unified_next_tick",
            Duration::from_millis(delay_ms),
            Arc::new(move || {
                let this = this.clone();
                let sched = sched.clone();
                Box::pin(async move {
                    this.run_unified_tick().await;
                    this.schedule_unified_next_tick(&sched);
                })
            }),
        );
    }

    /// 对齐 TS `startFertilizerBuyCheckTimer`
    fn start_fertilizer_buy_timer(self: &Arc<Self>, scheduler: &Scheduler) {
        if !self.auto_on("fertilizer_buy_organic") && !self.auto_on("fertilizer_buy_normal") {
            scheduler.clear("fertilizer_buy_check");
            return;
        }
        let snap = crate::models::store::account_config::get_account_config_snapshot(Some(
            &self.account.id,
        ));
        let minutes = snap.fertilizer_buy_check_interval_minutes.max(1) as u64;
        let this = Arc::clone(self);
        scheduler.set_interval_task(
            "fertilizer_buy_check",
            Duration::from_secs(minutes * 60),
            Arc::new(move || {
                let this = this.clone();
                Box::pin(async move {
                    this.check_fertilizer_buy_once().await;
                })
            }),
        );
        crate::services::panel_log::log(
            &self.account.id,
            "农场",
            format!("化肥自动购买检测定时器已启动，间隔 {minutes} 分钟"),
            crate::constants::PanelEvent::FertilizerBuyTimer,
            Some(serde_json::json!({
                "module": "farm",
                "result": "start",
                "intervalMinutes": minutes,
            })),
        );
    }

    async fn check_fertilizer_buy_once(&self) {
        if !self.auto_on("fertilizer_buy_organic") && !self.auto_on("fertilizer_buy_normal") {
            return;
        }
        let snap = crate::models::store::account_config::get_account_config_snapshot(Some(
            &self.account.id,
        ));
        let commerce = crate::services::commerce::CommerceService::new(
            self.mall.clone(),
            self.mystery_shop.clone(),
            self.warehouse.clone(),
        );
        let opts = crate::services::commerce::FertilizerBothOptions {
            buy_organic: snap.automation.fertilizer_buy_organic,
            buy_normal: snap.automation.fertilizer_buy_normal,
            organic_count: snap.fertilizer_buy_organic_count as i32,
            organic_threshold_hours: snap.fertilizer_buy_organic_threshold_hours as f64,
            normal_count: snap.fertilizer_buy_normal_count as i32,
            normal_threshold_hours: snap.fertilizer_buy_normal_threshold_hours as f64,
        };
        let _ = commerce.check_and_buy_fertilizer_both(opts).await;
    }

    /// 对齐 TS `runUnifiedTick`：串行执行，避免并发请求过多导致超时
    async fn run_unified_tick(&self) {
        if !self.login_ready() {
            return;
        }
        if self.unified_tick_running.swap(true, Ordering::AcqRel) {
            return;
        }
        let _guard = FlagGuard(&self.unified_tick_running);
        let now = now_ms();
        let (due_farm, due_help, due_steal) = {
            let guard = self.next_runs.lock();
            (
                guard.farm_at > 0 && now >= guard.farm_at,
                guard.help_at > 0 && now >= guard.help_at,
                guard.steal_at > 0 && now >= guard.steal_at,
            )
        };
        if due_farm {
            self.run_farm_tick().await;
        }
        if due_help {
            self.run_help_tick().await;
        }
        if due_steal {
            self.run_steal_tick().await;
        }
    }

    /// 触发 farm tick（对外暴露给 on_login_success 启动独立 task）
    pub async fn run_farm_tick(&self) {
        if self.farm_tick_running.swap(true, Ordering::AcqRel) {
            return;
        }
        let (min_ms, max_ms) = self.interval_range_ms("farm");
        let _guard = NextSlotGuard {
            flag: &self.farm_tick_running,
            next: &self.next_runs,
            kind: "farm",
            min_ms,
            max_ms,
        };
        if self.login_ready() {
            // 静默时段仅作用于好友帮助/偷菜；本田 tick 仍跑（对齐 bot worker.ts）
            if self.auto_on("farm") {
                let _ = self.farm.check_farm().await;
            }
            if self.auto_on("task") {
                let _ = self.task.check_and_claim_tasks().await;
            }
            // bot 无 auto.email：邮件只走日更 run_daily_routines，不在 farm tick 领取
            if self.auto_on("fertilizer_gift") {
                let _ = self.warehouse.auto_open_fertilizer_gift_packs().await;
            }
            self.sync_status();
        }
    }

    /// 触发 help tick
    pub async fn run_help_tick(&self) {
        if !self.login_ready() {
            return;
        }
        if !self.auto_on("friend_help") {
            let (min_ms, max_ms) = self.interval_range_ms("help");
            self.next_runs.lock().help_at = now_ms() + random_interval_ms(min_ms, max_ms) as i64;
            return;
        }
        if self.help_tick_running.swap(true, Ordering::AcqRel) {
            return;
        }
        let (min_ms, max_ms) = self.interval_range_ms("help");
        let _guard = NextSlotGuard {
            flag: &self.help_tick_running,
            next: &self.next_runs,
            kind: "help",
            min_ms,
            max_ms,
        };
        if crate::services::friend::visit_strategy::in_friend_quiet_hours_for(
            Some(&self.account.id),
            None,
        ) {
            return;
        }
        if self.auto_on("friend_help_exp_limit") && self.friend.is_help_exp_limit_reached() {
            return;
        }
        let _ = self.friend.check_friends_help(&self.account.id).await;
        self.sync_status();
    }

    /// 触发 steal tick
    pub async fn run_steal_tick(&self) {
        if !self.login_ready() {
            return;
        }
        if !self.auto_on("friend_steal") {
            let (min_ms, max_ms) = self.interval_range_ms("steal");
            self.next_runs.lock().steal_at = now_ms() + random_interval_ms(min_ms, max_ms) as i64;
            return;
        }
        if self.steal_tick_running.swap(true, Ordering::AcqRel) {
            return;
        }
        let (min_ms, max_ms) = self.interval_range_ms("steal");
        let _guard = NextSlotGuard {
            flag: &self.steal_tick_running,
            next: &self.next_runs,
            kind: "steal",
            min_ms,
            max_ms,
        };
        if crate::services::friend::visit_strategy::in_friend_quiet_hours_for(
            Some(&self.account.id),
            None,
        ) {
            return;
        }
        let stolen = self.friend.check_friends_steal(&self.account.id).await.unwrap_or(0);
        if stolen > 0 {
            tokio::time::sleep(Duration::from_millis(800)).await;
            let _ = self.warehouse.sell_all_fruits().await;
        }
        self.sync_status();
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
        self.farm.stop_check_loop();
        self.friend.stop_check_loop();
        if let Some(ace) = self.ace.lock().take() {
            ace.stop(false);
        }
    }

    /// 挂上 ACE runtime（登录成功后，断开时随 quiesce 停）
    pub fn attach_ace(&self, ace: Arc<crate::services::ace::AceShared>) {
        if let Some(old) = self.ace.lock().replace(ace) {
            old.stop(false);
        }
    }

    /// 重启 bot
    pub fn resume_bot(&self) {
        self.shutdown_started.store(false, Ordering::Release);
        self.is_running.store(true, Ordering::Release);
        self.login_ready.store(true, Ordering::Release);
        self.farm.set_external_scheduler(true);
        self.friend.set_external_scheduler(true);
        self.reset_unified_schedule();
    }

    /// 土地推送：自己的田走巡田；好友田只刷新该 gid 气泡。
    pub fn on_lands_notify(
        self: &Arc<Self>,
        host_gid: i64,
        changed_count: usize,
        lands: Vec<crate::proto::generated::gamepb::plantpb::LandInfo>,
    ) {
        let my = *self.gid.lock();
        if host_gid > 0 && my > 0 && host_gid != my {
            let friend = Arc::clone(&self.friend);
            tokio::spawn(async move {
                friend.on_friend_lands_notify(host_gid, lands).await;
            });
            return;
        }
        self.on_lands_changed(changed_count);
    }

    /// 对齐 TS `onLandsChangedPush`：farm_push 开启时由土地推送触发巡田
    pub fn on_lands_changed(self: &Arc<Self>, changed_count: usize) {
        if !self.login_ready() || !self.auto_on("farm_push") {
            return;
        }
        let now = now_ms();
        let last = self.last_lands_push_at.load(Ordering::Acquire);
        if now - last < 500 {
            return;
        }
        self.last_lands_push_at.store(now, Ordering::Release);
        crate::services::panel_log::log(
            &self.account.id,
            "农场",
            format!("收到推送: {changed_count}块土地变化，检查中..."),
            crate::constants::PanelEvent::LandsNotify,
            Some(serde_json::json!({
                "module": "farm",
                "result": "trigger_check",
                "count": changed_count,
            })),
        );
        let this = Arc::clone(self);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let _ = this.farm.check_farm().await;
        });
    }

    /// 同步状态（对齐原 worker `syncStatus`：getStats + nextChecks + automation）
    pub fn sync_status(&self) {
        let st = status_svc::status_data_for(&self.account.id);
        let user = serde_json::json!({
            "name": st.name,
            "avatar": st.avatar,
            "level": st.level,
            "gold": st.gold,
            "exp": st.exp,
            "platform": st.platform,
            "coupon": *self.coupon.lock(),
            "goldBean": *self.gold_bean.lock(),
        });
        let connected = self.login_ready();
        let limits = self.friend.get_operation_limits();
        let mut full = crate::services::stats::get_stats_for(
            &self.account.id,
            Some(&user),
            Some(&user),
            connected,
            limits,
        );
        let now = now_ms();
        let next = self.next_runs.lock().clone();
        let farm = ((next.farm_at - now) / 1000).max(0);
        let help = ((next.help_at - now) / 1000).max(0);
        let steal = ((next.steal_at - now) / 1000).max(0);
        let auto = crate::models::store::account_config::get_automation(Some(&self.account.id));
        let preferred =
            crate::models::store::account_config::get_preferred_seed(Some(&self.account.id));
        let (current, needed) =
            crate::config::game_config::global().get_level_exp_progress(st.level, st.exp);
        if let Some(obj) = full.as_object_mut() {
            obj.insert(
                "nextChecks".to_string(),
                serde_json::json!({
                    "farmRemainSec": farm,
                    "helpRemainSec": help,
                    "stealRemainSec": steal,
                    "friendRemainSec": help.max(steal),
                }),
            );
            obj.insert(
                "automation".to_string(),
                serde_json::to_value(&auto).unwrap_or(serde_json::json!({})),
            );
            obj.insert("preferredSeed".to_string(), serde_json::json!(preferred));
            obj.insert(
                "levelProgress".to_string(),
                serde_json::json!({ "current": current, "needed": needed }),
            );
            obj.insert(
                "configRevision".to_string(),
                serde_json::json!(self.applied_config_revision.load(Ordering::Acquire)),
            );
            obj.insert("accountId".to_string(), serde_json::json!(self.account.id));
            obj.insert("accountName".to_string(), serde_json::json!(self.account.display_name));
            obj.insert(
                "uptime".to_string(),
                serde_json::json!(self.started_at.elapsed().as_secs_f64()),
            );
        }
        let _ = self.event_tx.send(WorkerEvent::Status {
            account_id: self.account.id.clone(),
            account_name: self.account.display_name.clone(),
            status: full,
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
    let seed =
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    // 简易 LCG（确定性足够）
    let r = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
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
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AccountSession;
    use crate::network::gateway::{Gateway, GatewayConfig};
    use crate::services::activity_center::ActivityCenterService;

    fn make_account() -> AccountSession {
        AccountSession::new("acc-1", "code-1", "Test")
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

    fn make_loop() -> (Arc<WorkerLoop>, broadcast::Sender<WorkerEvent>) {
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
        let loop_ = Arc::new(WorkerLoop::new(
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
        ));
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
    fn heartbeat_silence_ignores_pending_and_uses_inbound_frames() {
        assert!(!heartbeat_silence_exceeded(1_000, 980, 0, 50));
        assert!(heartbeat_silence_exceeded(1_000, 900, 0, 50));
        // 有入站帧则不算静默，即使心跳很久没成功
        assert!(!heartbeat_silence_exceeded(1_000, 100, 980, 50));
        // pending 不是参数：从未收到过任何帧时不杀
        assert!(!heartbeat_silence_exceeded(1_000, 0, 0, 30));
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
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
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
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let (loop_, _) = make_loop();
        rt.block_on(async {
            let scheduler = Scheduler::new(format!("worker:{}", loop_.account_id()));
            loop_.start(&scheduler);
            // 至少 status_sync 任务注册了
            let snap = scheduler.snapshot();
            assert!(snap.tasks.iter().any(|t| t.name == "status_sync"));
            loop_.mark_login_ready();
            loop_.start_farm_ticks(&scheduler);
            let snap = scheduler.snapshot();
            assert!(snap.tasks.iter().any(|t| t.name == "unified_next_tick"));
            let next = loop_.next_runs.lock().clone();
            let now = now_ms();
            assert!(
                next.farm_at > now,
                "first farm tick must be delayed like TS resetUnifiedSchedule"
            );
            assert!(next.help_at > now);
            assert!(next.steal_at > now);
            scheduler.shutdown();
        });
    }

    #[test]
    fn default_config_reasonable() {
        let cfg = WorkerLoopConfig::default();
        assert_eq!(cfg.status_interval, Duration::from_secs(3));
        assert_eq!(cfg.daily_routine_interval, Duration::from_secs(30));
        assert_eq!(cfg.heartbeat_interval, Duration::from_secs(25));
    }
}
