//! 好友宠物每日同步 —— 把护主犬探测从「每轮帮忙」改成「每天一轮」。
//!
//! 对齐 bot `core/src/services/friend/pet-sync.ts`（HEAD=8dae528）。
//!
//! 护主犬只能从 `VisitService.Enter` 回包的 `brief_dog_info` 读到，所以本模块是
//! 唯一为了拿宠物信息而额外发 RPC 的地方；其余时候全靠 `FriendApi::enter_farm`
//! 的顺手写入（见 [`crate::services::friend::pet_cache`]）。
//!
//! 服务端对进出好友农场有速率限制：一轮连探几十位之后网关会对所有请求彻底
//! 静默，最后心跳三连失败掉线。因此：
//! - 串行 + 分批 + 固定 2 秒间隔，且一轮只探当前配额内的几位；
//! - 轮次配额与轮间间隔自适应（[`plan_next_sync_pacing`]）：
//!   干净跑完一轮就加量加速，让路一次就退回基线；
//! - 进每位好友前先给好友巡查让路（`is_checking`），等不到空闲就把剩下的
//!   好友留给下一轮（单向门控，好友巡查不会等同步）；
//! - 进每位好友前还要等网关空闲（队列里有在途请求时不排队，等不到空闲窗口
//!   就整轮让路），让路按「只是被主流程占着」（短退避）与「入站静默」
//!   （30 分钟冷却）分开计价。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use crate::error::Error;
use crate::network::error::NetworkError;
use crate::services::friend::pet_cache::{
    drop_stale_entries, get_friend_dog_state, get_friend_pet_cache_stats,
    is_full_sync_done_today, mark_full_sync_done, FriendDogState,
};
use crate::services::friend::scheduler::FriendService;

// —— 节奏参数（bot pet-sync.ts:54-74；瞬时速率是安全线，不参与自适应）——

/// 每批探测的好友数
pub const BATCH_SIZE: usize = 5;
/// 批内每两位好友之间的间隔（一位好友两个 RPC，约 0.9 RPC/s）
pub const GAP_MS: u64 = 2_000;
/// 批与批之间的间隔
pub const BATCH_GAP_MS: u64 = 3_000;
/// 轮次配额基线：每天从这里起步
pub const QUOTA_BASE: i64 = 10;
/// 干净跑完一轮后配额的上调步长
pub const QUOTA_STEP: i64 = 5;
/// 配额封顶
pub const QUOTA_CAP: i64 = 25;
/// 基线间隔：当天没活、开关关着、跨日等情况下的巡检节奏（10min）
pub const CHECK_INTERVAL_MS: i64 = 10 * 60 * 1000;
/// 干净跑完一轮但好友还没探完时的间隔（3min）
pub const FAST_INTERVAL_MS: i64 = 3 * 60 * 1000;
/// 只是抢不到空闲窗口（自家前台/农场请求正忙）时的短退避（60s）
pub const CONTENTION_RETRY_MS: i64 = 60 * 1000;
/// 服务端静默后的冷却（30min）：避免贴着服务端的限制反复试探
pub const BUSY_COOLDOWN_MS: i64 = 30 * 60 * 1000;
/// 启动错峰：登录序列跑完之后再排（90s）
pub const STARTUP_DELAY_MS: i64 = 90 * 1000;
/// 进每位好友前给好友巡查让路的最长等待
pub const FRIEND_TASK_WAIT_MAX_MS: u64 = 10_000;
/// 让路等待的轮询间隔
pub const FRIEND_TASK_POLL_MS: u64 = 250;
/// 进每位好友前等网关空闲的最长等待，等不到就整轮让路
pub const GATEWAY_IDLE_WAIT_MS: u64 = 8_000;
/// 判定「服务端静默」的入站静默阈值（对齐心跳静默判定）
const GATEWAY_SILENCE_MS: i64 = crate::constants::HEARTBEAT_SILENCE_MS as i64;

