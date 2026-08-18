use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::constants::{
    BEIJING_UTC_OFFSET_SECONDS, CONSTELLATION_ACTIVITY_TYPE, QINGMEI_DAILY_GRANT_ID,
    SECONDS_PER_DAY, SHOP_ACTIVITY_TYPE,
};
use crate::error::Result;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::activitypb::ConstellationData;
use crate::proto::generated::gamepb::activitypb::{
    ActivityContent, ActivityItem, ActivityOperateReply, StarSandGoods,
};
use crate::proto::generated::gamepb::seasonpb::{
    GetSeasonInfoReply, SeasonActivity, SeasonInfo, SeasonItem, SeasonPass, SeasonRewardNode,
};
use crate::proto::generated::gamepb::solartermspb::{
    GetSolarTermsReply, SolarTermInfo, SolarTermsConfig,
};
use crate::services::activity_center_state::{ActivityStateIdentity, ConstellationActivityState};
use crate::services::warehouse::WarehouseService;

use super::error::{ActivityError, ActivityErrorCode};

/// 赛季活动 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SeasonActivityDto {
    pub id: i64,
    #[serde(rename = "typeCode")]
    pub r#type: i64,
    pub name: String,
    pub begin_time: i64,
    pub start_time: i64,
    pub end_time: i64,
}

/// 通行证节点
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SeasonPassNodeDto {
    pub id: String,
    pub level: String,
    pub key_level: bool,
    pub locked: bool,
    pub claimed: bool,
    pub claimable: bool,
    pub current: bool,
    pub rewards: Vec<ItemDto>,
}

/// 赛季战斗通行证 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SeasonPassDto {
    pub activity_id: i64,
    pub title: String,
    pub current_level: i64,
    pub level: i64,
    pub current_progress: i64,
    pub progress: i64,
    pub progress_target: i64,
    pub progress_max: i64,
    pub node_count: i64,
    pub claimed_through_level: i64,
    pub field11_code: i64,
    pub field13_code: i64,
    pub field18_code: i64,
    pub field14_items: Vec<ItemDto>,
    pub rules: serde_json::Value,
    pub nodes: Vec<SeasonPassNodeDto>,
}

/// 赛季 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SeasonDto {
    pub id: i64,
    pub title: String,
    pub status_code: i64,
    pub field_4_code: i64,
    pub start_time: i64,
    pub end_time: i64,
    pub server_time: i64,
    pub activities: Vec<SeasonActivityDto>,
    pub constellation_activity: Option<SeasonActivityDto>,
    pub shop_activity: Option<SeasonActivityDto>,
    pub pass: Option<SeasonPassDto>,
}

/// 节气 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SolarTermDto {
    pub id: i64,
    pub status: i64,
    pub status_code: String,
    pub can_claim: bool,
    pub claimed: bool,
    pub locked: bool,
    pub current: bool,
    pub begin_time: i64,
    pub start_time: i64,
    pub end_time: i64,
    pub name: String,
    pub rewards: Vec<ItemDto>,
}

/// 节气 DTO（含 rules）
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SolarTermsConfigDto {
    pub id: i64,
    pub activity_id: i64,
    pub rules_text: String,
    #[serde(default)]
    pub rules: serde_json::Value,
}

/// 节气回复 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SolarTermsDto {
    pub server_time: i64,
    pub current_term_id: Option<i64>,
    pub terms: Vec<SolarTermDto>,
    pub current_config: Option<SolarTermsConfigDto>,
    pub configs: Vec<SolarTermsConfigDto>,
}

/// 商品 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StarSandGoodsDto {
    pub id: i64,
    pub activity_id: i64,
    pub name: String,
    pub category: String,
    pub item: ItemDto,
    pub cost: ItemDto,
    pub sort_order: i64,
    pub status_code: i64,
    pub owned: bool,
    pub exchangeable: bool,
    pub sold_out: bool,
    pub balance_known: bool,
    pub max_exchange_count: i64,
    pub max_exchange_count_known: bool,
    pub quality_code: i64,
}

/// 简化物品 DTO（用于商品内嵌的 item / cost）
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ItemDto {
    pub id: i64,
    pub count: i64,
    pub name: String,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub rarity: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance_known: Option<bool>,
}

/// 活动商店 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StarSandShopDto {
    pub activity_id: i64,
    pub name: String,
    pub start_time: i64,
    pub end_time: i64,
    pub server_time: i64,
    pub balance_known: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<String>,
    pub currencies: Vec<ItemDto>,
    pub categories: Vec<String>,
    pub goods: Vec<StarSandGoodsDto>,
    pub affordable_count: i32,
    pub exchangeable_count: i32,
    #[serde(default)]
    pub action: serde_json::Value,
}

