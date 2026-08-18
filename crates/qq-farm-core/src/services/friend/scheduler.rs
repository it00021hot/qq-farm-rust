//! 好友调度循环。
//!
//! 对应原 `core/src/services/friend/scheduler.ts`（858 行）。
//!
//! ## 阶段 1D 范围
//!
//! - 框架：start_check_loop / stop_check_loop / sync_friends
//! - 单次流程：同步 GID → 选 batch → 帮（避免重复）+ 偷（避免重复）
//! - 真实去重：通过 [`RecentHelpCache`] 记录已帮的 land
//! - 黑名单过滤：通过 [`VisitStrategy`]（重构版）按 host_gid 过滤
//!
//! ## 阶段 1D.2 范围（待办）
//!
//! - enter_farm / leave_farm 完整流程
//! - 安静时段（quiet hours）
//! - 好友黑名单持久化
//! - 帮 / 偷 / 巡 分批预算
//! - gift / wish 流程

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::runtime::scheduler::Scheduler;
use crate::services::friend::api::FriendApi;
use crate::services::friend::gid_manager::GidManager;
use crate::services::friend::visit_strategy::{
    analyze_friend_lands, is_enter_farm_banned_error, is_transient_network_error, now_ms,
    parse_rpc_error_code, steal_lands_with_reward_log, FriendSummary, HelpState, LandSnapshot,
    RecentHelpCache, HELP_RESULT_TTL_MS,
};

/// 巡访类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitKind {
    Help,
    Steal,
}

/// 访问策略（黑名单 + 批大小 + 访问记录）
pub struct VisitStrategy {
    blacklist: Arc<Mutex<HashSet<i64>>>,
    visited: Arc<Mutex<HashSet<i64>>>,
    batch_size: usize,
    recent_help: Arc<RecentHelpCache>,
}

impl VisitStrategy {
    /// 创建
    #[must_use]
    pub fn new(batch_size: usize) -> Self {
        Self {
            blacklist: Arc::new(Mutex::new(HashSet::new())),
            visited: Arc::new(Mutex::new(HashSet::new())),
            batch_size,
            recent_help: Arc::new(RecentHelpCache::new()),
        }
    }

    /// 加黑名单
    pub fn add_blacklist(&self, gid: i64) {
        self.blacklist.lock().insert(gid);
    }

    /// 是否黑名单
    #[must_use]
    pub fn is_blacklisted(&self, gid: i64) -> bool {
        self.blacklist.lock().contains(&gid)
    }

    /// 黑名单大小
    #[must_use]
    pub fn blacklist_count(&self) -> usize {
        self.blacklist.lock().len()
    }

    /// 选择本批（去重 + 黑名单 + 限流）
    #[must_use]
    pub fn select_batch(&self, candidates: &[i64]) -> Vec<i64> {
        let blacklist = self.blacklist.lock();
        let mut visited = self.visited.lock();
        let mut out = Vec::with_capacity(self.batch_size);
        for &gid in candidates {
            if out.len() >= self.batch_size {
                break;
            }
            if blacklist.contains(&gid) {
                continue;
            }
            if !visited.insert(gid) {
                continue;
            }
            out.push(gid);
        }
        out.sort_unstable();
        out
    }

    /// 标记已访问
    pub fn mark_visited(&self, _kind: VisitKind, gid: i64) {
        self.visited.lock().insert(gid);
    }

    /// 清空访问记录
    pub fn clear_visited(&self) {
        self.visited.lock().clear();
    }

    /// RecentHelp 缓存（用于 land-level 去重）
    #[must_use]
    pub fn recent_help(&self) -> &RecentHelpCache {
        &self.recent_help
    }

    /// 批大小
    #[must_use]
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }
}

/// 好友服务
pub struct FriendService {
    gateway: Arc<Gateway>,
    api: FriendApi,
    gid_manager: Arc<GidManager>,
    strategy: Arc<VisitStrategy>,
    scheduler: Scheduler,
    host_gid: Arc<Mutex<i64>>,
    account_id: Arc<Mutex<String>>,
    current_loop: Arc<Mutex<Option<CancellationToken>>>,
    event_tx: broadcast::Sender<FriendEvent>,
    bad_ran_on_startup: AtomicBool,
    operation_limits: Arc<Mutex<HashMap<i64, OpLimitEntry>>>,
    bad_operation_limit_reached: AtomicBool,
    /// 对齐 TS `lastResetDate`
    last_reset_date: Mutex<String>,
    /// 对齐 TS `helpAutoDisabledByLimit`
    help_auto_disabled: AtomicBool,
    /// 对齐 TS `isCheckingFriends`
    is_checking: AtomicBool,
    /// 对齐 TS `externalSchedulerMode`
    external_scheduler: AtomicBool,
    /// 对齐 TS `friendsListCache`（仅面板 HTTP，巡查不走这份缓存）
    friends_list_cache: Mutex<Option<(u64, Vec<serde_json::Value>)>>,
    /// 偷菜空访标记：gid → 当时 GetAll 的 steal_plant_num。
    /// 列表仍报「有可偷」但我进场无可偷时，避免每个 steal tick 空转重入。
    /// steal_plant_num 变化（新成熟/他人偷完）后自动解除。
    steal_noop_markers: Mutex<HashMap<i64, i64>>,
    /// 偷菜成功后暂清零可偷气泡，直到游戏 GetAll 追上。
    steal_cleared_gids: Mutex<HashSet<i64>>,
}

#[derive(Debug, Clone, Copy, Default)]
struct OpLimitEntry {
    day_times: i64,
    day_times_limit: i64,
    day_exp_times: i64,
    day_exp_times_limit: i64,
}

const OP_NAMES: [(i64, &str); 8] = [
    (10001, "浇水"),
    (10002, "除虫"),
    (10003, "捣乱共享额度"),
    (10004, "放虫"),
    (10005, "帮助操作 #10005"),
    (10006, "帮助操作 #10006"),
    (10007, "帮助操作 #10007"),
    (10008, "铲除"),
];
const BAD_SHARED_LIMIT_ID: i64 = 10003;

