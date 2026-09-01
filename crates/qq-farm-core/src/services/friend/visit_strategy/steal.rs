//! 好友土地分析与偷菜操作。

use std::collections::HashSet;
use std::sync::Mutex as StdMutex;


use crate::proto::generated::gamepb::plantpb::LandInfo;
use crate::services::friend::api::FriendApi;

use super::help::RecentHelpCache;

/// 偷菜可偷信息
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StealableInfo {
    pub land_id: i64,
    pub plant_id: i64,
    pub name: String,
}

/// 好友土地分析结果
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResult {
    pub stealable: Vec<i64>,
    pub stealable_info: Vec<StealableInfo>,
    pub need_water: Vec<i64>,
    pub need_weed: Vec<i64>,
    pub need_bug: Vec<i64>,
    pub can_put_weed: Vec<i64>,
    pub can_put_bug: Vec<i64>,
}

/// 偷菜结果
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct StealResult {
    pub ok: usize,
    pub stolen_infos: Vec<StealableInfo>,
    pub score_gained: i64,
}

/// 偷菜可偷的植物信息（plant_id, name）
pub fn get_plant_name(plant_id: i64) -> Option<String> {
    let cfg = crate::config::game_config::global();
    let name = cfg.get_plant_name(plant_id);
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

static ACTIVITY_PLANTS: std::sync::OnceLock<
    StdMutex<std::collections::HashMap<String, std::collections::HashSet<i64>>>,
> = std::sync::OnceLock::new();

fn activity_plants(
) -> &'static StdMutex<std::collections::HashMap<String, std::collections::HashSet<i64>>> {
    ACTIVITY_PLANTS.get_or_init(|| StdMutex::new(std::collections::HashMap::new()))
}

/// 是否活动植物（用于"仅偷活动植物"；按账号隔离）
#[must_use]
pub fn is_activity_plant(account_id: &str, land: &LandInfo) -> bool {
    if account_id.is_empty() {
        return false;
    }
    let plant_id = match land.plant.as_ref() {
        Some(p) => p.id,
        None => return false,
    };
    activity_plants().lock().unwrap().get(account_id).is_some_and(|set| set.contains(&plant_id))
}

/// 标记活动植物（在偷到带活动积分的植物时调用）
pub fn mark_activity_plant(account_id: &str, plant_id: i64) {
    if account_id.is_empty() {
        return;
    }
    activity_plants().lock().unwrap().entry(account_id.to_string()).or_default().insert(plant_id);
}

/// 阶段枚举（与原 TS PlantPhase 对齐）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlantPhase {
    Seed = 0,
    Sprout = 1,
    Growing = 2,
    Ripe = 3,
    Dead = 4,
}

impl PlantPhase {
    #[must_use]
    pub fn from_i32(v: i32) -> Self {
        match v {
            2 => Self::Sprout,
            3 | 4 | 5 => Self::Growing,
            6 => Self::Ripe,
            7 => Self::Dead,
            _ => Self::Seed,
        }
    }
}

/// 获取土地当前阶段（按 begin_time 取当前阶段，对齐 TS `getCurrentPhase`）
#[must_use]
pub fn get_current_phase(land: &LandInfo) -> Option<PlantPhase> {
    let plant = land.plant.as_ref()?;
    if plant.phases.is_empty() {
        return None;
    }
    crate::services::farm::land_analysis::PlantPhase::from_phases(&plant.phases).map(|p| match p {
        crate::services::farm::land_analysis::PlantPhase::Seed => PlantPhase::Seed,
        crate::services::farm::land_analysis::PlantPhase::Sprout => PlantPhase::Sprout,
        crate::services::farm::land_analysis::PlantPhase::Growing => PlantPhase::Growing,
        crate::services::farm::land_analysis::PlantPhase::Ripe => PlantPhase::Ripe,
        crate::services::farm::land_analysis::PlantPhase::Dead => PlantPhase::Dead,
    })
}