/// 星座活动 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConstellationDto {
    pub activity_id: i64,
    pub type_code: String,
    pub display_name: String,
    pub server_name: String,
    pub server_time: i64,
    pub start_time: i64,
    pub end_time: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_day: Option<i32>,
    pub field_1: i64,
    pub field_2: i64,
    pub field_3: i64,
    pub node_count: usize,
    pub group_count: usize,
    #[serde(default)]
    pub catalog_version: i64,
    #[serde(default)]
    pub catalog_status: String,
    #[serde(default)]
    pub rules: serde_json::Value,
    #[serde(default)]
    pub groups: Vec<ConstellationGroupDto>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConstellationGroupDto {
    pub id: String,
    pub node_id: String,
    pub name: String,
    pub category: String,
    pub explain: String,
    pub order: i32,
    pub chart_index: i32,
    pub rewards: Vec<ItemDto>,
    pub links_raw: String,
    pub node_ids: Vec<String>,
    pub visual_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opened: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lit: Option<bool>,
    pub state_known: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_status: Option<String>,
    pub status_source: String,
}

/// 青梅酿酒 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QingMeiDto {
    pub activity_id: String,
    pub daily_activity_id: String,
    pub name: String,
    pub start_time: String,
    pub end_time: String,
    pub rules: serde_json::Value,
    pub ingredient: ItemDto,
    pub ingredients: Vec<serde_json::Value>,
    pub balance: String,
    pub balance_known: bool,
    pub base_gold: String,
    pub base_price: String,
    pub guaranteed_price: String,
    pub current_round: i64,
    pub started: bool,
    pub max_rounds: i64,
    pub finished: bool,
    pub quote_prices: Vec<String>,
    pub quote_totals: Vec<String>,
    pub quote: Option<serde_json::Value>,
    pub daily_seed: serde_json::Value,
    pub actions: serde_json::Value,
}

/// 兑换结果 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeResultDto {
    pub purchase_count: String,
    pub total_item_count: String,
    pub total_cost: String,
    pub rewards: Vec<ItemDto>,
    pub received_items: Vec<ItemDto>,
    pub shop: StarSandShopDto,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<serde_json::Value>,
}

/// 把 `corepb::Item` 序列化为简化的 DTO
pub fn item_dto(item: &CoreItem) -> ItemDto {
    item_from_id(item.id, item.count)
}

/// 把 `activitypb::ActivityItem` 序列化为简化的 DTO
pub fn activity_item_dto(item: &ActivityItem) -> ItemDto {
    item_from_id(item.item_id, item.count)
}

pub(crate) fn item_from_id(id: i64, count: i64) -> ItemDto {
    let gc = crate::config::game_config::global();
    let meta = if id > 0 { gc.get_item_by_id(id) } else { None };
    ItemDto {
        id,
        count,
        name: meta.as_ref().map(|m| m.name.clone()).filter(|n| !n.is_empty()).unwrap_or_default(),
        image: crate::config::game_config::mapped_item_image(id),
        rarity: meta.and_then(|m| m.rarity).unwrap_or(0),
        balance: None,
        balance_known: None,
    }
}

pub(crate) fn season_item_dto(item: &SeasonItem) -> ItemDto {
    item_from_id(item.item_id, item.count)
}

pub(crate) fn solar_term_reward_dto(
    r: &crate::proto::generated::gamepb::solartermspb::SolarTermReward,
) -> ItemDto {
    item_from_id(r.item_id, r.count)
}

/// 把 proto `StarSandGoods` 转为轻量结构（只读 item/cost，不读 name 等 bytes）
#[derive(Debug, Clone, Default)]
pub(crate) struct RawStarSandGoods {
    pub id: i64,
    pub cost: Option<ItemDto>,
    pub item: Option<ItemDto>,
    pub status: i64,
    pub owned: bool,
    pub sort_order: i64,
    pub quality: i64,
    pub name_bytes: Vec<u8>,
    pub category_bytes: Vec<u8>,
}

pub(crate) fn extract_goods(reply: &ActivityOperateReply) -> Vec<RawStarSandGoods> {
    let Some(data) = reply.data.as_ref() else {
        return vec![];
    };
    let Some(catalog) = data.catalog.as_ref() else {
        return vec![];
    };
    catalog
        .goods
        .iter()
        .map(|g| RawStarSandGoods {
            id: g.goods_id,
            cost: g.cost.as_ref().map(activity_item_dto),
            item: g.item.as_ref().map(activity_item_dto),
            status: g.status,
            owned: g.owned,
            sort_order: g.sort_order,
            quality: g.field_10,
            name_bytes: g.name.to_vec(),
            category_bytes: g.category.to_vec(),
        })
        .collect()
}