/// 好友服务事件
#[derive(Debug, Clone)]
pub enum FriendEvent {
    /// 巡访完成
    Checked { batch_size: usize, helped: usize, stolen: usize, banned: usize },
    /// GID 列表已同步
    GidsSynced { count: usize },
    /// 进入农场被封（黑名单）
    FarmBanned { host_gid: i64 },
    /// 出错
    Error { message: String },
}

impl FriendService {
    /// 创建
    #[must_use]
    pub fn new(gateway: Arc<Gateway>, batch_size: usize) -> Self {
        let api = FriendApi::new(gateway.clone());
        let (event_tx, _) = broadcast::channel(64);
        Self {
            gateway,
            api,
            gid_manager: Arc::new(GidManager::new()),
            strategy: Arc::new(VisitStrategy::new(batch_size)),
            scheduler: Scheduler::new("friend-service"),
            host_gid: Arc::new(Mutex::new(0)),
            account_id: Arc::new(Mutex::new(String::new())),
            current_loop: Arc::new(Mutex::new(None)),
            event_tx,
            bad_ran_on_startup: AtomicBool::new(false),
            operation_limits: Arc::new(Mutex::new(HashMap::new())),
            bad_operation_limit_reached: AtomicBool::new(false),
            last_reset_date: Mutex::new(String::new()),
            help_auto_disabled: AtomicBool::new(false),
            is_checking: AtomicBool::new(false),
            external_scheduler: AtomicBool::new(false),
            friends_list_cache: Mutex::new(None),
            steal_noop_markers: Mutex::new(HashMap::new()),
            steal_cleared_gids: Mutex::new(HashSet::new()),
        }
    }

    /// GidManager
    #[must_use]
    pub fn gid_manager(&self) -> Arc<GidManager> {
        self.gid_manager.clone()
    }

    /// VisitStrategy
    #[must_use]
    pub fn strategy(&self) -> Arc<VisitStrategy> {
        self.strategy.clone()
    }

    /// 订阅事件
    pub fn subscribe(&self) -> broadcast::Receiver<FriendEvent> {
        self.event_tx.subscribe()
    }

    /// 设置 host_gid
    pub fn set_host_gid(&self, gid: i64) {
        *self.host_gid.lock() = gid;
    }

    pub fn set_account_id(&self, account_id: &str) {
        *self.account_id.lock() = account_id.to_string();
        self.api.set_account_id(account_id);
        self.check_daily_reset();
    }

    /// 对齐 TS `checkDailyReset`
    pub fn check_daily_reset(&self) {
        let today = beijing_date_key();
        let mut last = self.last_reset_date.lock();
        if *last == today {
            return;
        }
        if !last.is_empty() {
            tracing::info!("跨日重置，清空操作限制缓存");
        }
        self.operation_limits.lock().clear();
        if self.help_auto_disabled.swap(false, Ordering::AcqRel) {
            let acc = self.account_id.lock().clone();
            crate::services::panel_log::log(
                &acc,
                "好友",
                "新的一天已开始，自动恢复帮忙操作功能",
                crate::constants::PanelEvent::FriendCycle,
                Some(serde_json::json!({
                    "module": "friend",
                    "result": "ok",
                })),
            );
        }
        let stopped = load_bad_daily_stop(&self.account_id.lock(), &today);
        self.bad_operation_limit_reached.store(stopped, Ordering::Release);
        *last = today;
    }

    /// 对齐 TS `isBadOperationLimitReached`
    #[must_use]
    pub fn is_bad_operation_limit_reached(&self) -> bool {
        self.check_daily_reset();
        self.bad_operation_limit_reached.load(Ordering::Acquire)
    }

    /// 对齐 TS `getRemainingBadOperationTimes`
    #[must_use]
    pub fn get_remaining_bad_operation_times(&self) -> i64 {
        self.check_daily_reset();
        if self.bad_operation_limit_reached.load(Ordering::Acquire) {
            return 0;
        }
        let map = self.operation_limits.lock();
        let Some(limit) = map.get(&BAD_SHARED_LIMIT_ID) else {
            return 999;
        };
        if limit.day_times_limit <= 0 {
            return 999;
        }
        (limit.day_times_limit - limit.day_times).max(0)
    }

    /// 对齐 TS `markBadOperationLimitReached`
    pub fn mark_bad_operation_limit_reached(&self, method: &str) -> bool {
        self.check_daily_reset();
        if self.bad_operation_limit_reached.swap(true, Ordering::AcqRel) {
            return false;
        }
        let today = self.last_reset_date.lock().clone();
        let today = if today.is_empty() { beijing_date_key() } else { today };
        if let Err(e) = persist_bad_daily_stop(&self.account_id.lock(), &today) {
            tracing::warn!("保存当日捣乱停用状态失败: {e}");
        }
        let acc = self.account_id.lock().clone();
        crate::services::panel_log::log(
            &acc,
            "好友",
            "今日放虫/放草次数已达上限，停止两类操作",
            crate::constants::PanelEvent::BadActionLimit,
            Some(serde_json::json!({
                "module": "friend",
                "result": "limit",
                "code": 1001046,
                "method": method,
            })),
        );
        true
    }

    /// 好友操作限额（对齐 TS `getOperationLimits`）
    #[must_use]
    pub fn get_operation_limits(&self) -> serde_json::Value {
        let map = self.operation_limits.lock();
        let mut result = serde_json::Map::new();
        for (id, name) in OP_NAMES {
            if let Some(limit) = map.get(&id) {
                let remaining = self.remaining_times_locked(id, limit);
                result.insert(
                    id.to_string(),
                    serde_json::json!({
                        "name": name,
                        "dayTimes": limit.day_times,
                        "dayTimesLimit": limit.day_times_limit,
                        "dayExpTimes": limit.day_exp_times,
                        "dayExpTimesLimit": limit.day_exp_times_limit,
                        "remaining": remaining,
                    }),
                );
            }
        }
        serde_json::Value::Object(result)
    }