/// 是否"被占领的从地块"（对齐 TS `isOccupiedSlaveLand`：master 有植物才跳过）
#[must_use]
pub fn is_occupied_slave_land(
    land: &LandInfo,
    lands_map: &crate::services::farm::land_analysis::LandMap,
) -> bool {
    crate::services::farm::land_analysis::is_occupied_slave_land_with_map(land, lands_map)
}

/// 解析 `PlantInfo.steal_num`（bytes varint）→ 每人最大可偷次数，默认 2
#[must_use]
pub fn parse_max_steal_per_player(steal_num: &[u8]) -> i64 {
    if steal_num.is_empty() {
        return 2;
    }
    let mut v: i64 = 0;
    let mut shift = 0;
    for (i, b) in steal_num.iter().enumerate().take(10) {
        v |= i64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if i == 9 {
            break;
        }
    }
    if v > 0 {
        v
    } else {
        2
    }
}

/// 解析 `PlantInfo.stealers` 中「我」已偷次数
#[must_use]
pub fn my_steal_count_from_plant(
    plant: &crate::proto::generated::gamepb::plantpb::PlantInfo,
    my_gid: i64,
) -> i64 {
    use crate::proto::generated::gamepb::plantpb::StealPlayer;
    use prost::Message;
    let stealers: &[u8] = plant.stealers.as_ref();
    if stealers.is_empty() || stealers[0] != 0x08 {
        return 0;
    }
    match StealPlayer::decode(stealers) {
        Ok(sp) if sp.gid == my_gid => sp.num,
        _ => 0,
    }
}

/// 这块成熟地对我是否仍可偷（stealable + 未达每人上限）
#[must_use]
pub fn can_i_still_steal_plant(
    plant: &crate::proto::generated::gamepb::plantpb::PlantInfo,
    my_gid: i64,
) -> bool {
    if !plant.stealable {
        return false;
    }
    my_steal_count_from_plant(plant, my_gid) < parse_max_steal_per_player(plant.steal_num.as_ref())
}

/// 分析好友土地
#[must_use]
pub fn analyze_friend_lands(
    lands: &[LandInfo],
    my_gid: i64,
    plant_blacklist: &[i64],
    steal_activity_only: bool,
    account_id: &str,
) -> AnalyzeResult {
    let mut result = AnalyzeResult::default();
    let lands_map = crate::services::farm::land_analysis::build_land_map(lands);
    let land_ids: HashSet<i64> = lands.iter().map(|l| l.id).collect();
    for land in lands {
        if is_occupied_slave_land(land, &lands_map) {
            continue;
        }
        let plant = match land.plant.as_ref() {
            Some(p) => p,
            None => continue,
        };
        if plant.phases.is_empty() {
            continue;
        }
        let phase = match get_current_phase(land) {
            Some(p) => p,
            None => continue,
        };
        let id = land.id;

        if phase == PlantPhase::Ripe {
            // 微信没有「每人偷满」；只信服务器 stealable + 成熟主地。
            if plant.stealable {
                let plant_id = plant.id;
                let seed_id = crate::config::game_config::global()
                    .get_plant_by_id(plant_id)
                    .and_then(|p| p.seed_id)
                    .unwrap_or(0);
                if !plant_blacklist.is_empty() && seed_id > 0 && plant_blacklist.contains(&seed_id)
                {
                    continue;
                }
                if steal_activity_only && !is_activity_plant(account_id, land) {
                    continue;
                }
                result.stealable.push(id);
                result.stealable_info.push(StealableInfo {
                    land_id: id,
                    plant_id,
                    name: get_plant_name(plant_id).unwrap_or_else(|| "未知".to_string()),
                });
            }
            continue;
        }

        if phase == PlantPhase::Dead {
            continue;
        }

        if plant.dry_num > 0 {
            result.need_water.push(id);
        }
        if !plant.weed_owners.is_empty() {
            result.need_weed.push(id);
        }
        if !plant.insect_owners.is_empty() {
            result.need_bug.push(id);
        }

        let weed_count = plant.weed_owners.len();
        let insect_count = plant.insect_owners.len();
        let i_put_weed = plant.weed_owners.contains(&my_gid);
        let i_put_bug = plant.insect_owners.contains(&my_gid);
        if weed_count < 2 && !i_put_weed {
            result.can_put_weed.push(id);
        }
        if insect_count < 2 && !i_put_bug {
            result.can_put_bug.push(id);
        }
    }
    let _ = land_ids;
    result
}

