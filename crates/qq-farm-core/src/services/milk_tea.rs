//! 奶茶活动（bot `core/src/services/milk-tea.ts` + `milk-tea-work.ts`，1.15 新活动）。
//!
//! 协议：`gamepb.activitypb.ActivityService` 的 `GetGroup`（组 2026110100）与
//! `Operate`（operate_type 53-76，见 `activitypb.proto` / `milk-tea.proto`）。
//! 数值配置镜像 bot `activity-data/milk-tea-20261015.json`。
//!
//! 用户决策：任何消耗钻石（道具 1004 或 `diamond_cost_count > 0`）的路径一律拒绝。

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use prost::Message as _;
use serde::Deserialize;

use crate::config::game_config::global as global_game_config;
use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item;
use crate::proto::generated::gamepb::activitypb::{
    GetGroupRequest, MilkTeaBoostReq, MilkTeaClaimCashierReq, MilkTeaClaimWagesReq,
    MilkTeaCraftReq, MilkTeaCreateShopReq, MilkTeaGetGroupReply, MilkTeaGetHireListReq,
    MilkTeaGetRankReq, MilkTeaGetSalesLogReq, MilkTeaGetShopReq, MilkTeaGetWorkPageReq,
    MilkTeaGetWorkshopReq, MilkTeaHireInteract, MilkTeaHireItem, MilkTeaHireReq,
    MilkTeaInteractReq, MilkTeaMyShift, MilkTeaOperateReply, MilkTeaOperateRequest,
    MilkTeaRenameShopReq, MilkTeaSetHireDisableReq, MilkTeaShopView, MilkTeaSlotView,
    MilkTeaUpgradeReq, ShopBuyReq,
};
use crate::services::activity_text::strip_html;

const GROUP: i64 = 2_026_110_100;
const MILK: i64 = 2_026_110_101;
const SHOP: i64 = 2_026_110_102;
const FUND: i64 = 2_026_110_103;
const SERVICE: &str = "gamepb.activitypb.ActivityService";
/// 钻石（活动操作一律拒绝消耗）。
pub const DIAMOND_ITEM_ID: i64 = 1004;
/// 奶茶券 / 宣传喇叭（互动奖励展示用）。
const TOKEN_ITEM_ID: i64 = 1041;
const HORN_ITEM_ID: i64 = 1042;

#[derive(Debug, Deserialize)]
struct MilkTeaConfig {
    #[serde(rename = "MilkTeaActivityConfig")]
    activity: Vec<MilkTeaActivityConfig>,
    #[serde(rename = "MilkTeaHireDurationConfig")]
    hire_durations: Vec<HireDurationConfig>,
    #[serde(rename = "MilkTeaRecipeConfig")]
    recipes: Vec<RecipeConfig>,
    #[serde(rename = "MilkTeaStoreLevelConfig")]
    store_levels: Vec<StoreLevelConfig>,
    #[serde(rename = "MilkTeaTalentConfig")]
    talents: Vec<TalentConfig>,
    #[serde(rename = "MilkTeaInteractionType")]
    interaction_types: Vec<InteractionTypeConfig>,
    #[serde(rename = "MilkTeaInteractionConfig")]
    interactions: Vec<InteractionConfig>,
}

#[derive(Debug, Deserialize)]
struct MilkTeaActivityConfig {
    name_min_length: usize,
    name_max_length: usize,
    recommend_names: String,
}

#[derive(Debug, Deserialize)]
struct HireDurationConfig {
    duration_id: i64,
    duration_sec: i64,
    display_order: i64,
    duration_cost: String,
    min_remaining_business_sec: i64,
}

#[derive(Debug, Deserialize)]
struct RecipeConfig {
    recipe_id: i64,
    recipe_name: String,
    fruit_item_id: String,
    base_price: String,
    drink_desc: String,
}

#[derive(Debug, Deserialize)]
struct StoreLevelConfig {
    level: i64,
    revenue_required: i64,
    base_popularity: i64,
    advanced_price_percent: i64,
    employee_slot_count: i64,
    recipe_ids_string: String,
    #[serde(default)]
    rewards: String,
}

#[derive(Debug, Deserialize)]
struct TalentConfig {
    talent_id: i64,
    #[serde(default)]
    des: String,
}

#[derive(Debug, Deserialize)]
struct InteractionTypeConfig {
    interaction_id: i64,
    name: String,
    #[serde(default)]
    des: String,
}

#[derive(Debug, Deserialize)]
struct InteractionConfig {
    interaction_type: i64,
    duration_id: i64,
    random_drink_count: i64,
}

fn milk_tea_config() -> &'static MilkTeaConfig {
    static CONFIG: OnceLock<MilkTeaConfig> = OnceLock::new();
    CONFIG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../assets/activity-data/milk-tea-20261015.json"
        ))
        .expect("milk-tea-20261015.json 解析失败")
    })
}

fn activity_config() -> &'static MilkTeaActivityConfig {
    &milk_tea_config().activity[0]
}

fn recipe_config(recipe_id: i64) -> Option<&'static RecipeConfig> {
    milk_tea_config().recipes.iter().find(|recipe| recipe.recipe_id == recipe_id)
}

fn store_level(level: i64) -> Option<&'static StoreLevelConfig> {
    milk_tea_config().store_levels.iter().find(|entry| entry.level == level)
}

// ===== 通用 DTO =====

fn item_dto(item: &Item) -> serde_json::Value {
    let gc = global_game_config();
    serde_json::json!({
        "id": item.id,
        "count": item.count,
        "name": gc.get_item_by_id(item.id).map(|entry| entry.name).unwrap_or_else(|| format!("物品 #{id}", id = item.id)),
        "image": gc.get_item_image_by_id(item.id).unwrap_or_default(),
    })
}

fn items_dto(items: &[Item]) -> Vec<serde_json::Value> {
    items.iter().map(item_dto).collect()
}

/// `ShopItemInfo { id, count }` → 物品 DTO。
fn shop_items_dto(
    items: &[crate::proto::generated::gamepb::activitypb::ShopItemInfo],
) -> Vec<serde_json::Value> {
    items.iter().map(|entry| reward_dto(entry.id, entry.count)).collect()
}

fn reward_dto(id: i64, count: i64) -> serde_json::Value {
    item_dto(&Item { id, count, ..Default::default() })
}

