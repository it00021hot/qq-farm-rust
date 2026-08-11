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

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::runtime::scheduler::Scheduler;
use crate::services::friend::api::FriendApi;
use crate::services::friend::gid_manager::{GidEvent, GidManager};
use crate::services::friend::visit_strategy::{
    is_enter_farm_banned_error, is_transient_network_error, now_ms, parse_rpc_error_code,
    HelpState, LandSnapshot, RecentHelpCache, HELP_RESULT_TTL_MS,
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
    current_loop: Arc<Mutex<Option<CancellationToken>>>,
    event_tx: broadcast::Sender<FriendEvent>,
}

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
            current_loop: Arc::new(Mutex::new(None)),
            event_tx,
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

    /// 启动巡访循环
    pub fn start_check_loop(&self) {
        self.stop_check_loop();
        let cancel = CancellationToken::new();
        *self.current_loop.lock() = Some(cancel.clone());

        let api = self.api.clone();
        let gid_manager = self.gid_manager.clone();
        let strategy = self.strategy.clone();
        let event_tx = self.event_tx.clone();
        let host_gid = self.host_gid.clone();
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
                    match Self::run_one_cycle(&api, &gid_manager, &strategy, gid).await {
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
        Self::run_one_cycle(&self.api, &self.gid_manager, &self.strategy, gid).await
    }

    async fn run_one_cycle(
        api: &FriendApi,
        gid_manager: &GidManager,
        strategy: &VisitStrategy,
        host_gid: i64,
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

            // 3e. 偷（占位：阶段 1D.2 写真实偷菜流程）
            // —— 阶段 1D 只 mark visited，不实际调 steal_farm
            strategy.mark_visited(VisitKind::Steal, friend_gid);
            stolen += 1;

            // 3f. leave（即使失败也 swallow）
            if let Err(e) = api.leave_farm(friend_gid).await {
                tracing::warn!(friend_gid, error = %e, "leave_farm 失败（忽略）");
            }
        }

        Ok((batch_size, helped, stolen, banned))
    }

    /// 关闭
    pub fn shutdown(&self) {
        self.stop_check_loop();
        self.scheduler.shutdown();
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