    /// 对齐 TS `updateOperationLimits`
    pub fn update_operation_limits(
        &self,
        limits: &[crate::proto::generated::gamepb::plantpb::OperationLimit],
    ) {
        if limits.is_empty() {
            return;
        }
        let mut map = self.operation_limits.lock();
        for limit in limits {
            if limit.id <= 0 {
                continue;
            }
            let data = OpLimitEntry {
                day_times: limit.day_times,
                day_times_limit: limit.day_times_lt,
                day_exp_times: limit.day_exp_times,
                day_exp_times_limit: limit.day_ex_times_lt,
            };
            map.insert(limit.id, data);
            if limit.id == BAD_SHARED_LIMIT_ID
                && data.day_times_limit > 0
                && data.day_times >= data.day_times_limit
            {
                self.mark_bad_operation_limit_reached("operation_limit");
            }
        }
    }

    fn remaining_times_locked(&self, op_id: i64, limit: &OpLimitEntry) -> i64 {
        if (op_id == BAD_SHARED_LIMIT_ID || op_id == 10004)
            && self.bad_operation_limit_reached.load(Ordering::Acquire)
        {
            return 0;
        }
        if limit.day_times_limit <= 0 {
            return 999;
        }
        (limit.day_times_limit - limit.day_times).max(0)
    }

    /// 底层 API
    #[must_use]
    pub fn api(&self) -> &FriendApi {
        &self.api
    }

    /// 对齐 TS `canGetExp`
    #[must_use]
    pub fn can_get_exp(&self, op_id: i64) -> bool {
        let map = self.operation_limits.lock();
        let Some(limit) = map.get(&op_id) else {
            return false; // 没有限制信息，保守起见不帮助
        };
        if limit.day_exp_times_limit <= 0 {
            return true;
        }
        limit.day_exp_times < limit.day_exp_times_limit
    }

    /// 对齐 TS `canGetExpByCandidates`
    #[must_use]
    pub fn can_get_exp_by_candidates(&self, op_ids: &[i64]) -> bool {
        op_ids.iter().any(|id| self.can_get_exp(*id))
    }

    /// 对齐 TS `isHelpExpLimitReached`
    #[must_use]
    pub fn is_help_exp_limit_reached(&self) -> bool {
        self.help_auto_disabled.load(Ordering::Acquire)
    }

    /// 对齐 TS `autoDisableHelpByExpLimit`
    pub fn auto_disable_help_by_exp_limit(&self) {
        if self.help_auto_disabled.swap(true, Ordering::AcqRel) {
            return;
        }
        let acc = self.account_id.lock().clone();
        crate::services::panel_log::log(
            &acc,
            "好友",
            "今日帮助经验已达上限，自动停止帮忙",
            crate::constants::PanelEvent::FriendCycle,
            Some(serde_json::json!({
                "module": "friend",
                "result": "ok",
            })),
        );
    }

    /// 对齐 TS `checkAndAcceptApplications` / `onFriendApplicationReceived`
    pub async fn accept_friend_applications(&self, gids: Vec<i64>, names: &[String]) {
        if gids.is_empty() {
            return;
        }
        let acc = self.account_id.lock().clone();
        if !names.is_empty() {
            crate::services::panel_log::log(
                &acc,
                "申请",
                format!("收到 {} 个好友申请: {}", names.len(), names.join(", ")),
                crate::constants::PanelEvent::FriendRequest,
                Some(serde_json::json!({ "module": "friend"})),
            );
        }
        match self.api.accept_applications(gids).await {
            Ok(()) => crate::services::panel_log::log(
                &acc,
                "申请",
                "已同意好友申请",
                crate::constants::PanelEvent::AcceptFriendRequest,
                Some(serde_json::json!({ "module": "friend"})),
            ),
            Err(e) => crate::services::panel_log::log_warn(
                &acc,
                "申请",
                format!("同意失败: {e}"),
                crate::constants::PanelEvent::AcceptFriendRequest,
                Some(serde_json::json!({ "module": "friend", "event": "同意好友申请" })),
            ),
        }
    }

    /// 登录后拉一次待处理申请并同意（QQ 平台可能不支持，失败忽略）
    pub async fn check_and_accept_applications(&self) {
        let Ok(apps) = self.api.get_applications().await else {
            return;
        };
        if apps.is_empty() {
            return;
        }
        let names: Vec<String> = apps
            .iter()
            .map(|(gid, name)| if name.is_empty() { format!("GID:{gid}") } else { name.clone() })
            .collect();
        let gids: Vec<i64> = apps.into_iter().map(|(g, _)| g).collect();
        let acc = self.account_id.lock().clone();
        crate::services::panel_log::log(
            &acc,
            "申请",
            format!("发现 {} 个待处理申请: {}", names.len(), names.join(", ")),
            crate::constants::PanelEvent::PendingFriendRequest,
            Some(serde_json::json!({ "module": "friend"})),
        );
        self.accept_friend_applications(gids, &[]).await;
    }

    /// 仅帮忙巡查（对齐 TS `checkFriends({onlyHelp: true})`）
    pub async fn check_friends_help(&self, account_id: &str) -> Result<usize> {
        self.visit_batch(account_id, VisitKind::Help).await
    }

    /// 仅偷菜巡查（对齐 TS `checkFriends({onlySteal: true})`）
    pub async fn check_friends_steal(&self, account_id: &str) -> Result<usize> {
        self.visit_batch(account_id, VisitKind::Steal).await
    }

    /// 对齐 TS `startFriendCheckLoop({ externalScheduler: true })`
    pub fn set_external_scheduler(&self, enabled: bool) {
        self.external_scheduler.store(enabled, Ordering::Release);
        if enabled {
            self.stop_check_loop();
        }
    }

