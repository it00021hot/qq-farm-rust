//! 萌宠成长日记 — 快照 + 写操作（对齐 bot `pet-diary.ts`，bot main 2026-09-11）。
//!
//! - 活动组 `2026090100`：养成 `…01`（GetGroup 读取 + `PetDiaryOperateRequest` 写）、
//!   种子赠礼 `…02`（一键领取 op=21）、拾物小铺 `…03`（兑换 op=1 / 目录 op=7）
//! - 节令小礼走 `SolarTermsService.ClaimSolarTerms`，只允许领取与活动窗口重叠的节令
//! - 数值配置镜像 bot `activity-data/pet-diary-2026090101.json`；素材映射
//!   `pet-diary-assets.json`（path → `desktop-ui/public/activity-assets/pet-diary/<file>`）
//! - 任何可能消耗钻石（道具 1004 / `diamond_cost_count>0` / id 0 / 负数）的成本一律拒绝，
//!   付费刷新锦囊必须带 `expectedPaidRefreshCount` 且发送前复核点券余额

use std::collections::HashMap;
use std::sync::OnceLock;

use prost::Message;

use crate::constants::{
    ACTIVITY_SERVICE, DIAMOND_ITEM_ID, EXCHANGE_SHOP_OPERATE_TYPE, PET_DIARY_ACTIVITY_ID,
    PET_DIARY_BATTLE_OPERATE_TYPE, PET_DIARY_CHALLENGE_ITEM_IDS, PET_DIARY_CLAIM_DOG_OPERATE_TYPE,
    PET_DIARY_CLAIM_STORY_OPERATE_TYPE, PET_DIARY_COMPENSATION_OPERATE_TYPE,
    PET_DIARY_DRAW_OPERATE_TYPE, PET_DIARY_EQUIP_CHARM_OPERATE_TYPE, PET_DIARY_FEED_OPERATE_TYPE,
    PET_DIARY_FRIEND_INFO_OPERATE_TYPE, PET_DIARY_GROUP_ID, PET_DIARY_INITIALIZE_OPERATE_TYPE,
    PET_DIARY_INTERACT_LOG_OPERATE_TYPE, PET_DIARY_MARK_STORIES_OPERATE_TYPE,
    PET_DIARY_OPEN_TREASURE_OPERATE_TYPE, PET_DIARY_PLUNDER_LOG_OPERATE_TYPE,
    PET_DIARY_REFRESH_CHARM_OPERATE_TYPE, PET_DIARY_SEEDS_CLAIM_ALL_OPERATE_TYPE,
    PET_DIARY_SEEDS_ID, PET_DIARY_SHOP_ID, PET_DIARY_SKIP_BATTLE_OPERATE_TYPE,
    QUERY_SHOP_OPERATE_TYPE, SOLAR_TERMS_SERVICE,
};
use crate::error::Result;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::activitypb::{
    ActivityBodyPetTreasureHunt, GetGroupRequest, PetDiaryActivityData, PetDiaryActivityHead,
    PetDiaryGetGroupReply, PetDiaryOperateReply, PetDiaryOperateRequest, PetDiaryShopItemInfo,
    PetTreasureHuntLogEntry, PetTreasureHuntPlunderedLogEntry, PetTreasureHuntTreasure,
};
use crate::proto::generated::gamepb::solartermspb::{ClaimSolarTermsReply, ClaimSolarTermsRequest};
use crate::services::activity_center::dto::SolarTermsDto;
use crate::utils::time::get_server_time_secs;

use super::dto::{item_from_id, positive_decimal, text_content};
use super::error::{ActivityError, ActivityErrorCode};
use super::ActivityCenterService;

// ============ 数值配置（镜像 bot activity-data/pet-diary-2026090101.json） ============

#[derive(serde::Deserialize)]
struct PetCatalogRowBase {
    feed_items: String,
    #[serde(default)]
    growth_adult_threshold: i64,
    #[serde(default)]
    daily_feed_limit: i64,
    #[serde(default)]
    daily_treasure_limit: i64,
}

#[derive(serde::Deserialize)]
struct PetCatalogRowFight {
    #[serde(default)]
    daily_battle_limit: i64,
}

#[derive(serde::Deserialize)]
struct PetCatalogRowCharm {
    charm_id: i64,
    name: String,
    #[serde(default)]
    desc: String,
    #[serde(default)]
    short_desc: String,
    #[serde(default)]
    icon_path: String,
    #[serde(default)]
    use_limit: i64,
}

#[derive(serde::Deserialize)]
struct PetCatalogRowCharmRefresh {
    #[serde(default)]
    free_refresh_daily_limit: i64,
    #[serde(default)]
    manual_refresh_cost_id: i64,
    #[serde(default)]
    manual_refresh_cost_count: i64,
    #[serde(default)]
    manual_refresh_daily_limit: i64,
}

#[derive(serde::Deserialize)]
struct PetCatalog {
    #[serde(rename = "ActivityPetTreasureHuntBase")]
    base: Vec<PetCatalogRowBase>,
    #[serde(rename = "ActivityPetTreasureHuntFight")]
    fight: Vec<PetCatalogRowFight>,
    #[serde(rename = "ActivityPetTreasureHuntCharm")]
    charms: Vec<PetCatalogRowCharm>,
    #[serde(rename = "ActivityPetTreasureCharmRefresh")]
    charm_refresh: Vec<PetCatalogRowCharmRefresh>,
}

fn pet_catalog() -> &'static PetCatalog {
    static CATALOG: OnceLock<PetCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../assets/activity-data/pet-diary-2026090101.json"
        ))
        .expect("解析萌宠成长日记数值配置失败")
    })
}

fn pet_base() -> &'static PetCatalogRowBase {
    pet_catalog().base.first().expect("萌宠成长日记数值配置缺少 Base 行")
}

fn pet_fight() -> &'static PetCatalogRowFight {
    pet_catalog().fight.first().expect("萌宠成长日记数值配置缺少 Fight 行")
}

fn pet_charm_refresh() -> &'static PetCatalogRowCharmRefresh {
    pet_catalog().charm_refresh.first().expect("萌宠成长日记数值配置缺少 CharmRefresh 行")
}

/// "1028:700" → (1028, 700)
fn pet_parse_item_pair(spec: &str) -> Option<(i64, i64)> {
    let (id, count) = spec.split_once(':')?;
    let id: i64 = id.trim().parse().ok()?;
    let count: i64 = count.trim().parse().ok()?;
    Some((id, count))
}

fn pet_feed_costs() -> Vec<(i64, i64)> {
    pet_base().feed_items.split(';').filter_map(pet_parse_item_pair).collect()
}

fn pet_charm_refresh_cost() -> (i64, i64) {
    let row = pet_charm_refresh();
    (row.manual_refresh_cost_id, row.manual_refresh_cost_count)
}

// ============ 素材映射（pet-diary-assets.json → desktop-ui public 路径） ============

#[derive(serde::Deserialize)]
struct PetDiaryAssetEntry {
    path: String,
    file: String,
}

fn pet_asset_url(path: &str) -> String {
    static ASSETS: OnceLock<HashMap<String, String>> = OnceLock::new();
    let key = path.trim_end_matches("/spriteFrame");
    ASSETS
        .get_or_init(|| {
            let entries: Vec<PetDiaryAssetEntry> = serde_json::from_str(include_str!(
                "../../../../../assets/activity-data/pet-diary-assets.json"
            ))
            .expect("解析萌宠成长日记素材映射失败");
            entries
                .into_iter()
                .map(|e| (e.path.trim_end_matches("/spriteFrame").to_string(), e.file))
                .collect()
        })
        .get(key)
        .map(|file| format!("/activity-assets/pet-diary/{file}"))
        .unwrap_or_default()
}

// ============ 错误辅助 ============

fn pet_err(message: &str) -> ActivityError {
    ActivityError { code: ActivityErrorCode::PetDiaryUnavailable, message: message.to_string() }
}

// ============ Group 结构 ============

struct PetGroup {
    pet: PetDiaryActivityData,
    seeds: Option<PetDiaryActivityData>,
    shop: Option<PetDiaryActivityData>,
}

impl PetGroup {
    fn pet_head(&self) -> &PetDiaryActivityHead {
        self.pet.head.as_ref().expect("read_group 已校验 head 存在")
    }

    fn pet_state(&self) -> &ActivityBodyPetTreasureHunt {
        self.pet.pet_treasure_hunt.as_ref().expect("read_group 已校验 pet_treasure_hunt 存在")
    }

    fn seeds_head(&self) -> Option<&PetDiaryActivityHead> {
        self.seeds.as_ref().and_then(|s| s.head.as_ref())
    }

    fn shop_head(&self) -> Option<&PetDiaryActivityHead> {
        self.shop.as_ref().and_then(|s| s.head.as_ref())
    }
}

fn pet_head_active(head: Option<&PetDiaryActivityHead>) -> bool {
    let Some(head) = head else { return false };
    let now = get_server_time_secs();
    head.start_time > 0 && now >= head.start_time && now <= head.end_time
}