/// 从推送/进场土地统计可偷与帮忙数（推送可能只含变化地，调用方应与旧值取 max）。
#[must_use]
pub fn plant_summary_from_lands(
    lands: &[LandInfo],
    my_gid: i64,
) -> super::panel_dto::FriendPlantSummary {
    let status = analyze_friend_lands(lands, my_gid, &[], false, "");
    super::panel_dto::FriendPlantSummary {
        steal_num: status.stealable.len() as i64,
        dry_num: status.need_water.len() as i64,
        weed_num: status.need_weed.len() as i64,
        insect_num: status.need_bug.len() as i64,
    }
}

/// 推送只含变化地时，可偷/帮忙只升不降，避免把未出现在包里的地清掉。
#[must_use]
pub fn merge_partial_plant_summary(
    existing: Option<&super::panel_dto::FriendPlantSummary>,
    incoming: super::panel_dto::FriendPlantSummary,
) -> super::panel_dto::FriendPlantSummary {
    let old = existing.cloned().unwrap_or_default();
    super::panel_dto::FriendPlantSummary {
        steal_num: old.steal_num.max(incoming.steal_num),
        dry_num: old.dry_num.max(incoming.dry_num),
        weed_num: old.weed_num.max(incoming.weed_num),
        insect_num: old.insect_num.max(incoming.insect_num),
    }
}

fn steal_error_code(err: &crate::error::Error) -> Option<i64> {
    match err {
        crate::error::Error::Network(crate::network::error::NetworkError::Gateway {
            code, ..
        }) => Some(*code),
        _ => None,
    }
}

fn is_unstealable_error(err: &crate::error::Error) -> bool {
    steal_error_code(err) == Some(crate::constants::GATEWAY_UNSTEALABLE)
        || err.to_string().contains("1001040")
}

fn is_steal_transient(err: &crate::error::Error) -> bool {
    super::blacklist::is_transient_network_error(&err.to_string())
}

fn record_stolen_land(result: &mut StealResult, land_id: i64, stealable_info: &[StealableInfo]) {
    result.ok += 1;
    if let Some(info) = stealable_info.iter().find(|i| i.land_id == land_id) {
        result.stolen_infos.push(info.clone());
    }
}

/// 先一键 `is_all=true`，失败后再对主地 `is_all=false` 按地回退。
pub async fn steal_lands_with_reward_log(
    api: &FriendApi,
    _recent_help: &RecentHelpCache,
    friend_gid: i64,
    land_ids: &[i64],
    stealable_info: &[StealableInfo],
    _session: Option<()>,
) -> StealResult {
    let mut result = StealResult::default();
    if land_ids.is_empty() {
        return result;
    }
    match api.steal_farm(friend_gid, land_ids.to_vec(), true).await {
        Ok(()) => {
            result.ok = land_ids.len();
            result.stolen_infos = stealable_info.to_vec();
            return result;
        }
        Err(e) if is_steal_transient(&e) => {
            tracing::warn!(friend_gid, error = %e, "一键偷菜网络中断");
            return result;
        }
        Err(_) => {}
    }
    for land_id in land_ids {
        // 对齐 bot visit-strategy.ts:883-892 + api.ts:186-196：逐地兜底也用
        // is_all=true，且每地间 randomDelay(500,800)
        match api.steal_farm(friend_gid, vec![*land_id], true).await {
            Ok(()) => record_stolen_land(&mut result, *land_id, stealable_info),
            Err(e) if is_unstealable_error(&e) => {}
            Err(e) if is_steal_transient(&e) => {
                tracing::warn!(friend_gid, land_id, error = %e, "按地偷菜网络中断");
                break;
            }
            Err(_) => {}
        }
        crate::utils::random::random_delay(500, 800).await;
    }
    result
}