    async fn visit_batch(&self, account_id: &str, kind: VisitKind) -> Result<usize> {
        if self.is_checking.swap(true, Ordering::AcqRel) {
            return Ok(0);
        }
        struct Guard<'a>(&'a AtomicBool);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _guard = Guard(&self.is_checking);
        self.visit_batch_inner(account_id, kind).await
    }

    async fn visit_batch_inner(&self, account_id: &str, kind: VisitKind) -> Result<usize> {
        let my_gid = *self.host_gid.lock();
        if my_gid == 0 {
            return Ok(0);
        }
        if !crate::services::automation::is_automation_on_for(account_id, "friend") {
            return Ok(0);
        }
        if crate::services::friend::visit_strategy::in_friend_quiet_hours_for(
            Some(account_id),
            None,
        ) {
            return Ok(0);
        }
        let friends = match self.api.get_all_game_friends().await {
            Ok(f) => f,
            Err(e) => {
                let raw = e.to_string();
                let msg = raw.strip_prefix("network error: ").unwrap_or(&raw);
                crate::services::panel_log::log_warn(
                    account_id,
                    "好友",
                    format!("巡查异常: {msg}"),
                    crate::constants::PanelEvent::FriendCycle,
                    Some(serde_json::json!({
                        "module": "friend",
                        "result": "error",
                    })),
                );
                return Err(e);
            }
        };
        self.gid_manager.update(friends.iter().map(|f| f.gid).collect());
        if friends.is_empty() {
            crate::services::panel_log::log(
                account_id,
                "好友",
                "没有好友",
                crate::constants::PanelEvent::FriendScan,
                Some(serde_json::json!({
                    "module": "friend",
                    "result": "empty",
                })),
            );
            return Ok(0);
        }

        let cfg_blacklist: HashSet<i64> =
            crate::models::store::account_config::get_friend_blacklist(Some(account_id))
                .into_iter()
                .collect();
        let mut steal_friends: Vec<FriendSummary> = Vec::new();
        let mut help_friends: Vec<(FriendSummary, i64)> = Vec::new();
        let mut seen = HashSet::new();
        for f in friends {
            if f.gid == my_gid || f.gid <= 0 || !seen.insert(f.gid) {
                continue;
            }
            if cfg_blacklist.contains(&f.gid)
                || self.strategy.is_blacklisted(f.gid)
                || crate::services::friend::visit_strategy::is_friend_blacklisted(account_id, f.gid)
                || crate::services::friend::visit_strategy::is_known_friend_gid_invalid(f.gid)
            {
                continue;
            }
            let summary = crate::services::friend::visit_strategy::game_friend_to_summary(f);
            let steal_num = summary.plant.as_ref().map(|p| p.steal_num).unwrap_or(0);
            let help_need =
                summary.plant.as_ref().map(|p| p.dry_num + p.weed_num + p.insect_num).unwrap_or(0);
            if kind == VisitKind::Steal && steal_num > 0 {
                // 上次进场无可偷且 steal_plant_num 未变 → 跳过，避免空转刷日志
                let skip_noop = self
                    .steal_noop_markers
                    .lock()
                    .get(&summary.gid)
                    .copied()
                    .is_some_and(|prev| prev == steal_num);
                if !skip_noop {
                    steal_friends.push(summary);
                }
            } else if kind == VisitKind::Help && help_need > 0 {
                help_friends.push((summary, help_need));
            }
        }

        steal_friends.sort_by(|a, b| {
            let sa = a.plant.as_ref().map(|p| p.steal_num).unwrap_or(0);
            let sb = b.plant.as_ref().map(|p| p.steal_num).unwrap_or(0);
            sb.cmp(&sa)
        });
        help_friends.sort_by(|a, b| b.1.cmp(&a.1));

        let mut total = crate::services::friend::visit_strategy::TotalActions::default();
        let recent = self.strategy.recent_help();

        if kind == VisitKind::Steal && !steal_friends.is_empty() {
            // bot 侧该日志已注释；保留 debug 级噪音会让人以为卡死在「开始批量偷菜」
            for friend in &steal_friends {
                let list_steal_num = friend.plant.as_ref().map(|p| p.steal_num).unwrap_or(0);
                let visit = crate::services::friend::visit_strategy::visit_friend_for_steal(
                    &self.api, recent, friend, &mut total, my_gid, account_id,
                )
                .await;
                match visit {
                    Some(r) if r.acted => {
                        self.steal_noop_markers.lock().remove(&friend.gid);
                        self.mark_friend_steal_cleared(friend.gid);
                    }
                    // 已进场但无可偷 / 偷失败，或黑名单滤光（None）：记下当前列表指标，避免每 tick 重入
                    Some(r) if r.entered && !r.acted => {
                        self.steal_noop_markers.lock().insert(friend.gid, list_steal_num);
                    }
                    None => {
                        self.steal_noop_markers.lock().insert(friend.gid, list_steal_num);
                    }
                    _ => {}
                }
                crate::utils::random::random_delay(500, 800).await;
            }
        }

        if kind == VisitKind::Help && !help_friends.is_empty() {
            crate::services::panel_log::log(
                account_id,
                "好友",
                format!("开始批量帮助，共 {} 个好友需要帮助", help_friends.len()),
                crate::constants::PanelEvent::VisitFriend,
                Some(serde_json::json!({
                    "module": "friend",
                    "count": help_friends.len(),
                })),
            );
            for (i, (friend, _)) in help_friends.iter().enumerate() {
                if crate::services::automation::is_automation_on_for(
                    account_id,
                    "friend_help_exp_limit",
                ) && self.is_help_exp_limit_reached()
                {
                    crate::services::panel_log::log(
                        account_id,
                        "好友",
                        "批量帮助中断：经验已达上限",
                        crate::constants::PanelEvent::FriendCycle,
                        Some(serde_json::json!({
                            "module": "friend",
                            "reason": "exp_limit",
                        })),
                    );
                    break;
                }
                crate::services::panel_log::log(
                    account_id,
                    "好友",
                    format!("批量帮助第 {}/{} 个好友: {}", i + 1, help_friends.len(), friend.name),
                    crate::constants::PanelEvent::VisitFriend,
                    Some(serde_json::json!({
                        "module": "friend",
                        "index": i + 1,
                        "total": help_friends.len(),
                        "friendName": friend.name,
                    })),
                );
                let can_exp = self.can_get_exp_by_candidates(&[10005, 10006, 10007]);
                let _ = crate::services::friend::visit_strategy::visit_friend_for_help(
                    &self.api,
                    recent,
                    friend,
                    &mut total,
                    my_gid,
                    account_id,
                    false,
                    &self.help_auto_disabled,
                    can_exp,
                )
                .await;
                crate::utils::random::random_delay(500, 800).await;
            }
        }

        let mut summary: Vec<String> = Vec::new();
        if total.steal > 0 {
            summary.push(format!("偷{}", total.steal));
        }
        if total.farming > 0 {
            summary.push(format!("一键务农{}", total.farming));
        }
        if total.put_bug > 0 {
            summary.push(format!("放虫{}", total.put_bug));
        }
        if total.put_weed > 0 {
            summary.push(format!("放草{}", total.put_weed));
        }
        if !summary.is_empty() {
            crate::services::panel_log::log(
                account_id,
                "好友",
                format!("巡查完成 → {}", summary.join("/")),
                crate::constants::PanelEvent::FriendCycle,
                Some(serde_json::json!({
                    "module": "friend",
                    "result": "ok",
                    "visited": steal_friends.len() + help_friends.len(),
                    "summary": summary,
                })),
            );
        }

        Ok(if kind == VisitKind::Steal { total.steal } else { total.farming })
    }

    /// 启动时执行一次放虫放草（对齐 TS `runBadOnceOnStartup`）
    pub async fn run_bad_once_on_startup(&self, account_id: &str) -> Result<usize> {
        if self.bad_ran_on_startup.swap(true, Ordering::AcqRel) {
            return Ok(0);
        }
        if !crate::services::automation::is_automation_on_for(account_id, "friend_bad") {
            return Ok(0);
        }
        if self.is_bad_operation_limit_reached() {
            return Ok(0);
        }
        let my_gid = *self.host_gid.lock();
        if my_gid == 0 {
            return Ok(0);
        }
        let friends = match self.api.get_all_game_friends().await {
            Ok(f) => f,
            Err(_) => return Ok(0),
        };
        let blacklist =
            crate::models::store::account_config::get_friend_blacklist(Some(account_id));
        let mut bad_friends: Vec<crate::services::friend::visit_strategy::FriendSummary> =
            Vec::new();
        let mut seen = std::collections::HashSet::new();
        for f in friends {
            let summary = crate::services::friend::visit_strategy::game_friend_to_summary(f);
            if summary.gid == my_gid || summary.gid <= 0 || !seen.insert(summary.gid) {
                continue;
            }
            if blacklist.contains(&summary.gid)
                || self.strategy.is_blacklisted(summary.gid)
                || crate::services::friend::visit_strategy::is_friend_blacklisted(
                    account_id,
                    summary.gid,
                )
            {
                continue;
            }
            let idle = summary
                .plant
                .as_ref()
                .map(|p| p.steal_num == 0 && p.dry_num == 0 && p.weed_num == 0 && p.insect_num == 0)
                .unwrap_or(true);
            if idle {
                bad_friends.push(summary);
            }
        }
        bad_friends.sort_by(|a, b| b.level.cmp(&a.level));
        bad_friends.truncate(20);

        let recent = self.strategy.recent_help();
        let mut total = crate::services::friend::visit_strategy::TotalActions::default();
        let mut processed = 0usize;
        for friend in &bad_friends {
            if self.is_bad_operation_limit_reached() {
                break;
            }
            if self.get_remaining_bad_operation_times() <= 0 {
                self.mark_bad_operation_limit_reached("operation_limit");
                break;
            }
            let can_exp = self.can_get_exp_by_candidates(&[10005, 10006, 10007]);
            let _ = crate::services::friend::visit_strategy::visit_friend(
                &self.api, recent, friend, &mut total, my_gid, account_id, can_exp,
            )
            .await;
            processed += 1;
            crate::utils::random::random_delay(2000, 3500).await;
        }
        Ok(processed)
    }

    /// 启动巡访循环
    pub fn start_check_loop(&self) {
        if self.external_scheduler.load(Ordering::Acquire) {
            self.stop_check_loop();
            return;
        }
        self.stop_check_loop();
        let cancel = CancellationToken::new();
        *self.current_loop.lock() = Some(cancel.clone());

        let api = self.api.clone();
        let gid_manager = self.gid_manager.clone();
        let strategy = self.strategy.clone();
        let event_tx = self.event_tx.clone();
        let host_gid = self.host_gid.clone();
        let account_id = self.account_id.clone();
        let cancel_for_task = cancel.clone();
        let interval = Duration::from_secs(120);

        self.scheduler.set_interval_task(
            "friend_check",
            interval,
            Arc::new(move || {
                let api = api.clone();
                let gid_manager = gid_manager.clone();
                let strategy = strategy.clone();
                let event_tx = event_tx.clone();
                let host_gid = host_gid.clone();
                let account_id = account_id.clone();
                let cancel = cancel_for_task.clone();
                Box::pin(async move {
                    if cancel.is_cancelled() {
                        return;
                    }
                    let gid = *host_gid.lock();
                    if gid == 0 {
                        let _ = event_tx
                            .send(FriendEvent::Error { message: "host_gid not set".into() });
                        return;
                    }
                    let acc = account_id.lock().clone();
                    match Self::run_one_cycle(&api, &gid_manager, &strategy, gid, &acc).await {
                        Ok((batch_size, helped, stolen, banned)) => {
                            let _ = event_tx.send(FriendEvent::Checked {
                                batch_size,
                                helped,
                                stolen,
                                banned,
                            });
                        }
                        Err(e) => {
                            let _ = event_tx.send(FriendEvent::Error {
                                message: format!("friend cycle failed: {e}"),
                            });
                        }
                    }
                })
            }),
        );
    }

    /// 停止巡访循环
    pub fn stop_check_loop(&self) {
        if let Some(token) = self.current_loop.lock().take() {
            token.cancel();
        }
        self.scheduler.clear("friend_check");
    }

    /// 同步好友列表
    pub async fn sync_friends(&self) -> Result<usize> {
        let friends = self.api.get_friends_list().await?;
        self.gid_manager.update(friends.clone());
        let n = friends.len();
        let _ = self.event_tx.send(FriendEvent::GidsSynced { count: n });
        Ok(n)
    }

    /// 单次巡访
    pub async fn check_friends(&self) -> Result<(usize, usize, usize, usize)> {
        let gid = *self.host_gid.lock();
        let acc = self.account_id.lock().clone();
        Self::run_one_cycle(&self.api, &self.gid_manager, &self.strategy, gid, &acc).await
    }

    async fn run_one_cycle(
        api: &FriendApi,
        gid_manager: &GidManager,
        strategy: &VisitStrategy,
        host_gid: i64,
        account_id: &str,
    ) -> Result<(usize, usize, usize, usize)> {
        // 1. 同步 GID 列表（如果需要）
        let friends = if gid_manager.needs_sync() {
            let f = api.get_friends_list().await?;
            gid_manager.update(f.clone());
            f
        } else {
            gid_manager.cached()
        };

        if friends.is_empty() {
            crate::services::panel_log::log(
                account_id,
                "好友",
                "没有好友",
                crate::constants::PanelEvent::FriendScan,
                Some(serde_json::json!({ "module": "friend",  "result": "empty" })),
            );
            return Ok((0, 0, 0, 0));
        }

        // 2. 选 batch
        let batch = strategy.select_batch(&friends);
        let batch_size = batch.len();
        let mut helped = 0usize;
        let mut stolen = 0usize;
        let mut banned = 0usize;

        // 3. 对 batch 里每个好友：enter → 帮 → leave
        for &friend_gid in &batch {
            // 3a. enter
            let enter_reply = match api.enter_farm(friend_gid).await {
                Ok(r) => r,
                Err(e) => {
                    let msg = format!("{e}");
                    if is_enter_farm_banned_error(&msg) {
                        strategy.add_blacklist(friend_gid);
                        banned += 1;
                        tracing::warn!(friend_gid, "进入农场被封，加入黑名单");
                    } else if is_transient_network_error(&msg) {
                        tracing::warn!(friend_gid, error = %msg, "网络瞬态错误");
                    } else {
                        let code = parse_rpc_error_code(&msg);
                        tracing::warn!(friend_gid, code, error = %msg, "enter_farm 失败");
                    }
                    continue;
                }
            };

            // 3b. 构造 land snapshot key
            let land_snapshots: Vec<_> =
                enter_reply.lands.iter().map(LandSnapshot::from_land).collect();
            let snapshot_key = RecentHelpCache::make_snapshot_key(&land_snapshots);
            let now = now_ms();

            // 3c. 找"需要帮"的 land（去重 RecentHelp）
            let all_land_ids: Vec<i64> = enter_reply.lands.iter().map(|l| l.id).collect();
            let to_help =
                strategy.recent_help().filter(friend_gid, &all_land_ids, &snapshot_key, now);

            // 3d. 帮（锄草）
            if !to_help.is_empty() {
                match api.help_farm(friend_gid, to_help.clone()).await {
                    Ok(outcome) => {
                        strategy.recent_help().mark(
                            friend_gid,
                            &outcome.land_ids,
                            HelpState::Confirmed,
                            HELP_RESULT_TTL_MS,
                            &snapshot_key,
                            now,
                        );
                        if !outcome.land_ids.is_empty() {
                            helped += 1;
                        }
                    }
                    Err(e) => {
                        let msg = format!("{e}");
                        tracing::warn!(friend_gid, error = %msg, "help_farm 失败");
                    }
                }
            }

            // 3e. 偷菜：分析哪些可偷 → 真调 steal_farm
            let land_snapshots: Vec<LandSnapshot> =
                enter_reply.lands.iter().map(LandSnapshot::from_land).collect();
            let status = analyze_friend_lands(&enter_reply.lands, host_gid, &[], false, account_id);
            if !status.stealable.is_empty() {
                let summary = FriendSummary {
                    gid: friend_gid,
                    name: format!("GID:{friend_gid}"),
                    avatar_url: String::new(),
                    level: 0,
                    gold: 0,
                    plant: None,
                };
                let _ = summary; // 当前用于将来的 log / event
                let result = steal_lands_with_reward_log(
                    api,
                    strategy.recent_help(),
                    friend_gid,
                    &status.stealable,
                    &status.stealable_info,
                    None,
                )
                .await;
                stolen += result.ok;
            }
            let _ = land_snapshots; // 保持 move
            strategy.mark_visited(VisitKind::Steal, friend_gid);

            // 3f. leave（即使失败也 swallow）
            if let Err(e) = api.leave_farm(friend_gid).await {
                tracing::warn!(friend_gid, error = %e, "leave_farm 失败（忽略）");
            }
        }

        crate::services::panel_log::log(
            account_id,
            "好友",
            format!("巡查完成 → 帮{helped}/偷{stolen}/封{banned}"),
            crate::constants::PanelEvent::PatrolDone,
            Some(serde_json::json!({
                "module": "friend",
                "helped": helped,
                "stolen": stolen,
                "banned": banned,
            })),
        );
        Ok((batch_size, helped, stolen, banned))
    }

    /// 关闭
    pub fn shutdown(&self) {
        self.stop_check_loop();
        self.scheduler.shutdown();
    }

    /// 获取好友列表（1:1 对齐原 TS `getFriendsList`）
    pub async fn get_friends_list(&self, force: bool) -> Result<Vec<serde_json::Value>> {
        let account_id = self.account_id.lock().clone();
        let ttl_ms = crate::models::store::account_config::get_friends_list_cache_ttl_sec(
            if account_id.is_empty() { None } else { Some(account_id.as_str()) },
        )
        .max(10) as u64
            * 1000;
        let now = now_ms();
        if !force {
            if let Some((cached_at, cached)) = self.friends_list_cache.lock().as_ref() {
                if now.saturating_sub(*cached_at) < ttl_ms {
                    let mut list = cached.clone();
                    self.apply_steal_cleared_overrides(&mut list);
                    return Ok(list);
                }
            }
        }
        crate::services::panel_log::log(
            &account_id,
            "好友",
            "开始获取好友列表",
            crate::constants::PanelEvent::GetFriendList,
            Some(serde_json::json!({ "module": "friend"})),
        );
        let friends = match self.api.get_all_game_friends().await {
            Ok(f) => f,
            Err(e) => {
                let raw = e.to_string();
                let msg = raw.strip_prefix("network error: ").unwrap_or(&raw);
                crate::services::panel_log::log(
                    &account_id,
                    "好友",
                    format!("获取好友列表失败: {msg}"),
                    crate::constants::PanelEvent::GetFriendList,
                    Some(serde_json::json!({ "module": "friend",  "result": "error" })),
                );
                // 对齐 Go List：直播失败仍返回已有列表，不把面板点开变成空表/掉线。
                if let Some((_, cached)) = self.friends_list_cache.lock().as_ref() {
                    let mut list = cached.clone();
                    self.apply_steal_cleared_overrides(&mut list);
                    return Ok(list);
                }
                return Ok(Vec::new());
            }
        };
        let my_gid = *self.host_gid.lock();
        let mut result: Vec<crate::services::friend::visit_strategy::FriendSummary> = friends
            .into_iter()
            .filter(|f| f.gid != my_gid && f.name != "小小农夫" && f.remark != "小小农夫")
            .map(crate::services::friend::visit_strategy::game_friend_to_summary)
            .collect();
        result.sort_by(|a, b| a.name.cmp(&b.name).then(a.gid.cmp(&b.gid)));
        self.gid_manager.update(result.iter().map(|f| f.gid).collect());
        crate::services::panel_log::log(
            &account_id,
            "好友",
            format!("获取好友列表成功，共 {} 位好友", result.len()),
            crate::constants::PanelEvent::GetFriendList,
            Some(serde_json::json!({
                "module": "friend",
                "result": "ok",
                "count": result.len(),
            })),
        );
        let mut json: Vec<serde_json::Value> =
            result.into_iter().filter_map(|f| serde_json::to_value(f).ok()).collect();
        // 诊断：人机「小果」GID 10001 头像是否由游戏下发
        if let Some(npc) = json.iter().find(|f| {
            f.get("gid").and_then(|v| v.as_i64()) == Some(10001)
                || f.get("name").and_then(|v| v.as_str()).is_some_and(|n| n.contains("小果"))
        }) {
            let gid = npc.get("gid").and_then(|v| v.as_i64()).unwrap_or(0);
            let name = npc.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let avatar = npc.get("avatarUrl").and_then(|v| v.as_str()).unwrap_or("");
            crate::services::panel_log::log(
                &account_id,
                "好友",
                format!(
                    "人机头像诊断: name={name} gid={gid} avatarUrl={}",
                    if avatar.is_empty() { "<empty>" } else { avatar }
                ),
                crate::constants::PanelEvent::AvatarProbe,
                Some(serde_json::json!({
                    "module": "friend",
                    "gid": gid,
                    "name": name,
                    "avatarUrl": avatar,
                    "avatarEmpty": avatar.is_empty(),
                })),
            );
        }
        self.apply_steal_cleared_overrides(&mut json);
        *self.friends_list_cache.lock() = Some((now, json.clone()));
        Ok(json)
    }

    /// 清除好友列表缓存（1:1 对齐原 TS `clearFriendsListCache`）
    pub fn clear_friends_list_cache(&self) {
        self.gid_manager.clear_cache();
        self.api.invalidate_list_cache();
        *self.friends_list_cache.lock() = None;
        self.steal_noop_markers.lock().clear();
        let account_id = self.account_id.lock().clone();
        crate::services::friend::visit_strategy::clear_friends_list_cache(&account_id);
    }

    /// 偷菜成功后把该好友的可偷数清零（游戏 GetAll 常滞后，避免面板仍显示「可偷」）。
    pub fn mark_friend_steal_cleared(&self, gid: i64) {
        if gid <= 0 {
            return;
        }
        self.steal_noop_markers.lock().remove(&gid);
        self.steal_cleared_gids.lock().insert(gid);
        self.apply_steal_cleared_to_list();
    }

    fn apply_steal_cleared_to_list(&self) {
        let cleared: HashSet<i64> = self.steal_cleared_gids.lock().clone();
        if cleared.is_empty() {
            return;
        }
        let mut guard = self.friends_list_cache.lock();
        let Some((_, list)) = guard.as_mut() else {
            return;
        };
        Self::zero_steal_num_in_json_list(list, &cleared);
    }

    fn apply_steal_cleared_overrides(&self, list: &mut [serde_json::Value]) {
        let mut cleared = self.steal_cleared_gids.lock();
        if cleared.is_empty() {
            return;
        }
        let mut caught_up = Vec::new();
        for item in list.iter_mut() {
            let Some(obj) = item.as_object_mut() else {
                continue;
            };
            let gid = obj.get("gid").and_then(|v| v.as_i64()).unwrap_or(0);
            if gid <= 0 || !cleared.contains(&gid) {
                continue;
            }
            let live_steal = obj
                .get("plant")
                .and_then(|p| p.get("stealNum").or_else(|| p.get("steal_num")))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if live_steal == 0 {
                caught_up.push(gid);
                continue;
            }
            if let Some(plant) = obj.get_mut("plant").and_then(|p| p.as_object_mut()) {
                plant.insert("stealNum".into(), serde_json::json!(0));
                plant.insert("steal_num".into(), serde_json::json!(0));
            } else {
                obj.insert(
                    "plant".into(),
                    serde_json::json!({ "stealNum": 0, "dryNum": 0, "weedNum": 0, "insectNum": 0 }),
                );
            }
        }
        for gid in caught_up {
            cleared.remove(&gid);
        }
    }

    fn zero_steal_num_in_json_list(list: &mut [serde_json::Value], cleared: &HashSet<i64>) {
        for item in list.iter_mut() {
            let Some(obj) = item.as_object_mut() else {
                continue;
            };
            let gid = obj.get("gid").and_then(|v| v.as_i64()).unwrap_or(0);
            if !cleared.contains(&gid) {
                continue;
            }
            if let Some(plant) = obj.get_mut("plant").and_then(|p| p.as_object_mut()) {
                plant.insert("stealNum".into(), serde_json::json!(0));
                plant.insert("steal_num".into(), serde_json::json!(0));
            }
        }
    }

    /// 获取好友土地详情（1:1 对齐原 TS `getFriendLandsDetail`）
    pub async fn get_friend_lands_detail(&self, gid: i64) -> Result<serde_json::Value> {
        let enter_reply = self.api.enter_farm(gid).await?;
        let (lands, summary) =
            crate::services::farm::land_analysis::friend_lands_detail(&enter_reply.lands);
        let _ = self.api.leave_farm(gid).await;
        // Align Go `FormatFriendLandsResponse`: summary is land counts, not AnalyzeResult.
        Ok(serde_json::json!({
            "lands": lands,
            "summary": summary,
        }))
    }

    /// 好友操作（1:1 对齐原 TS `doFriendOperation`）
    pub async fn do_friend_operation(
        &self,
        op: crate::models::types::FriendOperation,
        gid: i64,
    ) -> Result<serde_json::Value> {
        let my_gid = *self.host_gid.lock();
        let account_id = self.account_id.lock().clone();
        let ret = crate::services::friend::visit_strategy::do_friend_operation(
            &self.api,
            self.strategy.recent_help(),
            gid,
            op,
            my_gid,
            &account_id,
        )
        .await;
        if matches!(op, crate::models::types::FriendOperation::Steal) {
            let stolen = ret.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
            if stolen > 0 {
                self.mark_friend_steal_cleared(gid);
            }
        }
        Ok(ret)
    }
}