// ============ 回包 selector 提取（请求与回包 selector 字段号相差 1，prost 类型自然区分） ============

fn core_items_dto(items: &[CoreItem]) -> Vec<serde_json::Value> {
    items
        .iter()
        .map(|i| serde_json::to_value(item_from_id(i.id, i.count)).unwrap_or_default())
        .collect()
}

fn shop_items_dto(items: &[PetDiaryShopItemInfo]) -> Vec<serde_json::Value> {
    items
        .iter()
        .map(|i| serde_json::to_value(item_from_id(i.id, i.count)).unwrap_or_default())
        .collect()
}

fn pet_reply_result(
    reply: &PetDiaryOperateReply,
    selector: PetSelector,
) -> Option<serde_json::Value> {
    let value = match selector {
        PetSelector::FinishCg => reply.pet_treasure_hunt_finish_cg.as_ref().map(|_| serde_json::json!({})),
        PetSelector::Feed => reply.pet_treasure_hunt_feed.as_ref().map(|r| {
            serde_json::json!({
                "growth": r.growth, "stage": r.stage, "becameAdult": r.became_adult,
                "feedCount": r.feed_count, "times": r.times,
                "costs": core_items_dto(&r.costs), "rewards": core_items_dto(&r.rewards),
            })
        }),
        PetSelector::Draw => reply.pet_treasure_hunt_draw.as_ref().map(|r| {
            serde_json::json!({
                "costs": core_items_dto(&r.costs),
                "treasureTotal": r.treasure_total.to_string(),
                "treasureCount": r.treasure_count,
                "rewards": core_items_dto(&r.rewards), "times": r.times,
            })
        }),
        PetSelector::ClaimStory => reply.pet_treasure_hunt_claim_story.as_ref().map(|r| {
            serde_json::json!({
                "awards": core_items_dto(&r.awards), "desc": r.desc,
            })
        }),
        PetSelector::RefreshCharmPool => {
            reply.pet_treasure_hunt_refresh_charm_pool.as_ref().map(|r| {
                serde_json::json!({
                    "charmDailyPool": r.charm_daily_pool,
                    "refreshTs": r.refresh_ts,
                    "costs": core_items_dto(&r.costs),
                    "freeRefresh": r.free_refresh,
                    "freeRefreshCount": r.free_refresh_count,
                    "paidRefreshCount": r.paid_refresh_count,
                })
            })
        }
        PetSelector::EquipCharms => reply.pet_treasure_hunt_equip_charms.as_ref().map(|r| {
            serde_json::json!({ "charmEquipped": r.charm_equipped })
        }),
        PetSelector::StartBattle => reply.pet_treasure_hunt_start_battle.as_ref().map(|r| {
            serde_json::json!({
                "won": r.won,
                "plundered": r.plundered.as_ref().map(|p| serde_json::to_value(item_from_id(p.id, p.count)).unwrap_or_default()),
                "defenderName": r.defender_name,
                "streakTriggered": r.streak_triggered,
                "streakReward": r.streak_reward.as_ref().map(|p| serde_json::to_value(item_from_id(p.id, p.count)).unwrap_or_default()),
                "rewards": core_items_dto(&r.rewards),
                "attackerDice": r.attacker_dice, "defenderDice": r.defender_dice,
                "charmResult": r.charm_result.as_ref().map(|c| serde_json::json!({
                    "attackerCharmIds": c.attacker_charm_ids,
                    "defenderCharmIds": c.defender_charm_ids,
                })),
                "needRefresh": r.need_refresh, "isFake": r.is_fake, "refreshReason": r.refresh_reason,
            })
        }),
        PetSelector::GetLog => reply.pet_treasure_hunt_get_log.as_ref().map(|_| serde_json::json!({})),
        PetSelector::GetPlunderedLog => reply
            .pet_treasure_hunt_get_plundered_log
            .as_ref()
            .map(|_| serde_json::json!({})),
        PetSelector::OpenTreasure => reply.pet_treasure_hunt_open_treasure.as_ref().map(|r| {
            serde_json::json!({ "rewards": core_items_dto(&r.rewards) })
        }),
        PetSelector::ClaimPlunderCompensation => {
            reply.pet_treasure_hunt_claim_plunder_compensation.as_ref().map(|r| {
                serde_json::json!({ "rewards": core_items_dto(&r.rewards) })
            })
        }
        PetSelector::GetFriendActivityInfo => reply
            .pet_treasure_hunt_get_friend_activity_info
            .as_ref()
            .map(|_| serde_json::json!({})),
        PetSelector::ClaimDog => reply.pet_treasure_hunt_claim_dog.as_ref().map(|r| {
            serde_json::json!({ "dogId": r.dog_id.to_string() })
        }),
        PetSelector::MarkStoryAnimated => {
            reply.pet_treasure_hunt_mark_story_animated.as_ref().map(|_| serde_json::json!({}))
        }
        PetSelector::SetSkipBattleCg => reply.pet_treasure_hunt_set_skip_battle_cg.as_ref().map(|r| {
            serde_json::json!({ "isSkipBattleCg": r.is_skip_battle_cg })
        }),
        PetSelector::ShopBuy => reply.shop_buy.as_ref().map(|r| {
            serde_json::json!({
                "awards": core_items_dto(&r.awards), "costs": core_items_dto(&r.costs),
            })
        }),
        PetSelector::MegaEventClaimAll => reply.mega_event_claim_all.as_ref().map(|r| {
            serde_json::json!({
                "newlyClaimedDays": r.newly_claimed_days,
                "awards": core_items_dto(&r.awards),
            })
        }),
    };
    value
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PetSelector {
    FinishCg,
    Feed,
    Draw,
    GetLog,
    ClaimStory,
    RefreshCharmPool,
    EquipCharms,
    StartBattle,
    GetPlunderedLog,
    OpenTreasure,
    ClaimPlunderCompensation,
    GetFriendActivityInfo,
    ClaimDog,
    MarkStoryAnimated,
    SetSkipBattleCg,
    ShopBuy,
    MegaEventClaimAll,
}

// ============ 参数解析辅助 ============

fn pet_param_string(params: &serde_json::Value, key: &str) -> String {
    match params.get(key) {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn pet_param_i64(params: &serde_json::Value, key: &str) -> Option<i64> {
    let text = pet_param_string(params, key);
    if text.is_empty() {
        None
    } else {
        text.parse().ok()
    }
}

fn pet_param_bool(params: &serde_json::Value, key: &str) -> Option<bool> {
    params.get(key).and_then(serde_json::Value::as_bool)
}

// ============ 成本 / 余额 ============

async fn pet_balances(service: &ActivityCenterService) -> Result<HashMap<i64, i64>> {
    let warehouse = service.warehouse.lock().clone();
    let bag = if let Some(wh) = warehouse {
        wh.get_bag().await?
    } else {
        crate::services::warehouse::WarehouseService::get_bag_via(&service.gateway).await?
    };
    let mut balances: HashMap<i64, i64> = HashMap::new();
    for item in crate::services::warehouse::get_bag_items(&bag) {
        *balances.entry(item.id).or_insert(0) += item.count.max(0);
    }
    Ok(balances)
}

/// 同币种合并后与背包比较；含钻石（1004 / id 0 / 负数）的成本一律拒绝。
fn pet_costs_available(
    costs: &[(i64, i64)],
    balances: Option<&HashMap<i64, i64>>,
    count: i64,
) -> bool {
    let Some(balances) = balances else { return false };
    if costs.is_empty() {
        return false;
    }
    let mut totals: HashMap<i64, i64> = HashMap::new();
    for &(id, cnt) in costs {
        if id == DIAMOND_ITEM_ID || id == 0 || cnt <= 0 {
            return false;
        }
        *totals.entry(id).or_insert(0) += cnt.saturating_mul(count);
    }
    totals.into_iter().all(|(id, amount)| balances.get(&id).copied().unwrap_or(0) >= amount)
}

// ============ 宝藏 / 日志 DTO ============

fn pet_treasure_dto(treasure: &PetTreasureHuntTreasure) -> serde_json::Value {
    serde_json::json!({
        "id": treasure.id,
        "status": treasure.status,
        "item": serde_json::to_value(item_from_id(treasure.item_id, treasure.count)).unwrap_or_default(),
        "protectedCount": treasure.protected_count.to_string(),
        "originalCount": treasure.original_count.to_string(),
        "maxCount": treasure.max_count.to_string(),
        "startTime": treasure.start_at.saturating_mul(1000),
        "endTime": treasure.end_at.saturating_mul(1000),
        "createdTime": treasure.created_at.saturating_mul(1000),
        "sourceCharmIds": treasure.source_charm_ids,
        "plunderCount": treasure.plunder_count,
        "maxPlunderCount": treasure.max_plunder_count,
        "previews": treasure.battle_previews.iter().map(|p| serde_json::json!({
            "challengeId": p.challenge_item_id.to_string(),
            "canStart": p.can_start,
            "maxProfit": p.max_profit.as_ref().map(|i| serde_json::to_value(item_from_id(i.id, i.count)).unwrap_or_default()),
            "maxLoss": p.max_loss.as_ref().map(|i| serde_json::to_value(item_from_id(i.id, i.count)).unwrap_or_default()),
            "plunderableCount": p.plunderable_count.to_string(),
        })).collect::<Vec<_>>(),
    })
}

fn pet_interact_log_dto(entry: &PetTreasureHuntLogEntry) -> serde_json::Value {
    serde_json::json!({
        "time": entry.ts.saturating_mul(1000),
        "type": entry.r#type,
        "costs": core_items_dto(&entry.costs),
        "rewards": core_items_dto(&entry.rewards),
        "dogId": entry.dog_id.to_string(),
        "skins": entry.dog_skin_ids.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    })
}

fn pet_plunder_log_dto(entry: &PetTreasureHuntPlunderedLogEntry) -> serde_json::Value {
    serde_json::json!({
        "time": entry.ts.saturating_mul(1000),
        "attackerGid": entry.attacker_gid.to_string(),
        "name": entry.attacker_name,
        "won": entry.attacker_won,
        "treasureId": entry.treasure_id,
        "challenge": serde_json::to_value(item_from_id(entry.challenge_item_id, 1)).unwrap_or_default(),
        "level": entry.attacker_level,
        "attackerCharms": entry.attacker_charm,
        "defenderCharms": entry.defender_charm,
        "lost": core_items_dto(&entry.lost_items),
        "injected": core_items_dto(&entry.injected_items),
        "fake": entry.is_fake,
    })
}

// ============ normalize ============

fn pet_desc_json(desc: &str) -> serde_json::Value {
    serde_json::from_str(desc).unwrap_or_else(|_| serde_json::json!({}))
}

impl ActivityCenterService {
    /// 读取活动组并拆出 pet / seeds / shop 三个子活动。
    async fn pet_read_group(&self) -> Result<PetGroup> {
        let body = self
            .gateway
            .request(
                ACTIVITY_SERVICE,
                "GetGroup",
                &GetGroupRequest { group_id: PET_DIARY_GROUP_ID }.encode_to_vec(),
            )
            .await?;
        let reply = PetDiaryGetGroupReply::decode(&body[..])?;
        let group = reply.group.ok_or_else(|| pet_err("服务端未返回萌宠成长日记活动"))?;
        let head = group.head.as_ref().ok_or_else(|| pet_err("服务端未返回萌宠成长日记活动"))?;
        if head.id != PET_DIARY_GROUP_ID {
            return Err(pet_err("服务端未返回萌宠成长日记活动").into());
        }
        let mut pet = None;
        let mut seeds = None;
        let mut shop = None;
        for child in group.children {
            let Some(child_head) = child.head.as_ref() else { continue };
            match child_head.id {
                PET_DIARY_ACTIVITY_ID => pet = Some(child),
                PET_DIARY_SEEDS_ID => seeds = Some(child),
                PET_DIARY_SHOP_ID => shop = Some(child),
                _ => {}
            }
        }
        let pet = pet
            .filter(|c| c.pet_treasure_hunt.is_some() && c.head.is_some())
            .ok_or_else(|| pet_err("服务端未返回萌宠养成状态"))?;
        Ok(PetGroup { pet, seeds, shop })
    }

    /// 发送 Operate 并校验回包 id / type；带 selector 时校验结果字段存在。
    /// 目录查询（op=7）无结果 selector，传 `None`。
    async fn pet_operate(
        &self,
        request: PetDiaryOperateRequest,
        selector: Option<PetSelector>,
    ) -> Result<(PetDiaryOperateReply, serde_json::Value)> {
        let activity_id = request.activity_id;
        let operate_type = request.operate_type;
        let body =
            self.gateway.request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec()).await?;
        let reply = PetDiaryOperateReply::decode(&body[..])?;
        if reply.activity_id != activity_id || reply.operate_type != operate_type {
            return Err(pet_err("活动响应不匹配，请刷新后查看结果").into());
        }
        let result = match selector {
            Some(selector) => pet_reply_result(&reply, selector)
                .ok_or_else(|| pet_err("活动响应缺少操作结果，请刷新后查看结果"))?,
            None => serde_json::json!({}),
        };
        Ok((reply, result))
    }

    fn pet_normalize(
        &self,
        group: &PetGroup,
        balances: Option<&HashMap<i64, i64>>,
        solar: Option<&SolarTermsDto>,
        warnings: Vec<String>,
    ) -> serde_json::Value {
        let base = pet_base();
        let fight = pet_fight();
        let refresh = pet_charm_refresh();
        let state = group.pet_state();
        let nurture = state.nurture.as_ref();
        let hunt = state.hunt.as_ref();
        let battle = state.battle.as_ref();
        let head = group.pet_head();
        let now = get_server_time_secs();
        let active = head.start_time > 0 && now >= head.start_time && now <= head.end_time;
        let adult = nurture.map(|n| n.stage == 2).unwrap_or(false);
        let stage = nurture.map(|n| n.stage).unwrap_or(0);
        let feed_count = state.feed.as_ref().map(|f| i64::from(f.feed_count)).unwrap_or(0);

        let charm_dto = |id: i64| {
            let config = pet_catalog().charms.iter().find(|c| c.charm_id == id);
            let remaining = battle
                .map(|b| {
                    b.charm_effect_remaining_count
                        .iter()
                        .filter(|e| i64::from(e.charm_id) == id)
                        .map(|e| e.remaining_count)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            serde_json::json!({
                "id": id,
                "name": config.map(|c| c.name.clone()).filter(|n| !n.is_empty())
                    .unwrap_or_else(|| format!("锦囊 {id}")),
                "description": config.map(|c| c.desc.clone()).unwrap_or_default(),
                "shortDescription": config
                    .map(|c| if c.short_desc.is_empty() { c.desc.clone() } else { c.short_desc.clone() })
                    .unwrap_or_default(),
                "useLimit": config.map(|c| c.use_limit).unwrap_or(0),
                "image": pet_asset_url(&config.map(|c| c.icon_path.clone()).unwrap_or_default()),
                "remaining": remaining,
            })
        };

        let feed_costs = pet_feed_costs();
        let treasure_costs: Vec<(i64, i64)> = hunt
            .map(|h| h.treasure_cost.iter().map(|i| (i.id, i.count)).collect())
            .unwrap_or_default();
        let charm_pool = battle.map(|b| b.charm_daily_pool.clone()).unwrap_or_default();
        let charm_equipped = battle.map(|b| b.charm_equipped.clone()).unwrap_or_default();
        let charm_pick_used = battle.map(|b| b.charm_pick_used).unwrap_or(false);
        let charm_needs_choice = !charm_equipped.is_empty() && !charm_pick_used;
        let free_refresh_remaining = (refresh.free_refresh_daily_limit
            - battle.map(|b| b.charm_free_refresh_count).unwrap_or(0))
        .max(0);
        let paid_refresh_count = battle.map(|b| b.charm_paid_refresh_count).unwrap_or(0);
        let paid_refresh_remaining =
            (refresh.manual_refresh_daily_limit - paid_refresh_count).max(0);
        let (refresh_cost_id, refresh_cost_count) = pet_charm_refresh_cost();

        let goods: Vec<serde_json::Value> = group
            .shop
            .as_ref()
            .and_then(|s| s.shop.as_ref())
            .map(|shop| {
                let mut goods: Vec<serde_json::Value> = shop
                    .goods
                    .iter()
                    .map(|g| {
                        let costs: Vec<(i64, i64)> = g.cost.iter().map(|c| (c.id, c.count)).collect();
                        let remaining = if g.purchase_limit > 0 {
                            Some((g.purchase_limit - g.purchased_count).max(0).to_string())
                        } else {
                            None
                        };
                        let safe_costs = !g.cost.is_empty()
                            && g.cost.iter().all(|c| c.id != DIAMOND_ITEM_ID)
                            && g.diamond_cost_count == 0;
                        let desc = pet_desc_json(&g.desc);
                        let image = pet_asset_url(
                            desc.get("res").and_then(serde_json::Value::as_str).unwrap_or(""),
                        );
                        let image = if image.is_empty() {
                            g.item.first()
                                .map(|i| item_from_id(i.id, i.count).image)
                                .unwrap_or_default()
                        } else {
                            image
                        };
                        serde_json::json!({
                            "id": g.id.to_string(),
                            "name": g.name,
                            "image": image,
                            "rewards": shop_items_dto(&g.item),
                            "costs": shop_items_dto(&g.cost),
                            "limit": g.purchase_limit.to_string(),
                            "purchased": g.purchased_count.to_string(),
                            "remaining": remaining,
                            "exchangeable": active
                                && pet_head_active(group.shop_head())
                                && safe_costs
                                && remaining.as_deref() != Some("0")
                                && pet_costs_available(&costs, balances, 1),
                            "safeCosts": safe_costs,
                            "order": g.order,
                            "category": if g.category_tag.is_empty() { "游记好礼".to_string() } else { g.category_tag.clone() },
                        })
                    })
                    .collect();
                goods.sort_by_key(|g| g.get("order").and_then(serde_json::Value::as_i64).unwrap_or(0));
                goods
            })
            .unwrap_or_default();

        let seeds_rewards = group
            .seeds
            .as_ref()
            .and_then(|s| s.mega_event.as_ref())
            .map(|m| m.rewards.as_slice())
            .unwrap_or(&[]);
        let can_claim_seeds = seeds_rewards.iter().any(|r| r.claimable && !r.claimed);

        let stories: Vec<serde_json::Value> = state
            .story
            .as_ref()
            .map(|s| {
                s.stories
                    .iter()
                    .map(|story| {
                        let desc = pet_desc_json(&story.selected_desc);
                        serde_json::json!({
                            "order": story.order,
                            "unlocked": story.unlocked,
                            "claimed": story.claimed,
                            "animated": story.animated,
                            "photo": pet_asset_url(desc.get("photo").and_then(serde_json::Value::as_str).unwrap_or("")),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let treasures: Vec<serde_json::Value> = state
            .pool
            .as_ref()
            .map(|p| p.treasures.iter().map(pet_treasure_dto).collect())
            .unwrap_or_default();

        let balances_list: Vec<serde_json::Value> = [1028, 1029, 80101, 80102, 80103, 1002]
            .iter()
            .map(|&id| {
                let count = balances.map(|b| b.get(&id).copied().unwrap_or(0)).unwrap_or(0);
                let mut dto = serde_json::to_value(item_from_id(id, count)).unwrap_or_default();
                if let Some(obj) = dto.as_object_mut() {
                    obj.insert("known".into(), serde_json::json!(balances.is_some()));
                }
                dto
            })
            .collect();

        let rules = text_content(head.desc.as_bytes());
        let desc_json = pet_desc_json(&head.desc);
        let tips2 = serde_json::to_string(
            &desc_json.get("tips2").cloned().unwrap_or_else(|| serde_json::json!({})),
        )
        .unwrap_or_else(|_| "{}".to_string());
        let treasure_rules = text_content(format!(r#"{{"tips":{tips2}}}"#).as_bytes());

        let solar_terms = solar.map(|s| {
            let mut value = serde_json::to_value(s).unwrap_or_default();
            if let Some(terms) = value.get_mut("terms").and_then(serde_json::Value::as_array_mut) {
                terms.retain(|term| {
                    let end = term.get("endTime").and_then(serde_json::Value::as_i64).unwrap_or(0);
                    let start =
                        term.get("startTime").and_then(serde_json::Value::as_i64).unwrap_or(0);
                    end >= head.start_time && start <= head.end_time
                });
            }
            value
        });

        let plants_list: Vec<serde_json::Value> = [20516, 29004, 25995, 21625, 20154, 21072]
            .iter()
            .map(|&id| {
                let count = balances.map(|b| b.get(&id).copied().unwrap_or(0)).unwrap_or(0);
                serde_json::to_value(item_from_id(id, count)).unwrap_or_default()
            })
            .collect();

        serde_json::json!({
            "activityId": PET_DIARY_ACTIVITY_ID.to_string(),
            "groupId": PET_DIARY_GROUP_ID.to_string(),
            "title": "萌宠成长日记",
            "active": active,
            "startTime": head.start_time.saturating_mul(1000),
            "endTime": head.end_time.saturating_mul(1000),
            "serverTime": now.saturating_mul(1000),
            "rules": rules.get("paragraphs").cloned().unwrap_or_else(|| serde_json::json!([])),
            "treasureRules": treasure_rules.get("paragraphs").cloned().unwrap_or_else(|| serde_json::json!([])),
            "balances": balances_list,
            "warnings": warnings,
            "nurture": {
                "initialized": nurture.map(|n| n.cg_played).unwrap_or(false),
                "adult": adult,
                "growth": nurture.map(|n| n.growth).unwrap_or(0),
                "adultGrowth": base.growth_adult_threshold,
                "dogGranted": nurture.map(|n| n.dog_granted).unwrap_or(false),
                "feedCount": feed_count,
                "feedLimit": base.daily_feed_limit,
                "feedCosts": feed_costs.iter().map(|&(id, count)| item_from_id(id, count))
                    .map(|dto| serde_json::to_value(dto).unwrap_or_default()).collect::<Vec<_>>(),
                "canFeed": active && !adult && stage == 1
                    && feed_count < base.daily_feed_limit
                    && pet_costs_available(&feed_costs, balances, 1),
            },
            "hunt": {
                "count": hunt.map(|h| h.treasure_count).unwrap_or(0),
                "limit": base.daily_treasure_limit,
                "total": hunt.map(|h| h.treasure_total.to_string()).unwrap_or_else(|| "0".to_string()),
                "luckyStarTotal": hunt.map(|h| h.lucky_star_gained_total.to_string())
                    .unwrap_or_else(|| "0".to_string()),
                "costs": treasure_costs.iter().map(|&(id, count)| item_from_id(id, count))
                    .map(|dto| serde_json::to_value(dto).unwrap_or_default()).collect::<Vec<_>>(),
                "canDraw": active && adult
                    && hunt.map(|h| i64::from(h.treasure_count)).unwrap_or(0) < base.daily_treasure_limit
                    && pet_costs_available(&treasure_costs, balances, 1),
                "canPlunder": active
                    && hunt.map(|h| h.can_play_plunder).unwrap_or(false)
                    && battle.map(|b| i64::from(b.battle_count)).unwrap_or(0) < fight.daily_battle_limit,
            },
            "seeds": {
                "canClaim": active && pet_head_active(group.seeds_head()) && can_claim_seeds,
                "days": seeds_rewards.iter().map(|r| serde_json::json!({
                    "day": r.unlock_day,
                    "claimed": r.claimed,
                    "claimable": r.claimable,
                    "rewards": r.reward.iter().map(|i| serde_json::to_value(item_from_id(i.id, i.count)).unwrap_or_default()).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            },
            "stories": stories,
            "charms": {
                "pool": charm_pool.iter().map(|&id| charm_dto(i64::from(id))).collect::<Vec<_>>(),
                "equipped": charm_equipped.iter().map(|&id| charm_dto(i64::from(id))).collect::<Vec<_>>(),
                "all": pet_catalog().charms.iter().map(|c| charm_dto(c.charm_id)).collect::<Vec<_>>(),
                "picked": charm_pick_used,
                "canChoose": active && adult && !charm_pick_used && !charm_pool.is_empty(),
                "freeRefreshRemaining": free_refresh_remaining,
                "freeRefreshLimit": refresh.free_refresh_daily_limit,
                "paidRefreshCount": paid_refresh_count,
                "paidRefreshRemaining": paid_refresh_remaining,
                "paidRefreshLimit": refresh.manual_refresh_daily_limit,
                "refreshCost": serde_json::to_value(item_from_id(refresh_cost_id, refresh_cost_count)).unwrap_or_default(),
                "refreshBalance": balances.map(|b| b.get(&refresh_cost_id).copied().unwrap_or(0).to_string()),
                "canRefresh": active && adult && !charm_needs_choice
                    && (free_refresh_remaining > 0
                        || (paid_refresh_remaining > 0
                            && pet_costs_available(&[pet_charm_refresh_cost()], balances, 1))),
                "refreshNote": format!(
                    "每日免费 {} 次，之后每次 {} 点券，今日还可付费刷新 {} 次。点券不足时不刷新。",
                    refresh.free_refresh_daily_limit, refresh_cost_count, paid_refresh_remaining
                ),
            },
            "treasures": treasures,
            "compensationCount": state
                .plunder
                .as_ref()
                .map(|p| p.plunder_compensation_count.to_string())
                .unwrap_or_else(|| "0".to_string()),
            "battleCount": battle.map(|b| b.battle_count).unwrap_or(0),
            "battleLimit": fight.daily_battle_limit,
            "skipBattle": battle.map(|b| b.is_skip_battle_cg).unwrap_or(false),
            "shop": goods,
            "solarTerms": solar_terms,
            "plants": plants_list,
        })
    }

    /// 快照 = GetGroup + 小铺目录(op=7) + 背包 + 节令；各源容错收集 warnings。
    async fn pet_read_snapshot(&self) -> Result<serde_json::Value> {
        let mut group = self.pet_read_group().await?;
        let mut warnings: Vec<String> = Vec::new();
        match self
            .pet_operate(
                PetDiaryOperateRequest {
                    activity_id: PET_DIARY_SHOP_ID,
                    operate_type: QUERY_SHOP_OPERATE_TYPE,
                    ..Default::default()
                },
                None,
            )
            .await
        {
            Ok((reply, _)) => {
                if reply.data.as_ref().and_then(|d| d.shop.as_ref()).is_some() {
                    group.shop = reply.data;
                } else {
                    warnings.push("拾物小铺：目录缺失".to_string());
                }
            }
            Err(error) => warnings.push(format!("拾物小铺：{error}")),
        }
        let balances = match pet_balances(self).await {
            Ok(balances) => Some(balances),
            Err(_) => {
                warnings.push("背包读取失败，消耗资源的操作已暂停".to_string());
                None
            }
        };
        let solar = match self.get_current_solar_terms().await {
            Ok(solar) => Some(solar),
            Err(_) => {
                warnings.push("节令小礼读取失败，请稍后刷新".to_string());
                None
            }
        };
        Ok(self.pet_normalize(&group, balances.as_ref(), solar.as_ref(), warnings))
    }

    /// 萌宠成长日记快照（独立单飞，不进活动中心总快照；对齐 bot `getPetDiary`）。
    pub async fn get_pet_diary(&self) -> Result<serde_json::Value> {
        let _guard = self.pet_snapshot_lock.lock().await;
        self.pet_read_snapshot().await
    }

    /// 互动 / 被夺日志（interact op=31 / plunder op=44）。
    pub async fn get_pet_diary_records(&self, kind: &str) -> Result<Vec<serde_json::Value>> {
        let operate_type = match kind {
            "interact" => PET_DIARY_INTERACT_LOG_OPERATE_TYPE,
            "plunder" => PET_DIARY_PLUNDER_LOG_OPERATE_TYPE,
            _ => return Err(pet_err("未知记录类型").into()),
        };
        let (reply, _) = self
            .pet_operate(
                PetDiaryOperateRequest {
                    activity_id: PET_DIARY_ACTIVITY_ID,
                    operate_type,
                    ..Default::default()
                },
                Some(if kind == "interact" {
                    PetSelector::GetLog
                } else {
                    PetSelector::GetPlunderedLog
                }),
            )
            .await?;
        let logs = if kind == "interact" {
            reply
                .pet_treasure_hunt_get_log
                .as_ref()
                .map(|r| r.logs.iter().map(pet_interact_log_dto).collect())
                .unwrap_or_default()
        } else {
            reply
                .pet_treasure_hunt_get_plundered_log
                .as_ref()
                .map(|r| r.logs.iter().map(pet_plunder_log_dto).collect())
                .unwrap_or_default()
        };
        Ok(logs)
    }

    /// 好友的萌宠活动信息（op=47）：护送中宝藏 + 防守锦囊。
    pub async fn get_pet_diary_friend(&self, gid: &str) -> Result<serde_json::Value> {
        let gid = positive_decimal(gid, ActivityErrorCode::InvalidPetFriendGid, "好友 GID")?;
        let friend = self.pet_friend_data(gid).await?;
        Ok(friend)
    }

    async fn pet_friend_data(&self, gid: i64) -> Result<serde_json::Value> {
        let (reply, _) = self
            .pet_operate(
                PetDiaryOperateRequest {
                    activity_id: PET_DIARY_ACTIVITY_ID,
                    operate_type: PET_DIARY_FRIEND_INFO_OPERATE_TYPE,
                    pet_treasure_hunt_get_friend_activity_info: Some(
                        crate::proto::generated::gamepb::activitypb::PetTreasureHuntGetFriendActivityInfoReq {
                            friend_gid: gid,
                        },
                    ),
                    ..Default::default()
                },
                Some(PetSelector::GetFriendActivityInfo),
            )
            .await?;
        let result = reply
            .pet_treasure_hunt_get_friend_activity_info
            .as_ref()
            .ok_or_else(|| pet_err("活动响应缺少操作结果，请刷新后查看结果"))?;
        if result.gid != gid {
            return Err(pet_err("好友响应不匹配").into());
        }
        let info = result.info.as_ref();
        Ok(serde_json::json!({
            "gid": gid.to_string(),
            "treasures": info.map(|i| i.treasures.iter().map(pet_treasure_dto).collect::<Vec<_>>())
                .unwrap_or_default(),
            "charms": info.map(|i| i.defender_charm_ids.clone()).unwrap_or_default(),
        }))
    }

    /// 萌宠日记写操作（面板 15 个动作 + solar 分流）。
    /// solar 分流在拿锁之前（claim 自带串行化，锁内再拿会死锁；对齐 bot 的
    /// serializeMutation 外分流）。
    pub async fn operate_pet_diary(
        &self,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        if action == "solar" {
            return self.claim_pet_diary_solar_term(&pet_param_string(params, "termId")).await;
        }
        let _guard = self.mutation_lock.lock().await;
        let group = self.pet_read_group().await?;
        if !pet_head_active(Some(group.pet_head())) {
            return Err(pet_err("萌宠成长日记当前不在活动时间内").into());
        }
        let state = group.pet_state();
        let nurture = state.nurture.as_ref();
        let battle = state.battle.as_ref();
        let stage = nurture.map(|n| n.stage).unwrap_or(0);

        let request;
        let selector;
        let mut message = "操作成功".to_string();

        match action {
            "initialize" => {
                if nurture.map(|n| n.cg_played).unwrap_or(false) {
                    return Err(pet_err("已领养比熊，请刷新状态").into());
                }
                request = PetDiaryOperateRequest {
                    activity_id: PET_DIARY_ACTIVITY_ID,
                    operate_type: PET_DIARY_INITIALIZE_OPERATE_TYPE,
                    pet_treasure_hunt_finish_cg: Some(Default::default()),
                    ..Default::default()
                };
                selector = Some(PetSelector::FinishCg);
            }
            "feed" | "draw" => {
                if action == "feed"
                    && (stage != 1
                        || state.feed.as_ref().map(|f| i64::from(f.feed_count)).unwrap_or(0)
                            >= pet_base().daily_feed_limit)
                {
                    return Err(pet_err("当前不可投喂").into());
                }
                if action == "draw"
                    && (stage != 2
                        || state.hunt.as_ref().map(|h| i64::from(h.treasure_count)).unwrap_or(0)
                            >= pet_base().daily_treasure_limit)
                {
                    return Err(pet_err("当前不可寻宝").into());
                }
                let costs = if action == "feed" {
                    pet_feed_costs()
                } else {
                    state
                        .hunt
                        .as_ref()
                        .map(|h| h.treasure_cost.iter().map(|i| (i.id, i.count)).collect())
                        .unwrap_or_default()
                };
                let balances = pet_balances(self).await?;
                if !pet_costs_available(&costs, Some(&balances), 1) {
                    return Err(pet_err("萌宠元气糕不足，请先种植活动作物").into());
                }
                if action == "feed" {
                    request = PetDiaryOperateRequest {
                        activity_id: PET_DIARY_ACTIVITY_ID,
                        operate_type: PET_DIARY_FEED_OPERATE_TYPE,
                        pet_treasure_hunt_feed: Some(Default::default()),
                        ..Default::default()
                    };
                    selector = Some(PetSelector::Feed);
                } else {
                    request = PetDiaryOperateRequest {
                        activity_id: PET_DIARY_ACTIVITY_ID,
                        operate_type: PET_DIARY_DRAW_OPERATE_TYPE,
                        pet_treasure_hunt_draw: Some(Default::default()),
                        ..Default::default()
                    };
                    selector = Some(PetSelector::Draw);
                }
            }
            "claimDog" => {
                if stage != 2 || nurture.map(|n| n.dog_granted).unwrap_or(false) {
                    return Err(pet_err("比熊尚未成年或已经领取").into());
                }
                request = PetDiaryOperateRequest {
                    activity_id: PET_DIARY_ACTIVITY_ID,
                    operate_type: PET_DIARY_CLAIM_DOG_OPERATE_TYPE,
                    pet_treasure_hunt_claim_dog: Some(Default::default()),
                    ..Default::default()
                };
                selector = Some(PetSelector::ClaimDog);
            }
            "story" => {
                let order = positive_decimal(
                    &pet_param_string(params, "order"),
                    ActivityErrorCode::InvalidPetStory,
                    "手记编号",
                )?;
                let matched = state.story.as_ref().is_some_and(|s| {
                    s.stories
                        .iter()
                        .any(|st| i64::from(st.order) == order && st.unlocked && !st.claimed)
                });
                if !matched {
                    return Err(pet_err("手记尚未解锁或已领取").into());
                }
                request = PetDiaryOperateRequest {
                    activity_id: PET_DIARY_ACTIVITY_ID,
                    operate_type: PET_DIARY_CLAIM_STORY_OPERATE_TYPE,
                    pet_treasure_hunt_claim_story: Some(
                        crate::proto::generated::gamepb::activitypb::PetTreasureHuntClaimStoryReq {
                            order: order as i32,
                        },
                    ),
                    ..Default::default()
                };
                selector = Some(PetSelector::ClaimStory);
            }
            "seeds" => {
                let seeds_rewards = group
                    .seeds
                    .as_ref()
                    .and_then(|s| s.mega_event.as_ref())
                    .map(|m| m.rewards.as_slice())
                    .unwrap_or(&[]);
                if !pet_head_active(group.seeds_head())
                    || !seeds_rewards.iter().any(|r| r.claimable && !r.claimed)
                {
                    return Err(pet_err("当前没有可领取的种子礼包").into());
                }
                request = PetDiaryOperateRequest {
                    activity_id: PET_DIARY_SEEDS_ID,
                    operate_type: PET_DIARY_SEEDS_CLAIM_ALL_OPERATE_TYPE,
                    mega_event_claim_all: Some(Default::default()),
                    ..Default::default()
                };
                selector = Some(PetSelector::MegaEventClaimAll);
            }
            "refreshCharm" => {
                if stage != 2 {
                    return Err(pet_err("比熊成年后才可刷新锦囊").into());
                }
                let charm_equipped = battle.map(|b| b.charm_equipped.clone()).unwrap_or_default();
                let charm_pick_used = battle.map(|b| b.charm_pick_used).unwrap_or(false);
                if !charm_equipped.is_empty() && !charm_pick_used {
                    return Err(pet_err("请先替换或保留当前锦囊，再刷新").into());
                }
                let free = battle.map(|b| b.charm_free_refresh_count).unwrap_or(0)
                    < pet_charm_refresh().free_refresh_daily_limit;
                // payment / allowDiamonds / expectedPaidRefreshCount 是面板前置条件，
                // 不进游戏请求；过期免费点击不允许变成付费刷新，重复付费点击不允许双扣。
                let payment = pet_param_string(params, "payment");
                if pet_param_bool(params, "allowDiamonds").unwrap_or(false)
                    || (!payment.is_empty() && payment != "free" && payment != "tickets")
                {
                    return Err(pet_err("锦囊刷新不支持使用钻石").into());
                }
                if free {
                    if !payment.is_empty() && payment != "free" {
                        return Err(pet_err("刷新次数已变化，请刷新状态后重试").into());
                    }
                } else {
                    if payment != "tickets" {
                        return Err(pet_err("免费刷新已用完，请确认点券费用后再操作").into());
                    }
                    let paid_count = battle.map(|b| b.charm_paid_refresh_count).unwrap_or(0);
                    if paid_count >= pet_charm_refresh().manual_refresh_daily_limit {
                        return Err(pet_err("今日付费刷新次数已用完").into());
                    }
                    let expected = pet_param_i64(params, "expectedPaidRefreshCount");
                    if !expected.is_some_and(|value| value == paid_count) {
                        return Err(pet_err("刷新次数已变化，请刷新状态后重试").into());
                    }
                    // 发送前重读点券余额：官方客户端点券不足时会自动落钻石。
                    let balances = pet_balances(self).await?;
                    if !pet_costs_available(&[pet_charm_refresh_cost()], Some(&balances), 1) {
                        return Err(pet_err("点券不足，已停止刷新，不使用钻石").into());
                    }
                }
                request = PetDiaryOperateRequest {
                    activity_id: PET_DIARY_ACTIVITY_ID,
                    operate_type: PET_DIARY_REFRESH_CHARM_OPERATE_TYPE,
                    pet_treasure_hunt_refresh_charm_pool: Some(Default::default()),
                    ..Default::default()
                };
                selector = Some(PetSelector::RefreshCharmPool);
            }
            "equipCharm" => {
                let charm_id = positive_decimal(
                    &pet_param_string(params, "charmId"),
                    ActivityErrorCode::InvalidPetCharm,
                    "锦囊编号",
                )?;
                let mut choices = battle.map(|b| b.charm_daily_pool.clone()).unwrap_or_default();
                choices.extend(battle.map(|b| b.charm_equipped.clone()).unwrap_or_default());
                if stage != 2
                    || battle.map(|b| b.charm_pick_used).unwrap_or(false)
                    || !choices.contains(&(charm_id as i32))
                {
                    return Err(pet_err("该锦囊不可选择或本轮已经选择").into());
                }
                request = PetDiaryOperateRequest {
                    activity_id: PET_DIARY_ACTIVITY_ID,
                    operate_type: PET_DIARY_EQUIP_CHARM_OPERATE_TYPE,
                    pet_treasure_hunt_equip_charms: Some(
                        crate::proto::generated::gamepb::activitypb::PetTreasureHuntEquipCharmsReq {
                            charm_ids: vec![charm_id as i32],
                        },
                    ),
                    ..Default::default()
                };
                selector = Some(PetSelector::EquipCharms);
            }
            "openTreasure" => {
                let now = get_server_time_secs();
                let has_finished = state.pool.as_ref().is_some_and(|p| {
                    p.treasures.iter().any(|t| {
                        t.status == 3 || (t.status == 2 && t.end_at > 0 && t.end_at <= now)
                    })
                });
                if !has_finished {
                    return Err(pet_err("还没有完成护送的宝藏").into());
                }
                request = PetDiaryOperateRequest {
                    activity_id: PET_DIARY_ACTIVITY_ID,
                    operate_type: PET_DIARY_OPEN_TREASURE_OPERATE_TYPE,
                    pet_treasure_hunt_open_treasure: Some(Default::default()),
                    ..Default::default()
                };
                selector = Some(PetSelector::OpenTreasure);
            }
            "compensation" => {
                if state.plunder.as_ref().map(|p| p.plunder_compensation_count).unwrap_or(0) <= 0 {
                    return Err(pet_err("当前没有可领取的夺宝补偿").into());
                }
                request = PetDiaryOperateRequest {
                    activity_id: PET_DIARY_ACTIVITY_ID,
                    operate_type: PET_DIARY_COMPENSATION_OPERATE_TYPE,
                    pet_treasure_hunt_claim_plunder_compensation: Some(Default::default()),
                    ..Default::default()
                };
                selector = Some(PetSelector::ClaimPlunderCompensation);
            }
            "exchange" => {
                let goods_id = positive_decimal(
                    &pet_param_string(params, "goodsId"),
                    ActivityErrorCode::InvalidPetGoods,
                    "商品编号",
                )?;
                let count_text = pet_param_string(params, "count");
                let count_text = if count_text.is_empty() { "1".to_string() } else { count_text };
                let count =
                    positive_decimal(&count_text, ActivityErrorCode::InvalidPetCount, "兑换数量")?;
                let (shop_reply, _) = self
                    .pet_operate(
                        PetDiaryOperateRequest {
                            activity_id: PET_DIARY_SHOP_ID,
                            operate_type: QUERY_SHOP_OPERATE_TYPE,
                            ..Default::default()
                        },
                        None,
                    )
                    .await?;
                let shop_data =
                    shop_reply.data.as_ref().ok_or_else(|| pet_err("拾物小铺目录缺失"))?;
                if !pet_head_active(shop_data.head.as_ref()) {
                    return Err(pet_err("拾物小铺当前不可兑换").into());
                }
                let goods = shop_data
                    .shop
                    .as_ref()
                    .map(|s| s.goods.iter().find(|g| g.id == goods_id))
                    .unwrap_or_default()
                    .ok_or_else(|| pet_err("服务端目录未发现该商品"))?;
                if goods.diamond_cost_count > 0
                    || goods.cost.iter().any(|c| c.id == DIAMOND_ITEM_ID)
                {
                    return Err(pet_err("该商品可能消耗钻石，已阻止兑换").into());
                }
                if goods.purchase_limit > 0
                    && goods.purchased_count.saturating_add(count) > goods.purchase_limit
                {
                    return Err(pet_err("兑换数量超过剩余限购次数").into());
                }
                let costs: Vec<(i64, i64)> = goods.cost.iter().map(|c| (c.id, c.count)).collect();
                let balances = pet_balances(self).await?;
                if !pet_costs_available(&costs, Some(&balances), count) {
                    return Err(pet_err("兑换余额不足").into());
                }
                request = PetDiaryOperateRequest {
                    activity_id: PET_DIARY_SHOP_ID,
                    operate_type: EXCHANGE_SHOP_OPERATE_TYPE,
                    shop_buy: Some(
                        crate::proto::generated::gamepb::activitypb::PetDiaryShopBuyReq {
                            goods_id,
                            count,
                        },
                    ),
                    ..Default::default()
                };
                selector = Some(PetSelector::ShopBuy);
            }
            "battle" => {
                let fight = pet_fight();
                if !state.hunt.as_ref().map(|h| h.can_play_plunder).unwrap_or(false)
                    || battle.map(|b| i64::from(b.battle_count)).unwrap_or(0)
                        >= fight.daily_battle_limit
                {
                    return Err(pet_err("当前不可夺宝").into());
                }
                let gid = positive_decimal(
                    &pet_param_string(params, "gid"),
                    ActivityErrorCode::InvalidPetFriendGid,
                    "好友 GID",
                )?;
                let challenge_id = positive_decimal(
                    &pet_param_string(params, "challengeId"),
                    ActivityErrorCode::InvalidPetChallenge,
                    "挑战书编号",
                )?;
                if !PET_DIARY_CHALLENGE_ITEM_IDS.contains(&challenge_id) {
                    return Err(pet_err("挑战书类型无效").into());
                }
                let treasure_id = pet_param_string(params, "treasureId");
                let friend = self.pet_friend_data(gid).await?;
                let friend_treasures = friend
                    .get("treasures")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let treasure = friend_treasures
                    .iter()
                    .find(|t| t.get("id").and_then(serde_json::Value::as_str) == Some(&treasure_id))
                    .ok_or_else(|| pet_err("好友宝藏状态已变化，请重新查看"))?;
                let previews = treasure
                    .get("previews")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let preview_ok = previews.iter().any(|p| {
                    p.get("challengeId").and_then(serde_json::Value::as_str)
                        == Some(&challenge_id.to_string())
                        && p.get("canStart").and_then(serde_json::Value::as_bool).unwrap_or(false)
                });
                if treasure.get("status").and_then(serde_json::Value::as_i64) != Some(2)
                    || !preview_ok
                {
                    return Err(pet_err("好友宝藏状态已变化，请重新查看").into());
                }
                let balances = pet_balances(self).await?;
                if !pet_costs_available(&[(challenge_id, 1)], Some(&balances), 1) {
                    return Err(pet_err("对应挑战书不足").into());
                }
                request = PetDiaryOperateRequest {
                    activity_id: PET_DIARY_ACTIVITY_ID,
                    operate_type: PET_DIARY_BATTLE_OPERATE_TYPE,
                    pet_treasure_hunt_start_battle: Some(
                        crate::proto::generated::gamepb::activitypb::PetTreasureHuntStartBattleReq {
                            defender_gid: gid,
                            treasure_id,
                            challenge_item_id: challenge_id,
                        },
                    ),
                    ..Default::default()
                };
                selector = Some(PetSelector::StartBattle);
            }
            "skipBattle" => {
                let Some(skip) = pet_param_bool(params, "skip") else {
                    return Err(pet_err("跳过动画设置无效").into());
                };
                request = PetDiaryOperateRequest {
                    activity_id: PET_DIARY_ACTIVITY_ID,
                    operate_type: PET_DIARY_SKIP_BATTLE_OPERATE_TYPE,
                    pet_treasure_hunt_set_skip_battle_cg: Some(
                        crate::proto::generated::gamepb::activitypb::PetTreasureHuntSetSkipBattleCgReq {
                            skip,
                        },
                    ),
                    ..Default::default()
                };
                selector = Some(PetSelector::SetSkipBattleCg);
            }
            "markStories" => {
                let orders: Vec<i32> = params
                    .get("orders")
                    .and_then(serde_json::Value::as_array)
                    .map(|list| {
                        list.iter()
                            .map(|v| match v {
                                serde_json::Value::Number(n) => n.as_i64().unwrap_or(0) as i32,
                                serde_json::Value::String(s) => s.parse().unwrap_or(0),
                                _ => 0,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if orders.is_empty()
                    || !orders.iter().all(|&order| {
                        state.story.as_ref().is_some_and(|s| {
                            s.stories.iter().any(|st| st.order == order && st.unlocked)
                        })
                    })
                {
                    return Err(pet_err("手记编号无效").into());
                }
                request = PetDiaryOperateRequest {
                    activity_id: PET_DIARY_ACTIVITY_ID,
                    operate_type: PET_DIARY_MARK_STORIES_OPERATE_TYPE,
                    pet_treasure_hunt_mark_story_animated: Some(
                        crate::proto::generated::gamepb::activitypb::PetTreasureHuntMarkStoryAnimatedReq {
                            orders,
                        },
                    ),
                    ..Default::default()
                };
                selector = Some(PetSelector::MarkStoryAnimated);
            }
            _ => return Err(pet_err("未知萌宠操作").into()),
        }

        let (_, result) = self.pet_operate(request, selector).await?;
        if action == "battle" {
            message = if result.get("won").and_then(serde_json::Value::as_bool).unwrap_or(false) {
                "夺宝成功".to_string()
            } else {
                "本次夺宝未获胜，已按规则结算".to_string()
            };
        }
        let rewards = result
            .get("rewards")
            .or_else(|| result.get("awards"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        let costs = result.get("costs").cloned().unwrap_or_else(|| serde_json::json!([]));
        let mut snapshot = serde_json::Value::Null;
        let mut refresh_error = String::new();
        match self.pet_read_snapshot().await {
            Ok(value) => snapshot = value,
            Err(error) => refresh_error = format!("操作已成功，刷新失败：{error}"),
        }
        Ok(serde_json::json!({
            "action": action,
            "rewards": rewards,
            "costs": costs,
            "result": result,
            "snapshot": snapshot,
            "refreshError": refresh_error,
            "message": message,
        }))
    }

    /// 节令小礼：只允许领取与萌宠活动窗口重叠且 canClaim 的节令。
    pub async fn claim_pet_diary_solar_term(&self, term_id: &str) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let term_id =
            positive_decimal(term_id, ActivityErrorCode::InvalidPetSolarTerm, "节令编号")?;
        let group = self.pet_read_group().await?;
        let head = group.pet_head();
        if !pet_head_active(Some(head)) {
            return Err(pet_err("萌宠成长日记当前不在活动时间内").into());
        }
        let solar = self.get_current_solar_terms().await?;
        let term = solar
            .terms
            .iter()
            .find(|t| {
                t.id == term_id && t.end_time >= head.start_time && t.start_time <= head.end_time
            })
            .ok_or_else(|| pet_err("该节令当前不可领取"))?;
        if !term.can_claim {
            return Err(pet_err("该节令当前不可领取").into());
        }
        let body = self
            .gateway
            .request(
                SOLAR_TERMS_SERVICE,
                "ClaimSolarTerms",
                &ClaimSolarTermsRequest { term_id }.encode_to_vec(),
            )
            .await?;
        let reply = ClaimSolarTermsReply::decode(&body[..])?;
        let claimed = reply.term.as_ref();
        if claimed.map(|t| t.term_id) != Some(term_id) || claimed.map(|t| t.status) != Some(3) {
            return Err(pet_err("节令回包不匹配，请刷新确认领取状态").into());
        }
        let rewards: Vec<serde_json::Value> = reply
            .rewards
            .iter()
            .map(|r| serde_json::to_value(item_from_id(r.item_id, r.count)).unwrap_or_default())
            .collect();
        let mut snapshot = serde_json::Value::Null;
        let mut refresh_error = String::new();
        match self.pet_read_snapshot().await {
            Ok(value) => snapshot = value,
            Err(error) => refresh_error = format!("领取已成功，刷新失败：{error}"),
        }
        Ok(serde_json::json!({
            "action": "solar",
            "rewards": rewards,
            "snapshot": snapshot,
            "refreshError": refresh_error,
            "message": "节令好礼领取成功",
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::PET_DIARY_DOG_ID;

    #[test]
    fn constants_align_with_bot() {
        assert_eq!(PET_DIARY_GROUP_ID, 2_026_090_100);
        assert_eq!(PET_DIARY_ACTIVITY_ID, 2_026_090_101);
        assert_eq!(PET_DIARY_SEEDS_ID, 2_026_090_102);
        assert_eq!(PET_DIARY_SHOP_ID, 2_026_090_103);
        assert_eq!(PET_DIARY_INITIALIZE_OPERATE_TYPE, 27);
        assert_eq!(PET_DIARY_FEED_OPERATE_TYPE, 29);
        assert_eq!(PET_DIARY_DRAW_OPERATE_TYPE, 30);
        assert_eq!(PET_DIARY_INTERACT_LOG_OPERATE_TYPE, 31);
        assert_eq!(PET_DIARY_CLAIM_STORY_OPERATE_TYPE, 32);
        assert_eq!(PET_DIARY_REFRESH_CHARM_OPERATE_TYPE, 41);
        assert_eq!(PET_DIARY_EQUIP_CHARM_OPERATE_TYPE, 42);
        assert_eq!(PET_DIARY_BATTLE_OPERATE_TYPE, 43);
        assert_eq!(PET_DIARY_PLUNDER_LOG_OPERATE_TYPE, 44);
        assert_eq!(PET_DIARY_OPEN_TREASURE_OPERATE_TYPE, 45);
        assert_eq!(PET_DIARY_COMPENSATION_OPERATE_TYPE, 46);
        assert_eq!(PET_DIARY_FRIEND_INFO_OPERATE_TYPE, 47);
        assert_eq!(PET_DIARY_CLAIM_DOG_OPERATE_TYPE, 48);
        assert_eq!(PET_DIARY_MARK_STORIES_OPERATE_TYPE, 49);
        assert_eq!(PET_DIARY_SKIP_BATTLE_OPERATE_TYPE, 50);
        assert_eq!(PET_DIARY_SEEDS_CLAIM_ALL_OPERATE_TYPE, 21);
        assert_eq!(DIAMOND_ITEM_ID, 1004);
        assert_eq!(PET_DIARY_CHALLENGE_ITEM_IDS, [80101, 80102, 80103]);
        assert_eq!(PET_DIARY_DOG_ID, 90031);
    }

    #[test]
    fn catalog_values_align_with_bot() {
        let base = pet_base();
        assert_eq!(base.feed_items, "1028:700");
        assert_eq!(base.growth_adult_threshold, 7000);
        assert_eq!(base.daily_feed_limit, 16);
        assert_eq!(base.daily_treasure_limit, 10);
        assert_eq!(pet_fight().daily_battle_limit, 20);
        let refresh = pet_charm_refresh();
        assert_eq!(refresh.free_refresh_daily_limit, 1);
        assert_eq!(refresh.manual_refresh_cost_id, 1002);
        assert_eq!(refresh.manual_refresh_cost_count, 30);
        assert_eq!(refresh.manual_refresh_daily_limit, 3);
        assert_eq!(pet_feed_costs(), vec![(1028, 700)]);
        let charm_ids: Vec<i64> = pet_catalog().charms.iter().map(|c| c.charm_id).collect();
        assert_eq!(charm_ids, vec![101, 102, 103, 104, 105]);
    }

    #[test]
    fn parse_item_pair_rejects_invalid_spec() {
        assert_eq!(pet_parse_item_pair("1028:700"), Some((1028, 700)));
        assert_eq!(pet_parse_item_pair("1028"), None);
        assert_eq!(pet_parse_item_pair(""), None);
        assert_eq!(pet_parse_item_pair("a:b"), None);
    }

    #[test]
    fn asset_url_maps_catalog_paths() {
        assert_eq!(
            pet_asset_url("gui/texture/Season/S3/S3SkillType/img_s3_skillType1"),
            "/activity-assets/pet-diary/img_s3_skillType1.webp"
        );
        assert_eq!(
            pet_asset_url("gui/texture/Season/S3/S3SkillType/img_s3_skillType1/spriteFrame"),
            "/activity-assets/pet-diary/img_s3_skillType1.webp"
        );
        assert_eq!(
            pet_asset_url("gui/texture/Season/S3/S3PhotoWallPhotos/img_s3PhotoWall_photo0"),
            "/activity-assets/pet-diary/img_s3PhotoWall_photo0.webp"
        );
        assert_eq!(pet_asset_url("unknown/path"), "");
    }

    fn balances(map: &[(i64, i64)]) -> HashMap<i64, i64> {
        map.iter().copied().collect()
    }

    #[test]
    fn costs_available_rejects_diamond_like_costs() {
        let bag = balances(&[(1028, 700), (1002, 30), (80101, 1)]);
        assert!(pet_costs_available(&[(1028, 700)], Some(&bag), 1));
        assert!(!pet_costs_available(&[(DIAMOND_ITEM_ID, 1)], Some(&bag), 1));
        assert!(!pet_costs_available(&[(0, 1)], Some(&bag), 1));
        assert!(!pet_costs_available(&[(1028, -1)], Some(&bag), 1));
        assert!(!pet_costs_available(&[(1028, 400), (1028, 400)], Some(&bag), 1));
        assert!(pet_costs_available(&[(1028, 100)], Some(&bag), 5));
        assert!(!pet_costs_available(&[(1028, 100)], Some(&bag), 8));
        assert!(!pet_costs_available(&[(1028, 1)], None, 1));
        assert!(!pet_costs_available(&[], Some(&bag), 1));
    }

    fn head() -> PetDiaryActivityHead {
        PetDiaryActivityHead {
            id: PET_DIARY_ACTIVITY_ID,
            start_time: 1,
            end_time: 9_999_999_999,
            ..Default::default()
        }
    }

    fn pet_group(stage: i32, feed_count: i32) -> PetGroup {
        use crate::proto::generated::gamepb::activitypb::{
            PetTreasureHuntFeed, PetTreasureHuntHunt, PetTreasureHuntNurture,
        };
        PetGroup {
            pet: PetDiaryActivityData {
                head: Some(head()),
                pet_treasure_hunt: Some(ActivityBodyPetTreasureHunt {
                    nurture: Some(PetTreasureHuntNurture {
                        cg_played: true,
                        stage,
                        growth: if stage == 2 { 7000 } else { 0 },
                        ..Default::default()
                    }),
                    feed: Some(PetTreasureHuntFeed { feed_count }),
                    hunt: Some(PetTreasureHuntHunt {
                        treasure_count: 0,
                        treasure_cost: vec![CoreItem {
                            id: 1028,
                            count: 700,
                            ..Default::default()
                        }],
                        can_play_plunder: true,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            seeds: None,
            shop: None,
        }
    }

    fn service() -> ActivityCenterService {
        use crate::network::encryptor::NoopEncryptor;
        use crate::network::gateway::{Gateway, GatewayConfig};
        let gateway = Gateway::new(
            GatewayConfig {
                server_url: "wss://gate.example.com/ws".to_string(),
                platform: "qq".to_string(),
                os: "Windows".to_string(),
                client_version: crate::config::DEFAULT_CLIENT_VERSION.to_string(),
                auth_code: "test".to_string(),
                headers: std::collections::HashMap::new(),
            },
            std::sync::Arc::new(NoopEncryptor),
        );
        ActivityCenterService::new(std::sync::Arc::new(gateway))
    }

    #[test]
    fn normalize_gates_feed_and_draw_by_stage_and_balance() {
        let service = service();
        let bag = balances(&[(1028, 700)]);
        let dto = service.pet_normalize(&pet_group(1, 0), Some(&bag), None, vec![]);
        assert_eq!(dto["active"], serde_json::json!(true));
        assert_eq!(dto["nurture"]["adult"], serde_json::json!(false));
        assert_eq!(dto["nurture"]["canFeed"], serde_json::json!(true));
        assert_eq!(dto["hunt"]["canDraw"], serde_json::json!(false));
        let adult = service.pet_normalize(&pet_group(2, 16), Some(&bag), None, vec![]);
        assert_eq!(adult["nurture"]["adult"], serde_json::json!(true));
        assert_eq!(adult["nurture"]["canFeed"], serde_json::json!(false));
        assert_eq!(adult["hunt"]["canDraw"], serde_json::json!(true));
        let poor =
            service.pet_normalize(&pet_group(2, 16), Some(&balances(&[(1028, 699)])), None, vec![]);
        assert_eq!(poor["hunt"]["canDraw"], serde_json::json!(false));
        let unknown = service.pet_normalize(&pet_group(1, 0), None, None, vec![]);
        assert_eq!(unknown["nurture"]["canFeed"], serde_json::json!(false));
        assert_eq!(unknown["balances"][0]["known"], serde_json::json!(false));
    }

    #[test]
    fn normalize_marks_diamond_goods_unexchangeable() {
        use crate::proto::generated::gamepb::activitypb::{
            PetDiaryActivityBodyShop, PetDiaryShopGoodsInfo, PetDiaryShopItemInfo,
        };
        let mut group = pet_group(2, 0);
        group.shop = Some(PetDiaryActivityData {
            head: Some(head()),
            shop: Some(PetDiaryActivityBodyShop {
                goods: vec![
                    PetDiaryShopGoodsInfo {
                        id: 1,
                        item: vec![PetDiaryShopItemInfo { id: 20516, count: 1 }],
                        cost: vec![PetDiaryShopItemInfo { id: DIAMOND_ITEM_ID, count: 10 }],
                        purchase_limit: 5,
                        purchased_count: 0,
                        ..Default::default()
                    },
                    PetDiaryShopGoodsInfo {
                        id: 2,
                        item: vec![PetDiaryShopItemInfo { id: 20516, count: 1 }],
                        cost: vec![PetDiaryShopItemInfo { id: 1028, count: 700 }],
                        purchase_limit: 1,
                        purchased_count: 1,
                        ..Default::default()
                    },
                ],
            }),
            ..Default::default()
        });
        let service = service();
        let bag = balances(&[(1028, 700)]);
        let dto = service.pet_normalize(&group, Some(&bag), None, vec![]);
        let goods = dto["shop"].as_array().expect("shop goods");
        assert_eq!(goods[0]["safeCosts"], serde_json::json!(false));
        assert_eq!(goods[0]["exchangeable"], serde_json::json!(false));
        assert_eq!(goods[1]["remaining"], serde_json::json!("0"));
        assert_eq!(goods[1]["exchangeable"], serde_json::json!(false));
    }

    #[test]
    fn param_helpers_accept_numbers_and_strings() {
        let params = serde_json::json!({
            "order": 3,
            "gid": "12345",
            "skip": true,
            "expectedPaidRefreshCount": 2,
        });
        assert_eq!(pet_param_string(&params, "order"), "3");
        assert_eq!(pet_param_string(&params, "gid"), "12345");
        assert_eq!(pet_param_i64(&params, "expectedPaidRefreshCount"), Some(2));
        assert_eq!(pet_param_bool(&params, "skip"), Some(true));
        assert_eq!(pet_param_string(&params, "missing"), "");
    }
}
