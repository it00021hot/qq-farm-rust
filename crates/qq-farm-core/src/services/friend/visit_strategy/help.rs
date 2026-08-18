//! RecentHelp 去重、帮好友务农与拜访主流程。

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use parking_lot::Mutex as PMutex;
use tokio::time::sleep;

use crate::constants::{HELP_CACHE_MAX, HELP_IN_FLIGHT_TTL_MS, HELP_RESULT_TTL_MS};
use crate::proto::generated::gamepb::plantpb::LandInfo;
use crate::services::friend::api::FriendApi;

use super::blacklist::{
    get_account_friend_blacklist, get_plant_blacklist, handle_friend_enter_error,
    FriendEnterErrorKind,
};
use super::now_ms;
use super::panel_dto::FriendSummary;
use super::steal::{analyze_friend_lands, steal_lands_with_reward_log};

/// 帮助状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpState {
    InFlight,
    Confirmed,
    Noop,
}

/// 帮助记录
#[derive(Debug, Clone)]
pub struct RecentHelpEntry {
    pub state: HelpState,
    pub snapshot_key: String,
    pub expires_at: super::ClockMs,
}

/// RecentHelp 缓存
pub struct RecentHelpCache {
    inner: PMutex<HashMap<String, RecentHelpEntry>>,
}

impl Default for RecentHelpCache {
    fn default() -> Self {
        Self::new()
    }
}

impl RecentHelpCache {
    #[must_use]
    pub fn new() -> Self {
        Self { inner: PMutex::new(HashMap::new()) }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }

    #[must_use]
    pub fn make_key(host_gid: i64, land_id: i64) -> String {
        format!("{host_gid}:{land_id}")
    }

    #[must_use]
    pub fn make_snapshot_key(lands: &[LandSnapshot]) -> String {
        lands
            .iter()
            .map(|land| {
                let weeds =
                    land.weed_owners.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
                let insects =
                    land.insect_owners.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
                format!(
                    "{}:{}:{}:{}:{}:{}",
                    land.id, land.plant_id, land.phase, land.dry_num, weeds, insects
                )
            })
            .collect::<Vec<_>>()
            .join("|")
    }

    pub fn prune(&self, now: super::ClockMs) {
        let mut map = self.inner.lock();
        map.retain(|_, entry| entry.expires_at > now);
        while map.len() > HELP_CACHE_MAX {
            if let Some(first_key) = map.keys().next().cloned() {
                map.remove(&first_key);
            } else {
                break;
            }
        }
    }

    pub fn filter(
        &self,
        host_gid: i64,
        land_ids: &[i64],
        snapshot_key: &str,
        now: super::ClockMs,
    ) -> Vec<i64> {
        self.prune(now);
        let mut map = self.inner.lock();
        let mut seen = HashSet::new();
        land_ids
            .iter()
            .filter(|&&id| id > 0)
            .filter(|&&id| seen.insert(id))
            .filter(|&&land_id| {
                let key = Self::make_key(host_gid, land_id);
                match map.get(&key) {
                    None => true,
                    Some(entry) if entry.expires_at <= now => true,
                    Some(entry) if entry.snapshot_key != snapshot_key => {
                        map.remove(&key);
                        true
                    }
                    Some(_) => false,
                }
            })
            .copied()
            .collect()
    }

    pub fn mark(
        &self,
        host_gid: i64,
        land_ids: &[i64],
        state: HelpState,
        ttl_ms: u64,
        snapshot_key: &str,
        now: super::ClockMs,
    ) {
        let mut map = self.inner.lock();
        for &land_id in land_ids {
            let key = Self::make_key(host_gid, land_id);
            map.insert(
                key,
                RecentHelpEntry {
                    state,
                    snapshot_key: snapshot_key.to_string(),
                    expires_at: now + ttl_ms,
                },
            );
        }
        drop(map);
        self.prune(now);
    }

    pub fn release(&self, host_gid: i64, land_ids: &[i64]) {
        let mut map = self.inner.lock();
        for &land_id in land_ids {
            map.remove(&Self::make_key(host_gid, land_id));
        }
    }

    #[must_use]
    pub fn get(&self, host_gid: i64, land_id: i64) -> Option<RecentHelpEntry> {
        self.inner.lock().get(&Self::make_key(host_gid, land_id)).cloned()
    }
}

/// 土地快照（用于 snapshot_key 构造）
#[derive(Debug, Clone, Default)]
pub struct LandSnapshot {
    pub id: i64,
    pub plant_id: i64,
    pub phase: i64,
    pub dry_num: i64,
    pub weed_owners: Vec<i64>,
    pub insect_owners: Vec<i64>,
}

