//! Worker 编排层 — 1:1 翻译原 `core/src/core/worker.ts`（965 行）。
//!
//! ## 职责
//!
//! 编排各 service 的执行节奏：
//! - 每日任务（email / share / monthcard / 商城免费 / qqvip）跨日调度
//! - 农场巡查（随机间隔 + 防重入）
//! - 好友巡查（统一 tick：帮助 + 偷菜 + 捣乱一次做完，对齐 bot checkFriends）
//! - 好友宠物每日同步（登录后 spawn，自适应节奏轮次链）
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

type HeartbeatTimeoutCallback = dyn Fn(String) + Send + Sync;
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
mod heartbeat;
mod status;

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
    weather: Arc<crate::services::weather_activity::WeatherActivityService>,

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
    on_heartbeat_timeout: Arc<Mutex<Option<Box<HeartbeatTimeoutCallback>>>>,
    farm_tick_running: AtomicBool,
    /// 统一好友 tick（帮助 + 偷菜 + 捣乱）防重入
    friend_tick_running: AtomicBool,
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
    /// 神秘商人自动化去重状态（visitKey 记忆）
    mystery_auto_state: Mutex<crate::services::mystery_shop_auto::MysteryShopAutoState>,
    /// 上次已广播的状态 JSON（内容不变时跳过广播，对齐 node 哈希门控）
    last_status_json: Mutex<String>,
    /// 状态脏标记：业务事件（收获/物品/推送/配置）置位，status_sync 只在
    /// 脏或距上次广播超过 30s 时才重建 payload（消除空转序列化）
    status_dirty: AtomicBool,
    /// 上次状态广播时间（ms）
    last_status_sent_ms: AtomicI64,
    /// worker 启动时刻（对齐 TS `process.uptime()`）
    started_at: std::time::Instant,
}

/// 心跳 miss 阈值
/// 对齐 node `keepalive-policy.ts` MAX_HEARTBEAT_MISSES：连续 3 次心跳失败且入站静默超阈值才判死。
const MAX_HEARTBEAT_MISS: u32 = 3;

/// AtomicBool/AtomicU64
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};

/// 下次执行时间
#[derive(Debug, Clone, Default)]
struct NextRuns {
    farm_at: i64,
    /// 统一好友巡查（帮助 + 偷菜 + 捣乱一次做完，对齐 bot checkFriends）
    friend_at: i64,
}

#[cfg(test)]
fn heartbeat_silence_exceeded(now: i64, last_hb: i64, last_rx: i64, silence_ms: i64) -> bool {
    let last = last_hb.max(last_rx);
    last > 0 && now.saturating_sub(last) > silence_ms
}

/// 心跳判死：连续 miss 达上限、入站静默超阈值，且**没有任何在途请求**。
///
/// pending 保护：巨型回包（大号好友 GetAll）下载期间没有完整消息到达 dispatch，
/// 入站静默会虚高、心跳回包也被压在后面——但连接是活的。所有业务 RPC 都有
/// 20s 超时，真断线时 pending 会在 20s 内归零、下一拍照常判死；保护窗封顶
/// [`PENDING_DEFER_MAX_SILENCE_MS`]，避免持续发请求的僵尸连接永远不被杀。
#[cfg_attr(test, allow(dead_code))]
fn heartbeat_should_force_disconnect(
    miss_n: u32,
    max_miss: u32,
    inbound_silence_ms: i64,
    stale_ms: i64,
    pending: usize,
) -> bool {
    if miss_n < max_miss {
        return false;
    }
    if inbound_silence_ms <= stale_ms {
        return false;
    }
    if pending > 0 && inbound_silence_ms <= PENDING_DEFER_MAX_SILENCE_MS {
        return false;
    }
    true
}