/// 一轮同步的结果（决定下一轮节奏，见 [`plan_next_sync_pacing`]）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncRoundOutcome {
    /// 干净跑完一轮；`deferred > 0` 表示配额分轮还有剩余（bot `deferred/round_quota`），
    /// `deferred == 0` 表示全部探完（bot `synced`）
    CleanRun { deferred: usize },
    /// 只是抢不到空闲窗口（自家前台/农场请求正占着连接）
    GatewayContention,
    /// 好友巡查占用，让位
    FriendTaskBusy,
    /// 服务端静默（心跳漏拍/在途请求卡住不回包），进入长冷却
    GatewayBusy,
    /// 当天已完成全量同步（bot `fresh/done_today`）
    Done,
    /// 所有好友当天已有结论（bot `fresh/all_known`）
    Fresh,
    /// 开关关闭 / 静默时段 / 未登录等（bot `skipped/*`）
    Skipped,
    /// 异常（bot `error`）
    Error,
}

/// 下一轮的节奏（bot `SyncPacing`，pet-sync.ts:131-142）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncPacing {
    /// 距离下一轮的等待时间
    pub next_delay_ms: i64,
    /// 下一轮的好友配额
    pub quota: i64,
    /// 下一轮是否仍允许上调配额（false = 当天锁死上调）
    pub escalate_quota: bool,
}

/// 根据本轮结果决定下一轮的节奏（纯函数，便于测试）。
///
/// 对齐 bot `planNextSyncPacing`（pet-sync.ts:154-173）：
/// - 让路收场（抢窗口失败 / 好友巡查占用 / 服务端静默）：配额回基线并锁死当天
///   的上调；抢窗口失败 60s 后重试，服务端静默直接进入 30 分钟冷却；
/// - 干净跑完但好友没探完：连接扛得住，配额 +STEP 封顶 CAP 并用较短的间隔接上；
/// - 其余情况（当天已完成 / 没活 / 开关关闭 / 异常）：回基线间隔，配额不动。
#[must_use]
pub fn plan_next_sync_pacing(
    outcome: SyncRoundOutcome,
    quota_before: i64,
    quota_ceiling_for_day: bool,
) -> SyncPacing {
    match outcome {
        SyncRoundOutcome::GatewayContention | SyncRoundOutcome::FriendTaskBusy => SyncPacing {
            next_delay_ms: CONTENTION_RETRY_MS,
            quota: QUOTA_BASE,
            escalate_quota: false,
        },
        SyncRoundOutcome::GatewayBusy => SyncPacing {
            next_delay_ms: BUSY_COOLDOWN_MS,
            quota: QUOTA_BASE,
            escalate_quota: false,
        },
        SyncRoundOutcome::CleanRun { deferred } if deferred > 0 => SyncPacing {
            next_delay_ms: FAST_INTERVAL_MS,
            quota: if quota_ceiling_for_day {
                quota_before
            } else {
                (quota_before + QUOTA_STEP).min(QUOTA_CAP)
            },
            escalate_quota: !quota_ceiling_for_day,
        },
        SyncRoundOutcome::CleanRun { .. } | SyncRoundOutcome::Done
        | SyncRoundOutcome::Fresh
        | SyncRoundOutcome::Skipped
        | SyncRoundOutcome::Error => SyncPacing {
            next_delay_ms: CHECK_INTERVAL_MS,
            quota: quota_before,
            escalate_quota: !quota_ceiling_for_day,
        },
    }
}

/// 让路型错误：网关没余力或连接已断，不是这位好友的问题
/// （bot `isGatewayYieldError`，pet-sync.ts 里 low-priority-gate 的分类）。
fn is_gateway_yield_error(err: &Error) -> bool {
    matches!(
        err,
        Error::Network(
            NetworkError::QueueFull { .. }
                | NetworkError::Timeout { .. }
                | NetworkError::Closed { .. }
                | NetworkError::Phase(_)
                | NetworkError::WebSocket(_)
                | NetworkError::IntentionalClose(_)
        )
    )
}

