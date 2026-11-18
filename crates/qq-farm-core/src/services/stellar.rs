//! 千星游记（bot `activity-center/stellar.ts`）。
//!
//! 通行证走 `SeasonService.GetSeasonInfo` / `ClaimBattlePassRewards`。
//! 星砂商店是 `ActivityService.Operate` 7 和 1。点星是 `Operate` 21。
//! 节令不在这里再实现一套。

use std::sync::{Arc, Mutex};

use prost::Message as _;
use serde::Deserialize;

use crate::config::game_config::global as global_game_config;
use crate::error::{Error, Result};
use crate::network::error::NetworkError;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::activitypb::{
    ActivityOperateReply, ConstellationNode, ExchangeShopOperateParams, ExchangeShopRequest,
    OperateConstellationRequest, QueryActivityRequest, StarSandGoods,
};
use crate::proto::generated::gamepb::itempb::{BagReply, BagRequest};
use crate::proto::generated::gamepb::seasonpb::{
    ClaimBattlePassRewardsReply, ClaimBattlePassRewardsRequest, GetSeasonInfoReply,
    GetSeasonInfoRequest, SeasonActivity, SeasonInfo, SeasonPass, SeasonRewardNode,
};
use crate::services::activity_text::strip_tags;

const SEASON_SERVICE: &str = "gamepb.seasonpb.SeasonService";
const ACTIVITY_SERVICE: &str = "gamepb.activitypb.ActivityService";
const ITEM_SERVICE: &str = "gamepb.itempb.ItemService";
const SHOP_TYPE: i64 = 3;
const CONSTELLATION_TYPE: i64 = 13;
const QUERY_SHOP: i64 = 7;
const EXCHANGE_SHOP: i64 = 1;
const LIGHT: i64 = 21;
const NOTHING_TO_CLAIM: i64 = 1_034_038;
const SESSION_MOVED: &str = "账号会话已变化，未发送操作";
const DAY_SECONDS: i64 = 86_400;
const BEIJING_OFFSET: i64 = 8 * 60 * 60;

#[derive(Debug, Deserialize)]
struct CatalogFile {
    #[serde(rename = "catalogVersion")]
    catalog_version: i64,
    #[serde(rename = "activityId")]
    activity_id: String,
    #[serde(rename = "displayName")]
    display_name: String,
    groups: Vec<CatalogGroup>,
}

#[derive(Debug, Deserialize)]
struct CatalogGroup {
    id: String,
    order: i64,
    #[serde(rename = "nodeId")]
    node_id: i64,
    name: String,
    rewards: Vec<CatalogReward>,
}

#[derive(Debug, Deserialize)]
struct CatalogReward {
    #[serde(rename = "itemId")]
    item_id: String,
    count: i64,
}

fn catalog() -> &'static CatalogFile {
    static FILE: std::sync::OnceLock<CatalogFile> = std::sync::OnceLock::new();
    FILE.get_or_init(|| {
        serde_json::from_str(include_str!("fixtures/constellation-2026072701.json"))
            .expect("constellation-2026072701.json 解析失败")
    })
}

/// 基准 `constellationDayFromBeijingMidnight`。开始为 0，或现在早于开始，返回空。
#[must_use]
pub fn constellation_day(start: i64, now: i64) -> Option<i64> {
    if start <= 0 || now < start {
        return None;
    }
    let start_index = (start + BEIJING_OFFSET).div_euclid(DAY_SECONDS);
    let now_index = (now + BEIJING_OFFSET).div_euclid(DAY_SECONDS);
    let day = now_index - start_index + 1;
    Some(day.clamp(1, 28))
}

fn text_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

fn item_name(id: i64) -> String {
    global_game_config()
        .get_item_by_id(id)
        .map(|item| item.name)
        .unwrap_or_else(|| format!("物品 #{id}"))
}

pub struct StellarService {
    gateway: Arc<Gateway>,
    mutation_lock: tokio::sync::Mutex<()>,
    nodes: Mutex<Vec<ConstellationNode>>,
    no_claimable_day: Mutex<Option<i64>>,
}