/// 字节流转字符串（lossy）
pub(crate) fn bytes_to_text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

pub(crate) fn text_content(bytes: &[u8]) -> serde_json::Value {
    let text = bytes_to_text(bytes);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return serde_json::json!({ "title": "", "paragraphs": [] });
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return v;
    }
    serde_json::json!({ "title": "", "paragraphs": [trimmed] })
}

pub(crate) fn constellation_identity(
    season: &SeasonDto,
    activity_id: i64,
) -> ActivityStateIdentity {
    ActivityStateIdentity {
        season_id: season.id.to_string(),
        activity_id: activity_id.to_string(),
        catalog_version: constellation_catalog_version() as i32,
    }
}

pub(crate) fn constellation_dto(
    activity: &SeasonActivityDto,
    server_time: i64,
    dynamic: Option<&ConstellationData>,
    confirmed: &ConstellationActivityState,
) -> ConstellationDto {
    let catalog = constellation_catalog_json();
    let catalog_activity_id = catalog.get("activityId").and_then(|v| v.as_str()).unwrap_or("");
    let catalog_supported = catalog_activity_id == activity.id.to_string();
    let display_name =
        catalog.get("displayName").and_then(|v| v.as_str()).unwrap_or("观星礼录").to_string();
    let catalog_server_name =
        catalog.get("serverName").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if !catalog_supported {
        return ConstellationDto {
            activity_id: activity.id,
            type_code: activity.r#type.to_string(),
            display_name: activity.name.clone(),
            server_name: activity.name.clone(),
            server_time,
            start_time: activity.start_time,
            end_time: activity.end_time,
            current_day: None,
            catalog_version: 0,
            catalog_status: "unsupported".to_string(),
            rules: serde_json::Value::Null,
            groups: vec![],
            ..Default::default()
        };
    }

    let calculated = constellation_day_from_beijing_midnight(activity.start_time, server_time);
    let current_day = calculated.map(|d| d.clamp(1, 28));
    let dynamic_nodes: HashMap<String, (bool, bool)> = dynamic
        .map(|data| {
            data.nodes.iter().map(|n| (n.node_id.to_string(), (n.field_2, n.field_3))).collect()
        })
        .unwrap_or_default();
    let confirmed_opened: HashSet<String> =
        confirmed.confirmed_opened_node_ids.iter().cloned().collect();
    let confirmed_lit: HashSet<String> = confirmed.confirmed_lit_node_ids.iter().cloned().collect();
    let catalog_groups = constellation_catalog_groups()
        .as_array()
        .cloned()
        .unwrap_or_default();

    let groups: Vec<ConstellationGroupDto> = catalog_groups
        .iter()
        .filter_map(|group| {
            let id = json_text(group.get("id"));
            if id.is_empty() {
                return None;
            }
            let node_id = json_text(group.get("nodeId").or_else(|| group.get("node_id")));
            let order = group.get("order").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let chart_index = group
                .pointer("/links/chartIndex")
                .and_then(|v| v.as_i64())
                .unwrap_or_else(|| i64::from((order.saturating_sub(1)) / 7))
                as i32;
            let node_ids = group
                .pointer("/links/nodeIds")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter().map(|v| json_text(Some(v))).filter(|s| !s.is_empty()).collect()
                })
                .unwrap_or_default();
            let rewards = group
                .get("rewards")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|reward| {
                            let rid = json_text(
                                reward
                                    .get("itemId")
                                    .or_else(|| reward.get("item_id"))
                                    .or_else(|| reward.get("id")),
                            )
                            .parse::<i64>()
                            .unwrap_or(0);
                            let count = json_text(reward.get("count")).parse::<i64>().unwrap_or(0);
                            item_from_id(rid, count)
                        })
                        .collect()
                })
                .unwrap_or_default();

            let confirmed_opened_node = confirmed_opened.contains(&node_id);
            let confirmed_lit_node = confirmed_lit.contains(&node_id);
            let (dynamic_opened, dynamic_lit) =
                dynamic_nodes.get(&node_id).copied().unwrap_or((false, false));
            let dynamic_lightable = dynamic_opened && !dynamic_lit;
            let no_claimable = current_day == Some(order)
                && confirmed.no_claimable_days.contains_key(&order.to_string());

            let (opened, lit, state_known, visual_state, claim_status, status_source) =
                if confirmed_lit_node || dynamic_lit || no_claimable {
                    (
                        Some(true),
                        Some(true),
                        true,
                        "lit",
                        if no_claimable { Some("confirmed-no-claimable") } else { None },
                        if no_claimable {
                            "server-rejection"
                        } else if confirmed_lit_node {
                            "persisted"
                        } else {
                            "authoritative"
                        },
                    )
                } else if dynamic_lightable {
                    (Some(true), Some(false), true, "lightable", None, "authoritative")
                } else if current_day.is_some_and(|d| order > d) {
                    (Some(false), Some(false), false, "locked", None, "schedule")
                } else if current_day == Some(order) {
                    (
                        if confirmed_opened_node || dynamic_opened { Some(true) } else { None },
                        None,
                        false,
                        "claimableUnknown",
                        None,
                        if confirmed_opened_node {
                            "persisted"
                        } else if dynamic_opened {
                            "authoritative"
                        } else {
                            "schedule"
                        },
                    )
                } else {
                    (
                        if confirmed_opened_node || dynamic_opened { Some(true) } else { None },
                        None,
                        false,
                        "unknown",
                        None,
                        if confirmed_opened_node {
                            "persisted"
                        } else if dynamic_opened {
                            "authoritative"
                        } else {
                            "schedule"
                        },
                    )
                };

            Some(ConstellationGroupDto {
                id,
                node_id,
                name: json_text(group.get("name")),
                category: json_text(group.get("category")),
                explain: json_text(group.get("explain")),
                order,
                chart_index,
                rewards,
                links_raw: json_text(group.get("linksRaw").or_else(|| group.get("links_raw"))),
                node_ids,
                visual_state: visual_state.to_string(),
                opened,
                lit,
                state_known,
                claim_status: claim_status.map(str::to_string),
                status_source: status_source.to_string(),
            })
        })
        .collect();

    let node_count = dynamic.map(|d| d.nodes.len()).unwrap_or(0);
    let group_count = groups.len();
    ConstellationDto {
        activity_id: activity.id,
        type_code: CONSTELLATION_ACTIVITY_TYPE.to_string(),
        display_name,
        server_name: if activity.name.is_empty() {
            catalog_server_name
        } else {
            activity.name.clone()
        },
        server_time,
        start_time: activity.start_time,
        end_time: activity.end_time,
        current_day,
        field_1: dynamic.map(|d| d.field_1).unwrap_or(0),
        field_2: dynamic.map(|d| d.field_2).unwrap_or(0),
        field_3: dynamic.map(|d| d.field_3).unwrap_or(0),
        node_count,
        group_count,
        catalog_version: constellation_catalog_version(),
        catalog_status: "supported".to_string(),
        rules: constellation_catalog_rules(),
        groups,
    }
}