/// 拿不到空闲窗口分两种情况，代价差 30 倍，必须分开
/// （bot `classifyGatewayDefer`，pet-sync.ts:111-120）：
/// - 入站数据正常，只是自家前台操作 / 农场巡检正占着连接 → 抢窗口失败，
///   几十秒后再来；
/// - 入站静默超过心跳阈值（服务端真的不回包）→ 30 分钟冷却，别再试探。
fn classify_gateway_defer(service: &FriendService) -> SyncRoundOutcome {
    let gateway = service.api().gateway();
    let now = crate::utils::time::now_ms();
    let last_rx = gateway.last_rx_ms();
    if last_rx > 0 && now.saturating_sub(last_rx) > GATEWAY_SILENCE_MS {
        SyncRoundOutcome::GatewayBusy
    } else {
        SyncRoundOutcome::GatewayContention
    }
}

/// 等好友巡查让路：`is_checking` 为真时轮询等待，最多 `FRIEND_TASK_WAIT_MAX_MS`
/// （bot pet-sync.ts:175-182 `waitForFriendTaskIdle`）。等不到返回 false。
async fn wait_friend_task_idle(service: &FriendService) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(FRIEND_TASK_WAIT_MAX_MS);
    while service.is_checking() {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(FRIEND_TASK_POLL_MS)).await;
    }
    true
}

/// 等网关空闲：有在途请求时不排队（只观察不加压），最多 `GATEWAY_IDLE_WAIT_MS`
/// （bot `waitForGatewayIdle`；rust 网关没有请求分级，用 pending 数近似）。
async fn wait_gateway_idle(service: &FriendService) -> bool {
    let gateway = service.api().gateway();
    let deadline = tokio::time::Instant::now() + Duration::from_millis(GATEWAY_IDLE_WAIT_MS);
    while gateway.pending_count() > 0 {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(FRIEND_TASK_POLL_MS)).await;
    }
    true
}

/// 待同步名单：过滤掉自己 / 黑名单 / 失效好友 / 当天已有结论的好友
/// （bot pet-sync.ts:211-225 `collectPendingFriends`）。
#[must_use]
pub fn collect_pending_friends(
    service: &FriendService,
    account_id: &str,
    my_gid: i64,
    friends: &[crate::proto::generated::gamepb::friendpb::GameFriend],
) -> Vec<(i64, String)> {
    let blacklist: HashSet<i64> =
        crate::models::store::account_config::get_friend_blacklist(Some(account_id))
            .into_iter()
            .collect();
    let mut pending = Vec::new();
    let mut seen = HashSet::new();
    for friend in friends {
        let gid = friend.gid;
        if gid <= 0 || gid == my_gid || !seen.insert(gid) {
            continue;
        }
        if blacklist.contains(&gid)
            || service.strategy().is_blacklisted(gid)
            || crate::services::friend::visit_strategy::is_known_friend_gid_invalid(gid)
        {
            continue;
        }
        // 当天已经有结论的不重复同步（帮忙/偷菜顺手写入的）
        if get_friend_dog_state(account_id, gid) != FriendDogState::Unknown {
            continue;
        }
        let name = if friend.remark.is_empty() {
            if friend.name.is_empty() { format!("GID:{gid}") } else { friend.name.clone() }
        } else {
            friend.remark.clone()
        };
        pending.push((gid, name));
    }
    pending
}

/// 轮次链的自适应状态（对齐 bot 模块级 roundQuota/quotaRampLocked/pacingDateKey）
#[derive(Debug, Clone)]
struct ChainState {
    quota: i64,
    /// 当天是否已锁死配额上调
    locked: bool,
    date_key: String,
}