impl LandSnapshot {
    pub fn from_land(land: &crate::proto::generated::gamepb::plantpb::LandInfo) -> Self {
        let plant = land.plant.as_ref();
        let phases = plant.map(|p| &p.phases);
        let phase = phases.and_then(|p| p.last()).map(|p| p.phase as i64).unwrap_or(0);
        Self {
            id: land.id,
            plant_id: plant.map(|p| p.id).unwrap_or(0),
            dry_num: plant.map(|p| p.dry_num).unwrap_or(0),
            weed_owners: plant.map(|p| p.weed_owners.clone()).unwrap_or_default(),
            insect_owners: plant.map(|p| p.insect_owners.clone()).unwrap_or_default(),
            phase,
        }
    }
}

/// 批量操作 fallback（先尝试批量，失败则逐个）
pub async fn run_batch_with_fallback<F, S, FutB, FutS>(
    ids: &[i64],
    batch_fn: F,
    single_fn: S,
) -> usize
where
    F: FnOnce(Vec<i64>) -> FutB,
    S: Fn(i64) -> FutS,
    FutB: std::future::Future<Output = Result<(), crate::error::Error>>,
    FutS: std::future::Future<Output = Result<(), crate::error::Error>>,
{
    let target: Vec<i64> = ids.iter().copied().filter(|&i| i > 0).collect();
    if target.is_empty() {
        return 0;
    }
    if (batch_fn(target.clone()).await).is_ok() {
        return target.len();
    }
    let mut ok = 0usize;
    for id in target {
        if (single_fn(id).await).is_ok() {
            ok += 1;
        }
        sleep(Duration::from_millis(100)).await;
    }
    ok
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct FarmingOutcome {
    pub effect: FarmingEffect,
    pub operation_count: i64,
    pub land_count: usize,
    pub land_ids: Vec<i64>,
    pub operation_limits: Vec<serde_json::Value>,
    pub code: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FarmingEffect {
    #[default]
    Noop,
    Confirmed,
    Uncertain,
}

#[must_use]
pub fn empty_farming_outcome(effect: FarmingEffect) -> FarmingOutcome {
    FarmingOutcome {
        effect,
        operation_count: 0,
        land_count: 0,
        land_ids: Vec::new(),
        operation_limits: Vec::new(),
        code: 0,
    }
}

#[must_use]
pub fn merge_farming_outcomes(outcomes: &[FarmingOutcome]) -> FarmingOutcome {
    let confirmed: Vec<&FarmingOutcome> =
        outcomes.iter().filter(|o| o.effect == FarmingEffect::Confirmed).collect();
    let mut land_ids: Vec<i64> =
        confirmed.iter().flat_map(|o| o.land_ids.iter().copied()).collect();
    land_ids.sort_unstable();
    land_ids.dedup();
    let mut operation_limits: Vec<serde_json::Value> =
        confirmed.iter().flat_map(|o| o.operation_limits.iter().cloned()).collect();
    operation_limits.sort_by_key(|v| serde_json::to_string(v).unwrap_or_default());
    operation_limits.dedup_by(|a, b| {
        serde_json::to_string(a).unwrap_or_default() == serde_json::to_string(b).unwrap_or_default()
    });

    let effect = if !confirmed.is_empty() {
        FarmingEffect::Confirmed
    } else if outcomes.iter().any(|o| o.effect == FarmingEffect::Uncertain) {
        FarmingEffect::Uncertain
    } else {
        FarmingEffect::Noop
    };

    let operation_count: i64 = confirmed.iter().map(|o| o.operation_count).sum();

    FarmingOutcome {
        effect,
        operation_count,
        land_count: land_ids.len(),
        land_ids,
        operation_limits,
        code: 0,
    }
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct VisitResult {
    pub acted: bool,
    pub entered: bool,
}

pub async fn run_farming_with_fallback(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    host_gid: i64,
    ids: &[i64],
    _stop_when_exp_limit: bool,
    snapshot_key: &str,
) -> FarmingOutcome {
    use crate::services::friend::api::{HelpFarmEffect, HelpFarmOutcome};
    let target = recent_help.filter(host_gid, ids, snapshot_key, now_ms());
    if target.is_empty() {
        return empty_farming_outcome(FarmingEffect::Noop);
    }
    recent_help.mark(
        host_gid,
        &target,
        HelpState::InFlight,
        HELP_IN_FLIGHT_TTL_MS,
        snapshot_key,
        now_ms(),
    );
    match api.help_farm(host_gid, target.clone()).await {
        Ok(outcome) if outcome.effect == HelpFarmEffect::Noop => {
            recent_help.release(host_gid, &target);
            empty_farming_outcome(FarmingEffect::Noop)
        }
        Ok(outcome) => {
            let confirmed_ids = outcome.land_ids.clone();
            recent_help.mark(
                host_gid,
                &confirmed_ids,
                HelpState::Confirmed,
                HELP_RESULT_TTL_MS,
                snapshot_key,
                now_ms(),
            );
            let unconfirmed: Vec<i64> =
                target.iter().copied().filter(|id| !confirmed_ids.contains(id)).collect();
            recent_help.release(host_gid, &unconfirmed);
            FarmingOutcome {
                effect: if confirmed_ids.is_empty() {
                    FarmingEffect::Uncertain
                } else {
                    FarmingEffect::Confirmed
                },
                operation_count: outcome.operation_count,
                land_count: confirmed_ids.len(),
                land_ids: confirmed_ids,
                operation_limits: Vec::new(),
                code: outcome.code as i32,
            }
        }
        Err(_) => {
            recent_help.release(host_gid, &target);
            let mut outcomes = Vec::new();
            for land_id in target {
                recent_help.mark(
                    host_gid,
                    &[land_id],
                    HelpState::InFlight,
                    HELP_IN_FLIGHT_TTL_MS,
                    snapshot_key,
                    now_ms(),
                );
                let outcome = match api.help_farm(host_gid, vec![land_id]).await {
                    Ok(HelpFarmOutcome { effect: HelpFarmEffect::Noop, .. }) => {
                        recent_help.release(host_gid, &[land_id]);
                        empty_farming_outcome(FarmingEffect::Noop)
                    }
                    Ok(o) => {
                        recent_help.mark(
                            host_gid,
                            &o.land_ids,
                            HelpState::Confirmed,
                            HELP_RESULT_TTL_MS,
                            snapshot_key,
                            now_ms(),
                        );
                        FarmingOutcome {
                            effect: if o.land_ids.is_empty() {
                                FarmingEffect::Uncertain
                            } else {
                                FarmingEffect::Confirmed
                            },
                            operation_count: o.operation_count,
                            land_count: o.land_ids.len(),
                            land_ids: o.land_ids,
                            operation_limits: Vec::new(),
                            code: o.code as i32,
                        }
                    }
                    Err(_) => {
                        recent_help.release(host_gid, &[land_id]);
                        empty_farming_outcome(FarmingEffect::Uncertain)
                    }
                };
                outcomes.push(outcome);
                sleep(Duration::from_millis(100)).await;
            }
            merge_farming_outcomes(&outcomes)
        }
    }
}

/// 拜访好友（帮 + 偷 + 捣乱，按账号 automation 分派）
pub async fn visit_friend(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    friend: &FriendSummary,
    total_actions: &mut TotalActions,
    my_gid: i64,
    account_id: &str,
    can_get_exp_by_candidates: bool,
) -> VisitResult {
    use crate::services::automation::is_automation_on_for;

    let friend_gid = friend.gid;
    let friend_name = friend.name.clone();

    let enter_reply = match api.enter_farm(friend_gid).await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("{e}");
            let kind = handle_friend_enter_error(account_id, friend_gid, &friend_name, &msg);
            if kind != FriendEnterErrorKind::Error {
                return VisitResult { acted: false, entered: false };
            }
            tracing::warn!(friend_gid, error = %msg, "进入好友农场失败");
            return VisitResult { acted: false, entered: false };
        }
    };

    let lands = enter_reply.lands.clone();
    if lands.is_empty() {
        let _ = api.leave_farm(friend_gid).await;
        return VisitResult { acted: false, entered: true };
    }

    let plant_blacklist = get_plant_blacklist(account_id);
    let friend_blacklist = get_account_friend_blacklist(account_id);
    if friend_blacklist.contains(&friend_gid) {
        let _ = api.leave_farm(friend_gid).await;
        return VisitResult { acted: false, entered: true };
    }
    let status = analyze_friend_lands(&lands, my_gid, &plant_blacklist, false, account_id);
    let snapshot_key = RecentHelpCache::make_snapshot_key(
        &lands.iter().map(LandSnapshot::from_land).collect::<Vec<_>>(),
    );

    let mut actions: Vec<String> = Vec::new();

    let help_enabled = is_automation_on_for(account_id, "friend_help");
    let stop_when_exp_limit = is_automation_on_for(account_id, "friend_help_exp_limit");
    let allow_by_exp = !stop_when_exp_limit || can_get_exp_by_candidates;
    if help_enabled && allow_by_exp {
        let all_help_ids: Vec<i64> = status
            .need_weed
            .iter()
            .chain(status.need_bug.iter())
            .chain(status.need_water.iter())
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if !all_help_ids.is_empty() {
            let outcome = run_farming_with_fallback(
                api,
                recent_help,
                friend_gid,
                &all_help_ids,
                stop_when_exp_limit,
                &snapshot_key,
            )
            .await;
            if outcome.land_count > 0 {
                let mut parts = Vec::new();
                if !status.need_weed.is_empty() {
                    parts.push(format!("草{}", status.need_weed.len()));
                }
                if !status.need_bug.is_empty() {
                    parts.push(format!("虫{}", status.need_bug.len()));
                }
                if !status.need_water.is_empty() {
                    parts.push(format!("水{}", status.need_water.len()));
                }
                actions.push(format!(
                    "一键务农{}块/{}项({})",
                    outcome.land_count,
                    outcome.operation_count,
                    parts.join("/")
                ));
                total_actions.farming += outcome.land_count;
            }
        }
    }

    if is_automation_on_for(account_id, "friend_steal") && !status.stealable.is_empty() {
        let steal_result = steal_lands_with_reward_log(
            api,
            recent_help,
            friend_gid,
            &status.stealable,
            &status.stealable_info,
            None,
        )
        .await;
        if steal_result.ok > 0 {
            let plant_names: Vec<String> = steal_result
                .stolen_infos
                .iter()
                .filter_map(|i| if i.name.is_empty() { None } else { Some(i.name.clone()) })
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            let score_hint = if steal_result.score_gained > 0 {
                format!("+积分x{}", steal_result.score_gained)
            } else {
                String::new()
            };
            actions.push(format!(
                "偷{}{}{}",
                steal_result.ok,
                if plant_names.is_empty() {
                    String::new()
                } else {
                    format!("({})", plant_names.join("/"))
                },
                score_hint
            ));
            total_actions.steal += steal_result.ok;
        }
    }

    if is_automation_on_for(account_id, "friend_bad")
        && api.remaining_bad_times() > 0
        && (!status.can_put_weed.is_empty() || !status.can_put_bug.is_empty())
    {
        if api.remaining_bad_times() > 0 && !status.can_put_weed.is_empty() {
            let remaining = api.remaining_bad_times() as usize;
            let to_process: Vec<i64> =
                status.can_put_weed.iter().copied().take(remaining).collect();
            let n = api.put_weeds(friend_gid, to_process).await.unwrap_or(0);
            if n > 0 {
                actions.push(format!("放草{n}"));
                total_actions.put_weed += n;
            }
            if api.remaining_bad_times() > 0 {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
        if api.remaining_bad_times() > 0 && !status.can_put_bug.is_empty() {
            let remaining = api.remaining_bad_times() as usize;
            let to_process: Vec<i64> = status.can_put_bug.iter().copied().take(remaining).collect();
            let n = api.put_insects(friend_gid, to_process).await.unwrap_or(0);
            if n > 0 {
                actions.push(format!("放虫{n}"));
                total_actions.put_bug += n;
            }
        }
    }

    if !actions.is_empty() {
        tracing::info!(
            friend_gid,
            friend_name = %friend_name,
            actions = ?actions,
            "完成好友拜访"
        );
        crate::services::panel_log::log(
            account_id,
            "好友",
            format!("{friend_name}: {}", actions.join("/")),
            crate::constants::PanelEvent::CareFriend,
            Some(serde_json::json!({
                "module": "friend",
                "friendName": friend_name,
                "friendGid": friend_gid,
                "actions": actions,
            })),
        );
    }

    let _ = api.leave_farm(friend_gid).await;
    VisitResult { acted: !actions.is_empty(), entered: true }
}

/// 拜访好友 - 仅帮助
pub async fn visit_friend_for_help(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    friend: &FriendSummary,
    total_actions: &mut TotalActions,
    my_gid: i64,
    account_id: &str,
    ignore_exp_limit: bool,
    help_auto_disabled: &std::sync::atomic::AtomicBool,
    can_get_exp_by_candidates: bool,
) -> Option<VisitResult> {
    let friend_gid = friend.gid;
    let friend_name = friend.name.clone();
    let stop_when_exp_limit =
        crate::services::automation::is_automation_on_for(account_id, "friend_help_exp_limit")
            && !ignore_exp_limit;
    if !stop_when_exp_limit {
        help_auto_disabled.store(false, std::sync::atomic::Ordering::Release);
    } else if help_auto_disabled.load(std::sync::atomic::Ordering::Acquire) {
        return Some(VisitResult { acted: false, entered: false });
    }

    let enter_reply = match api.enter_farm(friend_gid).await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("{e}");
            let kind = handle_friend_enter_error(account_id, friend_gid, &friend_name, &msg);
            if kind != FriendEnterErrorKind::Error {
                return Some(VisitResult { acted: false, entered: false });
            }
            return Some(VisitResult { acted: false, entered: false });
        }
    };

    let lands = enter_reply.lands.clone();
    if lands.is_empty() {
        let _ = api.leave_farm(friend_gid).await;
        return Some(VisitResult { acted: false, entered: true });
    }

    let status = analyze_friend_lands(&lands, my_gid, &[], false, account_id);
    let snapshot_key = RecentHelpCache::make_snapshot_key(
        &lands.iter().map(LandSnapshot::from_land).collect::<Vec<_>>(),
    );

    let mut actions: Vec<String> = Vec::new();
    let all_help_ids: Vec<i64> = status
        .need_weed
        .iter()
        .chain(status.need_bug.iter())
        .chain(status.need_water.iter())
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let allow_by_exp = !stop_when_exp_limit || can_get_exp_by_candidates;
    if !all_help_ids.is_empty() && allow_by_exp {
        let before_exp = crate::services::status::status_data_for(account_id).exp;
        let outcome = run_farming_with_fallback(
            api,
            recent_help,
            friend_gid,
            &all_help_ids,
            stop_when_exp_limit,
            &snapshot_key,
        )
        .await;
        if outcome.land_count > 0 {
            actions.push(format!("帮{}块", outcome.land_count));
            total_actions.farming += outcome.land_count;
            crate::services::stats::record_operation_for(
                account_id,
                "helpFarming",
                outcome.land_count as i64,
            );
            if stop_when_exp_limit {
                tokio::time::sleep(Duration::from_millis(200)).await;
                let after_exp = crate::services::status::status_data_for(account_id).exp;
                if after_exp <= before_exp {
                    help_auto_disabled.store(true, std::sync::atomic::Ordering::Release);
                    crate::services::panel_log::log(
                        account_id,
                        "好友",
                        "今日帮助经验已达上限，自动停止帮忙",
                        crate::constants::PanelEvent::FriendCycle,
                        Some(serde_json::json!({
                            "module": "friend",
                            "result": "ok",
                        })),
                    );
                }
            }
        }
    }

    if !actions.is_empty() {
        crate::services::panel_log::log(
            account_id,
            "好友",
            format!("{}: {}", friend_name, actions.join("/")),
            crate::constants::PanelEvent::VisitFriend,
            Some(serde_json::json!({
                "module": "friend",
                "result": "ok",
                "friendName": friend_name,
                "friendGid": friend_gid,
                "actions": actions,
            })),
        );
    }

    let _ = api.leave_farm(friend_gid).await;
    Some(VisitResult { acted: !actions.is_empty(), entered: true })
}

/// 总操作计数器（与原 TS `totalActions` 一致）
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct TotalActions {
    pub farming: usize,
    pub steal: usize,
    pub put_weed: usize,
    pub put_bug: usize,
}

/// 面板手动好友操作
pub async fn do_friend_operation(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    friend_gid: i64,
    op: crate::models::types::FriendOperation,
    my_gid: i64,
    account_id: &str,
) -> serde_json::Value {
    if friend_gid == 0 {
        return serde_json::json!({"ok": false, "message": "无效好友ID", "opType": op.as_str()});
    }

    let op_str = op.as_str();

    let enter_reply = match api.enter_farm(friend_gid).await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("{e}");
            let kind = handle_friend_enter_error(
                account_id,
                friend_gid,
                &format!("GID:{friend_gid}"),
                &msg,
            );
            match kind {
                FriendEnterErrorKind::Blacklist => {
                    return serde_json::json!({"ok": true, "opType": op_str, "count": 0, "message": "好友已自动加入黑名单"});
                }
                FriendEnterErrorKind::InvalidRemoved => {
                    return serde_json::json!({"ok": true, "opType": op_str, "count": 0, "message": "好友 GID 已失效"});
                }
                FriendEnterErrorKind::Error => {
                    return serde_json::json!({"ok": false, "opType": op_str, "count": 0, "message": format!("进入好友农场失败: {msg}")});
                }
            }
        }
    };

    let result = match op {
        crate::models::types::FriendOperation::Steal => {
            super::steal::do_steal_op(api, recent_help, friend_gid, &enter_reply.lands, my_gid)
                .await
        }
        crate::models::types::FriendOperation::Farming
        | crate::models::types::FriendOperation::Water
        | crate::models::types::FriendOperation::Weed
        | crate::models::types::FriendOperation::Insecticide => {
            do_farm_op(api, recent_help, friend_gid, op, &enter_reply.lands, my_gid).await
        }
        crate::models::types::FriendOperation::Bad => {
            do_bad_op(api, friend_gid, &enter_reply.lands, my_gid).await
        }
        crate::models::types::FriendOperation::Fertilize => {
            serde_json::json!({"ok": true, "opType": op_str, "count": 0, "message": "施肥功能暂未对接"})
        }
    };
    let _ = api.leave_farm(friend_gid).await;
    result
}