/// pending>0 时判死保护窗的上限（2 分钟）。
const PENDING_DEFER_MAX_SILENCE_MS: i64 = 120_000;

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
                "friend" => g.friend_at = at,
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
        let weather = Arc::new(crate::services::weather_activity::WeatherActivityService::new(
            std::sync::Arc::clone(&gateway),
        ));
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
            weather,
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
            friend_tick_running: AtomicBool::new(false),
            unified_tick_running: AtomicBool::new(false),
            unified_scheduler_running: AtomicBool::new(false),
            last_lands_push_at: AtomicI64::new(0),
            harvest_sell_running: Arc::new(AtomicBool::new(false)),
            harvest_sell_pending: Arc::new(AtomicBool::new(false)),
            last_fertilizer_mode: Mutex::new(crate::models::types::FertilizerMode::None),
            coupon: Mutex::new(0),
            gold_bean: Mutex::new(0),
            ace: Mutex::new(None),
            mystery_auto_state: Mutex::new(
                crate::services::mystery_shop_auto::MysteryShopAutoState::default(),
            ),
            last_status_json: Mutex::new(String::new()),
            status_dirty: AtomicBool::new(true),
            last_status_sent_ms: AtomicI64::new(0),
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

    /// 当前登录 GID（0 = 未登录）
    #[must_use]
    pub fn current_gid(&self) -> i64 {
        *self.gid.lock()
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
        // 统一好友 tick 的间隔 = help/steal 两组中较快一组（旧账号迁移对齐 bot，
        // 不改存储格式，读取处直接取 min）
        let (min_sec, max_sec) = match kind {
            "friend" => (i.help_min.min(i.steal_min), i.help_max.min(i.steal_max)),
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
            self.start_mystery_shop_timer(scheduler);

            // 对齐 bot：神秘商人配置变更后 2s 补查一次（不等下一个 10min tick）
            {
                let this = Arc::clone(self);
                scheduler.set_timeout_task(
                    "mystery_shop_after_save",
                    Duration::from_millis(
                        crate::services::mystery_shop_auto::AUTO_BUY_AFTER_SAVE_DELAY_MS,
                    ),
                    Arc::new(move || {
                        let this = Arc::clone(&this);
                        Box::pin(async move {
                            this.check_mystery_shop_once().await;
                        })
                    }),
                );
            }

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
                            let account_id = this.account.id.clone();
                            let planting = this.farm.planting();
                            let _ = crate::infra::automation_lock::run_exclusive_automation_task(
                                &account_id,
                                "fertilizer_immediate",
                                async move {
                                    let gid = *this.gid.lock();
                                    let planting = planting.lock().await;
                                    let _ = planting
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
        self.weather.set_account_id(&self.account.id);
        self.weather.set_own_gid(gid);
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
                            // 对齐 bot：收获后出售走互斥任务（runExclusiveAutomationTask）
                            let warehouse = Arc::clone(&warehouse);
                            let account = harvest_account_id.clone();
                            let _ = crate::infra::automation_lock::run_exclusive_automation_task(
                                &account,
                                "harvest_sell",
                                async move { warehouse.sell_all_fruits().await },
                            )
                            .await;
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

        // 对齐 worker.ts onLoginSuccess：先背包点券/金豆 → 统计基线（互斥）→ 邀请码 → 礼包
        {
            let this = Arc::clone(self);
            let account_id = self.account.id.clone();
            let _ = crate::infra::automation_lock::run_exclusive_automation_task(
                &account_id,
                "login_bag_init",
                async move {
                    if let Ok(bag) = this.warehouse.get_bag().await {
                        let items = crate::services::warehouse::get_bag_items(&bag);
                        let coupon =
                            items.iter().find(|i| i.id == 1002).map(|i| i.count).unwrap_or(0);
                        let gold_bean =
                            items.iter().find(|i| i.id == 1005).map(|i| i.count).unwrap_or(0);
                        *this.coupon.lock() = coupon.max(0);
                        if gold_bean > 0 {
                            *this.gold_bean.lock() = gold_bean;
                        }
                        let st = status_svc::status_data_for(&this.account.id);
                        crate::services::stats::init_stats_with_persistence(
                            &this.account.id,
                            st.gold,
                            st.exp,
                            coupon.max(0),
                        );
                        crate::services::stats::reset_session_gains_for(&this.account.id);
                    }
                },
            )
            .await;
        }

        {
            let invite = crate::services::invite::InviteService::new(self.gateway.clone());
            let account_id = self.account.id.clone();
            let _ = crate::infra::automation_lock::run_exclusive_automation_task(
                &account_id,
                "invite_codes",
                async move { invite.process_invite_codes().await },
            )
            .await;
        }

        if self.auto_on("fertilizer_gift") {
            let warehouse = self.warehouse.clone();
            let _ = crate::infra::automation_lock::run_exclusive_automation_task(
                &self.account.id,
                "fertilizer_gifts_login",
                async move { warehouse.auto_open_fertilizer_gift_packs().await },
            )
            .await;
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

        // 对齐 bot runStartupSequence（b487b0f）：登录期领取串行跑完后才挂
        // farm/friend 主循环与周期定时器，避免启动期任务叠跑、重复领取。
        {
            *self.last_daily_date.lock() = get_local_date_key();
            let this = Arc::clone(self);
            let account_id = this.account.id.clone();
            crate::infra::automation_lock::run_exclusive_automation_task(
                &account_id,
                "daily_routines",
                async move { this.run_daily_routines(true).await },
            )
            .await;
            // bot 启动序列在日更后立即领取一次任务
            let this = Arc::clone(self);
            crate::infra::automation_lock::run_exclusive_automation_task(
                &self.account.id,
                "startup_tasks",
                async move { this.task.check_and_claim_tasks().await },
            )
            .await;
        }
        self.start_farm_ticks(scheduler);
        self.start_fertilizer_buy_timer(scheduler);
        self.start_mystery_shop_timer(scheduler);
        {
            let this = Arc::clone(self);
            scheduler.set_timeout_task(
                "friend_check_bootstrap_applications",
                Duration::from_secs(3),
                Arc::new(move || {
                    let this = this.clone();
                    Box::pin(async move {
                        let own_level = this.own_level();
                        this.friend.check_and_accept_applications(own_level).await;
                    })
                }),
            );
        }
        // 好友宠物每日同步（对齐 bot startFriendCheckLoop 里的
        // startFriendPetSyncTimer，scheduler.ts:473）：启动错峰 90s 后首跑，
        // 轮次链自带节奏自适应；worker 停止（quiesce → stop_check_loop）时中止。
        {
            let friend = Arc::clone(&self.friend);
            let account_id = self.account.id.clone();
            let wl = Arc::clone(self);
            let is_running = Arc::new(move || wl.login_ready() && !wl.shutdown_started());
            crate::services::friend::pet_sync::spawn_friend_pet_sync(
                &friend, account_id, is_running,
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
        self.mark_status_dirty();
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
        self.mark_status_dirty();
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

        // 心跳：每 25s 发 HeartbeatRequest（20s 请求超时）。
        // 对齐 node 最新 keepalive-policy：miss 按心跳请求失败累计，
        // 仅当 miss>=MAX_HEARTBEAT_MISS 且入站静默 >30s 双条件才判死。
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
                let cv_snapshot = client_version.clone();
                let stale_ms = hb_timeout.as_millis() as i64;
                Box::pin(async move {
                    // 对齐 network.ts：phase !== 'online' || !gid 则跳过
                    if gateway.phase() != crate::network::gateway::ConnectionPhase::Online {
                        return;
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
                    // 对齐 network.ts：preventOverlap + sendMsgAsync(20s)；发完即返回，不阻塞 interval
                    let gateway = gateway.clone();
                    let last_resp = last_resp.clone();
                    let miss = miss.clone();
                    let acc_id = acc_id.clone();
                    // 对齐 network.ts:811 每拍实时读 client_version（配置热更立即生效）
                    let cv_for_req = {
                        let rt = crate::config::get_runtime_config();
                        if rt.client_version.is_empty() { cv_snapshot.clone() } else { rt.client_version }
                    };
                    let on_timeout = on_timeout.clone();
                    tokio::spawn(async move {
                        let (_now_start, prev_hb, prev_rx, rebuilding) = {
                            let now = crate::utils::time::now_ms();
                            let last_hb = *last_resp.lock();
                            let last_rx = gateway.last_rx_ms();
                            (now, last_hb, last_rx, gateway.is_rebuilding())
                        };
                        // TSDK 重建期放宽静默阈值到 90s（重建期间 encrypt 短暂失败是正常的）
                        let effective_stale_ms = if rebuilding {
                            stale_ms.max(90_000)
                        } else {
                            stale_ms
                        };
                        match gateway.heartbeat(current_gid, &cv_for_req).await {
                            Ok(_reply) => {
                                *last_resp.lock() = crate::utils::time::now_ms();
                                *miss.lock() = 0;
                            }
                            Err(e) => {
                                let miss_n = {
                                    let mut g = miss.lock();
                                    *g += 1;
                                    *g
                                };
                                // 对齐 network.ts:831-833：RPC 结束后取样（含 20s 等待时间）
                                let now = crate::utils::time::now_ms();
                                let hb_silence = now.saturating_sub(prev_hb);
                                let inbound_silence = now.saturating_sub(prev_rx);
                                tracing::warn!(
                                    account_id = %acc_id,
                                    miss = miss_n,
                                    max = MAX_HEARTBEAT_MISS,
                                    heartbeat_s = hb_silence / 1000,
                                    inbound_s = inbound_silence / 1000,
                                    pending = gateway.pending_count(),
                                    pending_methods = ?gateway.pending_methods(),
                                    error = %e,
                                    "心跳未响应"
                                );
                                // 判死需三条件：miss 达上限 + 入站静默超阈值 + 无在途请求
                                //（巨型回包下载期间 pending>0，连接仍在收数据，不杀）
                                let pending = gateway.pending_count();
                                if !heartbeat_should_force_disconnect(
                                    miss_n,
                                    MAX_HEARTBEAT_MISS,
                                    inbound_silence,
                                    effective_stale_ms,
                                    pending,
                                ) {
                                    if pending > 0 && inbound_silence > effective_stale_ms {
                                        tracing::debug!(
                                            account_id = %acc_id,
                                            pending,
                                            inbound_s = inbound_silence / 1000,
                                            "心跳超时但仍有在途请求，暂不判死"
                                        );
                                    }
                                    return;
                                }
                                tracing::error!(account_id = %acc_id, "连续心跳超时且连接无入站数据，触发重连");
                                crate::services::panel_log::log(
                                    &acc_id,
                                    "心跳",
                                    format!(
                                        "连接可能已断开 ({}s 无入站数据，连续 {} 次心跳失败)",
                                        inbound_silence / 1000, miss_n
                                    ),
                                    crate::constants::PanelEvent::HeartbeatTimeout,
                                    Some(serde_json::json!({
                                        "module": "heartbeat",
                                        "isWarn": true,
                                        "inboundSilenceMs": inbound_silence,
                                        "missCount": miss_n,
                                    })),
                                );
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
        let (friend_min, friend_max) = self.interval_range_ms("friend");
        let mut next = self.next_runs.lock();
        next.farm_at = now + random_interval_ms(farm_min, farm_max) as i64;
        next.friend_at = now + random_interval_ms(friend_min, friend_max) as i64;
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
        self.friend_tick_running.store(false, Ordering::Release);
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
            let friend = if g.friend_at > 0 { g.friend_at } else { now + 1000 };
            farm.min(friend)
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

    /// 神秘商人监控 tick（对齐 node `checkMysteryShopTick`）
    async fn check_mystery_shop_once(self: &Arc<Self>) {
        if !self.login_ready() {
            return;
        }
        let automation =
            crate::models::store::account_config::get_automation(Some(&self.account.id));
        if !crate::services::mystery_shop_auto::is_watch_enabled(&automation) {
            return;
        }
        // 对齐 bot：神秘商店 tick 在互斥任务里执行（runExclusiveAutomationTask）
        let this = Arc::clone(self);
        crate::infra::automation_lock::run_exclusive_automation_task(
            &self.account.id,
            "mystery_shop",
            async move {
                let commerce = Arc::new(crate::services::commerce::CommerceService::new(
                    this.mall.clone(),
                    this.mystery_shop.clone(),
                    this.warehouse.clone(),
                ));
                // 克隆去重状态，避免 guard 跨 await（Future 需要 Send）
                let mut state = this.mystery_auto_state.lock().clone();
                let account_id = this.account.id.clone();
                let outcome = crate::services::mystery_shop_auto::check_tick(
                    &commerce,
                    &automation,
                    &mut state,
                    &account_id,
                )
                .await;
                *this.mystery_auto_state.lock() = state;
                if let Some((title, content)) = outcome.push {
                    // 推送走 worker 事件总线（面板通知 + relogin_reminder 通知链路）
                    let _ = this.event_tx.send(WorkerEvent::Notify {
                        account_id: account_id.clone(),
                        account_name: this.account.display_name.clone(),
                        title,
                        message: content,
                    });
                }
            },
        )
        .await;
    }

    /// 神秘商人监控定时器：登录后 10s 首查，之后每 10min tick（对齐 node `startMysteryShopTimer`）
    pub fn start_mystery_shop_timer(self: &Arc<Self>, scheduler: &Scheduler) {
        let this = Arc::clone(self);
        scheduler.set_timeout_task(
            "mystery_shop_initial",
            Duration::from_millis(crate::services::mystery_shop_auto::AUTO_BUY_INITIAL_DELAY_MS),
            Arc::new(move || {
                let this = Arc::clone(&this);
                Box::pin(async move {
                    this.check_mystery_shop_once().await;
                })
            }),
        );
        let this = Arc::clone(self);
        scheduler.set_interval_task(
            "mystery_shop_timer",
            Duration::from_millis(crate::services::mystery_shop_auto::AUTO_BUY_CHECK_INTERVAL_MS),
            Arc::new(move || {
                let this = Arc::clone(&this);
                Box::pin(async move {
                    this.check_mystery_shop_once().await;
                })
            }),
        );
    }

    /// 对齐 TS `runUnifiedTick`：串行执行，避免并发请求过多导致超时
    async fn run_unified_tick(self: &Arc<Self>) {
        if !self.login_ready() {
            return;
        }
        if self.unified_tick_running.swap(true, Ordering::AcqRel) {
            return;
        }
        let _guard = FlagGuard(&self.unified_tick_running);
        let now = now_ms();
        let (due_farm, due_friend) = {
            let guard = self.next_runs.lock();
            (
                guard.farm_at > 0 && now >= guard.farm_at,
                guard.friend_at > 0 && now >= guard.friend_at,
            )
        };
        let mut changed = false;
        if due_farm {
            self.run_farm_tick().await;
            changed = true;
        }
        if due_friend {
            self.run_friend_tick().await;
            changed = true;
        }
        if changed {
            self.mark_status_dirty();
        }
    }

    /// 触发 farm tick（对外暴露给 on_login_success 启动独立 task）
    pub async fn run_farm_tick(self: &Arc<Self>) {
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
            // 对齐 bot：farm tick 整体在互斥任务里执行（runExclusiveAutomationTask）
            let this = Arc::clone(self);
            let account_id = self.account.id.clone();
            crate::infra::automation_lock::run_exclusive_automation_task(
                &account_id,
                "farm_tick",
                async move {
                    // 静默时段默认只停帮助/偷菜；好友静默开启 continueFarm=false 时本田巡查也停
                    // （对齐 bot checkFarm → inFarmQuietHours）
                    let farm_quiet =
                        crate::services::friend::visit_strategy::in_farm_quiet_hours_for(
                            Some(&this.account.id),
                            None,
                        );
                    if this.auto_on("farm") && !farm_quiet {
                        let _ = this.farm.check_farm().await;
                    }
                    if this.auto_on("task") {
                        let _ = this.task.check_and_claim_tasks().await;
                    }
                    if this.auto_on("fertilizer_gift") {
                        let _ = this.warehouse.auto_open_fertilizer_gift_packs().await;
                    }
                    this.sync_status();
                },
            )
            .await;
        }
    }

    /// 触发统一好友 tick（帮助 + 偷菜 + 捣乱一次做完，对齐 bot `checkFriends` +
    /// visit-plan：每位好友只进一次农场）。
    ///
    /// - 门控：好友自动化总开关（friend）——偷/帮/捣乱的细分开关在
    ///   `check_friends_unified` 的计划阶段逐位判定；
    /// - 静默时段跳过；偷到 > 0 时沿用「sleep 800ms → sell_all_fruits」。
    pub async fn run_friend_tick(self: &Arc<Self>) {
        if !self.login_ready() {
            return;
        }
        if !self.auto_on("friend") {
            let (min_ms, max_ms) = self.interval_range_ms("friend");
            self.next_runs.lock().friend_at = now_ms() + random_interval_ms(min_ms, max_ms) as i64;
            return;
        }
        if self.friend_tick_running.swap(true, Ordering::AcqRel) {
            return;
        }
        let (min_ms, max_ms) = self.interval_range_ms("friend");
        let _guard = NextSlotGuard {
            flag: &self.friend_tick_running,
            next: &self.next_runs,
            kind: "friend",
            min_ms,
            max_ms,
        };
        if crate::services::friend::visit_strategy::in_friend_quiet_hours_for(
            Some(&self.account.id),
            None,
        ) {
            return;
        }
        // 对齐 bot：friend tick 整体在互斥任务里执行（runExclusiveAutomationTask）
        let this = Arc::clone(self);
        let account_id = self.account.id.clone();
        crate::infra::automation_lock::run_exclusive_automation_task(
            &account_id,
            "friend_tick",
            async move {
                let stolen = this.friend.check_friends_unified(&this.account.id).await.unwrap_or(0);
                if stolen > 0 {
                    tokio::time::sleep(Duration::from_millis(800)).await;
                    let _ = this.warehouse.sell_all_fruits().await;
                }
                this.sync_status();
            },
        )
        .await;
    }

    /// 跑每日任务（对齐 bot：整个日更在互斥任务里执行，嵌套调用直接内联）
    pub async fn run_daily_routines(self: &Arc<Self>, force: bool) {
        if !self.login_ready() && !force {
            return;
        }
        let this = Arc::clone(self);
        crate::infra::automation_lock::run_exclusive_automation_task(
            &self.account.id,
            "daily_routines",
            async move {
                // email
                let _ = this.email.check_and_claim_emails(force).await;
                // share
                let _ = this.share.check_daily_share_status(force).await;
                // monthcard
                let _ = this.monthcard.perform_daily_month_card_gift(force).await;
                // 商城免费礼包
                let _ = this.mall.buy_free_gifts(force).await;
                // qqvip
                let _ = this.qqvip.perform_daily_vip_gift(force).await;
            },
        )
        .await;
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
        _lands: Vec<crate::proto::generated::gamepb::plantpb::LandInfo>,
    ) {
        self.mark_status_dirty();
        let my = *self.gid.lock();
        if host_gid > 0 && my > 0 && host_gid != my {
            // 对齐 bot network.ts:452-464：好友田推送直接丢弃（不触发任何拉取）。
            // 事件驱动的好友田 GetGameFriends 拉取是 rust 独有模式，bot 没有。
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

    /// 对齐 bot `onFarmSocialEventsChangedPush`：青蛙等农场级社交事件推送，
    /// 与土地推送同路径（farm_push 门控 + 500ms 节流）触发巡查清理。
    pub fn on_farm_social_events_push(self: &Arc<Self>, changed_count: usize) {
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
            format!("收到推送: {changed_count}个农场社交事件，检查中..."),
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
    pub fn weather(&self) -> &Arc<crate::services::weather_activity::WeatherActivityService> {
        &self.weather
    }

    /// 登录后刷新活动窗口（对齐 bot worker.ts:568-572 的 refreshActivityWindows，
    /// 发 activitypb.ActivityService.List）。失败忽略。
    pub async fn refresh_activity_windows(&self) {
        let _ = self.activity_center.get_activity_center_snapshot().await;
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
    /// 自己的角色 GID（登录成功后写入；未登录为 0）
    #[must_use]
    pub fn own_gid(&self) -> i64 {
        *self.gid.lock()
    }
    /// 自己的等级（来自登录 BasicInfo / BasicNotify 更新的状态）
    #[must_use]
    pub fn own_level(&self) -> i64 {
        crate::infra::status::status_data_for(&self.account.id).level
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
    // 对齐 bot worker.ts:182-188 Math.random() 均匀分布（手写 LCG 分布质量差）
    let range = (max_sec - min_sec + 1) as u64;
    let sec = min_sec as u64 + crate::utils::random::random_u64(0, range - 1);
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
    fn heartbeat_kill_requires_miss_silence_and_no_pending() {
        let max = 3_u32;
        let stale = 30_000_i64;
        // miss 不足
        assert!(!heartbeat_should_force_disconnect(2, max, 60_000, stale, 0));
        // 静默不足
        assert!(!heartbeat_should_force_disconnect(3, max, 20_000, stale, 0));
        // 经典判死：miss 达标 + 静默超阈值 + 无在途
        assert!(heartbeat_should_force_disconnect(3, max, 60_000, stale, 0));
        // 巨型回包下载中：有在途请求 → 不杀
        assert!(!heartbeat_should_force_disconnect(3, max, 60_000, stale, 2));
        // 保护窗封顶：静默超过 2 分钟，即使有在途也判死
        assert!(heartbeat_should_force_disconnect(
            3,
            max,
            PENDING_DEFER_MAX_SILENCE_MS + 1,
            stale,
            5
        ));
        assert!(!heartbeat_should_force_disconnect(3, max, PENDING_DEFER_MAX_SILENCE_MS, stale, 5));
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
            assert!(next.friend_at > now);
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