pub(crate) fn json_text(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

pub(crate) fn json_i64(value: Option<&serde_json::Value>) -> Option<i64> {
    match value? {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

pub(crate) fn json_positive_decimal(
    value: &serde_json::Value,
    code: ActivityErrorCode,
    field_name: &str,
) -> Result<i64> {
    let text = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => String::new(),
    };
    positive_decimal(&text, code, field_name)
}

pub(crate) fn settled_error<T>(result: &Result<T>) -> serde_json::Value {
    match result {
        Ok(_) => serde_json::Value::Null,
        Err(e) => serde_json::Value::String(e.to_string()),
    }
}

pub(crate) fn build_actions(
    season: &Option<SeasonDto>,
    solar: &Option<SolarTermsDto>,
    constellation: Option<&ConstellationDto>,
    shop: Option<&StarSandShopDto>,
) -> serde_json::Value {
    let pass = season.as_ref().and_then(|s| s.pass.as_ref());
    let claimable_pass = pass.map(|p| p.nodes.iter().filter(|n| n.claimable).count()).unwrap_or(0);
    let has_claimable_solar =
        solar.as_ref().map(|s| s.terms.iter().any(|t| t.can_claim)).unwrap_or(false);
    let lightable = constellation
        .map(|c| c.groups.iter().filter(|g| g.visual_state == "lightable").count())
        .unwrap_or(0);
    let attemptable = constellation
        .map(|c| {
            c.groups
                .iter()
                .filter(|g| g.visual_state == "lightable" || g.visual_state == "claimableUnknown")
                .count()
        })
        .unwrap_or(0);
    let current_day = constellation.and_then(|c| c.current_day);
    let current_groups_known = constellation
        .map(|c| {
            let current: Vec<_> =
                c.groups.iter().filter(|g| current_day.is_some_and(|d| g.order == d)).collect();
            !current.is_empty() && current.iter().all(|g| g.state_known)
        })
        .unwrap_or(false);
    let availability_known = lightable > 0 || current_groups_known;
    let catalog_supported = constellation.map(|c| c.catalog_status == "supported").unwrap_or(false);
    let constellation_act = season.as_ref().and_then(|s| s.constellation_activity.as_ref());
    let server_time = season.as_ref().map(|s| s.server_time).unwrap_or(0);
    let start_time = constellation_act.map(|a| a.start_time).unwrap_or(0);
    let end_time = constellation_act.map(|a| a.end_time).unwrap_or(0);
    let constellation_active = constellation_act.is_some()
        && (server_time <= 0 || start_time <= 0 || server_time >= start_time)
        && (server_time <= 0 || end_time <= 0 || server_time <= end_time);
    let affordable = shop.map(|s| s.affordable_count).unwrap_or(0);
    let mut exchange = serde_json::json!({
        "supported": true,
        "enabled": shop.map(|s| s.action.get("enabled").and_then(|v| v.as_bool()).unwrap_or(affordable > 0)).unwrap_or(false),
        "available": shop.map(|s| s.action.get("available").and_then(|v| v.as_bool()).unwrap_or(affordable > 0)).unwrap_or(false),
        "availabilityKnown": shop.is_some(),
        "count": shop
            .and_then(|s| json_i64(s.action.get("count")))
            .unwrap_or(i64::from(affordable)),
    });
    if shop.is_none() {
        exchange["reason"] = serde_json::json!("活动商店目录当前不可用");
    } else if let Some(reason) = shop.and_then(|s| s.action.get("reason").cloned()) {
        if !reason.is_null() && reason.as_str() != Some("") {
            exchange["reason"] = reason;
        }
    }
    serde_json::json!({
        "claimPass": {
            "supported": true,
            "enabled": pass.is_some(),
            "available": claimable_pass > 0,
            "count": claimable_pass,
        },
        "lightConstellation": {
            "supported": true,
            "enabled": constellation_active && attemptable > 0,
            "available": lightable > 0,
            "attemptable": attemptable > 0,
            "availabilityKnown": constellation.is_some() && catalog_supported && availability_known,
            "count": lightable,
            "attemptableCount": attemptable,
        },
        "claimSolar": { "supported": true, "enabled": has_claimable_solar },
        "exchange": exchange,
    })
}

/// 从赛季回复中找出指定 type 的活动
pub fn find_season_activity(
    season_reply: &GetSeasonInfoReply,
    type_code: i64,
) -> Option<&SeasonActivity> {
    let activities = season_reply.season_info.as_ref()?.activities.as_slice();
    activities.iter().find(|a| a.r#type == type_code)
}

/// 把赛季 proto 消息归一化为 DTO
#[must_use]
pub fn normalize_season(reply: &GetSeasonInfoReply) -> Option<SeasonDto> {
    let season: &SeasonInfo = reply.season_info.as_ref()?;
    let activities: Vec<SeasonActivityDto> = season.activities.iter().map(activity_dto).collect();
    let constellation =
        activities.iter().find(|a| a.r#type == CONSTELLATION_ACTIVITY_TYPE).cloned();
    let shop = activities.iter().find(|a| a.r#type == SHOP_ACTIVITY_TYPE).cloned();
    let pass = season.pass.as_ref().map(pass_dto);
    Some(SeasonDto {
        id: season.season_id,
        title: bytes_to_text(&season.name),
        status_code: season.status,
        field_4_code: season.field_4,
        start_time: season.begin_time,
        end_time: season.end_time,
        server_time: season.server_time,
        activities,
        constellation_activity: constellation,
        shop_activity: shop,
        pass,
    })
}

/// 把 `SeasonActivity` 转为 DTO
#[must_use]
pub fn activity_dto(a: &SeasonActivity) -> SeasonActivityDto {
    SeasonActivityDto {
        id: a.activity_id,
        r#type: a.r#type,
        name: bytes_to_text(&a.name),
        begin_time: a.begin_time,
        start_time: a.begin_time,
        end_time: a.end_time,
    }
}

/// 把 `SeasonPass` 转为 DTO
#[must_use]
pub fn pass_dto(p: &SeasonPass) -> SeasonPassDto {
    let current_level = p.current_level;
    let claimed_through = p.claimed_through_level;
    let nodes: Vec<SeasonPassNodeDto> =
        p.nodes.iter().map(|node| pass_node_dto(node, current_level, claimed_through)).collect();
    SeasonPassDto {
        activity_id: p.activity_id,
        title: bytes_to_text(&p.title),
        current_level,
        level: current_level,
        current_progress: p.current_progress,
        progress: p.current_progress,
        progress_target: p.progress_target,
        progress_max: p.progress_target,
        node_count: p.node_count,
        claimed_through_level: claimed_through,
        field11_code: p.field_11,
        field13_code: p.field_13,
        field18_code: p.field_18,
        field14_items: p.field_14.iter().map(season_item_dto).collect(),
        rules: text_content(&p.rules_json),
        nodes,
    }
}

fn pass_node_dto(
    node: &SeasonRewardNode,
    current_level: i64,
    claimed_through: i64,
) -> SeasonPassNodeDto {
    let level = node.node_id;
    let claimed = level != 0 && level <= claimed_through;
    let locked = level == 0 || level > current_level;
    SeasonPassNodeDto {
        id: level.to_string(),
        level: level.to_string(),
        key_level: node.is_key_level,
        locked,
        claimed,
        claimable: !locked && !claimed,
        current: level != 0 && level == current_level,
        rewards: node.rewards.iter().map(season_item_dto).collect(),
    }
}

/// 把节气回复归一化
#[must_use]
pub fn normalize_solar_terms(reply: &GetSolarTermsReply) -> SolarTermsDto {
    let server_time = reply.server_time;
    let terms: Vec<SolarTermDto> = reply.terms.iter().map(solar_term_dto).collect();
    let current_term_id = terms
        .iter()
        .find(|t| {
            t.begin_time > 0
                && t.end_time > 0
                && server_time >= t.begin_time
                && server_time <= t.end_time
        })
        .map(|t| t.id);
    let terms: Vec<SolarTermDto> = terms
        .into_iter()
        .map(|mut t| {
            t.current = current_term_id == Some(t.id);
            t
        })
        .collect();
    let configs: Vec<SolarTermsConfigDto> =
        reply.configs.iter().map(solar_terms_config_dto).collect();
    let current_config = reply.current_config.as_ref().map(solar_terms_config_dto);
    SolarTermsDto { server_time, current_term_id, terms, current_config, configs }
}

pub(crate) fn solar_term_dto(t: &SolarTermInfo) -> SolarTermDto {
    let status = t.status;
    SolarTermDto {
        id: t.term_id,
        status,
        status_code: status.to_string(),
        can_claim: status == 2,
        claimed: status == 3,
        locked: status == 0,
        current: false,
        begin_time: t.begin_time,
        start_time: t.begin_time,
        end_time: t.end_time,
        name: bytes_to_text(&t.name),
        rewards: t.rewards.iter().map(solar_term_reward_dto).collect(),
    }
}

fn solar_terms_config_dto(c: &SolarTermsConfig) -> SolarTermsConfigDto {
    let rules_text = bytes_to_text(&c.rules_json);
    SolarTermsConfigDto {
        id: c.config_id,
        activity_id: c.activity_id,
        rules: text_content(&c.rules_json),
        rules_text,
    }
}

/// 把活动商品 proto 转为 DTO
#[must_use]
pub fn star_sand_goods_dto(
    goods: &StarSandGoods,
    activity_id: i64,
    balances: Option<&std::collections::HashMap<i64, i64>>,
) -> StarSandGoodsDto {
    let cost_id = goods.cost.as_ref().map(|c| c.item_id).unwrap_or(0);
    let cost_count = goods.cost.as_ref().map(|c| c.count).unwrap_or(0);
    let cost_valid = cost_id > 0 && cost_count > 0;
    let balance_known = balances.is_some();
    let balance = balances.and_then(|m| m.get(&cost_id).copied()).unwrap_or(0);
    let max_count =
        if cost_valid && balance_known && cost_count > 0 { balance / cost_count } else { 0 };
    StarSandGoodsDto {
        id: goods.goods_id,
        activity_id,
        name: bytes_to_text(&goods.name),
        category: bytes_to_text(&goods.category),
        item: goods.item.as_ref().map(activity_item_dto).unwrap_or_default(),
        cost: goods.cost.as_ref().map(activity_item_dto).unwrap_or_default(),
        sort_order: goods.sort_order,
        status_code: goods.status,
        owned: goods.owned,
        exchangeable: cost_valid, // 原 TS：status=100 已在成功兑换后出现，不视为禁用
        sold_out: false,
        max_exchange_count: max_count,
        max_exchange_count_known: balance_known,
        balance_known,
        quality_code: goods.field_10,
    }
}

/// 把活动商品回复归一化为活动商店 DTO
#[must_use]
pub fn normalize_shop_from_reply(
    season_reply: &GetSeasonInfoReply,
    shop_activity: &SeasonActivity,
    reply: &ActivityOperateReply,
    balances: Option<&std::collections::HashMap<i64, i64>>,
) -> StarSandShopDto {
    let raw_goods_list = extract_goods(reply);
    let activity_id = reply.activity_id;
    let balance_known = balances.is_some();

    let goods: Vec<StarSandGoodsDto> = raw_goods_list
        .iter()
        .map(|g| {
            let cost_id = g.cost.as_ref().map(|c| c.id).unwrap_or(0);
            let cost_count = g.cost.as_ref().map(|c| c.count).unwrap_or(0);
            let cost_valid = cost_id > 0 && cost_count > 0;
            let balance = balances.and_then(|m| m.get(&cost_id).copied()).unwrap_or(0);
            let max_count = if cost_valid && balance_known && cost_count > 0 {
                balance / cost_count
            } else {
                0
            };
            StarSandGoodsDto {
                id: g.id,
                activity_id,
                name: bytes_to_text(&g.name_bytes),
                category: bytes_to_text(&g.category_bytes),
                item: g.item.clone().unwrap_or_default(),
                cost: g.cost.clone().unwrap_or_default(),
                sort_order: g.sort_order,
                status_code: g.status,
                owned: g.owned,
                exchangeable: cost_valid,
                sold_out: false,
                balance_known,
                max_exchange_count: max_count,
                max_exchange_count_known: balance_known,
                quality_code: g.quality,
            }
        })
        .collect();

    let exchangeable_count = goods.iter().filter(|g| g.exchangeable).count() as i32;
    let affordable_count = goods
        .iter()
        .filter(|g| g.exchangeable && (!g.max_exchange_count_known || g.max_exchange_count > 0))
        .count() as i32;

    let mut categories: Vec<String> =
        goods.iter().map(|g| g.category.clone()).filter(|s| !s.is_empty()).collect();
    categories.sort();
    categories.dedup();

    let currencies: Vec<ItemDto> = if let Some(balances) = balances {
        let mut ids: Vec<i64> = goods
            .iter()
            .filter_map(|g| if g.cost.id > 0 { Some(g.cost.id) } else { None })
            .collect();
        ids.sort();
        ids.dedup();
        ids.into_iter()
            .map(|id| {
                let bal = *balances.get(&id).unwrap_or(&0);
                let mut item = item_from_id(id, bal);
                item.balance = Some(bal.to_string());
                item.balance_known = Some(true);
                item
            })
            .collect()
    } else {
        vec![]
    };

    let server_time = season_reply.season_info.as_ref().map(|s| s.server_time).unwrap_or(0);
    let name = reply
        .data
        .as_ref()
        .and_then(|d| d.activity.as_ref())
        .map(|a: &ActivityContent| a.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| bytes_to_text(&shop_activity.name));

    let mut shop_action = serde_json::json!({
        "supported": true,
        "enabled": affordable_count > 0,
        "available": affordable_count > 0,
        "count": affordable_count,
        "availabilityKnown": true,
    });
    if exchangeable_count == 0 {
        shop_action["reason"] = serde_json::json!("当前目录没有明确可兑换的商品");
    } else if affordable_count == 0 {
        shop_action["reason"] = serde_json::json!("当前余额不足以兑换目录商品");
    }
    let balance =
        currencies.first().and_then(
            |c| {
                if balance_known {
                    Some(c.count.to_string())
                } else {
                    None
                }
            },
        );

    StarSandShopDto {
        activity_id,
        name,
        start_time: shop_activity.begin_time,
        end_time: shop_activity.end_time,
        server_time,
        balance_known,
        balance,
        currencies,
        categories,
        goods,
        affordable_count,
        exchangeable_count,
        action: shop_action,
    }
}

/// 拉取背包中指定货币 id 的余额（best-effort）
pub(crate) async fn read_bag_balances(
    warehouse: &WarehouseService,
    currency_ids: &[i64],
) -> Option<std::collections::HashMap<i64, i64>> {
    let wanted: std::collections::HashSet<i64> = currency_ids.iter().copied().collect();
    if wanted.is_empty() {
        return Some(Default::default());
    }
    let bag = warehouse.get_bag().await.ok()?;
    let mut balances = std::collections::HashMap::new();
    for item in crate::services::warehouse::get_bag_items(&bag) {
        if wanted.contains(&item.id) {
            balances.insert(item.id, item.count.max(0));
        }
    }
    Some(balances)
}

/// 把"00xxx" / 数字字符串解析为正整数
pub fn positive_decimal(value: &str, code: ActivityErrorCode, field_name: &str) -> Result<i64> {
    let text = value.trim();
    // 必须非空、只含 ASCII 数字（不接 '-'，原 TS 用 `^[1-9]\d*$`）
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) {
        return Err(ActivityError {
            code,
            message: format!("{} must be a positive integer", field_name),
        }
        .into());
    }
    if text == "0" {
        return Err(ActivityError {
            code,
            message: format!("{} must be a positive integer", field_name),
        }
        .into());
    }
    let n: i64 = text
        .parse()
        .map_err(|_| ActivityError { code, message: format!("{} is too large", field_name) })?;
    if n < 0 {
        return Err(ActivityError {
            code,
            message: format!("{} must be a positive integer", field_name),
        }
        .into());
    }
    Ok(n)
}

/// 活动 ID / 开始时间（server）→ 北京零点起算的天数
#[must_use]
pub fn constellation_day_from_beijing_midnight(
    start_time_sec: i64,
    server_time_sec: i64,
) -> Option<i32> {
    if start_time_sec <= 0 || server_time_sec <= 0 {
        return None;
    }
    // 转换为北京时间（UTC+8）的"日历天数"
    let beijing_offset = BEIJING_UTC_OFFSET_SECONDS;
    let start_day = (start_time_sec + beijing_offset) / SECONDS_PER_DAY;
    let server_day = (server_time_sec + beijing_offset) / SECONDS_PER_DAY;
    let day_diff = (server_day - start_day) + 1; // 1-based：第 1 天为活动开始当天
    if day_diff < 1 {
        return None;
    }
    i32::try_from(day_diff).ok()
}

pub(crate) fn beijing_date_key() -> String {
    use chrono::{Datelike, TimeZone};
    let dt = chrono::Utc
        .timestamp_opt(crate::utils::time::now_ms() / 1000, 0)
        .single()
        .unwrap_or_else(chrono::Utc::now)
        + chrono::Duration::hours(8);
    format!("{:04}-{:02}-{:02}", dt.year(), dt.month(), dt.day())
}

pub(crate) fn is_qingmei_already_claimed_message(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("1034014")
        || msg.contains("已经领取")
        || msg.contains("无需重复领取")
        || msg.contains("已领取")
}

pub(crate) fn qingmei_seed_claimed_path(account_id: &str) -> std::path::PathBuf {
    use sha2::{Digest, Sha256};
    let token = hex::encode(Sha256::digest(account_id.as_bytes()));
    crate::config::paths::get_data_file(&format!("qingmei-seed-claimed-{token}.json"))
}

pub(crate) fn load_qingmei_seed_claimed_date(account_id: &str) -> Option<String> {
    if account_id.is_empty() {
        return None;
    }
    let path = qingmei_seed_claimed_path(account_id);
    let state: serde_json::Value =
        crate::services::json_db::read_json_with_default(&path, || serde_json::json!({}));
    let date = state.get("date").and_then(|v| v.as_str())?;
    if date == beijing_date_key() {
        Some(date.to_string())
    } else {
        None
    }
}

pub(crate) fn persist_qingmei_seed_claimed_date(
    account_id: &str,
    today: &str,
) -> std::io::Result<()> {
    let path = qingmei_seed_claimed_path(account_id);
    crate::services::json_db::write_json_file_atomic(
        &path,
        &serde_json::json!({
            "date": today,
            "claimed": true,
        }),
    )
}

pub(crate) fn force_qingmei_seed_claimed_in_snapshot(snapshot: &mut Option<serde_json::Value>) {
    let Some(snap) = snapshot.as_mut() else {
        return;
    };
    let Some(qm) = snap.get_mut("qingMei").and_then(|v| v.as_object_mut()) else {
        return;
    };
    if let Some(seed) = qm.get_mut("dailySeed").and_then(|v| v.as_object_mut()) {
        seed.insert("claimed".into(), serde_json::json!(true));
    } else {
        qm.insert(
            "dailySeed".into(),
            serde_json::json!({
                "claimed": true,
                "grantId": QINGMEI_DAILY_GRANT_ID.to_string(),
                "reward": null,
            }),
        );
    }
    if let Some(actions) = qm.get_mut("actions").and_then(|v| v.as_object_mut()) {
        actions.insert(
            "claimSeed".into(),
            serde_json::json!({ "enabled": false, "available": false }),
        );
    }
}

pub(crate) fn constellation_catalog_json() -> serde_json::Value {
    static RAW: &str =
        include_str!("../../../../../assets/activity-data/constellation-2026072701.json");
    serde_json::from_str(RAW).unwrap_or(serde_json::Value::Null)
}

pub(crate) fn constellation_catalog_version() -> i64 {
    constellation_catalog_json().get("catalogVersion").and_then(|v| v.as_i64()).unwrap_or(0)
}

pub(crate) fn constellation_catalog_rules() -> serde_json::Value {
    constellation_catalog_json().get("rules").cloned().unwrap_or(serde_json::Value::Null)
}

pub(crate) fn constellation_catalog_groups() -> serde_json::Value {
    constellation_catalog_json().get("groups").cloned().unwrap_or_else(|| serde_json::json!([]))
}
