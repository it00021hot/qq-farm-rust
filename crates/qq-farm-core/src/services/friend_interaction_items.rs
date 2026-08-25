//! 特殊互动道具：库存发现、协议适配与顺序批量使用（好友农场与自己农场共用）。
//!
//! 1:1 对应原 `core/src/services/friend-interaction-items.ts`：
//! - 库存识别：type=23 + interaction_type=additemuseitem + can_use>0，
//!   白名单 301101/301102/301103 或描述含"好友/他人"；
//! - 好友模式：Enter → 逐地块 `ItemService.Use`（UseTarget.host_gid + land_ids）→ Leave；
//! - 自己模式：仅 301103（SELF_USABLE 白名单），AllLands 快照后逐地块；
//! - 堆叠按过期时间升序消耗；非网关错误触发 BATCH_ABORTED 中断后续地块。

use std::sync::Arc;

use prost::Message as _;

use crate::config::game_config::{global as global_game_config, Item as ItemCfg};
use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::itempb::{UseReply, UseRequest, UseTarget};
use crate::proto::generated::gamepb::plantpb::LandInfo;
use crate::services::warehouse::{get_bag_items, BagItemLite, WarehouseService};

const ITEM_SERVICE: &str = "gamepb.itempb.ItemService";
const SPECIAL_INTERACTION_TYPE: &str = "additemuseitem";
const MAX_BATCH_LANDS: usize = 48;

/// 可以对自己农场使用的互动道具白名单（种草/黄金虫/足球只能作用于他人农场）
const SELF_USABLE_INTERACTION_ITEM_IDS: &[i64] = &[301103];

fn is_friend_interaction_metadata(info: &ItemCfg) -> bool {
    if info.item_type != 23 {
        return false;
    }
    if info.can_use.unwrap_or(0) <= 0 {
        return false;
    }
    if info.interaction_type.as_deref().unwrap_or("").trim().to_lowercase() != SPECIAL_INTERACTION_TYPE {
        return false;
    }
    if [301101i64, 301102, 301103].contains(&info.id) {
        return true;
    }
    // 排除同属 additemuseItem、但描述明确只作用于自己作物的物品
    let desc = info.desc.as_deref().unwrap_or("");
    desc.contains("好友") || desc.contains("他人")
}

fn is_self_interaction_metadata(info: &ItemCfg) -> bool {
    is_friend_interaction_metadata(info) && SELF_USABLE_INTERACTION_ITEM_IDS.contains(&info.id)
}

/// 可用堆叠（按过期时间升序，无过期排最后）
#[derive(Debug, Clone)]
struct UsableStack {
    uid: i64,
    remaining: i64,
    expire_time: i64,
    sale_condition_satisfied: bool,
}

fn stack_sale_condition_satisfied(info: &ItemCfg, expire_time: i64) -> bool {
    let Some(cond) = info.sell_cond.as_ref() else { return false };
    let cond_text = serde_json::to_string(cond).unwrap_or_default();
    let cond_text = cond_text.trim().trim_matches('"');
    if cond_text.is_empty() {
        return false;
    }
    let ctx = crate::config::sell_conditions::SellConditionContext::now(
        crate::utils::time::get_server_time_secs(),
    )
    .with_expire(expire_time);
    crate::config::sell_conditions::is_sell_condition_satisfied(cond_text, &ctx)
}

fn eligible_stacks(bag_items: &[BagItemLite], item_id: i64, info: &ItemCfg) -> Vec<UsableStack> {
    let mut stacks: Vec<UsableStack> = bag_items
        .iter()
        .filter(|s| s.id == item_id && s.uid > 0 && s.count > 0)
        .map(|s| UsableStack {
            uid: s.uid,
            remaining: s.count.max(0),
            expire_time: s.expire_time,
            sale_condition_satisfied: stack_sale_condition_satisfied(info, s.expire_time),
        })
        .collect();
    stacks.sort_by_key(|s| if s.expire_time > 0 { s.expire_time } else { i64::MAX });
    stacks
}