async fn do_farm_op(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    friend_gid: i64,
    op: crate::models::types::FriendOperation,
    lands: &[LandInfo],
    my_gid: i64,
) -> serde_json::Value {
    let status = analyze_friend_lands(lands, my_gid, &[], false, "");
    let land_ids: Vec<i64> = match op {
        crate::models::types::FriendOperation::Farming => status
            .need_weed
            .iter()
            .chain(status.need_bug.iter())
            .chain(status.need_water.iter())
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect(),
        crate::models::types::FriendOperation::Water => status.need_water,
        crate::models::types::FriendOperation::Weed => status.need_weed,
        crate::models::types::FriendOperation::Insecticide => status.need_bug,
        _ => Vec::new(),
    };
    if land_ids.is_empty() {
        return serde_json::json!({"ok": true, "opType": op.as_str(), "count": 0, "message": "没有需要照顾的土地"});
    }
    let snapshot_key = RecentHelpCache::make_snapshot_key(
        &lands.iter().map(LandSnapshot::from_land).collect::<Vec<_>>(),
    );
    let outcome =
        run_farming_with_fallback(api, recent_help, friend_gid, &land_ids, false, &snapshot_key)
            .await;
    serde_json::json!({
        "ok": true,
        "opType": op.as_str(),
        "count": outcome.land_count,
        "landCount": outcome.land_count,
        "operationCount": outcome.operation_count,
        "message": format!("一键务农完成 {} 块 / {} 项操作", outcome.land_count, outcome.operation_count),
    })
}

