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
    parse_rpc_error_code, steal_lands_with_reward_log, HelpState, LandSnapshot, RecentHelpCache,
    FriendSummary, HELP_RESULT_TTL_MS,
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
    /// 对齐 TS `helpAutoDisabledByLimit`
    help_auto_disabled: AtomicBool,
    /// 对齐 TS `isCheckingFriends`
    is_checking: AtomicBool,
    /// 对齐 TS `externalSchedulerMode`
    external_scheduler: AtomicBool,
    /// 对齐 TS `friendsListCache`（仅面板 HTTP，巡查不走这份缓存）
    friends_list_cache: Mutex<Option<(u64, Vec<serde_json::Value>)>>,
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
    Checked {
        batch_size: usize,
        helped: usize,
        stolen: usize,
        banned: usize,
    },
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
            help_auto_disabled: AtomicBool::new(false),
            is_checking: AtomicBool::new(false),
            external_scheduler: AtomicBool::new(false),
            friends_list_cache: Mutex::new(None),
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
                self.bad_operation_limit_reached
                    .store(true, Ordering::Release);
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
            Some(serde_json::json!({
                "module": "friend",
                "event": "friend_cycle",
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
                Some(serde_json::json!({ "module": "friend", "event": "好友申请" })),
            );
        }
        match self.api.accept_applications(gids).await {
            Ok(()) => crate::services::panel_log::log(
                &acc,
                "申请",
                "已同意好友申请",
                Some(serde_json::json!({ "module": "friend", "event": "同意好友申请" })),
            ),
            Err(e) => crate::services::panel_log::log_warn(
                &acc,
                "申请",
                format!("同意失败: {e}"),
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
            .map(|(gid, name)| {
                if name.is_empty() {
                    format!("GID:{gid}")
                } else {
                    name.clone()
                }
            })
            .collect();
        let gids: Vec<i64> = apps.into_iter().map(|(g, _)| g).collect();
        let acc = self.account_id.lock().clone();
        crate::services::panel_log::log(
            &acc,
            "申请",
            format!("发现 {} 个待处理申请: {}", names.len(), names.join(", ")),
            Some(serde_json::json!({ "module": "friend", "event": "待处理申请" })),
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
        let result = self.visit_batch_inner(account_id, kind).await;
        self.is_checking.store(false, Ordering::Release);
        result
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
                    Some(serde_json::json!({
                        "module": "friend",
                        "event": "friend_cycle",
                        "result": "error",
                    })),
                );
                return Err(e);
            }
        };
        self.gid_manager
            .update(friends.iter().map(|f| f.gid).collect());
        if friends.is_empty() {
            crate::services::panel_log::log(
                account_id,
                "好友",
                "没有好友",
                Some(serde_json::json!({
                    "module": "friend",
                    "event": "好友扫描",
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
            if cfg_blacklist.contains(&f.gid) || self.strategy.is_blacklisted(f.gid) {
                continue;
            }
            let summary =
                crate::services::friend::visit_strategy::game_friend_to_summary(f);
            let steal_num = summary
                .plant
                .as_ref()
                .map(|p| p.steal_num)
                .unwrap_or(0);
            let help_need = summary
                .plant
                .as_ref()
                .map(|p| p.dry_num + p.weed_num + p.insect_num)
                .unwrap_or(0);
            if kind == VisitKind::Steal && steal_num > 0 {
                steal_friends.push(summary);
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
            crate::services::panel_log::log(
                account_id,
                "好友",
                format!("开始批量偷菜，共 {} 个好友有可偷", steal_friends.len()),
                Some(serde_json::json!({
                    "module": "friend",
                    "event": "visit_friend",
                    "count": steal_friends.len(),
                })),
            );
            for friend in &steal_friends {
                let _ = crate::services::friend::visit_strategy::visit_friend_for_steal(
                    &self.api,
                    recent,
                    friend,
                    &mut total,
                    my_gid,
                    account_id,
                )
                .await;
                crate::utils::random::random_delay(500, 800).await;
            }
        }

        if kind == VisitKind::Help && !help_friends.is_empty() {
            crate::services::panel_log::log(
                account_id,
                "好友",
                format!("开始批量帮助，共 {} 个好友需要帮助", help_friends.len()),
                Some(serde_json::json!({
                    "module": "friend",
                    "event": "visit_friend",
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
                        Some(serde_json::json!({
                            "module": "friend",
                            "event": "friend_cycle",
                            "reason": "exp_limit",
                        })),
                    );
                    break;
                }
                crate::services::panel_log::log(
                    account_id,
                    "好友",
                    format!(
                        "批量帮助第 {}/{} 个好友: {}",
                        i + 1,
                        help_friends.len(),
                        friend.name
                    ),
                    Some(serde_json::json!({
                        "module": "friend",
                        "event": "visit_friend",
                        "index": i + 1,
                        "total": help_friends.len(),
                        "friendName": friend.name,
                    })),
                );
                let _ = crate::services::friend::visit_strategy::visit_friend_for_help(
                    &self.api,
                    recent,
                    friend,
                    &mut total,
                    my_gid,
                    account_id,
                    false,
                    &self.help_auto_disabled,
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
                Some(serde_json::json!({
                    "module": "friend",
                    "event": "friend_cycle",
                    "result": "ok",
                    "visited": steal_friends.len() + help_friends.len(),
                    "summary": summary,
                })),
            );
        }

        Ok(if kind == VisitKind::Steal {
            total.steal
        } else {
            total.farming
        })
    }

    /// 启动时执行一次放虫放草（对齐 TS `runBadOnceOnStartup`）
    pub async fn run_bad_once_on_startup(&self, account_id: &str) -> Result<usize> {
        if self.bad_ran_on_startup.swap(true, Ordering::AcqRel) {
            return Ok(0);
        }
        if !crate::services::automation::is_automation_on_for(account_id, "friend_bad") {
            return Ok(0);
        }
        let my_gid = *self.host_gid.lock();
        if my_gid == 0 {
            return Ok(0);
        }
        let friends = self.api.get_friends_list().await.unwrap_or_default();
        let mut acted = 0usize;
        for &fg in friends.iter().take(20) {
            if fg == my_gid || self.strategy.is_blacklisted(fg) {
                continue;
            }
            let enter = match self.api.enter_farm(fg).await {
                Ok(r) => r,
                Err(_) => continue,
            };
            let result =
                crate::services::friend::visit_strategy::do_bad_op(&self.api, fg, &enter.lands)
                    .await;
            let _ = self.api.leave_farm(fg).await;
            if result.get("count").and_then(|v| v.as_u64()).unwrap_or(0) > 0 {
                acted += 1;
            }
        }
        Ok(acted)
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
                        let _ = event_tx.send(FriendEvent::Error {
                            message: "host_gid not set".into(),
                        });
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
                Some(serde_json::json!({ "module": "friend", "event": "好友扫描", "result": "empty" })),
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
            let land_snapshots: Vec<_> = enter_reply.lands.iter().map(LandSnapshot::from_land).collect();
            let snapshot_key = RecentHelpCache::make_snapshot_key(&land_snapshots);
            let now = now_ms();

            // 3c. 找"需要帮"的 land（去重 RecentHelp）
            let all_land_ids: Vec<i64> = enter_reply.lands.iter().map(|l| l.id).collect();
            let to_help = strategy.recent_help().filter(
                friend_gid,
                &all_land_ids,
                &snapshot_key,
                now,
            );

            // 3d. 帮（锄草）
            if !to_help.is_empty() {
                match api.help_farm(friend_gid, to_help.clone()).await {
                    Ok(confirmed_lands) => {
                        let confirmed_ids: Vec<i64> =
                            confirmed_lands.iter().map(|l| l.id).collect();
                        strategy.recent_help().mark(
                            friend_gid,
                            &confirmed_ids,
                            HelpState::Confirmed,
                            HELP_RESULT_TTL_MS,
                            &snapshot_key,
                            now,
                        );
                        helped += 1;
                    }
                    Err(e) => {
                        let msg = format!("{e}");
                        tracing::warn!(friend_gid, error = %msg, "help_farm 失败");
                    }
                }
            }

            // 3e. 偷菜：分析哪些可偷 → 真调 steal_farm
            let land_snapshots: Vec<LandSnapshot> = enter_reply
                .lands
                .iter()
                .map(LandSnapshot::from_land)
                .collect();
            let status = analyze_friend_lands(
                &enter_reply.lands,
                host_gid,
                &[],
                false,
            );
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
            Some(serde_json::json!({
                "module": "friend",
                "event": "巡查完成",
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
            if account_id.is_empty() {
                None
            } else {
                Some(account_id.as_str())
            },
        )
        .max(10) as u64
            * 1000;
        let now = now_ms();
        if !force {
            if let Some((cached_at, cached)) = self.friends_list_cache.lock().as_ref() {
                if now.saturating_sub(*cached_at) < ttl_ms {
                    return Ok(cached.clone());
                }
            }
        }
        crate::services::panel_log::log(
            &account_id,
            "好友",
            "开始获取好友列表",
            Some(serde_json::json!({ "module": "friend", "event": "获取好友列表" })),
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
                    Some(serde_json::json!({ "module": "friend", "event": "获取好友列表", "result": "error" })),
                );
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
        self.gid_manager
            .update(result.iter().map(|f| f.gid).collect());
        crate::services::panel_log::log(
            &account_id,
            "好友",
            format!("获取好友列表成功，共 {} 位好友", result.len()),
            Some(serde_json::json!({
                "module": "friend",
                "event": "获取好友列表",
                "result": "ok",
                "count": result.len(),
            })),
        );
        let json: Vec<serde_json::Value> = result
            .into_iter()
            .filter_map(|f| serde_json::to_value(f).ok())
            .collect();
        *self.friends_list_cache.lock() = Some((now, json.clone()));
        Ok(json)
    }

    /// 清除好友列表缓存（1:1 对齐原 TS `clearFriendsListCache`）
    pub fn clear_friends_list_cache(&self) {
        self.gid_manager.clear_cache();
        self.api.invalidate_list_cache();
        *self.friends_list_cache.lock() = None;
        crate::services::friend::visit_strategy::clear_friends_list_cache();
    }

    /// 获取好友土地详情（1:1 对齐原 TS `getFriendLandsDetail`）
    pub async fn get_friend_lands_detail(&self, gid: i64) -> Result<serde_json::Value> {
        let enter_reply = self.api.enter_farm(gid).await?;
        let my_gid = *self.host_gid.lock();
        let account_id = self.account_id.lock().clone();
        let blacklist = crate::models::store::account_config::get_plant_blacklist(
            if account_id.is_empty() {
                None
            } else {
                Some(account_id.as_str())
            },
        );
        let analyzed = crate::services::friend::visit_strategy::analyze_friend_lands(
            &enter_reply.lands,
            my_gid,
            &blacklist,
            false,
        );
        let lands = crate::services::farm::land_analysis::friend_lands_detail(&enter_reply.lands);
        let _ = self.api.leave_farm(gid).await;
        Ok(serde_json::json!({
            "lands": lands,
            "summary": analyzed,
        }))
    }

    /// 好友操作（1:1 对齐原 TS `doFriendOperation`）
    pub async fn do_friend_operation(
        &self,
        op: crate::models::types::FriendOperation,
        gid: i64,
    ) -> Result<serde_json::Value> {
        Ok(crate::services::friend::visit_strategy::do_friend_operation(
            &self.api,
            self.strategy.recent_help(),
            gid,
            op,
        )
        .await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