fn item_dto(info: &ItemCfg, stacks: &[UsableStack]) -> serde_json::Value {
    let count: i64 = stacks.iter().map(|s| s.remaining).sum();
    let sale_condition_satisfied_count: i64 = stacks
        .iter()
        .filter(|s| s.sale_condition_satisfied)
        .map(|s| s.remaining)
        .sum();
    let nearest_expire = stacks
        .iter()
        .map(|s| s.expire_time)
        .filter(|t| *t > 0)
        .min()
        .unwrap_or(0);
    let gc = global_game_config();
    serde_json::json!({
        "id": info.id.to_string(),
        "itemId": info.id.to_string(),
        "name": info.name,
        "image": gc.get_item_image_by_id(info.id).unwrap_or_default(),
        "count": count,
        "saleConditionSatisfiedCount": sale_condition_satisfied_count,
        "interactionType": info.interaction_type.clone().unwrap_or_default(),
        "protocol": "item-use",
        "selfUsable": SELF_USABLE_INTERACTION_ITEM_IDS.contains(&info.id),
        "description": info.desc.clone().unwrap_or_default(),
        "sellCondition": info.sell_cond.clone().unwrap_or_else(|| serde_json::json!("")),
        "nearestExpireTime": nearest_expire,
        "serverValidationRequired": true,
    })
}

struct Inventory {
    items: Vec<serde_json::Value>,
    stacks_by_item_id: std::collections::HashMap<i64, Vec<UsableStack>>,
}

async fn collect_inventory(gateway: &Arc<Gateway>) -> Result<Inventory> {
    let bag = WarehouseService::get_bag_via(gateway).await?;
    let bag_items = get_bag_items(&bag);
    let gc = global_game_config();

    let mut item_ids: Vec<i64> = bag_items
        .iter()
        .filter(|s| s.id > 0 && s.count > 0)
        .map(|s| s.id)
        .collect();
    item_ids.sort_unstable();
    item_ids.dedup();

    let mut out = Inventory { items: Vec::new(), stacks_by_item_id: Default::default() };
    for item_id in item_ids {
        let Some(info) = gc.get_item_by_id(item_id) else { continue };
        if !is_friend_interaction_metadata(&info) {
            continue;
        }
        let stacks = eligible_stacks(&bag_items, item_id, &info);
        let dto = item_dto(&info, &stacks);
        if dto["count"].as_i64().unwrap_or(0) <= 0 {
            continue;
        }
        out.items.push(dto);
        out.stacks_by_item_id.insert(item_id, stacks);
    }
    out.items.sort_by(|a, b| {
        let ca = a["count"].as_i64().unwrap_or(0);
        let cb = b["count"].as_i64().unwrap_or(0);
        cb.cmp(&ca).then(
            a["itemId"].as_str().unwrap_or("0").parse::<i64>().unwrap_or(0).cmp(
                &b["itemId"].as_str().unwrap_or("0").parse::<i64>().unwrap_or(0),
            ),
        )
    });
    Ok(out)
}

/// 好友互动道具库存
pub async fn get_friend_interaction_items(gateway: &Arc<Gateway>) -> Result<serde_json::Value> {
    let inv = collect_inventory(gateway).await?;
    let count = inv.items.len();
    Ok(serde_json::json!({
        "items": inv.items,
        "count": count,
        "serverValidationRequired": true,
        "confirmationRequired": true,
        "message": if count > 0 { "请选择好友农场中符合条件的土地使用" } else { "背包中暂无可用于好友土地的特殊互动道具" },
    }))
}

/// 自用互动道具库存（仅 SELF_USABLE 白名单）
pub async fn get_self_interaction_items(gateway: &Arc<Gateway>) -> Result<serde_json::Value> {
    let inv = collect_inventory(gateway).await?;
    let items: Vec<_> = inv
        .items
        .iter()
        .filter(|it| it["selfUsable"].as_bool().unwrap_or(false))
        .cloned()
        .collect();
    let count = items.len();
    Ok(serde_json::json!({
        "items": items,
        "count": count,
        "serverValidationRequired": true,
        "confirmationRequired": true,
        "message": if count > 0 { "请选择自己农场中符合条件的土地使用" } else { "背包中暂无可对自己农场使用的特殊互动道具" },
    }))
}