impl StellarService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            mutation_lock: tokio::sync::Mutex::new(()),
            nodes: Mutex::new(Vec::new()),
            no_claimable_day: Mutex::new(None),
        }
    }

    pub async fn snapshot(&self) -> Result<serde_json::Value> {
        let season = self.season().await?;
        let shop = match find_activity(&season, SHOP_TYPE) {
            Some(activity) => match self.shop_value(&season, activity).await {
                Ok(value) => value,
                Err(error) => serde_json::json!({ "error": error.to_string() }),
            },
            None => serde_json::Value::Null,
        };
        Ok(serde_json::json!({
            "season": season_value(&season),
            "shop": shop,
            "constellation": constellation_value(&season, &self.nodes.lock().expect("nodes"), *self.no_claimable_day.lock().expect("day")),
        }))
    }

    pub async fn claim_pass(&self) -> Result<serde_json::Value> {
        self.claim_pass_if(|| true).await
    }

    pub async fn claim_pass_if(
        &self,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let season = self.season().await?;
        let pass =
            season.pass.as_ref().ok_or_else(|| Error::Business("服务端未发现可用游记".into()))?;
        if !pass_has_claimable(pass) {
            return Err(Error::Business("当前没有可领取的游记奖励".into()));
        }
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let body = ClaimBattlePassRewardsRequest {}.encode_to_vec();
        let bytes = self.gateway.request(SEASON_SERVICE, "ClaimBattlePassRewards", &body).await?;
        let reply = ClaimBattlePassRewardsReply::decode(&bytes[..])?;
        let mut payload = serde_json::json!({
            "rewards": reply.rewards.iter().map(|item| serde_json::json!({
                "id": item.item_id,
                "count": item.count,
                "name": item_name(item.item_id),
            })).collect::<Vec<_>>(),
            "pass": reply.pass.as_ref().map(pass_value),
        });
        match self.season().await {
            Ok(season) => payload["season"] = season_value(&season),
            Err(_) => payload["refreshRequired"] = serde_json::json!(true),
        }
        Ok(payload)
    }

    pub async fn exchange(&self, goods_id: i64, count: i64) -> Result<serde_json::Value> {
        self.exchange_if(goods_id, count, || true).await
    }

    pub async fn exchange_if(
        &self,
        goods_id: i64,
        count: i64,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if goods_id <= 0 {
            return Err(Error::Business("活动商店中未找到指定商品".into()));
        }
        if count <= 0 {
            return Err(Error::Business("兑换数量无效".into()));
        }
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let season = self.season().await?;
        let activity = find_activity(&season, SHOP_TYPE)
            .ok_or_else(|| Error::Business("当前赛季未发现活动商店".into()))?;
        let (_reply, goods) = self.shop_goods(&activity).await?;
        let raw = goods
            .iter()
            .find(|goods| goods.goods_id == goods_id)
            .ok_or_else(|| Error::Business("活动商店中未找到指定商品".into()))?;
        let unit = raw.cost.as_ref().map(|cost| cost.count).unwrap_or(0);
        let currency = raw.cost.as_ref().map(|cost| cost.item_id).unwrap_or(0);
        if currency == 0 || unit <= 0 {
            return Err(Error::Business("商品兑换成本无效，请刷新商店后重试".into()));
        }
        let balances = self.balances(&[currency]).await?;
        let balance = *balances.get(&currency).unwrap_or(&0);
        let total = unit.saturating_mul(count);
        if balance < total {
            return Err(Error::Business("星砂余额不足，无法完成本次兑换".into()));
        }
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let request = ExchangeShopRequest {
            activity_id: activity.activity_id,
            operate_type: EXCHANGE_SHOP,
            exchange_shop_operate: Some(ExchangeShopOperateParams { goods_id, count }),
        };
        let bytes =
            self.gateway.request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec()).await?;
        let reply = ActivityOperateReply::decode(&bytes[..])?;
        if reply.activity_id != activity.activity_id || reply.operate_type != EXCHANGE_SHOP {
            return Err(Error::Business(
                "活动商店兑换回包不匹配，请刷新确认结果，勿重复提交".into(),
            ));
        }
        let item_count =
            raw.item.as_ref().map(|item| item.count).unwrap_or(0).saturating_mul(count);
        Ok(serde_json::json!({
            "purchaseCount": count,
            "totalCost": total,
            "currencyId": currency,
            "rewards": [{ "id": raw.item.as_ref().map(|item| item.item_id).unwrap_or(0), "count": item_count, "name": item_name(raw.item.as_ref().map(|item| item.item_id).unwrap_or(0)) }],
            "shop": shop_value(&reply, &season, &balances).unwrap_or(serde_json::Value::Null),
        }))
    }

    pub async fn light(&self) -> Result<serde_json::Value> {
        self.light_if(|| true).await
    }

    pub async fn light_if(&self, still_current: impl Fn() -> bool) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let season = self.season().await?;
        let activity = find_activity(&season, CONSTELLATION_TYPE)
            .ok_or_else(|| Error::Business("服务端未发现星座活动".into()))?;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let request = OperateConstellationRequest {
            activity_id: activity.activity_id,
            operate_type: LIGHT,
            field_119: Some(Default::default()),
        };
        let sent =
            self.gateway.request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec()).await;
        let bytes = match sent {
            Ok(bytes) => bytes,
            Err(NetworkError::Gateway { code, .. }) if code == NOTHING_TO_CLAIM => {
                let now = season.server_time;
                let day = constellation_day(activity.begin_time, now).unwrap_or(0);
                let active =
                    constellation_write_window(activity.begin_time, activity.end_time, now);
                if active && (1..=28).contains(&day) {
                    *self.no_claimable_day.lock().expect("day") = Some(day);
                    return Ok(serde_json::json!({
                        "outcome": "nothingToClaim",
                        "message": "今日星宿奖励已经领取，无需重复操作",
                    }));
                }
                return Err(Error::Network(NetworkError::Gateway {
                    code,
                    service_name: ACTIVITY_SERVICE.into(),
                    method_name: "Operate".into(),
                    error_message: String::new(),
                    client_seq: 0,
                }));
            }
            Err(error) => return Err(Error::Network(error)),
        };
        let reply = ActivityOperateReply::decode(&bytes[..])?;
        if reply.activity_id != activity.activity_id || reply.operate_type != LIGHT {
            return Err(Error::Business("星座操作回包不匹配，请刷新确认结果，勿重复提交".into()));
        }
        let data = reply
            .data
            .as_ref()
            .and_then(|data| data.constellation.clone())
            .ok_or_else(|| Error::Business("星座操作成功但回包缺少动态状态".into()))?;
        *self.nodes.lock().expect("nodes") = data.nodes.clone();
        Ok(serde_json::json!({
            "outcome": "lighted",
            "constellation": constellation_value(&season, &data.nodes, None),
        }))
    }

    async fn season(&self) -> Result<SeasonInfo> {
        let body = GetSeasonInfoRequest {}.encode_to_vec();
        let bytes = self.gateway.request(SEASON_SERVICE, "GetSeasonInfo", &body).await?;
        let reply = GetSeasonInfoReply::decode(&bytes[..])?;
        reply.season_info.ok_or_else(|| Error::Business("当前赛季数据为空".into()))
    }

    async fn shop_goods(
        &self,
        activity: &SeasonActivity,
    ) -> Result<(ActivityOperateReply, Vec<StarSandGoods>)> {
        let request =
            QueryActivityRequest { activity_id: activity.activity_id, operate_type: QUERY_SHOP };
        let bytes =
            self.gateway.request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec()).await?;
        let reply = ActivityOperateReply::decode(&bytes[..])?;
        if reply.activity_id != activity.activity_id || reply.operate_type != QUERY_SHOP {
            return Err(Error::Business("活动商店查询回包不匹配".into()));
        }
        let goods = reply
            .data
            .as_ref()
            .and_then(|data| data.catalog.clone())
            .map(|catalog| catalog.goods)
            .ok_or_else(|| Error::Business("活动商店查询回包缺少商品目录".into()))?;
        Ok((reply, goods))
    }

    async fn shop_value(
        &self,
        _season: &SeasonInfo,
        activity: &SeasonActivity,
    ) -> Result<serde_json::Value> {
        let (_reply, goods) = self.shop_goods(activity).await?;
        let ids: Vec<i64> = goods
            .iter()
            .filter_map(|goods| goods.cost.as_ref().map(|cost| cost.item_id))
            .filter(|id| *id != 0)
            .collect();
        let balances = self.balances(&ids).await.unwrap_or_default();
        let known = !balances.is_empty() || ids.is_empty();
        Ok(shop_rows(activity, &goods, if known { Some(&balances) } else { None }))
    }

    async fn balances(&self, ids: &[i64]) -> Result<std::collections::HashMap<i64, i64>> {
        let bytes =
            self.gateway.request(ITEM_SERVICE, "Bag", &BagRequest {}.encode_to_vec()).await?;
        let reply = BagReply::decode(&bytes[..])?;
        let mut totals = std::collections::HashMap::new();
        let Some(bag) = reply.item_bag else {
            return Ok(totals);
        };
        for item in bag.items {
            if ids.contains(&item.id) {
                *totals.entry(item.id).or_insert(0) += item.count;
            }
        }
        Ok(totals)
    }
}