pub async fn do_bad_op(
    api: &FriendApi,
    friend_gid: i64,
    lands: &[LandInfo],
    my_gid: i64,
) -> serde_json::Value {
    if api.remaining_bad_times() <= 0 {
        return serde_json::json!({"ok": true, "opType": "bad", "count": 0, "bugCount": 0, "weedCount": 0, "message": "今日捣乱次数已达上限", "limitReached": true});
    }
    let status = analyze_friend_lands(lands, my_gid, &[], false, "");
    if status.can_put_bug.is_empty() && status.can_put_weed.is_empty() {
        return serde_json::json!({"ok": true, "opType": "bad", "count": 0, "bugCount": 0, "weedCount": 0, "message": "没有可捣乱土地"});
    }
    let weed_count = if api.remaining_bad_times() > 0 && !status.can_put_weed.is_empty() {
        let remaining = api.remaining_bad_times() as usize;
        let to_process: Vec<i64> = status.can_put_weed.iter().copied().take(remaining).collect();
        api.put_weeds(friend_gid, to_process).await.unwrap_or(0)
    } else {
        0
    };
    let bug_count = if api.remaining_bad_times() > 0 && !status.can_put_bug.is_empty() {
        let remaining = api.remaining_bad_times() as usize;
        let to_process: Vec<i64> = status.can_put_bug.iter().copied().take(remaining).collect();
        api.put_insects(friend_gid, to_process).await.unwrap_or(0)
    } else {
        0
    };
    serde_json::json!({
        "ok": true,
        "opType": "bad",
        "count": bug_count + weed_count,
        "bugCount": bug_count,
        "weedCount": weed_count,
        "message": format!("捣乱完成 虫{}/草{}", bug_count, weed_count),
        "limitReached": api.remaining_bad_times() <= 0,
    })
}