/// 启动好友宠物同步轮次链（对齐 bot `startFriendPetSyncTimer`，pet-sync.ts:390-394）。
///
/// - `service`：好友服务（enter/leave 与 is_checking 让位判定用）；
/// - `account_id`：账号（pet_cache 落盘与开关门控键）；
/// - `is_running`：账号登录态与 worker 存活判定（worker 停止即整链停）。
///
/// 重复调用会先停掉旧链；[`stop_friend_pet_sync`] 取消后由
/// [`crate::services::friend::scheduler::FriendService::stop_check_loop`]
/// 在停机时触发（对齐 bot `stopFriendPetSyncTimer`，重连后从基线重新开始）。
pub fn spawn_friend_pet_sync(
    service: &Arc<FriendService>,
    account_id: String,
    is_running: Arc<dyn Fn() -> bool + Send + Sync>,
) {
    stop_friend_pet_sync(&account_id);
    let token = CancellationToken::new();
    active_tokens().lock().insert(account_id.clone(), token.clone());
    let service = Arc::clone(service);
    crate::runtime::safe_spawn::spawn_logged("friend_pet_sync", async move {
        run_sync_chain(service, account_id, is_running, token).await;
    });
}

/// 停止好友宠物同步轮次链（对齐 bot `stopFriendPetSyncTimer`，pet-sync.ts:396-404）
pub fn stop_friend_pet_sync(account_id: &str) {
    if let Some(token) = active_tokens().lock().remove(account_id) {
        token.cancel();
    }
}

/// 轮次链是否仍在跑（测试与诊断用）
#[must_use]
pub fn is_friend_pet_sync_active(account_id: &str) -> bool {
    active_tokens().lock().contains_key(account_id)
}

fn active_tokens() -> &'static Mutex<HashMap<String, CancellationToken>> {
    static TOKENS: std::sync::OnceLock<Mutex<HashMap<String, CancellationToken>>> =
        std::sync::OnceLock::new();
    TOKENS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 轮次链：每轮跑完由 [`plan_next_sync_pacing`] 决定下一轮什么时候来、探几位。
/// 自我续期而不是固定 interval，间隔才能跟着连接状态变（bot pet-sync.ts:373-388）。
async fn run_sync_chain(
    service: Arc<FriendService>,
    account_id: String,
    is_running: Arc<dyn Fn() -> bool + Send + Sync>,
    token: CancellationToken,
) {
    let mut state = ChainState { quota: QUOTA_BASE, locked: false, date_key: String::new() };
    let mut delay_ms = STARTUP_DELAY_MS;
    loop {
        tokio::select! {
            _ = token.cancelled() => break,
            _ = tokio::time::sleep(Duration::from_millis(delay_ms.max(1_000) as u64)) => {}
        }
        if token.is_cancelled() {
            break;
        }
        let outcome = run_one_round(&service, &account_id, is_running.as_ref(), &mut state).await;
        let pacing = plan_next_sync_pacing(outcome, state.quota, state.locked);
        state.quota = pacing.quota;
        state.locked = !pacing.escalate_quota;
        delay_ms = pacing.next_delay_ms;
    }
}