fn find_activity(season: &SeasonInfo, type_code: i64) -> Option<&SeasonActivity> {
    season.activities.iter().find(|activity| activity.r#type == type_code)
}

fn pass_has_claimable(pass: &SeasonPass) -> bool {
    pass.nodes.iter().any(|node| node_claimable(pass, node))
}

fn node_claimable(pass: &SeasonPass, node: &SeasonRewardNode) -> bool {
    let level = node.node_id;
    let locked = level == 0 || level > pass.current_level;
    let claimed = level != 0 && level <= pass.claimed_through_level;
    !locked && !claimed
}

fn pass_value(pass: &SeasonPass) -> serde_json::Value {
    let nodes = pass
        .nodes
        .iter()
        .map(|node| {
            let level = node.node_id;
            serde_json::json!({
                "id": level.to_string(),
                "level": level,
                "keyLevel": node.is_key_level,
                "locked": level == 0 || level > pass.current_level,
                "claimed": level != 0 && level <= pass.claimed_through_level,
                "claimable": node_claimable(pass, node),
                "rewards": node.rewards.iter().map(|item| serde_json::json!({
                    "id": item.item_id, "count": item.count, "name": item_name(item.item_id),
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "activityId": pass.activity_id.to_string(),
        "title": text_bytes(&pass.title),
        "level": pass.current_level,
        "progress": pass.current_progress,
        "progressMax": pass.progress_target,
        "claimedThroughLevel": pass.claimed_through_level,
        "nodes": nodes,
    })
}

fn season_value(season: &SeasonInfo) -> serde_json::Value {
    serde_json::json!({
        "id": season.season_id.to_string(),
        "title": text_bytes(&season.name),
        "startTime": season.begin_time,
        "endTime": season.end_time,
        "serverTime": season.server_time,
        "pass": season.pass.as_ref().map(pass_value),
        "constellationActivity": find_activity(season, CONSTELLATION_TYPE).map(activity_value),
        "shopActivity": find_activity(season, SHOP_TYPE).map(activity_value),
    })
}

fn activity_value(activity: &SeasonActivity) -> serde_json::Value {
    serde_json::json!({
        "id": activity.activity_id.to_string(),
        "type": activity.r#type,
        "name": text_bytes(&activity.name),
        "startTime": activity.begin_time,
        "endTime": activity.end_time,
    })
}

fn shop_rows(
    activity: &SeasonActivity,
    goods: &[StarSandGoods],
    balances: Option<&std::collections::HashMap<i64, i64>>,
) -> serde_json::Value {
    let rows = goods
        .iter()
        .map(|goods| {
            let cost_id = goods.cost.as_ref().map(|cost| cost.item_id).unwrap_or(0);
            let cost_count = goods.cost.as_ref().map(|cost| cost.count).unwrap_or(0);
            let exchangeable = cost_id != 0 && cost_count > 0;
            let balance = balances.and_then(|map| map.get(&cost_id).copied());
            let max = match (exchangeable, balance) {
                (true, Some(balance)) => balance / cost_count,
                _ => 0,
            };
            serde_json::json!({
                "id": goods.goods_id.to_string(),
                "name": text_bytes(&goods.name),
                "statusCode": goods.status.to_string(),
                "owned": goods.owned,
                "item": goods.item.as_ref().map(|item| serde_json::json!({"id": item.item_id, "count": item.count, "name": item_name(item.item_id)})),
                "cost": { "id": cost_id, "count": cost_count, "name": item_name(cost_id) },
                "exchangeable": exchangeable,
                "balanceKnown": balances.is_some(),
                "maxExchangeCount": max,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "activityId": activity.activity_id.to_string(),
        "startTime": activity.begin_time,
        "endTime": activity.end_time,
        "goods": rows,
    })
}

fn shop_value(
    reply: &ActivityOperateReply,
    season: &SeasonInfo,
    balances: &std::collections::HashMap<i64, i64>,
) -> Option<serde_json::Value> {
    let activity = find_activity(season, SHOP_TYPE)?;
    let goods = reply.data.as_ref()?.catalog.as_ref()?;
    Some(shop_rows(activity, &goods.goods, Some(balances)))
}

/// 点星写路径里解释 1034038 用的窗口。开始或现在为 0 时不把该错误当成「今日已领」。
#[must_use]
pub fn constellation_write_window(start: i64, end: i64, now: i64) -> bool {
    now > 0 && start > 0 && now >= start && (end <= 0 || now <= end)
}

fn constellation_value(
    season: &SeasonInfo,
    nodes: &[ConstellationNode],
    no_claimable_day: Option<i64>,
) -> serde_json::Value {
    let Some(activity) = find_activity(season, CONSTELLATION_TYPE) else {
        return serde_json::Value::Null;
    };
    let day = constellation_day(activity.begin_time, season.server_time);
    let file = catalog();
    let supported = activity.activity_id.to_string() == file.activity_id;
    if !supported {
        return serde_json::json!({
            "activityId": activity.activity_id.to_string(),
            "catalogStatus": "unsupported",
            "startTime": activity.begin_time,
            "endTime": activity.end_time,
            "serverTime": season.server_time,
            "currentDay": day,
            "groups": [],
        });
    }
    let groups = file
        .groups
        .iter()
        .map(|group| {
            let node = nodes.iter().find(|node| node.node_id == group.node_id);
            let visual = visual_state(group.order, day, node, no_claimable_day);
            serde_json::json!({
                "id": group.id,
                "nodeId": group.node_id.to_string(),
                "name": group.name,
                "order": group.order,
                "visualState": visual,
                "rewards": group.rewards.iter().map(|reward| serde_json::json!({
                    "id": reward.item_id,
                    "count": reward.count,
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "activityId": activity.activity_id.to_string(),
            "displayName": file.display_name,
            "catalogVersion": file.catalog_version,
        "catalogStatus": "supported",
        "startTime": activity.begin_time,
        "endTime": activity.end_time,
        "serverTime": season.server_time,
        "currentDay": day,
        "rules": file.groups.first().map(|_| strip_tags(&file.display_name)).unwrap_or_default(),
        "groups": groups,
    })
}

fn visual_state(
    order: i64,
    day: Option<i64>,
    node: Option<&ConstellationNode>,
    no_claimable_day: Option<i64>,
) -> &'static str {
    let lit = node.is_some_and(|node| node.field_3) || no_claimable_day == Some(order);
    let lightable = node.is_some_and(|node| node.field_2 && !node.field_3);
    if lit {
        "lit"
    } else if lightable {
        "lightable"
    } else if day.is_some_and(|day| order > day) {
        "locked"
    } else if day.is_some_and(|day| order == day) {
        "claimableUnknown"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::encryptor::NoopEncryptor;
    use crate::network::gateway::{Gateway, GatewayConfig};
    use crate::proto::generated::corepb::{Item as CoreItem, ItemBag};
    use crate::proto::generated::gamepb::activitypb::{
        ActivityData, ActivityItem, StarSandGoodsList,
    };
    use crate::proto::generated::gamepb::seasonpb::SeasonItem;

    fn gateway() -> Arc<Gateway> {
        let gateway = Arc::new(Gateway::new(
            GatewayConfig {
                server_url: "wss://gate.example.com/ws".into(),
                platform: "qq".into(),
                os: "Windows".into(),
                client_version: "1".into(),
                auth_code: "test".into(),
                headers: std::collections::HashMap::new(),
            },
            Arc::new(NoopEncryptor),
        ));
        gateway.mark_online();
        gateway
    }

    fn season(now: i64, claimed: i64, level: i64) -> SeasonInfo {
        SeasonInfo {
            season_id: 7,
            server_time: now,
            activities: vec![
                SeasonActivity {
                    activity_id: 2_026_072_701,
                    r#type: CONSTELLATION_TYPE,
                    begin_time: now - 86_400,
                    end_time: now + 86_400,
                    ..SeasonActivity::default()
                },
                SeasonActivity {
                    activity_id: 900,
                    r#type: SHOP_TYPE,
                    begin_time: 0,
                    end_time: 0,
                    ..SeasonActivity::default()
                },
            ],
            pass: Some(SeasonPass {
                activity_id: 11,
                current_level: level,
                current_progress: 3,
                progress_target: 10,
                claimed_through_level: claimed,
                nodes: vec![
                    SeasonRewardNode {
                        node_id: 1,
                        rewards: vec![SeasonItem { item_id: 1002, count: 5 }],
                        is_key_level: false,
                    },
                    SeasonRewardNode {
                        node_id: 2,
                        rewards: vec![SeasonItem { item_id: 1002, count: 9 }],
                        is_key_level: true,
                    },
                ],
                ..SeasonPass::default()
            }),
            ..SeasonInfo::default()
        }
    }

    #[test]
    fn day_zero_start_is_not_a_closed_calendar_and_end_is_inclusive_for_the_error() {
        assert_eq!(constellation_day(0, 1_000), None);
        assert_eq!(constellation_day(1_000, 1_000).is_some(), true);
        assert!(constellation_write_window(100, 0, 150));
        assert!(constellation_write_window(100, 150, 150));
        assert!(!constellation_write_window(0, 200, 150));
        assert!(!constellation_write_window(100, 150, 0));
    }

    #[tokio::test]
    async fn pass_claim_sends_once_and_keeps_rewards_when_refresh_fails() {
        let _data = crate::config::paths::TestDataDir::enter();
        let calls = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let log = Arc::clone(&calls);
        let gateway = gateway();
        gateway.install_reply_script(move |_service, method, _body| {
            log.lock().expect("log").push(method.to_owned());
            if method == "ClaimBattlePassRewards" {
                return Ok(ClaimBattlePassRewardsReply {
                    rewards: vec![SeasonItem { item_id: 1002, count: 5 }],
                    pass: Some(SeasonPass {
                        claimed_through_level: 1,
                        current_level: 1,
                        nodes: vec![SeasonRewardNode { node_id: 1, ..SeasonRewardNode::default() }],
                        ..SeasonPass::default()
                    }),
                    ..ClaimBattlePassRewardsReply::default()
                }
                .encode_to_vec());
            }
            let reads =
                log.lock().expect("log").iter().filter(|method| *method == "GetSeasonInfo").count();
            if reads > 1 {
                return Err(NetworkError::Phase("refresh".into()));
            }
            Ok(GetSeasonInfoReply { season_info: Some(season(1_700_000_000, 0, 1)) }
                .encode_to_vec())
        });
        let value = StellarService::new(gateway).claim_pass().await.unwrap();
        assert_eq!(value["rewards"][0]["count"], 5);
        assert_eq!(value["refreshRequired"], true);
        assert_eq!(
            calls
                .lock()
                .expect("log")
                .iter()
                .filter(|method| *method == "ClaimBattlePassRewards")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn claimed_pass_and_zero_exchange_do_not_send_writes() {
        let _data = crate::config::paths::TestDataDir::enter();
        let calls = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let log = Arc::clone(&calls);
        let gateway = gateway();
        gateway.install_reply_script(move |_service, method, _body| {
            log.lock().expect("log").push(method.to_owned());
            Ok(GetSeasonInfoReply { season_info: Some(season(1_700_000_000, 1, 1)) }
                .encode_to_vec())
        });
        let service = StellarService::new(gateway);
        assert!(service.claim_pass().await.unwrap_err().to_string().contains("没有可领取"));
        assert!(service.exchange(1, 0).await.unwrap_err().to_string().contains("数量"));
        assert!(calls
            .lock()
            .expect("log")
            .iter()
            .all(|method| method != "ClaimBattlePassRewards" && method != "Operate"));
    }

    fn goods(cost: i64) -> ActivityOperateReply {
        ActivityOperateReply {
            activity_id: 900,
            operate_type: QUERY_SHOP,
            data: Some(ActivityData {
                catalog: Some(StarSandGoodsList {
                    goods: vec![StarSandGoods {
                        goods_id: 4,
                        item: Some(ActivityItem { item_id: 8, count: 2 }),
                        cost: Some(ActivityItem { item_id: 3, count: cost }),
                        ..StarSandGoods::default()
                    }],
                }),
                ..ActivityData::default()
            }),
            ..ActivityOperateReply::default()
        }
    }

    fn operate_kind(body: &[u8]) -> i64 {
        QueryActivityRequest::decode(body).map(|request| request.operate_type).unwrap_or(0)
    }

    #[tokio::test]
    async fn exchange_stops_on_balance_and_does_not_replay_a_timeout() {
        let _data = crate::config::paths::TestDataDir::enter();
        let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = Arc::clone(&writes);
        let gateway = gateway();
        gateway.install_reply_script(move |_service, method, body| match method {
            "GetSeasonInfo" => {
                Ok(GetSeasonInfoReply { season_info: Some(season(1_700_000_000, 0, 1)) }
                    .encode_to_vec())
            }
            "Bag" => Ok(BagReply {
                item_bag: Some(ItemBag {
                    items: vec![CoreItem { id: 3, count: 5, ..CoreItem::default() }],
                    ..ItemBag::default()
                }),
            }
            .encode_to_vec()),
            "Operate" if operate_kind(body) == QUERY_SHOP => Ok(goods(3).encode_to_vec()),
            "Operate" => {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err(NetworkError::Timeout {
                    client_seq: 1,
                    service_name: ACTIVITY_SERVICE.into(),
                    method_name: "Operate".into(),
                    pending: 1,
                })
            }
            _ => Err(NetworkError::Phase(method.into())),
        });
        let service = StellarService::new(gateway);
        assert!(service.exchange(4, 2).await.unwrap_err().to_string().contains("余额不足"));
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 0);
        let error = service.exchange(4, 1).await.unwrap_err();
        assert!(error.to_string().contains("请求超时"), "{error}");
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn light_sends_once_and_only_swallows_the_known_code_inside_the_window() {
        let _data = crate::config::paths::TestDataDir::enter();
        let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = Arc::clone(&writes);
        let closed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let closed_flag = Arc::clone(&closed);
        let gateway = gateway();
        gateway.install_reply_script(move |_service, method, body| {
            if method == "Operate" {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                return Err(NetworkError::Gateway {
                    code: NOTHING_TO_CLAIM,
                    service_name: ACTIVITY_SERVICE.into(),
                    method_name: "Operate".into(),
                    error_message: String::new(),
                    client_seq: 1,
                });
            }
            let mut info = season(1_700_000_000, 0, 1);
            if closed_flag.load(std::sync::atomic::Ordering::SeqCst) {
                if let Some(activity) = info
                    .activities
                    .iter_mut()
                    .find(|activity| activity.r#type == CONSTELLATION_TYPE)
                {
                    activity.begin_time = 10;
                    activity.end_time = 20;
                }
                info.server_time = 50;
            }
            let _ = body;
            Ok(GetSeasonInfoReply { season_info: Some(info) }.encode_to_vec())
        });
        let service = StellarService::new(gateway);
        let value = service.light().await.unwrap();
        assert_eq!(value["outcome"], "nothingToClaim");
        closed.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(service.light().await.is_err());
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