const BAD_DAILY_STATE_VERSION: i64 = 1;

fn beijing_date_key() -> String {
    use chrono::{Datelike, FixedOffset, Utc};
    let offset = FixedOffset::east_opt(8 * 3600).expect("bj offset");
    let now = Utc::now().with_timezone(&offset);
    format!("{}-{:02}-{:02}", now.year(), now.month(), now.day())
}

fn bad_daily_state_path(account_id: &str) -> std::path::PathBuf {
    use sha2::{Digest, Sha256};
    let token = hex::encode(Sha256::digest(account_id.as_bytes()));
    crate::config::paths::get_data_file(&format!("friend-bad-state-{token}.json"))
}

fn load_bad_daily_stop(account_id: &str, today: &str) -> bool {
    let path = bad_daily_state_path(if account_id.is_empty() { "default" } else { account_id });
    let state: serde_json::Value =
        crate::services::json_db::read_json_with_default(&path, || serde_json::json!({}));
    state.get("version").and_then(|v| v.as_i64()) == Some(BAD_DAILY_STATE_VERSION)
        && state.get("date").and_then(|v| v.as_str()) == Some(today)
        && state.get("stopped").and_then(|v| v.as_bool()) == Some(true)
}

fn persist_bad_daily_stop(account_id: &str, today: &str) -> std::io::Result<()> {
    let path = bad_daily_state_path(if account_id.is_empty() { "default" } else { account_id });
    crate::services::json_db::write_json_file_atomic(
        &path,
        &serde_json::json!({
            "version": BAD_DAILY_STATE_VERSION,
            "date": today,
            "stopped": true,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_daily_state_roundtrip() {
        let acc = format!("test-bad-{}", std::process::id());
        let today = beijing_date_key();
        assert!(!load_bad_daily_stop(&acc, &today));
        persist_bad_daily_stop(&acc, &today).expect("persist");
        assert!(load_bad_daily_stop(&acc, &today));
        let _ = std::fs::remove_file(bad_daily_state_path(&acc));
    }

    #[test]
    fn strategy_blacklist() {
        let s = VisitStrategy::new(10);
        s.add_blacklist(42);
        assert!(s.is_blacklisted(42));
        assert_eq!(s.blacklist_count(), 1);
    }

    #[test]
    fn strategy_batch_dedup() {
        let s = VisitStrategy::new(3);
        s.add_blacklist(2);
        let b = s.select_batch(&[1, 2, 3, 3, 4, 5, 6, 7]);
        assert_eq!(b, vec![1, 3, 4]);
    }

    #[test]
    fn gid_manager_update_dedupes() {
        let m = GidManager::new();
        m.update(vec![3, 1, 2, 1]);
        assert_eq!(m.cached(), vec![1, 2, 3]);
    }

    #[test]
    fn run_one_cycle_no_friends_returns_zero() {
        // 阶段 1D.2 接入真实 visit_farm 后再扩展此测试
        // 这里只能测试纯函数逻辑
    }
}