/// 目标地块：已开垦 + 有作物 + 未枯死；多格作物按主地去重。
fn build_target_land_map(lands: &[LandInfo]) -> std::collections::HashMap<i64, &LandInfo> {
    let mut seen_master: std::collections::HashSet<i64> = Default::default();
    let mut targets: std::collections::HashMap<i64, &LandInfo> = Default::default();
    for land in lands {
        if !land.unlocked {
            continue;
        }
        let Some(plant) = land.plant.as_ref() else { continue };
        if plant.id <= 0 || plant.phases.is_empty() {
            continue;
        }
        if crate::services::farm::land_analysis::is_dead(land) {
            continue;
        }
        // 多格作物：以主地为准，跳过从地
        if land.master_land_id > 0 {
            continue;
        }
        if land.plant.as_ref().map(|p| p.id).unwrap_or(0) > 0
            && !seen_master.insert(land.id)
        {
            continue;
        }
        targets.insert(land.id, land);
    }
    targets
}

async fn send_targeted_use(
    gateway: &Arc<Gateway>,
    item_id: i64,
    stack: &UsableStack,
    host_gid: i64,
    land_id: i64,
) -> Result<UseReply> {
    let req = UseRequest {
        item: Some(CoreItem { id: item_id, count: 1, uid: stack.uid, ..Default::default() }),
        target: Some(UseTarget { host_gid, land_ids: vec![land_id], use_config_id: 0 }),
    };
    let body = gateway.request(ITEM_SERVICE, "Use", &req.encode_to_vec()).await?;
    Ok(UseReply::decode(&body[..])?)
}

/// 单次尝试结果
struct Attempt {
    land_id: i64,
    ok: bool,
    code: String,
    message: String,
    updated_land: Option<serde_json::Value>,
    interaction_effects: Vec<serde_json::Value>,
}

fn land_detail_json(land: &LandInfo) -> serde_json::Value {
    serde_json::json!({
        "id": land.id,
        "unlocked": land.unlocked,
        "plantName": land.plant.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
        "occupiedLandIds": land.slave_land_ids.iter().map(|i| i.to_string()).collect::<Vec<_>>(),
    })
}

/// 回包确认的互动效果：优先解析 PlantInfo.interaction_uses，兜底单条 use-reply 记录
fn confirmed_effects(reply_land: Option<&LandInfo>, item_id: i64, land_id: i64, item_name: &str) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    if let Some(land) = reply_land {
        if let Some(plant) = land.plant.as_ref() {
            for uses in &plant.interaction_uses {
                if uses.item_id == item_id {
                    out.push(serde_json::json!({
                        "landId": land_id.to_string(),
                        "itemId": item_id.to_string(),
                        "itemName": item_name,
                        "effectType": uses.effect_type,
                        "confirmed": true,
                        "source": "use-reply",
                    }));
                }
            }
        }
    }
    if out.is_empty() {
        out.push(serde_json::json!({
            "landId": land_id.to_string(),
            "itemId": item_id.to_string(),
            "itemName": item_name,
            "effectType": 0,
            "confirmed": true,
            "source": "use-reply",
        }));
    }
    out
}

fn is_network_error(err: &Error) -> bool {
    matches!(err, Error::Network(_))
}