/// 活动说明（head.extra JSON 的 tips.txt，剥离 HTML）。
fn rules_from_extra(extra: &[u8]) -> Vec<String> {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(extra) else {
        return Vec::new();
    };
    value
        .pointer("/tips/txt")
        .and_then(serde_json::Value::as_array)
        .map(|lines| {
            lines.iter().filter_map(|line| line.as_str()).map(strip_html).collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

/// JSON 数值近似 bot `Number()`；`integer()` 对齐 `Number.isSafeInteger`。
fn json_number(value: Option<&serde_json::Value>) -> Option<f64> {
    match value? {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn integer(value: Option<&serde_json::Value>, min: i64, max: i64) -> Result<i64> {
    let number = json_number(value).filter(|number| {
        number.is_finite() && number.fract() == 0. && *number >= min as f64 && *number <= max as f64
    });
    let Some(number) = number else {
        return Err(Error::Business("数量或编号无效".into()));
    };
    Ok(number as i64)
}

/// 拆分 `"1041:3"` → (1041, 3)。
fn split_pair(text: &str) -> (i64, i64) {
    let mut parts = text.split(':');
    let first = parts.next().and_then(|value| value.trim().parse().ok()).unwrap_or(0);
    let second = parts.next().and_then(|value| value.trim().parse().ok()).unwrap_or(0);
    (first, second)
}

fn recipe_fruit_id(recipe: &RecipeConfig) -> i64 {
    split_pair(&recipe.fruit_item_id).0
}

fn recipe_base_price(recipe: &RecipeConfig) -> i64 {
    split_pair(&recipe.base_price).1
}

fn duration_base_wage(config: &HireDurationConfig) -> i64 {
    split_pair(&config.duration_cost).1
}

// ===== 服务 =====

pub struct MilkTeaService {
    gateway: Arc<Gateway>,
    /// 写操作串行化（对齐 bot `tail` 链）
    mutation_lock: tokio::sync::Mutex<()>,
}

impl MilkTeaService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway, mutation_lock: tokio::sync::Mutex::new(()) }
    }

    async fn fetch_group(
        &self,
    ) -> Result<crate::proto::generated::gamepb::activitypb::MilkTeaActivityGroup> {
        let body = GetGroupRequest { group_id: GROUP }.encode_to_vec();
        let reply_bytes =
            self.gateway.request(SERVICE, "GetGroup", &body).await.map_err(Error::from)?;
        let reply = MilkTeaGetGroupReply::decode(&reply_bytes[..])?;
        let group = reply.group.ok_or_else(|| Error::Business("活动目录回包不匹配".into()))?;
        if group.head.as_ref().map(|head| head.activity_id) != Some(GROUP) {
            return Err(Error::Business("活动目录回包不匹配".into()));
        }
        Ok(group)
    }

    /// 发送一次 operate 请求并做回包一致性校验（对齐 bot `rpc()`）。
    async fn send_operate(
        &self,
        action: &str,
        request: MilkTeaOperateRequest,
    ) -> Result<MilkTeaOperateReply> {
        let expect_cmd = request.operate_type;
        let expect_activity = request.activity_id;
        let body = request.encode_to_vec();
        let reply_bytes =
            self.gateway.request(SERVICE, "Operate", &body).await.map_err(Error::from)?;
        let reply = MilkTeaOperateReply::decode(&reply_bytes[..])?;
        if reply.activity_id != expect_activity
            || reply.operate_type != expect_cmd
            || !reply_has_field(action, &reply)
        {
            return Err(Error::Business("操作回包不完整，请刷新确认结果，勿重复提交".into()));
        }
        Ok(reply)
    }
}

fn reply_has_field(action: &str, reply: &MilkTeaOperateReply) -> bool {
    match action {
        "create" => reply.milk_tea_create_shop.is_some(),
        "rename" => reply.milk_tea_rename_shop.is_some(),
        "shop" => reply.milk_tea_get_shop.is_some(),
        "hireList" => reply.milk_tea_get_hire_list.is_some(),
        "hire" => reply.milk_tea_hire.is_some(),
        "workshop" => reply.milk_tea_get_workshop.is_some(),
        "claim" => reply.milk_tea_claim_cashier.is_some(),
        "boost" => reply.milk_tea_boost.is_some(),
        "upgrade" => reply.milk_tea_upgrade.is_some(),
        "craft" => reply.milk_tea_craft.is_some(),
        "work" => reply.milk_tea_get_work_page.is_some(),
        "wages" => reply.milk_tea_claim_wages.is_some(),
        "interact" => reply.milk_tea_interact.is_some(),
        "rank" => reply.milk_tea_get_rank.is_some(),
        "logs" => reply.milk_tea_get_sales_log.is_some(),
        "hireSetting" => reply.milk_tea_set_hire_disable.is_some(),
        "exchange" => reply.shop_buy.is_some(),
        "unlock" => reply.growth_fund_unlock.is_some(),
        "fundClaim" => reply.growth_fund_claim.is_some(),
        _ => false,
    }
}

/// 读 / 写动作到 (cmd, 活动 id)。
fn operation(action: &str) -> Option<(i64, i64)> {
    Some(match action {
        "create" => (53, MILK),
        "rename" => (54, MILK),
        "shop" => (55, MILK),
        "hireList" => (56, MILK),
        "hire" => (57, MILK),
        "workshop" => (58, MILK),
        "claim" => (59, MILK),
        "boost" => (60, MILK),
        "upgrade" => (61, MILK),
        "craft" => (62, MILK),
        "work" => (63, MILK),
        "wages" => (64, MILK),
        "interact" => (65, MILK),
        "rank" => (66, MILK),
        "logs" => (67, MILK),
        "hireSetting" => (68, MILK),
        "exchange" => (1, SHOP),
        "unlock" => (75, FUND),
        "fundClaim" => (76, FUND),
        _ => return None,
    })
}

fn is_read_action(action: &str) -> bool {
    matches!(action, "shop" | "hireList" | "workshop" | "work" | "rank" | "logs")
}

/// 费用校验（对齐 bot `checkCost`；额外拒绝钻石消耗——用户决策）。
fn check_cost(
    costs: &[Item],
    expected: Option<&serde_json::Value>,
    balances: &HashMap<i64, i64>,
    count: i64,
) -> Result<()> {
    let signature = |list: &[Item]| {
        let mut parts: Vec<String> =
            list.iter().map(|item| format!("{}:{}", item.id, item.count)).collect();
        parts.sort();
        parts.join("|")
    };
    let expected_signature = expected.and_then(serde_json::Value::as_array).map(|list| {
        let mut parts: Vec<String> = list
            .iter()
            .map(|entry| {
                let id = json_number(entry.get("id")).unwrap_or(0.) as i64;
                let count = json_number(entry.get("count")).unwrap_or(0.) as i64;
                format!("{id}:{count}")
            })
            .collect();
        parts.sort();
        parts.join("|")
    });
    if expected_signature.as_deref() != Some(signature(costs).as_str()) {
        return Err(Error::Business("费用已变化，请刷新后重新确认".into()));
    }
    for cost in costs {
        if cost.id == DIAMOND_ITEM_ID {
            return Err(Error::Business("费用无效".into()));
        }
        let multiplied = cost.count.checked_mul(count);
        if cost.count < 0 || multiplied.is_none() {
            return Err(Error::Business("费用无效".into()));
        }
        let need = multiplied.unwrap_or(0);
        if balances.get(&cost.id).copied().unwrap_or(0) < need {
            return Err(Error::Business("货币或道具数量不足".into()));
        }
    }
    Ok(())
}

// ===== 视图 DTO（数据形态对齐 bot `plain()` 转换结果的字段名）=====

fn slot_dto(slot: &MilkTeaSlotView) -> serde_json::Value {
    let employee = slot.employee.as_ref().map(|employee| {
        serde_json::json!({
            "gid": employee.gid,
            "name": employee.name,
            "avatar": employee.avatar,
            "level": employee.level,
        })
    });
    serde_json::json!({
        "slotIndex": slot.slot_index,
        "isRobot": slot.is_robot,
        "shiftId": slot.shift_id,
        "beginAt": slot.begin_at,
        "endAt": slot.end_at,
        "cost": slot.cost,
        "talentId": slot.talent_id,
        "talentName": slot.talent_name,
        "interactType": slot.interact_type,
        "interactAt": slot.interact_at,
        "employee": employee,
    })
}

fn shop_dto(shop: &MilkTeaShopView) -> serde_json::Value {
    serde_json::json!({
        "level": shop.level,
        "totalRevenue": shop.total_revenue,
        "nextLevelRevenue": shop.next_level_revenue,
        "cashierCapacitySec": shop.cashier_capacity_sec,
        "cashierElapsedSec": shop.cashier_elapsed_sec,
        "slots": shop.slots.iter().map(slot_dto).collect::<Vec<_>>(),
        "drinkCups": shop.drink_cups,
        "secPerCup": f64::from(shop.sec_per_cup),
        "nextFullAt": shop.next_full_at,
        "salesItemId": shop.sales_item_id,
        "salesDurationSec": shop.sales_duration_sec,
        "isSettling": shop.is_settling,
        "pendingRewards": items_dto(&shop.pending_rewards),
        "pendingCupsByRecipe": shop.pending_cups_by_recipe,
        "pendingCupsBySource": shop.pending_cups_by_source,
        "boostCupsPerUse": shop.boost_cups_per_use,
        "boostPreviewRewards": items_dto(&shop.boost_preview_rewards),
        "effectiveCupsPerHour": shop.effective_cups_per_hour,
        "boostCoinPrice": shop.boost_coin_price,
    })
}

fn summary_dto(
    summary: &crate::proto::generated::gamepb::activitypb::MilkTeaActivitySummary,
) -> serde_json::Value {
    serde_json::json!({
        "shopOpenAt": summary.shop_open_at,
        "shopName": summary.shop_name,
        "renameCdUntilAt": summary.rename_cd_until_at,
        "level": summary.level,
        "totalRevenue": summary.total_revenue,
        "hireDisabled": summary.hire_disabled,
        "cashierFull": summary.cashier_full,
        "cashierWarning": summary.cashier_warning,
        "canUpgrade": summary.can_upgrade,
        "openingGiftPreview": items_dto(&summary.opening_gift_preview),
        "hasWorkerShift": summary.has_worker_shift,
        "hasUnclaimedWage": summary.has_unclaimed_wage,
        "guideIds": summary.guide_ids,
        "nextCupAt": summary.next_cup_at,
    })
}

fn recipe_dto(
    recipe: &crate::proto::generated::gamepb::activitypb::MilkTeaRecipeView,
    advanced_price_percent: i64,
) -> serde_json::Value {
    let config = recipe_config(recipe.recipe_id);
    let bonus = if recipe.is_advanced { advanced_price_percent } else { 0 };
    let base_price = config.map(recipe_base_price).unwrap_or(0);
    serde_json::json!({
        "recipeId": recipe.recipe_id,
        "recipeName": recipe.recipe_name,
        "unlocked": recipe.unlocked,
        "isAdvanced": recipe.is_advanced,
        "tokenPrice": recipe.token_price,
        "coinPrice": recipe.coin_price,
        "stockCups": recipe.stock_cups,
        "clearCoinPrice": recipe.clear_coin_price,
        "clearCoinTotal": recipe.clear_coin_total,
        "image": format!("/activity-assets/milk-tea/recipe-{}.png", recipe.recipe_id),
        "description": config.map(|entry| entry.drink_desc.clone()).unwrap_or_default(),
        "fruitId": config.map(recipe_fruit_id).unwrap_or(0),
        "boostTokenPrice": base_price * (10_000 + bonus) / 10_000,
    })
}

fn levels_dto() -> Vec<serde_json::Value> {
    let config = milk_tea_config();
    config
        .store_levels
        .iter()
        .map(|level| {
            let rewards = level
                .rewards
                .split(';')
                .filter(|text| !text.is_empty())
                .filter_map(|text| {
                    let mut parts = text.split(':');
                    let id = parts.next()?.trim().parse::<i64>().ok()?;
                    let count = parts.next()?.trim().parse::<i64>().ok()?;
                    Some(reward_dto(id, count))
                })
                .collect::<Vec<_>>();
            let recipes = level
                .recipe_ids_string
                .split(';')
                .filter_map(|id| id.trim().parse::<i64>().ok())
                .filter_map(|id| recipe_config(id).map(|recipe| recipe.recipe_name.clone()))
                .collect::<Vec<_>>();
            serde_json::json!({
                "level": level.level,
                "revenueRequired": level.revenue_required,
                "basePopularity": level.base_popularity,
                "advancedPricePercent": level.advanced_price_percent,
                "employeeSlotCount": level.employee_slot_count,
                "recipeIds": level.recipe_ids_string,
                "rewards": rewards,
                "recipes": recipes,
            })
        })
        .collect()
}

fn goods_dto(
    goods: &crate::proto::generated::gamepb::activitypb::ShopGoodsInfo,
    settling: bool,
) -> serde_json::Value {
    let limit = goods.purchase_limit;
    let bought = goods.purchased_count;
    let conditions: Vec<serde_json::Value> = goods
        .cond_hints
        .iter()
        .map(|hint| {
            serde_json::json!({
                "type": hint.cond_type,
                "satisfied": hint.satisfied,
                "current": hint.current,
                "target": hint.target,
            })
        })
        .collect();
    // 官方商店以正数 purchase_limit 表示有限购上限
    let remaining = if limit > 0 { Some((limit - bought).max(0)) } else { None };
    let conditions_ok = goods.cond_hints.iter().all(|hint| hint.satisfied);
    // 用户决策：钻石商品一律不可用
    let available = remaining != Some(0)
        && conditions_ok
        && !(settling && goods.category_tag == "seed")
        && goods.diamond_cost_count <= 0;
    serde_json::json!({
        "id": goods.id.to_string(),
        "name": goods.name,
        "description": goods.desc,
        "rewards": shop_items_dto(&goods.item),
        "costs": shop_items_dto(&goods.cost),
        "remaining": remaining,
        "conditions": conditions,
        "category": goods.category_tag,
        "available": available,
    })
}

fn fund_dto(
    fund: &crate::proto::generated::gamepb::activitypb::ActivityBodyGrowthFund,
) -> serde_json::Value {
    let tiers: Vec<serde_json::Value> = fund
        .tiers
        .iter()
        .map(|tier| {
            let conditions: Vec<serde_json::Value> = tier
                .conditions
                .iter()
                .map(|condition| {
                    serde_json::json!({
                        "type": condition.r#type,
                        "current": condition.current_value,
                        "target": condition.target_value,
                        "reached": condition.reached,
                    })
                })
                .collect();
            serde_json::json!({
                "id": tier.tier_id,
                "state": tier.state,
                "rewards": items_dto(&tier.reward),
                "conditions": conditions,
            })
        })
        .collect();
    serde_json::json!({
        "unlocked": fund.unlocked,
        "costs": items_dto(&fund.unlock_cost),
        "tiers": tiers,
    })
}

fn hire_friend_dto(
    friend: &crate::proto::generated::gamepb::activitypb::MilkTeaHireFriend,
) -> serde_json::Value {
    let basic = friend.friend.as_ref().map(|basic| {
        serde_json::json!({
            "gid": basic.gid,
            "name": basic.name,
            "avatar": basic.avatar,
            "level": basic.level,
        })
    });
    serde_json::json!({
        "friend": basic,
        "isRobot": friend.is_robot,
        "talentId": friend.talent_id,
        "talentName": friend.talent_name,
        "state": friend.state,
        "stateUntilAt": friend.state_until_at,
        "rehireAvailableAt": friend.rehire_available_at,
    })
}

fn hire_list_dto(
    hire: &crate::proto::generated::gamepb::activitypb::MilkTeaGetHireListRsp,
) -> serde_json::Value {
    serde_json::json!({
        "durationId": hire.duration_id,
        "wageTotal": hire.wage_total,
        "minRemainBizSec": hire.min_remain_biz_sec,
        "inviteEntry": hire.invite_entry,
        "friends": hire.friends.iter().map(hire_friend_dto).collect::<Vec<_>>(),
        "robots": hire.robots.iter().map(hire_friend_dto).collect::<Vec<_>>(),
    })
}

/// 互动结果展示（对齐 bot `interactionResult`）。
fn interaction_result(interact: Option<&MilkTeaHireInteract>) -> Option<serde_json::Value> {
    let interact = interact?;
    if interact.interaction_id == 0 {
        return None;
    }
    let definition = milk_tea_config()
        .interaction_types
        .iter()
        .find(|entry| entry.interaction_id == i64::from(interact.r#type))?;
    let friend = interact.friend.as_ref();
    let mut reward =
        reward_dto(TOKEN_ITEM_ID, friend.map(|friend| friend.bonus_amount).unwrap_or(0));
    if interact.r#type == 4 && interact.success {
        reward = reward_dto(HORN_ITEM_ID, 1);
    } else if interact.r#type == 3 && interact.success {
        let recipe_id = friend.map(|friend| friend.taste_recipe_id).unwrap_or(0);
        let recipe = recipe_config(recipe_id);
        reward = serde_json::json!({
            "id": 0,
            "count": friend.map(|friend| friend.taste_cups).unwrap_or(0),
            "name": recipe.map(|entry| entry.recipe_name.clone()).unwrap_or_else(|| "高级奶茶".to_string()),
            "image": if recipe.is_some() { format!("/activity-assets/milk-tea/recipe-{recipe_id}.png") } else { String::new() },
        });
    }
    Some(serde_json::json!({ "name": definition.name, "reward": reward }))
}

fn shift_dto(shift: &MilkTeaMyShift) -> serde_json::Value {
    let employer = shift.employer.as_ref().map(|basic| {
        serde_json::json!({
            "gid": basic.gid,
            "name": basic.name,
            "avatar": basic.avatar,
            "level": basic.level,
        })
    });
    // 官方按完整班次时长匹配工资档；未知时长不套用四小时档
    let duration = milk_tea_config()
        .hire_durations
        .iter()
        .find(|entry| entry.duration_sec == shift.end_at - shift.begin_at);
    serde_json::json!({
        "employer": employer,
        "shopName": shift.shop_name,
        "shiftId": shift.shift_id,
        "talentId": shift.talent_id,
        "talentName": shift.talent_name,
        "beginAt": shift.begin_at,
        "endAt": shift.end_at,
        "settleStartAt": shift.settle_start_at,
        "interactCount": shift.interact_count,
        "interactLimit": shift.interact_limit,
        "wageTotal": shift.wage_total,
        "baseWage": duration.map(duration_base_wage),
        "interactionResult": interaction_result(shift.interact.as_ref()),
    })
}

fn wage_slip_dto(
    slip: &crate::proto::generated::gamepb::activitypb::MilkTeaWageSlip,
) -> serde_json::Value {
    let employer = slip.employer.as_ref().map(|basic| {
        serde_json::json!({
            "gid": basic.gid,
            "name": basic.name,
            "avatar": basic.avatar,
            "level": basic.level,
        })
    });
    let wage_parts = slip.wages.as_ref().map(
        |wages| serde_json::json!({ "baseWage": wages.base_wage, "finalWage": wages.final_wage }),
    );
    serde_json::json!({
        "employer": employer,
        "shopName": slip.shop_name,
        "shiftId": slip.shift_id,
        "beginAt": slip.begin_at,
        "endAt": slip.end_at,
        "workSec": slip.work_sec,
        "wages": wage_parts,
        "truncated": slip.truncated,
        "wageStatus": slip.wage_status,
        "interactionResult": interaction_result(slip.interact.as_ref()),
    })
}

/// 工作页归一（对齐 bot `normalizeMilkTeaWork`）。
fn normalize_work(
    work: &crate::proto::generated::gamepb::activitypb::MilkTeaGetWorkPageRsp,
) -> (serde_json::Value, Vec<serde_json::Value>) {
    let shift = work.my_shift.as_ref();
    let duration = shift.and_then(|shift| {
        milk_tea_config()
            .hire_durations
            .iter()
            .find(|entry| entry.duration_sec == shift.end_at - shift.begin_at)
    });
    let talent_id =
        shift.map(|shift| shift.talent_id).filter(|id| *id != 0).unwrap_or(work.talent_id);
    let talent =
        milk_tea_config().talents.iter().find(|entry| entry.talent_id == i64::from(talent_id));
    let promote = shift.and_then(|shift| {
        shift
            .interact_previews
            .iter()
            .find(|preview| preview.interaction_type == 1)
            .map(|preview| preview.promote_cups)
    });
    let interactions: Vec<serde_json::Value> = milk_tea_config()
        .interaction_types
        .iter()
        .map(|entry| {
            let detail = milk_tea_config().interactions.iter().find(|config| {
                Some(config.duration_id) == duration.map(|entry| entry.duration_id)
                    && config.interaction_type == entry.interaction_id
            });
            let count = if entry.interaction_id == 1 {
                promote
            } else {
                detail.map(|d| d.random_drink_count)
            };
            let ready = !entry.des.contains("{0}") || count.is_some();
            let description = strip_html(&entry.des)
                .replace("{0}", &count.map_or_else(|| "—".to_string(), |value| value.to_string()));
            serde_json::json!({
                "interactionId": entry.interaction_id,
                "name": entry.name,
                "image": format!("/activity-assets/milk-tea/interact-{}.png", entry.interaction_id),
                "description": description,
                "ready": ready,
            })
        })
        .collect();
    let work_dto = serde_json::json!({
        "talentId": work.talent_id,
        "talentName": work.talent_name,
        "talentDescription": strip_html(talent.map(|entry| entry.des.as_str()).unwrap_or("")),
        "interactDone": work.interact_done,
        "hireSubscribeSetBefore": work.hire_subscribe_set_before,
        "myShift": shift.map(shift_dto),
        "slips": work.slips.iter().map(wage_slip_dto).collect::<Vec<_>>(),
    });
    (work_dto, interactions)
}

fn rank_dto(
    page_number: i64,
    rank: &crate::proto::generated::gamepb::activitypb::MilkTeaGetRankRsp,
) -> serde_json::Value {
    let ranks: Vec<serde_json::Value> = rank
        .ranks
        .iter()
        .map(|item| {
            let owner = item.owner.as_ref().map(|basic| {
                serde_json::json!({
                    "gid": basic.gid,
                    "name": basic.name,
                    "avatar": basic.avatar,
                    "level": basic.level,
                })
            });
            serde_json::json!({
                "owner": owner,
                "shopName": item.shop_name,
                "level": item.level,
                "totalRevenue": item.total_revenue,
                "rank": item.rank,
                "canShowOff": item.can_show_off,
            })
        })
        .collect();
    serde_json::json!({
        "page": page_number,
        "myRank": rank.my_rank,
        "snapshotTime": rank.snapshot_time,
        "hasMore": rank.has_more,
        "ranks": ranks,
    })
}

fn logs_dto(
    logs: &crate::proto::generated::gamepb::activitypb::MilkTeaGetSalesLogRsp,
) -> serde_json::Value {
    let hires: Vec<serde_json::Value> = logs
        .hires
        .iter()
        .map(|record| {
            let friend = record.friend.as_ref().map(|basic| {
                serde_json::json!({
                    "gid": basic.gid,
                    "name": basic.name,
                    "avatar": basic.avatar,
                    "level": basic.level,
                })
            });
            serde_json::json!({
                "friend": friend,
                "hireAt": record.hire_at,
                "durationSec": record.duration_sec,
                "workSec": record.work_sec,
                "settled": record.settled,
                "shiftId": record.shift_id,
                "talentName": record.talent_name,
                "wages": record.wages.as_ref().map(|wages| serde_json::json!({
                    "baseWage": wages.base_wage, "finalWage": wages.final_wage,
                })),
                "interactionResult": interaction_result(record.interact.as_ref()),
            })
        })
        .collect();
    serde_json::json!({
        "day": logs.day,
        "token": logs.token,
        "coin": logs.coin,
        "cups": logs.cups,
        "cupsByRecipe": logs.cups_by_recipe,
        "cupsBySource": logs.cups_by_source,
        "hires": hires,
        "items": items_dto(&logs.items),
    })
}

fn request_for(action: &str, build: MilkTeaRequestParams) -> MilkTeaOperateRequest {
    let (cmd, activity_id) = operation(action).expect("known action");
    match build {
        MilkTeaRequestParams::Empty => {
            let mut request =
                MilkTeaOperateRequest { activity_id, operate_type: cmd, ..Default::default() };
            match action {
                "shop" => request.milk_tea_get_shop = Some(MilkTeaGetShopReq {}),
                "workshop" => request.milk_tea_get_workshop = Some(MilkTeaGetWorkshopReq {}),
                "claim" => request.milk_tea_claim_cashier = Some(MilkTeaClaimCashierReq {}),
                "upgrade" => request.milk_tea_upgrade = Some(MilkTeaUpgradeReq {}),
                "work" => request.milk_tea_get_work_page = Some(MilkTeaGetWorkPageReq {}),
                "wages" => request.milk_tea_claim_wages = Some(MilkTeaClaimWagesReq {}),
                _ => {}
            }
            request
        }
        MilkTeaRequestParams::ShopName(name) => {
            let mut request =
                MilkTeaOperateRequest { activity_id, operate_type: cmd, ..Default::default() };
            if action == "create" {
                request.milk_tea_create_shop = Some(MilkTeaCreateShopReq { shop_name: name });
            } else {
                request.milk_tea_rename_shop = Some(MilkTeaRenameShopReq { shop_name: name });
            }
            request
        }
        MilkTeaRequestParams::Duration(duration_id) => MilkTeaOperateRequest {
            activity_id,
            operate_type: cmd,
            milk_tea_get_hire_list: Some(MilkTeaGetHireListReq { duration_id }),
            ..Default::default()
        },
        MilkTeaRequestParams::Hire(hires) => MilkTeaOperateRequest {
            activity_id,
            operate_type: cmd,
            milk_tea_hire: Some(MilkTeaHireReq { hires }),
            ..Default::default()
        },
        MilkTeaRequestParams::Count(count) => MilkTeaOperateRequest {
            activity_id,
            operate_type: cmd,
            milk_tea_boost: Some(MilkTeaBoostReq { count }),
            ..Default::default()
        },
        MilkTeaRequestParams::FruitUids(fruit_uids) => MilkTeaOperateRequest {
            activity_id,
            operate_type: cmd,
            milk_tea_craft: Some(MilkTeaCraftReq { fruit_uids }),
            ..Default::default()
        },
        MilkTeaRequestParams::Page(page) => MilkTeaOperateRequest {
            activity_id,
            operate_type: cmd,
            milk_tea_get_rank: Some(MilkTeaGetRankReq { page }),
            ..Default::default()
        },
        MilkTeaRequestParams::Day(day) => MilkTeaOperateRequest {
            activity_id,
            operate_type: cmd,
            milk_tea_get_sales_log: Some(MilkTeaGetSalesLogReq { day }),
            ..Default::default()
        },
        MilkTeaRequestParams::Interact { target_gid, interaction_type } => MilkTeaOperateRequest {
            activity_id,
            operate_type: cmd,
            milk_tea_interact: Some(MilkTeaInteractReq { target_gid, interaction_type }),
            ..Default::default()
        },
        MilkTeaRequestParams::HireDisabled(hire_disabled) => MilkTeaOperateRequest {
            activity_id,
            operate_type: cmd,
            milk_tea_set_hire_disable: Some(MilkTeaSetHireDisableReq { hire_disabled }),
            ..Default::default()
        },
        MilkTeaRequestParams::Goods { goods_id, count } => MilkTeaOperateRequest {
            activity_id,
            operate_type: cmd,
            shop_buy: Some(ShopBuyReq { goods_id, count }),
            ..Default::default()
        },
        MilkTeaRequestParams::FundClaim => {
            // unlock / fundClaim 请求不带参数字段（协议里仅有 activity_id + operate_type）
            let request =
                MilkTeaOperateRequest { activity_id, operate_type: cmd, ..Default::default() };
            request
        }
    }
}

enum MilkTeaRequestParams {
    Empty,
    ShopName(String),
    Duration(i32),
    Hire(Vec<MilkTeaHireItem>),
    Count(i32),
    FruitUids(Vec<i64>),
    Page(i32),
    Day(i64),
    Interact { target_gid: i64, interaction_type: i32 },
    HireDisabled(bool),
    Goods { goods_id: i64, count: i64 },
    FundClaim,
}

fn child(
    group: &crate::proto::generated::gamepb::activitypb::MilkTeaActivityGroup,
    activity_id: i64,
) -> Option<&crate::proto::generated::gamepb::activitypb::MilkTeaActivityData> {
    group
        .children
        .iter()
        .find(|entry| entry.head.as_ref().map(|head| head.activity_id) == Some(activity_id))
}

impl MilkTeaService {
    /// 活动快照（对齐 bot `getMilkTeaActivity`）。
    pub async fn get_activity(
        &self,
        page: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        if ![
            "shop", "settings", "upgrade", "workshop", "hireList", "exchange", "fund", "work",
            "rank", "logs",
        ]
        .contains(&page)
        {
            return Err(Error::Business("未知的活动页面".into()));
        }
        let group = self.fetch_group().await?;
        let entry =
            child(&group, MILK).ok_or_else(|| Error::Business("奶茶活动进度未返回".into()))?;
        let head =
            entry.head.as_ref().ok_or_else(|| Error::Business("奶茶活动进度未返回".into()))?;
        let summary = entry
            .milk_tea
            .as_ref()
            .and_then(|milk_tea| milk_tea.summary.as_ref())
            .ok_or_else(|| Error::Business("奶茶活动进度未返回".into()))?;
        let now = crate::utils::time::get_server_time_secs();
        let active = now >= head.begin_time && now < head.end_time;
        let settling = head.settle_start_time > 0 && now >= head.settle_start_time;
        let mut result = serde_json::json!({
            "id": MILK,
            "title": head.name,
            "serverTime": now,
            "startTime": head.begin_time,
            "endTime": head.end_time,
            "settleStartTime": head.settle_start_time,
            "active": active,
            "settling": settling,
            "summary": summary_dto(summary),
            "rules": rules_from_extra(&head.extra),
            "page": page,
            "recommendedNames": activity_config().recommend_names.split(';').collect::<Vec<_>>(),
            "levels": levels_dto(),
        });
        let inventory = self.inventory(page == "fund" || page == "exchange").await?;
        result["balances"] = serde_json::json!(inventory.1);
        if page == "exchange" {
            let shop = child(&group, SHOP)
                .and_then(|entry| entry.shop.as_ref())
                .ok_or_else(|| Error::Business("兑换商店未返回".into()))?;
            result["goods"] = serde_json::json!(shop
                .goods
                .iter()
                .map(|goods| goods_dto(goods, settling))
                .collect::<Vec<_>>());
        } else if page == "fund" {
            let fund = child(&group, FUND)
                .and_then(|entry| entry.growth_fund.as_ref())
                .ok_or_else(|| Error::Business("成长基金未返回".into()))?;
            result["fund"] = fund_dto(fund);
        } else if active && summary.shop_open_at > 0 {
            if ["shop", "upgrade", "workshop", "hireList"].contains(&page) {
                let reply = self
                    .send_operate("shop", request_for("shop", MilkTeaRequestParams::Empty))
                    .await?;
                let shop = reply.milk_tea_get_shop.and_then(|rsp| rsp.shop).ok_or_else(|| {
                    Error::Business("操作回包不完整，请刷新确认结果，勿重复提交".into())
                })?;
                result["shop"] = shop_dto(&shop);
            }
            if page == "shop" || page == "workshop" {
                let reply = self
                    .send_operate("workshop", request_for("workshop", MilkTeaRequestParams::Empty))
                    .await?;
                let recipes =
                    reply.milk_tea_get_workshop.map(|rsp| rsp.recipes).unwrap_or_default();
                let level =
                    result.pointer("/shop/level").and_then(serde_json::Value::as_i64).unwrap_or(0);
                let bonus =
                    store_level(level).map(|entry| entry.advanced_price_percent).unwrap_or(0);
                result["recipes"] = serde_json::json!(recipes
                    .iter()
                    .map(|recipe| recipe_dto(recipe, bonus))
                    .collect::<Vec<_>>());
            }
            if page == "workshop" {
                let recipes = result
                    .get("recipes")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let unlocked_fruits: Vec<i64> = recipes
                    .iter()
                    .filter(|recipe| {
                        recipe.get("unlocked").and_then(serde_json::Value::as_bool) == Some(true)
                    })
                    .filter_map(|recipe| recipe.get("fruitId").and_then(serde_json::Value::as_i64))
                    .collect();
                let fruits: Vec<serde_json::Value> = inventory
                    .0
                    .iter()
                    .filter(|bag| bag.id > 0 && unlocked_fruits.contains(&bag.id))
                    .map(|bag| {
                        serde_json::json!({
                            "id": bag.id,
                            "count": bag.count,
                            "name": global_game_config().get_item_by_id(bag.id).map(|entry| entry.name).unwrap_or_else(|| format!("物品 #{}", bag.id)),
                            "image": global_game_config().get_item_image_by_id(bag.id).unwrap_or_default(),
                            "uid": bag.uid.to_string(),
                            "locked": bag.locked,
                            "mutations": bag.mutant_types,
                        })
                    })
                    .collect();
                result["fruits"] = serde_json::json!(fruits);
            }
            if page == "hireList" {
                let duration =
                    integer(input.get("durationId").or(Some(&serde_json::json!(1))), 1, 3)?;
                let reply = self
                    .send_operate(
                        "hireList",
                        request_for("hireList", MilkTeaRequestParams::Duration(duration as i32)),
                    )
                    .await?;
                let hire = reply.milk_tea_get_hire_list.ok_or_else(|| {
                    Error::Business("操作回包不完整，请刷新确认结果，勿重复提交".into())
                })?;
                result["hire"] = hire_list_dto(&hire);
                result["durations"] = serde_json::json!(milk_tea_config()
                    .hire_durations
                    .iter()
                    .map(|entry| serde_json::json!({
                        "durationId": entry.duration_id,
                        "durationSec": entry.duration_sec,
                        "displayOrder": entry.display_order,
                        "cost": duration_base_wage(entry),
                        "minRemainingBusinessSec": entry.min_remaining_business_sec,
                    }))
                    .collect::<Vec<_>>());
            }
            if page == "work" {
                let reply = self
                    .send_operate("work", request_for("work", MilkTeaRequestParams::Empty))
                    .await?;
                let work = reply.milk_tea_get_work_page.ok_or_else(|| {
                    Error::Business("操作回包不完整，请刷新确认结果，勿重复提交".into())
                })?;
                let (work_dto, interactions) = normalize_work(&work);
                result["work"] = work_dto;
                result["interactions"] = serde_json::json!(interactions);
            }
            if page == "rank" || page == "logs" {
                let today = ((now + 8 * 3_600) / 86_400) * 86_400 - 8 * 3_600;
                if page == "rank" {
                    let page_number =
                        integer(input.get("pageNumber").or(Some(&serde_json::json!(1))), 1, 100)?;
                    let reply = self
                        .send_operate(
                            "rank",
                            request_for("rank", MilkTeaRequestParams::Page(page_number as i32)),
                        )
                        .await?;
                    let rank = reply.milk_tea_get_rank.ok_or_else(|| {
                        Error::Business("操作回包不完整，请刷新确认结果，勿重复提交".into())
                    })?;
                    result["rank"] = rank_dto(page_number, &rank);
                } else {
                    let day = match input.get("day") {
                        Some(value) => integer(Some(value), 1, today)?,
                        None => today,
                    };
                    let reply = self
                        .send_operate("logs", request_for("logs", MilkTeaRequestParams::Day(day)))
                        .await?;
                    let logs = reply.milk_tea_get_sales_log.ok_or_else(|| {
                        Error::Business("操作回包不完整，请刷新确认结果，勿重复提交".into())
                    })?;
                    result["logs"] = logs_dto(&logs);
                }
            }
        }
        Ok(result)
    }

    /// 背包列表 + 余额。
    async fn inventory(
        &self,
        diamonds: bool,
    ) -> Result<(Vec<crate::services::warehouse::BagItemLite>, HashMap<i64, i64>)> {
        let bag = crate::services::warehouse::WarehouseService::get_bag_via(&self.gateway).await?;
        let bag_items = crate::services::warehouse::get_bag_items(&bag);
        let mut balances: HashMap<i64, i64> = HashMap::new();
        for entry in &bag_items {
            *balances.entry(entry.id).or_insert(0) += entry.count;
        }
        if diamonds {
            let pay = crate::services::pay::PayService::new(self.gateway.clone());
            let value = pay.get_diamond_balance().await.unwrap_or(0);
            balances.insert(DIAMOND_ITEM_ID, value);
        }
        Ok((bag_items, balances))
    }

    /// 执行写操作（对齐 bot `operateMilkTeaActivity` → `perform`）。
    pub async fn operate(
        &self,
        action: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        if operation(action).is_none() || is_read_action(action) {
            return Err(Error::Business("未知的活动操作".into()));
        }
        let _guard = self.mutation_lock.lock().await;
        let page = page_for_action(action);
        let state = self.get_activity(page, input).await?;
        let now = crate::utils::time::get_server_time_secs();
        if state.get("active").and_then(serde_json::Value::as_bool) != Some(true) {
            return Err(Error::Business("活动尚未开放或已经结束".into()));
        }
        let shop_open_at =
            state.pointer("/summary/shopOpenAt").and_then(serde_json::Value::as_i64).unwrap_or(0);
        if action != "create" && shop_open_at == 0 {
            return Err(Error::Business("请先开业".into()));
        }
        let settling = state.get("settling").and_then(serde_json::Value::as_bool) == Some(true);
        if settling && ["create", "rename", "hire", "boost", "interact", "craft"].contains(&action)
        {
            return Err(Error::Business("结算期不可进行此操作".into()));
        }
        let balances: HashMap<i64, i64> = state
            .get("balances")
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .unwrap_or_default();
        let request =
            self.build_operate_request(action, input, &state, &balances, now, shop_open_at)?;
        let reply = self.send_operate(action, request).await?;
        let (result, mut rewards) = consume_reply(action, &reply)?;
        if action == "boost" {
            let reply = reply.milk_tea_boost.clone().unwrap_or_default();
            if reply.token_gain > 0 {
                rewards.push(Item {
                    id: TOKEN_ITEM_ID,
                    count: reply.token_gain,
                    ..Default::default()
                });
            }
            if reply.coin_gain > 0 {
                rewards.push(Item { id: 1001, count: reply.coin_gain, ..Default::default() });
            }
        }
        let mut payload = serde_json::json!({
            "result": result,
            "rewards": items_dto(&rewards),
        });
        match self.get_activity(page, input).await {
            Ok(activity) => {
                payload["activity"] = activity;
            }
            Err(_) => {
                payload["activity"] = serde_json::Value::Null;
                payload["refreshRequired"] = serde_json::json!(true);
            }
        }
        Ok(payload)
    }

    fn build_operate_request(
        &self,
        action: &str,
        input: &serde_json::Value,
        state: &serde_json::Value,
        balances: &HashMap<i64, i64>,
        now: i64,
        shop_open_at: i64,
    ) -> Result<MilkTeaOperateRequest> {
        const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
        Ok(match action {
            "create" | "rename" => {
                let name = input
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let rule = activity_config();
                let length = name.chars().count();
                if length < rule.name_min_length || length > rule.name_max_length {
                    return Err(Error::Business(format!(
                        "店名需为{}—{}个字",
                        rule.name_min_length, rule.name_max_length
                    )));
                }
                if action == "create" && shop_open_at > 0 {
                    return Err(Error::Business("店铺已经开业".into()));
                }
                let rename_cd = state
                    .pointer("/summary/renameCdUntilAt")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0);
                if action == "rename" && rename_cd > now {
                    return Err(Error::Business("尚未到修改店名时间".into()));
                }
                request_for(action, MilkTeaRequestParams::ShopName(name))
            }
            "boost" => {
                let count = integer(input.get("count"), 1, 9_999)?;
                if state.pointer("/shop/isSettling").and_then(serde_json::Value::as_bool)
                    == Some(true)
                {
                    return Err(Error::Business("结算期不可加速".into()));
                }
                let sales_item_id = state
                    .pointer("/shop/salesItemId")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0);
                check_cost(
                    &[Item { id: sales_item_id, count: 1, ..Default::default() }],
                    input.get("expectedCosts"),
                    balances,
                    count,
                )?;
                request_for("boost", MilkTeaRequestParams::Count(count as i32))
            }
            "claim" => {
                let has_pending = state
                    .pointer("/shop/pendingRewards")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|rewards| {
                        rewards.iter().any(|reward| {
                            reward.get("count").and_then(serde_json::Value::as_i64).unwrap_or(0) > 0
                        })
                    });
                if !has_pending {
                    return Err(Error::Business("暂无可领取收益".into()));
                }
                request_for("claim", MilkTeaRequestParams::Empty)
            }
            "upgrade" => {
                let expected = integer(input.get("expectedLevel"), 1, MAX_SAFE_INTEGER)?;
                let level = state
                    .pointer("/summary/level")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0);
                if level != expected {
                    return Err(Error::Business("店铺等级已变化，请刷新后重新确认".into()));
                }
                if state.pointer("/summary/canUpgrade").and_then(serde_json::Value::as_bool)
                    != Some(true)
                {
                    return Err(Error::Business("尚未达到店铺升级条件".into()));
                }
                request_for("upgrade", MilkTeaRequestParams::Empty)
            }
            "craft" => {
                let Some(selected) = input.get("fruits").and_then(serde_json::Value::as_array)
                else {
                    return Err(Error::Business("请选择制作材料".into()));
                };
                if selected.is_empty() || selected.len() > 200 {
                    return Err(Error::Business("请选择制作材料".into()));
                }
                let fruits = state
                    .get("fruits")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let mut seen = std::collections::HashSet::new();
                let mut uids = Vec::new();
                for entry in selected {
                    let uid = entry
                        .get("uid")
                        .map(|value| match value {
                            serde_json::Value::String(text) => text.clone(),
                            other => other.to_string(),
                        })
                        .unwrap_or_default();
                    if !seen.insert(uid.clone()) {
                        return Err(Error::Business("制作材料重复".into()));
                    }
                    let expected_count = integer(entry.get("count"), 1, MAX_SAFE_INTEGER)?;
                    let fruit = fruits.iter().find(|fruit| {
                        fruit.get("uid").and_then(serde_json::Value::as_str) == Some(uid.as_str())
                    });
                    let valid = fruit.is_some_and(|fruit| {
                        fruit.get("locked").and_then(serde_json::Value::as_bool) != Some(true)
                            && fruit.get("count").and_then(serde_json::Value::as_i64)
                                == Some(expected_count)
                    });
                    if !valid {
                        return Err(Error::Business("果实数量或锁定状态已变化，请重新选择".into()));
                    }
                    let uid_number = uid.parse::<i64>().map_err(|_| {
                        Error::Business("果实数量或锁定状态已变化，请重新选择".into())
                    })?;
                    uids.push(uid_number);
                }
                request_for("craft", MilkTeaRequestParams::FruitUids(uids))
            }
            "hire" => {
                let level =
                    state.pointer("/shop/level").and_then(serde_json::Value::as_i64).unwrap_or(0);
                let slot_count =
                    store_level(level).map(|entry| entry.employee_slot_count).unwrap_or(0);
                let slots = state
                    .pointer("/shop/slots")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let free_slot = (0..slot_count).find(|slot| {
                    !slots.iter().any(|entry| {
                        entry.get("slotIndex").and_then(serde_json::Value::as_i64) == Some(*slot)
                            && entry.get("endAt").and_then(serde_json::Value::as_i64).unwrap_or(0)
                                > now
                    })
                });
                let Some(slot_index) = free_slot else {
                    return Err(Error::Business("员工位置已满".into()));
                };
                let target = input
                    .get("targetGid")
                    .map(|value| match value {
                        serde_json::Value::String(text) => text.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default();
                let hire = state.get("hire").cloned().unwrap_or(serde_json::Value::Null);
                let mut candidates = Vec::new();
                for key in ["friends", "robots"] {
                    if let Some(list) = hire.get(key).and_then(serde_json::Value::as_array) {
                        candidates.extend(list.iter().cloned());
                    }
                }
                let candidate = candidates.iter().find(|entry| {
                    entry
                        .pointer("/friend/gid")
                        .map(|value| match value {
                            serde_json::Value::String(text) => text.clone(),
                            other => other.to_string(),
                        })
                        .as_deref()
                        == Some(target.as_str())
                });
                let valid = candidate.is_some_and(|candidate| {
                    candidate.get("state").and_then(serde_json::Value::as_i64) == Some(0)
                        && candidate
                            .get("rehireAvailableAt")
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or(0)
                            <= now
                });
                if !valid {
                    return Err(Error::Business("该员工当前不可雇佣".into()));
                }
                let settle_start =
                    state.get("settleStartTime").and_then(serde_json::Value::as_i64).unwrap_or(0);
                let min_remain =
                    hire.get("minRemainBizSec").and_then(serde_json::Value::as_i64).unwrap_or(0);
                if settle_start - now < min_remain {
                    return Err(Error::Business("剩余营业时间不足".into()));
                }
                let wage_total =
                    hire.get("wageTotal").and_then(serde_json::Value::as_i64).unwrap_or(0);
                check_cost(
                    &[Item { id: TOKEN_ITEM_ID, count: wage_total, ..Default::default() }],
                    input.get("expectedCosts"),
                    balances,
                    1,
                )?;
                let duration_id =
                    hire.get("durationId").and_then(serde_json::Value::as_i64).unwrap_or(0);
                let target_gid = target
                    .parse::<i64>()
                    .map_err(|_| Error::Business("该员工当前不可雇佣".into()))?;
                request_for(
                    "hire",
                    MilkTeaRequestParams::Hire(vec![MilkTeaHireItem {
                        slot_index: slot_index as i32,
                        duration_id: duration_id as i32,
                        target_gid,
                    }]),
                )
            }
            "exchange" => {
                let goods_id_text = input
                    .get("goodsId")
                    .map(|value| match value {
                        serde_json::Value::String(text) => text.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default();
                let count = integer(input.get("count"), 1, 9_999)?;
                let goods = state
                    .get("goods")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|goods| {
                        goods.iter().find(|goods| {
                            goods.get("id").and_then(serde_json::Value::as_str)
                                == Some(goods_id_text.as_str())
                        })
                    })
                    .cloned();
                let available = goods
                    .as_ref()
                    .and_then(|goods| goods.get("available"))
                    .and_then(serde_json::Value::as_bool)
                    == Some(true);
                let remaining = goods
                    .as_ref()
                    .and_then(|goods| goods.get("remaining"))
                    .and_then(serde_json::Value::as_i64);
                if !available || remaining.is_some_and(|remaining| count > remaining) {
                    return Err(Error::Business("商品条件或限购数量不满足".into()));
                }
                let costs: Vec<Item> = goods
                    .as_ref()
                    .and_then(|goods| goods.get("costs"))
                    .and_then(serde_json::Value::as_array)
                    .map(|costs| {
                        costs
                            .iter()
                            .map(|cost| Item {
                                id: cost.get("id").and_then(serde_json::Value::as_i64).unwrap_or(0),
                                count: cost
                                    .get("count")
                                    .and_then(serde_json::Value::as_i64)
                                    .unwrap_or(0),
                                ..Default::default()
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                check_cost(&costs, input.get("expectedCosts"), balances, count)?;
                let goods_id = goods_id_text
                    .parse::<i64>()
                    .map_err(|_| Error::Business("商品条件或限购数量不满足".into()))?;
                request_for("exchange", MilkTeaRequestParams::Goods { goods_id, count })
            }
            "unlock" => {
                if state.pointer("/fund/unlocked").and_then(serde_json::Value::as_bool)
                    == Some(true)
                {
                    return Err(Error::Business("成长基金已解锁".into()));
                }
                let costs: Vec<Item> = state
                    .pointer("/fund/costs")
                    .and_then(serde_json::Value::as_array)
                    .map(|costs| {
                        costs
                            .iter()
                            .map(|cost| Item {
                                id: cost.get("id").and_then(serde_json::Value::as_i64).unwrap_or(0),
                                count: cost
                                    .get("count")
                                    .and_then(serde_json::Value::as_i64)
                                    .unwrap_or(0),
                                ..Default::default()
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                check_cost(&costs, input.get("expectedCosts"), balances, 1)?;
                request_for("unlock", MilkTeaRequestParams::Empty)
            }
            "fundClaim" => {
                let unlocked = state.pointer("/fund/unlocked").and_then(serde_json::Value::as_bool)
                    == Some(true);
                let claimable = state
                    .pointer("/fund/tiers")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|tiers| {
                        tiers.iter().any(|tier| {
                            tier.get("state").and_then(serde_json::Value::as_i64) == Some(2)
                        })
                    });
                if !unlocked || !claimable {
                    return Err(Error::Business("暂无可领取的成长基金奖励".into()));
                }
                request_for("fundClaim", MilkTeaRequestParams::FundClaim)
            }
            "wages" => {
                let claimable = state
                    .pointer("/work/slips")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|slips| {
                        slips.iter().any(|slip| {
                            slip.get("wageStatus").and_then(serde_json::Value::as_i64) == Some(1)
                        })
                    });
                if !claimable {
                    return Err(Error::Business("暂无可领取工资".into()));
                }
                request_for("wages", MilkTeaRequestParams::Empty)
            }
            "interact" => {
                let kind = integer(input.get("interactionType"), 1, 5)?;
                let shift =
                    state.pointer("/work/myShift").cloned().unwrap_or(serde_json::Value::Null);
                let interact_done =
                    state.pointer("/work/interactDone").and_then(serde_json::Value::as_bool)
                        == Some(true);
                let shift_valid = shift
                    .get("endAt")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0)
                    > now
                    && shift.get("interactCount").and_then(serde_json::Value::as_i64).unwrap_or(0)
                        < shift
                            .get("interactLimit")
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or(0);
                if shift.is_null() || interact_done || !shift_valid {
                    return Err(Error::Business("当前班次不可互动".into()));
                }
                let expected_shift = input
                    .get("expectedShiftId")
                    .map(|value| match value {
                        serde_json::Value::String(text) => text.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default();
                let shift_id = shift
                    .get("shiftId")
                    .map(|value| match value {
                        serde_json::Value::String(text) => text.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default();
                if expected_shift != shift_id {
                    return Err(Error::Business("班次已变化，请刷新后重新选择".into()));
                }
                let ready = state
                    .get("interactions")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|interactions| {
                        interactions.iter().any(|entry| {
                            entry.get("interactionId").and_then(serde_json::Value::as_i64)
                                == Some(kind)
                                && entry.get("ready").and_then(serde_json::Value::as_bool)
                                    == Some(true)
                        })
                    });
                if !ready {
                    return Err(Error::Business("工作事项数据不完整，请刷新后重试".into()));
                }
                let target_gid =
                    shift.pointer("/employer/gid").and_then(serde_json::Value::as_i64).unwrap_or(0);
                request_for(
                    "interact",
                    MilkTeaRequestParams::Interact { target_gid, interaction_type: kind as i32 },
                )
            }
            "hireSetting" => {
                let Some(disabled) = input.get("disabled").and_then(serde_json::Value::as_bool)
                else {
                    return Err(Error::Business("雇佣设置无效".into()));
                };
                request_for("hireSetting", MilkTeaRequestParams::HireDisabled(disabled))
            }
            _ => return Err(Error::Business("未知的活动操作".into())),
        })
    }
}

fn page_for_action(action: &str) -> &'static str {
    match action {
        "exchange" => "exchange",
        "unlock" | "fundClaim" => "fund",
        "hire" => "hireList",
        "craft" => "workshop",
        "wages" | "interact" => "work",
        "rename" | "hireSetting" => "settings",
        "upgrade" => "upgrade",
        _ => "shop",
    }
}

fn consume_reply(
    action: &str,
    reply: &MilkTeaOperateReply,
) -> Result<(serde_json::Value, Vec<Item>)> {
    Ok(match action {
        "create" => {
            let rsp = reply.milk_tea_create_shop.clone().unwrap_or_default();
            (serde_json::json!({}), rsp.opening_gift)
        }
        "rename" | "hireSetting" | "unlock" => (serde_json::json!({}), Vec::new()),
        "claim" => {
            let rsp = reply.milk_tea_claim_cashier.clone().unwrap_or_default();
            (serde_json::json!({ "items": items_dto(&rsp.items) }), rsp.items)
        }
        "boost" => {
            let rsp = reply.milk_tea_boost.clone().unwrap_or_default();
            (
                serde_json::json!({
                    "cups": rsp.cups,
                    "tokenGain": rsp.token_gain,
                    "coinGain": rsp.coin_gain,
                }),
                Vec::new(),
            )
        }
        "upgrade" => {
            let rsp = reply.milk_tea_upgrade.clone().unwrap_or_default();
            (
                serde_json::json!({
                    "level": rsp.level,
                    "rewards": items_dto(&rsp.rewards),
                }),
                rsp.rewards,
            )
        }
        "craft" => {
            let rsp = reply.milk_tea_craft.clone().unwrap_or_default();
            (
                serde_json::json!({
                    "craftedCups": rsp.crafted_cups,
                    "drinkCups": rsp.drink_cups,
                    "usedFruits": items_dto(&rsp.used_fruits),
                }),
                Vec::new(),
            )
        }
        "wages" => {
            let rsp = reply.milk_tea_claim_wages.clone().unwrap_or_default();
            (
                serde_json::json!({
                    "items": items_dto(&rsp.items),
                    "claimedCount": rsp.claimed_count,
                }),
                rsp.items,
            )
        }
        "interact" => {
            let rsp = reply.milk_tea_interact.clone().unwrap_or_default();
            (
                serde_json::json!({
                    "interactionType": rsp.interaction_type,
                    "success": rsp.success,
                    "interactionId": rsp.interaction_id,
                    "wageAdjusted": rsp.wage_adjusted,
                    "itemsGained": items_dto(&rsp.items_gained),
                }),
                rsp.items_gained,
            )
        }
        "hire" => {
            let rsp = reply.milk_tea_hire.clone().unwrap_or_default();
            if !rsp.results.iter().any(|result| result.success) {
                let message = rsp
                    .results
                    .first()
                    .map(|result| result.error_msg.clone())
                    .filter(|message| !message.is_empty())
                    .unwrap_or_else(|| "雇佣未成功，请刷新确认".to_string());
                return Err(Error::Business(message));
            }
            let results: Vec<serde_json::Value> = rsp
                .results
                .iter()
                .map(|result| {
                    serde_json::json!({
                        "slotIndex": result.slot_index,
                        "durationId": result.duration_id,
                        "targetGid": result.target_gid,
                        "success": result.success,
                        "refundToken": result.refund_token,
                        "errorCode": result.error_code,
                        "errorMsg": result.error_msg,
                    })
                })
                .collect();
            (serde_json::json!({ "results": results }), Vec::new())
        }
        "exchange" => {
            let rsp = reply.shop_buy.clone().unwrap_or_default();
            (
                serde_json::json!({
                    "awards": items_dto(&rsp.awards),
                    "costs": items_dto(&rsp.costs),
                }),
                rsp.awards,
            )
        }
        "fundClaim" => {
            let rsp = reply.growth_fund_claim.clone().unwrap_or_default();
            (serde_json::json!({ "claimedTierIds": rsp.claimed_tier_ids }), rsp.awards)
        }
        _ => return Err(Error::Business("未知的活动操作".into())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> MilkTeaService {
        let gateway = Arc::new(Gateway::new(
            crate::network::gateway::GatewayConfig {
                server_url: "wss://gate.example.com/ws".into(),
                platform: "qq".into(),
                os: "Windows".into(),
                client_version: "1.15.0.2_20261001".into(),
                auth_code: "test".into(),
                headers: std::collections::HashMap::new(),
            },
            Arc::new(crate::network::encryptor::NoopEncryptor),
        ));
        MilkTeaService::new(gateway)
    }

    fn balances(entries: &[(i64, i64)]) -> HashMap<i64, i64> {
        entries.iter().copied().collect()
    }

    #[test]
    fn config_matches_bot_numbers() {
        let config = milk_tea_config();
        assert_eq!(activity_config().name_min_length, 2);
        assert_eq!(activity_config().name_max_length, 8);
        let duration = config.hire_durations.iter().find(|entry| entry.duration_id == 1).unwrap();
        assert_eq!(duration.duration_sec, 14_400);
        assert_eq!(duration_base_wage(duration), 40);
        let recipe = recipe_config(1).unwrap();
        assert_eq!(recipe.recipe_name, "普通奶茶");
        assert_eq!(recipe_base_price(recipe), 3);
        assert_eq!(recipe_fruit_id(recipe), 0);
        assert_eq!(store_level(1).unwrap().employee_slot_count, 1);
    }

    #[test]
    fn integer_matches_bot_safe_integer_semantics() {
        assert_eq!(integer(Some(&serde_json::json!(1)), 1, 9_999).unwrap(), 1);
        assert_eq!(integer(Some(&serde_json::json!(9_999)), 1, 9_999).unwrap(), 9_999);
        for bad in [
            serde_json::json!(null),
            serde_json::json!(false),
            serde_json::json!(true),
            serde_json::json!("0"),
            serde_json::json!(0.5),
            serde_json::json!(0),
            serde_json::json!(10_000),
        ] {
            assert!(integer(Some(&bad), 1, 9_999).is_err(), "{bad}");
        }
    }

    #[test]
    fn cost_check_rejects_changed_diamond_and_insufficient() {
        let costs = [Item { id: TOKEN_ITEM_ID, count: 40, ..Default::default() }];
        let expected = serde_json::json!([{ "id": TOKEN_ITEM_ID, "count": 40 }]);
        assert!(check_cost(&costs, Some(&expected), &balances(&[(TOKEN_ITEM_ID, 100)]), 1).is_ok());
        let stale = serde_json::json!([{ "id": TOKEN_ITEM_ID, "count": 41 }]);
        assert!(check_cost(&costs, Some(&stale), &balances(&[(TOKEN_ITEM_ID, 100)]), 1).is_err());
        let diamond = [Item { id: DIAMOND_ITEM_ID, count: 1, ..Default::default() }];
        let diamond_expected = serde_json::json!([{ "id": DIAMOND_ITEM_ID, "count": 1 }]);
        let error =
            check_cost(&diamond, Some(&diamond_expected), &balances(&[(DIAMOND_ITEM_ID, 100)]), 1)
                .unwrap_err();
        assert!(error.to_string().contains("费用无效"), "{error}");
        let error =
            check_cost(&costs, Some(&expected), &balances(&[(TOKEN_ITEM_ID, 39)]), 1).unwrap_err();
        assert!(error.to_string().contains("货币或道具数量不足"), "{error}");
        assert!(check_cost(&costs, Some(&expected), &balances(&[(TOKEN_ITEM_ID, 80)]), 2).is_ok());
        assert!(check_cost(&costs, Some(&expected), &balances(&[(TOKEN_ITEM_ID, 79)]), 2).is_err());
    }

    #[test]
    fn interaction_result_matches_bot_rewards() {
        let horn = MilkTeaHireInteract {
            r#type: 4,
            success: true,
            interaction_id: 90_004,
            ..Default::default()
        };
        let result = interaction_result(Some(&horn)).unwrap();
        assert_eq!(result["name"], "划水摸鱼");
        assert_eq!(result["reward"]["id"].as_i64(), Some(HORN_ITEM_ID));
        let none = MilkTeaHireInteract { r#type: 4, success: true, ..Default::default() };
        assert!(interaction_result(Some(&none)).is_none());
        let token = MilkTeaHireInteract {
            r#type: 1,
            success: true,
            interaction_id: 90_001,
            friend: Some(
                crate::proto::generated::gamepb::activitypb::MilkTeaInteractFriendEffect {
                    bonus_amount: 12,
                    ..Default::default()
                },
            ),
            ..Default::default()
        };
        let result = interaction_result(Some(&token)).unwrap();
        assert_eq!(result["name"], "宣传推广");
        assert_eq!(result["reward"]["id"].as_i64(), Some(TOKEN_ITEM_ID));
        assert_eq!(result["reward"]["count"].as_i64(), Some(12));
    }

    #[test]
    fn normalize_work_computes_base_wage_and_interactions() {
        let work = crate::proto::generated::gamepb::activitypb::MilkTeaGetWorkPageRsp {
            my_shift: Some(MilkTeaMyShift {
                shop_name: "奶茶小铺".into(),
                shift_id: 7,
                begin_at: 100,
                end_at: 100 + 14_400,
                interact_count: 1,
                interact_limit: 3,
                wage_total: 40,
                interact_previews: vec![
                    crate::proto::generated::gamepb::activitypb::MilkTeaInteractPreview {
                        interaction_type: 1,
                        promote_cups: 8,
                    },
                ],
                ..Default::default()
            }),
            slips: vec![],
            talent_id: 9001,
            ..Default::default()
        };
        let (work_dto, interactions) = normalize_work(&work);
        assert_eq!(
            work_dto.pointer("/myShift/baseWage").and_then(serde_json::Value::as_i64),
            Some(40)
        );
        assert_eq!(interactions.len(), milk_tea_config().interaction_types.len());
        assert_eq!(interactions[0]["interactionId"].as_i64(), Some(1));
        assert_eq!(interactions[0]["ready"].as_bool(), Some(true));
        assert!(interactions[0]["description"].as_str().unwrap().contains('8'));
        assert!(!work_dto["talentDescription"].as_str().unwrap().contains('<'));
    }

    #[test]
    fn operate_rejects_read_and_unknown_actions_before_network() {
        let service = service();
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            for action in ["shop", "unknown"] {
                let error = service.operate(action, &serde_json::json!({})).await.unwrap_err();
                assert!(error.to_string().contains("未知的活动操作"), "{action}: {error}");
            }
        });
    }

    #[test]
    fn create_name_gate_matches_bot() {
        let service = service();
        let state = serde_json::json!({});
        let error = service
            .build_operate_request(
                "create",
                &serde_json::json!({ "name": "一" }),
                &state,
                &HashMap::new(),
                100,
                0,
            )
            .unwrap_err();
        assert!(error.to_string().contains("店名需为2—8个字"), "{error}");
        assert!(service
            .build_operate_request(
                "create",
                &serde_json::json!({ "name": "奶茶小铺" }),
                &state,
                &HashMap::new(),
                100,
                0,
            )
            .is_ok());
        let error = service
            .build_operate_request(
                "create",
                &serde_json::json!({ "name": "奶茶小铺" }),
                &state,
                &HashMap::new(),
                100,
                123,
            )
            .unwrap_err();
        assert!(error.to_string().contains("店铺已经开业"), "{error}");
    }

    #[test]
    fn exchange_gate_rejects_unavailable_and_insufficient_goods() {
        let service = service();
        let state = serde_json::json!({
            "goods": [{
                "id": "9",
                "available": true,
                "remaining": 2,
                "costs": [{ "id": TOKEN_ITEM_ID, "count": 5 }],
            }]
        });
        let input = serde_json::json!({
            "goodsId": "9",
            "count": 1,
            "expectedCosts": [{ "id": TOKEN_ITEM_ID, "count": 5 }],
        });
        assert!(service
            .build_operate_request(
                "exchange",
                &input,
                &state,
                &balances(&[(TOKEN_ITEM_ID, 5)]),
                100,
                123
            )
            .is_ok());
        let over = serde_json::json!({
            "goodsId": "9",
            "count": 3,
            "expectedCosts": [{ "id": TOKEN_ITEM_ID, "count": 5 }],
        });
        assert!(service
            .build_operate_request(
                "exchange",
                &over,
                &state,
                &balances(&[(TOKEN_ITEM_ID, 100)]),
                100,
                123
            )
            .unwrap_err()
            .to_string()
            .contains("商品条件或限购数量不满足"));
    }

    #[test]
    fn interact_gate_requires_shift_and_ready_interaction() {
        let service = service();
        let now = 1_000;
        let state = serde_json::json!({
            "work": {
                "interactDone": false,
                "myShift": {
                    "shiftId": "7",
                    "endAt": now + 600,
                    "interactCount": 0,
                    "interactLimit": 3,
                    "employer": { "gid": 42 }
                }
            },
            "interactions": [{ "interactionId": 1, "ready": true }]
        });
        let input = serde_json::json!({ "interactionType": 1, "expectedShiftId": "7" });
        assert!(service
            .build_operate_request("interact", &input, &state, &HashMap::new(), now, 123)
            .is_ok());
        let stale = serde_json::json!({ "interactionType": 1, "expectedShiftId": "8" });
        assert!(service
            .build_operate_request("interact", &stale, &state, &HashMap::new(), now, 123)
            .unwrap_err()
            .to_string()
            .contains("班次已变化"));
        let done = serde_json::json!({
            "work": { "interactDone": true, "myShift": { "shiftId": "7", "endAt": now + 600 } },
            "interactions": []
        });
        assert!(service
            .build_operate_request("interact", &input, &done, &HashMap::new(), now, 123)
            .unwrap_err()
            .to_string()
            .contains("当前班次不可互动"));
    }
}