// steal_side_help（偷菜顺手帮忙）已删除：bot 的偷菜流只偷不帮。

/// 已进场后的偷菜动作：QQ 日配额截断 + 一键 `is_all=true`、失败按地主回退 +
/// 日志/统计。从 `visit_friend_for_steal` 与统一巡查 `visit_friend_combined`
/// 共用，避免复制逻辑。
///
/// 返回偷到的地块数；产生的动作文案 push 进 `actions`。
pub async fn perform_steal_actions(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    account_id: &str,
    friend_gid: i64,
    status: &mut AnalyzeResult,
    total_actions: &mut super::help::TotalActions,
    actions: &mut Vec<String>,
) -> usize {
    if status.stealable.is_empty() {
        return 0;
    }
    let mut skip_steal = false;
    // 微信 10008 无限：不调 CheckCanOperate，也不用 can_steal_num 截断。
    if crate::constants::steal_daily_quota_applies(&api.platform()) {
        match api.check_can_operate(friend_gid, crate::constants::OP_STEAL).await {
            Ok((false, _)) => skip_steal = true,
            Ok((true, can_steal)) if can_steal > 0 => {
                let cap = can_steal as usize;
                if status.stealable.len() > cap {
                    status.stealable.truncate(cap);
                    status.stealable_info.truncate(cap);
                }
            }
            _ => {}
        }
    }
    if skip_steal {
        return 0;
    }
    let steal_result = steal_lands_with_reward_log(
        api,
        recent_help,
        friend_gid,
        &status.stealable,
        &status.stealable_info,
        None,
    )
    .await;
    let stolen = steal_result.ok;
    if stolen > 0 {
        let plant_names: Vec<String> = steal_result
            .stolen_infos
            .iter()
            .map(|i| i.name.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let names = plant_names.join("/");
        // 与 `do_steal_op()` 保持一致：当 `score_gained > 0` 时追加价值提示
        let score_hint = if steal_result.score_gained > 0 {
            format!("，获得积分x{}", steal_result.score_gained)
        } else {
            String::new()
        };
        actions.push(if names.is_empty() {
            format!("偷{stolen}{score_hint}")
        } else {
            format!("偷{stolen}({names}){score_hint}")
        });
        total_actions.steal += stolen;
        crate::services::stats::record_operation_for(account_id, "steal", stolen as i64);
        crate::utils::random::random_delay(500, 800).await;
    }
    stolen
}

/// 拜访好友 - 仅偷菜
pub async fn visit_friend_for_steal(
    api: &FriendApi,
    _recent_help: &RecentHelpCache,
    friend: &super::panel_dto::FriendSummary,
    total_actions: &mut super::help::TotalActions,
    my_gid: i64,
    account_id: &str,
) -> Option<super::help::VisitResult> {
    use super::blacklist::{handle_friend_enter_error, FriendEnterErrorKind};
    use super::help::VisitResult;

    let friend_gid = friend.gid;
    let friend_name = friend.name.clone();

    let enter_reply = match api.enter_farm(friend_gid).await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("{e}");
            let kind = handle_friend_enter_error(account_id, friend_gid, &friend_name, &msg);
            if kind != FriendEnterErrorKind::Error {
                return Some(VisitResult { acted: false, entered: false, stolen: 0 });
            }
            crate::services::panel_log::log_warn(
                account_id,
                "好友",
                format!("进入 {friend_name} 农场失败: {msg}"),
                crate::constants::PanelEvent::EnterFarm,
                Some(serde_json::json!({
                    "module": "friend",
                    "result": "error",
                    "friendName": friend_name,
                    "friendGid": friend_gid,
                })),
            );
            return Some(VisitResult { acted: false, entered: false, stolen: 0 });
        }
    };

    let lands = enter_reply.lands.clone();
    if lands.is_empty() {
        let _ = api.leave_farm(friend_gid).await;
        return Some(VisitResult { acted: false, entered: true, stolen: 0 });
    }

    let plant_blacklist =
        crate::models::store::account_config::get_plant_blacklist(Some(account_id));
    let lands_map = crate::services::farm::land_analysis::build_land_map(&lands);
    let has_stealable_before_filter = lands.iter().any(|land| {
        if is_occupied_slave_land(land, &lands_map) {
            return false;
        }
        let plant = match land.plant.as_ref() {
            Some(p) if !p.phases.is_empty() => p,
            _ => return false,
        };
        matches!(get_current_phase(land), Some(PlantPhase::Ripe)) && plant.stealable
    });
    let mut status = analyze_friend_lands(&lands, my_gid, &plant_blacklist, false, account_id);

    if has_stealable_before_filter && status.stealable.is_empty() {
        let _ = api.leave_farm(friend_gid).await;
        return None;
    }

    let mut actions: Vec<String> = Vec::new();
    // 偷菜流只偷不帮（对齐 bot visitFriendForSteal，顺手帮忙已删除）
    let stolen = perform_steal_actions(
        api,
        _recent_help,
        account_id,
        friend_gid,
        &mut status,
        total_actions,
        &mut actions,
    )
    .await;

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
    Some(VisitResult { acted: !actions.is_empty(), entered: true, stolen })
}