/// 执行一轮同步（对齐 bot `runFriendPetSyncRound`，pet-sync.ts:235-367）。
async fn run_one_round(
    service: &FriendService,
    account_id: &str,
    is_running: &(dyn Fn() -> bool + Sync),
    state: &mut ChainState,
) -> SyncRoundOutcome {
    // 门控顺序（bot pet-sync.ts:96-105 + 240-247）：护主犬开关关闭时这份数据
    // 没有消费方，一个额外 RPC 都不应该花
    if !crate::services::automation::is_automation_on_for(account_id, "friend") {
        return SyncRoundOutcome::Skipped;
    }
    if !crate::services::automation::is_automation_on_for(account_id, "friend_help") {
        return SyncRoundOutcome::Skipped;
    }
    if !crate::services::automation::is_automation_on_for(
        account_id,
        "friend_help_protect_dog_ignore_exp_limit",
    ) {
        return SyncRoundOutcome::Skipped;
    }
    if is_full_sync_done_today(account_id) {
        return SyncRoundOutcome::Done;
    }
    // 安静时段不进好友农场，与统一巡查保持一致
    if crate::services::friend::visit_strategy::in_friend_quiet_hours_for(Some(account_id), None)
    {
        return SyncRoundOutcome::Skipped;
    }
    if !is_running() || service.host_gid() == 0 {
        return SyncRoundOutcome::Skipped;
    }

    drop_stale_entries(account_id);
    // 跨日重新开始爬配额：昨天撞过限制不代表今天也会（bot pet-sync.ts:249-255）
    let today = crate::utils::time::today_system_date_key();
    if state.date_key != today {
        state.date_key = today;
        state.quota = QUOTA_BASE;
        state.locked = false;
    }

    // 好友列表：网关正忙的时候连它都不该排队（复用 GetAll 短缓存）
    if !wait_gateway_idle(service).await {
        return classify_gateway_defer(service);
    }
    let friends = match service.api().get_all_game_friends().await {
        Ok(f) => f,
        Err(e) => {
            if is_gateway_yield_error(&e) {
                return classify_gateway_defer(service);
            }
            tracing::warn!(account_id, error = %e, "好友宠物同步拉取好友列表失败");
            return SyncRoundOutcome::Error;
        }
    };
    let my_gid = service.host_gid();
    let pending = collect_pending_friends(service, account_id, my_gid, &friends);
    if pending.is_empty() {
        mark_full_sync_done(account_id);
        return SyncRoundOutcome::Fresh;
    }

    // 一轮只探配额内的这几位，剩下的等下一轮——瞬时突发量越小越不容易踩到限制
    let take = (state.quota.max(0) as usize).min(pending.len());
    let targets = &pending[..take];
    let mut deferred = pending.len() - targets.len();
    let mut checked = 0usize;
    let mut failed = 0usize;
    let mut outcome = SyncRoundOutcome::CleanRun { deferred };

    let stats_before = get_friend_pet_cache_stats(account_id);
    crate::services::panel_log::log(
        account_id,
        "好友",
        format!("开始同步好友宠物，本轮 {} 位，待确认共 {} 位", targets.len(), pending.len()),
        crate::constants::PanelEvent::FriendCycle,
        Some(serde_json::json!({
            "module": "friend",
            "event": "好友宠物同步",
            "result": "start",
            "batch": targets.len(),
            "quota": state.quota,
            "deferred": deferred,
            "known": stats_before.get("known").cloned().unwrap_or(serde_json::json!(0)),
            "protect": stats_before.get("protect").cloned().unwrap_or(serde_json::json!(0)),
        })),
    );

    'outer: for (batch_index, batch) in targets.chunks(BATCH_SIZE).enumerate() {
        // 让位好友任务：等不到就把剩余好友记 deferred 留给下一轮
        if !wait_friend_task_idle(service).await {
            deferred = pending.len() - checked - failed;
            outcome = SyncRoundOutcome::FriendTaskBusy;
            break;
        }
        for (gid, name) in batch {
            // 运行中被关掉也当场停下（bot 每位好友前重新核对开关）
            if !crate::services::automation::is_automation_on_for(account_id, "friend")
                || !crate::services::automation::is_automation_on_for(account_id, "friend_help")
                || !crate::services::automation::is_automation_on_for(
                    account_id,
                    "friend_help_protect_dog_ignore_exp_limit",
                )
            {
                deferred = pending.len() - checked - failed;
                outcome = SyncRoundOutcome::Skipped;
                break 'outer;
            }
            // 主流程有请求在飞就不插队；等不到空闲窗口整轮让路
            if !wait_gateway_idle(service).await {
                deferred = pending.len() - checked - failed;
                outcome = classify_gateway_defer(service);
                break 'outer;
            }
            // 回包里的 brief_dog_info 由 enter_farm 统一写进缓存，这里不再解析
            match service.api().enter_farm(*gid).await {
                Ok(_) => {
                    checked += 1;
                    // Leave 必须配对送出：留下「还在别人农场里」的服务端状态
                    // 会让后续 Enter 全部失败；失败也不逐个刷告警
                    let _ = service.api().leave_farm(*gid).await;
                }
                Err(e) if is_gateway_yield_error(&e) => {
                    // 网关没余力或连接已断：不是这位好友的问题，整轮让路
                    deferred = pending.len() - checked - failed;
                    outcome = classify_gateway_defer(service);
                    break 'outer;
                }
                Err(e) => {
                    // 复用已有的封禁加黑、失效好友清理逻辑
                    let _ = crate::services::friend::visit_strategy::handle_friend_enter_error(
                        account_id,
                        *gid,
                        name,
                        &e.to_string(),
                    );
                    tracing::warn!(account_id, gid, error = %e, "同步宠物时进入好友农场失败");
                    failed += 1;
                }
            }
            tokio::time::sleep(Duration::from_millis(GAP_MS)).await;
        }
        // 批间再放一拍（最后一批之后不等，对齐 bot pet-sync.ts:327）
        if (batch_index + 1) * BATCH_SIZE < targets.len() {
            tokio::time::sleep(Duration::from_millis(BATCH_GAP_MS)).await;
        }
    }

    // 只有真正跑完才标记当日完成，否则下一次定时检查继续补剩下的
    if matches!(outcome, SyncRoundOutcome::CleanRun { deferred: 0 }) {
        mark_full_sync_done(account_id);
    }

    let stats = get_friend_pet_cache_stats(account_id);
    crate::services::panel_log::log(
        account_id,
        "好友",
        format!(
            "好友宠物同步完成：确认 {checked}，失败 {failed}，待补 {deferred}，当日护主犬 {} 位",
            stats.get("protect").and_then(|v| v.as_i64()).unwrap_or(0)
        ),
        crate::constants::PanelEvent::FriendCycle,
        Some(serde_json::json!({
            "module": "friend",
            "event": "好友宠物同步",
            "result": if deferred > 0 { "deferred" } else { "ok" },
            "checked": checked,
            "failed": failed,
            "deferred": deferred,
            "known": stats.get("known").cloned().unwrap_or(serde_json::json!(0)),
            "protect": stats.get("protect").cloned().unwrap_or(serde_json::json!(0)),
        })),
    );

    outcome
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pacing_clean_run_with_deferred_escalates() {
        let p = plan_next_sync_pacing(SyncRoundOutcome::CleanRun { deferred: 30 }, QUOTA_BASE, false);
        assert_eq!(p.next_delay_ms, FAST_INTERVAL_MS);
        assert_eq!(p.quota, QUOTA_BASE + QUOTA_STEP);
        assert!(p.escalate_quota);
    }

    #[test]
    fn pacing_clean_run_escalation_caps_at_cap() {
        let p = plan_next_sync_pacing(SyncRoundOutcome::CleanRun { deferred: 5 }, QUOTA_CAP, false);
        assert_eq!(p.quota, QUOTA_CAP);
        assert_eq!(p.next_delay_ms, FAST_INTERVAL_MS);
    }

    #[test]
    fn pacing_clean_run_locked_for_day_keeps_quota() {
        let p = plan_next_sync_pacing(SyncRoundOutcome::CleanRun { deferred: 5 }, 15, true);
        assert_eq!(p.quota, 15);
        assert!(!p.escalate_quota);
        assert_eq!(p.next_delay_ms, FAST_INTERVAL_MS);
    }

    #[test]
    fn pacing_clean_run_without_deferred_returns_baseline() {
        let p = plan_next_sync_pacing(SyncRoundOutcome::CleanRun { deferred: 0 }, 15, false);
        assert_eq!(p.next_delay_ms, CHECK_INTERVAL_MS);
        assert_eq!(p.quota, 15);
        assert!(p.escalate_quota);
    }

    #[test]
    fn pacing_contention_resets_quota_and_locks() {
        let p = plan_next_sync_pacing(SyncRoundOutcome::GatewayContention, QUOTA_CAP, false);
        assert_eq!(p.next_delay_ms, CONTENTION_RETRY_MS);
        assert_eq!(p.quota, QUOTA_BASE);
        assert!(!p.escalate_quota);
        let p = plan_next_sync_pacing(SyncRoundOutcome::FriendTaskBusy, QUOTA_CAP, false);
        assert_eq!(p.next_delay_ms, CONTENTION_RETRY_MS);
        assert_eq!(p.quota, QUOTA_BASE);
        assert!(!p.escalate_quota);
    }

    #[test]
    fn pacing_gateway_busy_enters_cooldown() {
        let p = plan_next_sync_pacing(SyncRoundOutcome::GatewayBusy, QUOTA_CAP, false);
        assert_eq!(p.next_delay_ms, BUSY_COOLDOWN_MS);
        assert_eq!(p.quota, QUOTA_BASE);
        assert!(!p.escalate_quota);
    }

    #[test]
    fn pacing_terminal_outcomes_keep_baseline() {
        for outcome in [
            SyncRoundOutcome::Done,
            SyncRoundOutcome::Fresh,
            SyncRoundOutcome::Skipped,
            SyncRoundOutcome::Error,
        ] {
            let p = plan_next_sync_pacing(outcome, 17, false);
            assert_eq!(p.next_delay_ms, CHECK_INTERVAL_MS, "{outcome:?}");
            assert_eq!(p.quota, 17, "{outcome:?}");
            assert!(p.escalate_quota, "{outcome:?}");
        }
        // 锁死状态在终止结局下保持锁死
        let p = plan_next_sync_pacing(SyncRoundOutcome::Done, 17, true);
        assert!(!p.escalate_quota);
        assert_eq!(p.quota, 17);
    }

    #[test]
    fn gateway_yield_error_classification() {
        let queue_full = Error::Network(NetworkError::QueueFull { pending: 1, queued: 1 });
        assert!(is_gateway_yield_error(&queue_full));
        let timeout = Error::Network(NetworkError::Timeout {
            client_seq: 1,
            service_name: "s".into(),
            method_name: "m".into(),
            pending: 1,
        });
        assert!(is_gateway_yield_error(&timeout));
        let closed = Error::Network(NetworkError::Closed { code: 1006, reason: "x".into() });
        assert!(is_gateway_yield_error(&closed));
        let business = Error::Network(NetworkError::Gateway {
            code: 1002003,
            service_name: "visitpb.VisitService".into(),
            method_name: "Enter".into(),
            error_message: "banned".into(),
            client_seq: 0,
        });
        assert!(!is_gateway_yield_error(&business), "业务错误不是让路错误");
    }

    #[test]
    fn constants_match_bot_tuning() {
        // 对齐 bot pet-sync.ts:54-74 的节奏参数
        assert_eq!(BATCH_SIZE, 5);
        assert_eq!(GAP_MS, 2_000);
        assert_eq!(BATCH_GAP_MS, 3_000);
        assert_eq!(QUOTA_BASE, 10);
        assert_eq!(QUOTA_STEP, 5);
        assert_eq!(QUOTA_CAP, 25);
        assert_eq!(CHECK_INTERVAL_MS, 10 * 60 * 1000);
        assert_eq!(FAST_INTERVAL_MS, 3 * 60 * 1000);
        assert_eq!(CONTENTION_RETRY_MS, 60 * 1000);
        assert_eq!(BUSY_COOLDOWN_MS, 30 * 60 * 1000);
        assert_eq!(STARTUP_DELAY_MS, 90 * 1000);
        assert_eq!(FRIEND_TASK_WAIT_MAX_MS, 10_000);
        assert_eq!(FRIEND_TASK_POLL_MS, 250);
        assert_eq!(GATEWAY_IDLE_WAIT_MS, 8_000);
    }

    #[test]
    fn stop_friend_pet_sync_cancels_token() {
        let acc = format!("pet-sync-test-{}", crate::services::friend::visit_strategy::now_ms());
        assert!(!is_friend_pet_sync_active(&acc));
        active_tokens().lock().insert(acc.clone(), CancellationToken::new());
        assert!(is_friend_pet_sync_active(&acc));
        stop_friend_pet_sync(&acc);
        assert!(!is_friend_pet_sync_active(&acc));
    }
}