/// 批量顺序使用（同一农场会话内）。好友模式由调用方 Enter/Leave。
#[allow(clippy::too_many_arguments)]
async fn run_interaction_batch(
    gateway: &Arc<Gateway>,
    item_id: i64,
    item_name: &str,
    stacks: &mut Vec<UsableStack>,
    host_gid: i64,
    lands: &[LandInfo],
    land_ids: &[i64],
) -> Vec<Attempt> {
    let target_map = build_target_land_map(lands);
    let mut attempts = Vec::new();

    for (index, &land_id) in land_ids.iter().enumerate() {
        if !target_map.contains_key(&land_id) {
            attempts.push(Attempt {
                land_id,
                ok: false,
                code: "FRIEND_INTERACTION_TARGET_UNAVAILABLE".into(),
                message: "所选地块已无可互动作物".into(),
                updated_land: None,
                interaction_effects: vec![],
            });
            continue;
        }
        // 取第一个还有余量的堆叠
        let Some(stack) = stacks.iter_mut().find(|s| s.remaining > 0) else {
            attempts.push(Attempt {
                land_id,
                ok: false,
                code: "FRIEND_INTERACTION_ITEM_DEPLETED".into(),
                message: "本次可用库存已经用完".into(),
                updated_land: None,
                interaction_effects: vec![],
            });
            continue;
        };
        let uid = stack.uid;
        match send_targeted_use(gateway, item_id, &UsableStack { uid, remaining: 1, expire_time: 0, sale_condition_satisfied: false }, host_gid, land_id).await {
            Ok(reply) => {
                stack.remaining -= 1;
                let updated = reply.land.as_ref().map(land_detail_json);
                let effects = confirmed_effects(reply.land.as_ref(), item_id, land_id, item_name);
                attempts.push(Attempt {
                    land_id,
                    ok: true,
                    code: String::new(),
                    message: format!("第 {land_id} 块地使用成功"),
                    updated_land: updated,
                    interaction_effects: effects,
                });
            }
            Err(err) => {
                let code = if let Error::Network(crate::network::error::NetworkError::Gateway { code, .. }) = &err {
                    code.to_string()
                } else {
                    "FRIEND_INTERACTION_USE_FAILED".to_string()
                };
                let message = match code.as_str() {
                    "1001065" => format!("该地块当前不符合{item_name}的使用条件，作物品级或状态可能已变化"),
                    "1003008" => format!("该农场当前已达到{item_name}的使用限制"),
                    _ => format!("服务器未接受该地块的{item_name}使用请求: {err}"),
                };
                attempts.push(Attempt {
                    land_id,
                    ok: false,
                    code,
                    message,
                    updated_land: None,
                    interaction_effects: vec![],
                });
                // 非网关业务错误（网络层中断）→ 中断后续地块
                if is_network_error(&err) {
                    for &rest in &land_ids[index + 1..] {
                        attempts.push(Attempt {
                            land_id: rest,
                            ok: false,
                            code: "FRIEND_INTERACTION_BATCH_ABORTED".into(),
                            message: "前序请求被中断，本地未继续提交该地块".into(),
                            updated_land: None,
                            interaction_effects: vec![],
                        });
                    }
                    break;
                }
            }
        }
    }
    attempts
}

fn normalize_land_ids(input: &[i64]) -> Result<Vec<i64>> {
    let mut unique: Vec<i64> = input.iter().copied().filter(|id| *id > 0).collect();
    unique.sort_unstable();
    unique.dedup();
    if unique.is_empty() {
        return Err(Error::Business("至少选择一块地".into()));
    }
    if unique.len() > MAX_BATCH_LANDS {
        return Err(Error::Business(format!("单次最多选择 {MAX_BATCH_LANDS} 块地")));
    }
    Ok(unique)
}