pub(crate) async fn do_steal_op(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    friend_gid: i64,
    lands: &[LandInfo],
    my_gid: i64,
) -> serde_json::Value {
    let status = analyze_friend_lands(lands, my_gid, &[], false, "");
    if status.stealable.is_empty() {
        return serde_json::json!({"ok": true, "opType": "steal", "count": 0, "message": "没有可偷取土地"});
    }
    // Harvest 回包不带获得物品，实际所得走紧随的 ItemNotify；窗口内捕获精确数量
    let (result, deltas) = crate::services::item_capture::capture_deltas(
        api.gateway(),
        steal_lands_with_reward_log(
            api,
            recent_help,
            friend_gid,
            &status.stealable,
            &status.stealable_info,
            None,
        ),
    )
    .await;
    let gains = crate::services::item_capture::aggregate_deltas(&deltas, true);
    let items = crate::services::item_capture::gain_dtos(&gains);
    let summary = if result.ok > 0 {
        // 精确数量优先；没有 ItemNotify 时退化用被偷地块的作物名
        let detail = if !gains.is_empty() {
            crate::services::item_capture::format_gains(&gains)
        } else {
            result
                .stolen_infos
                .iter()
                .map(|info| info.name.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join("、")
        };
        if detail.is_empty() {
            format!("偷取 {} 块地", result.ok)
        } else {
            format!("偷取 {} 块地：{detail}", result.ok)
        }
    } else {
        String::new()
    };
    let msg = if result.ok > 0 {
        let score_hint = if result.score_gained > 0 {
            format!("，获得积分x{}", result.score_gained)
        } else {
            String::new()
        };
        format!("一键偷取完成 {} 块{}{}", result.ok, if summary.is_empty() { String::new() } else { format!("（{summary}）") }, score_hint)
    } else {
        "一键偷取失败或无可偷".to_string()
    };
    serde_json::json!({
        "ok": true,
        "opType": "steal",
        "count": result.ok,
        "message": msg,
        "summary": summary,
        "items": items,
    })
}