/// 在好友农场批量使用互动道具（Enter → 逐块 Use → Leave）
pub async fn use_friend_interaction_item_batch(
    gateway: &Arc<Gateway>,
    friend_gid: i64,
    item_id: i64,
    land_ids: &[i64],
) -> Result<serde_json::Value> {
    if friend_gid <= 0 {
        return Err(Error::Business("好友 GID 必须是正整数".into()));
    }
    let land_ids = normalize_land_ids(land_ids)?;
    let gc = global_game_config();
    let Some(info) = gc.get_item_by_id(item_id) else {
        return Err(Error::Business("该物品不是可用于好友土地的特殊互动道具".into()));
    };
    if !is_friend_interaction_metadata(&info) {
        return Err(Error::Business("该物品不是可用于好友土地的特殊互动道具".into()));
    }
    let item_name = info.name.clone();
    let mut stacks = resolve_usable_stocks(gateway, item_id, land_ids.len()).await?;

    let friend_api = crate::services::friend::api::FriendApi::new(gateway.clone());
    let enter = friend_api.enter_farm(friend_gid).await?;
    let attempts = {
        let actual_gid = enter.basic.as_ref().map(|b| b.gid).unwrap_or(0);
        if actual_gid != 0 && actual_gid != friend_gid {
            return Err(Error::Business("进入的好友农场与所选 GID 不一致".into()));
        }
        let lands = enter.lands.clone();
        run_interaction_batch(gateway, item_id, &item_name, &mut stacks, friend_gid, &lands, &land_ids)
            .await
    };
    // Leave 失败不影响结果
    let _ = friend_api.leave_farm(friend_gid).await;

    let owner_name = enter
        .basic
        .as_ref()
        .map(|b| if b.remark.is_empty() { b.name.clone() } else { b.remark.clone() })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("GID:{friend_gid}"));
    finish_batch_response(gateway, attempts, friend_gid, &owner_name, item_id, &item_name, &land_ids, false).await
}

/// 在自己农场批量使用互动道具（仅 SELF_USABLE 白名单）
pub async fn use_self_interaction_item_batch(
    gateway: &Arc<Gateway>,
    host_gid: i64,
    item_id: i64,
    land_ids: &[i64],
) -> Result<serde_json::Value> {
    if host_gid <= 0 {
        return Err(Error::Business("当前账号 GID 不可用".into()));
    }
    let land_ids = normalize_land_ids(land_ids)?;
    let gc = global_game_config();
    let Some(info) = gc.get_item_by_id(item_id) else {
        return Err(Error::Business("该道具只能在好友农场使用，不能对自己的农场使用".into()));
    };
    if !is_self_interaction_metadata(&info) {
        return Err(Error::Business("该道具只能在好友农场使用，不能对自己的农场使用".into()));
    }
    let item_name = info.name.clone();
    let mut stacks = resolve_usable_stocks(gateway, item_id, land_ids.len()).await?;

    let farm_api = crate::services::farm::api::Api::new(gateway.clone());
    let reply = farm_api.get_all_lands(host_gid).await?;
    let lands = reply.lands.clone();
    let attempts =
        run_interaction_batch(gateway, item_id, &item_name, &mut stacks, host_gid, &lands, &land_ids)
            .await;
    finish_batch_response(gateway, attempts, host_gid, "我的农场", item_id, &item_name, &land_ids, true).await
}

async fn resolve_usable_stocks(gateway: &Arc<Gateway>, item_id: i64, land_count: usize) -> Result<Vec<UsableStack>> {
    let inv = collect_inventory(gateway).await?;
    let stacks = inv.stacks_by_item_id.get(&item_id).cloned().unwrap_or_default();
    let available: i64 = stacks.iter().map(|s| s.remaining).sum();
    if available <= 0 {
        return Err(Error::Business(format!("物品{item_id}当前没有可提交服务器校验的库存")));
    }
    if land_count as i64 > available {
        return Err(Error::Business(format!(
            "已选择 {land_count} 块地，但当前只有 {available} 个道具"
        )));
    }
    Ok(stacks)
}

async fn finish_batch_response(
    gateway: &Arc<Gateway>,
    attempts: Vec<Attempt>,
    host_gid: i64,
    owner_name: &str,
    item_id: i64,
    item_name: &str,
    requested: &[i64],
    is_self: bool,
) -> Result<serde_json::Value> {
    let succeeded: Vec<&Attempt> = attempts.iter().filter(|a| a.ok).collect();
    let failed: Vec<&Attempt> = attempts.iter().filter(|a| !a.ok).collect();
    let refreshed = if is_self {
        get_self_interaction_items(gateway).await?
    } else {
        get_friend_interaction_items(gateway).await?
    };
    let updated_lands: Vec<_> = succeeded.iter().filter_map(|a| a.updated_land.clone()).collect();
    let interaction_effects: Vec<_> = succeeded.iter().flat_map(|a| a.interaction_effects.clone()).collect();
    let message = if !failed.is_empty() {
        format!("已在{owner_name}的农场按顺序使用 {} 个{item_name}，跳过 {} 块地", succeeded.len(), failed.len())
    } else {
        format!("已在{owner_name}的农场按顺序使用 {} 个{item_name}", succeeded.len())
    };
    let results: Vec<serde_json::Value> = attempts
        .iter()
        .map(|a| {
            serde_json::json!({
                "landId": a.land_id.to_string(),
                "ok": a.ok,
                "code": a.code,
                "message": a.message,
                "updatedLand": a.updated_land,
                "interactionEffects": a.interaction_effects,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "hostGid": host_gid.to_string(),
        "ownerName": owner_name,
        "isSelf": is_self,
        "itemId": item_id.to_string(),
        "itemName": item_name,
        "protocol": "item-use",
        "requestedLandIds": requested.iter().map(|i| i.to_string()).collect::<Vec<_>>(),
        "usedLandIds": succeeded.iter().map(|a| a.land_id.to_string()).collect::<Vec<_>>(),
        "failedLandIds": failed.iter().map(|a| a.land_id.to_string()).collect::<Vec<_>>(),
        "successCount": succeeded.len(),
        "failureCount": failed.len(),
        "results": results,
        "updatedLands": updated_lands,
        "interactionEffects": interaction_effects,
        "items": refreshed["items"].clone(),
        "message": message,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::game_config::Item;

    fn interaction_item(id: i64, desc: &str) -> Item {
        Item {
            id,
            item_type: 23,
            name: format!("道具{id}"),
            interaction_type: Some("additemuseitem".into()),
            can_use: Some(1),
            desc: Some(desc.into()),
            ..Default::default()
        }
    }

    #[test]
    fn whitelist_and_description_metadata() {
        assert!(is_friend_interaction_metadata(&interaction_item(301101, "")));
        assert!(is_friend_interaction_metadata(&interaction_item(999999, "对好友农场使用")));
        // 描述只作用于自己的不算好友道具
        assert!(!is_friend_interaction_metadata(&interaction_item(999999, "仅自己可用的肥料")));
        // 非 additemuseitem 不算
        let mut other = interaction_item(301101, "");
        other.interaction_type = Some("other".into());
        assert!(!is_friend_interaction_metadata(&other));
    }

    #[test]
    fn self_usable_only_dew() {
        assert!(is_self_interaction_metadata(&interaction_item(301103, "")));
        assert!(!is_self_interaction_metadata(&interaction_item(301101, "")));
    }

    #[test]
    fn land_ids_normalized_and_capped() {
        let ids = vec![5, 3, 5, 0, -1, 4];
        assert_eq!(normalize_land_ids(&ids).unwrap(), vec![3, 4, 5]);
        let too_many: Vec<i64> = (0..50).collect();
        assert!(normalize_land_ids(&too_many).is_err());
        assert!(normalize_land_ids(&[]).is_err());
    }

    #[test]
    fn stacks_sorted_by_expire_time() {
        let info = interaction_item(301103, "");
        let bag = vec![
            BagItemLite { id: 301103, count: 2, uid: 1, expire_time: 0, mutant_types: vec![], locked: false, },
            BagItemLite { id: 301103, count: 1, uid: 2, expire_time: 100, mutant_types: vec![], locked: false, },
            BagItemLite { id: 301103, count: 3, uid: 3, expire_time: 50, mutant_types: vec![], locked: false, },
        ];
        let stacks = eligible_stacks(&bag, 301103, &info);
        let uids: Vec<i64> = stacks.iter().map(|s| s.uid).collect();
        assert_eq!(uids, vec![3, 2, 1]);
    }
}
