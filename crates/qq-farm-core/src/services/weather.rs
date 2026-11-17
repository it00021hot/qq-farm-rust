//! 雨落成诗：自己农场的天气、采集瓶兑换、雷雨召唤和气象研究。

use std::collections::HashMap;
use std::sync::Arc;

use prost::Message as _;
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::network::error::NetworkError;
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::activitypb::{
    ActivityContent, ActivityData, AdvanceWeatherResearchRequest, CollectWeatherRequest,
    ExchangeShopOperateParams, ExchangeShopRequest, GetGroupReply, GetGroupRequest, StarSandGoods,
    WeatherActivityTask, WeatherCollectOperateParams, WeatherResearchNode,
    WeatherResearchOperateParams,
};
use crate::proto::generated::gamepb::itempb::{UseRequest, UseTarget};
use crate::proto::generated::gamepb::weatherpb::{
    GetWeatherStatusReply, GetWeatherStatusRequest, WeatherStatus,
};
use crate::services::friend::api::FriendApi;
use crate::services::warehouse::{get_bag_items, BagItemLite, WarehouseService};

const ACTIVITY_SERVICE: &str = "gamepb.activitypb.ActivityService";
const WEATHER_SERVICE: &str = "gamepb.weatherpb.WeatherService";
const ITEM_SERVICE: &str = "gamepb.itempb.ItemService";
const GROUP_ID: i64 = 2_026_070_300;
const SHOP_ACTIVITY_ID: i64 = 2_026_070_301;
const RESEARCH_ACTIVITY_ID: i64 = 2_026_070_304;
const TASK_ACTIVITY_ID: i64 = 2_026_070_305;
const EXCHANGE_OPERATE_TYPE: i64 = 1;
const ADVANCE_OPERATE_TYPE: i64 = 40;
const COLLECTOR_GOODS_ID: i64 = 200;
const BADGE_ID: i64 = 1027;
const SUMMON_BOTTLE_ID: i64 = 5002;
const COLLECTOR_BOTTLE_ID: i64 = 5001;
const BOTTLE_ACTIVITY_ID: i64 = 2_026_070_303;
const COLLECT_OPERATE_TYPE: i64 = 9;
const COLLECTED_MARKER: i64 = 4;
const ALREADY_COLLECTED: i64 = 1_034_040;
const THUNDERSTORM_TYPE: i64 = 1;
const STATUS_AVAILABLE: i64 = 2;
const STATUS_COMPLETED: i64 = 4;
const GOODS_CLOSED: i64 = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchNode {
    pub id: i64,
    pub completed: bool,
    pub available: bool,
    pub locked: bool,
    pub cost_id: i64,
    pub cost_count: i64,
    pub affordable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnWeather {
    pub weather_type: i64,
    pub active: bool,
    pub thunderstorm: bool,
    pub remaining_sec: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopOffer {
    pub goods_id: i64,
    pub item_id: i64,
    pub item_count: i64,
    pub cost_id: i64,
    pub cost_count: i64,
    pub balance: i64,
    pub owned: bool,
    pub available: bool,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeatherTaskView {
    pub id: i64,
    pub title: String,
    pub current: i64,
    pub limit: i64,
    pub reward_id: i64,
    pub reward_count: i64,
}

pub struct WeatherService {
    gateway: Arc<Gateway>,
    mutation_lock: tokio::sync::Mutex<()>,
}

impl WeatherService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway, mutation_lock: tokio::sync::Mutex::new(()) }
    }

    pub async fn snapshot(&self) -> Result<Value> {
        Ok(board_json(&self.board().await?))
    }

    pub async fn advance(&self, node_id: i64) -> Result<Value> {
        if node_id <= 0 {
            return Err(Error::Business("气象研究节点不存在".into()));
        }
        let _guard = self.mutation_lock.lock().await;
        let board = self.board().await?;
        let node = research_target(&board, node_id)?;
        if node.completed {
            return Err(Error::Business("该气象研究节点已经完成".into()));
        }
        if !node.available {
            return Err(Error::Business("请先完成前置气象研究节点".into()));
        }
        if !node.affordable {
            return Err(Error::Business("雷电徽章不足".into()));
        }
        let request = AdvanceWeatherResearchRequest {
            activity_id: RESEARCH_ACTIVITY_ID,
            operate_type: ADVANCE_OPERATE_TYPE,
            weather_research_operate: Some(WeatherResearchOperateParams { node_id }),
        };
        self.gateway.request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec()).await?;
        Ok(board_json(&self.board().await?))
    }

    pub async fn exchange(&self) -> Result<Value> {
        let _guard = self.mutation_lock.lock().await;
        let board = self.board().await?;
        let shop =
            board.shop.as_ref().ok_or_else(|| Error::Business("天气采集瓶商店暂不可用".into()))?;
        if shop.owned {
            return Err(Error::Business("今日已经兑换过天气采集瓶".into()));
        }
        if !shop.available {
            let reason = if shop.reason.is_empty() {
                "天气采集瓶当前不可兑换"
            } else {
                shop.reason.as_str()
            };
            return Err(Error::Business(reason.to_owned()));
        }
        let request = ExchangeShopRequest {
            activity_id: SHOP_ACTIVITY_ID,
            operate_type: EXCHANGE_OPERATE_TYPE,
            exchange_shop_operate: Some(ExchangeShopOperateParams {
                goods_id: shop.goods_id,
                count: 1,
            }),
        };
        self.gateway.request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec()).await?;
        Ok(board_json(&self.board().await?))
    }

    pub async fn summon(&self, host_gid: i64) -> Result<Value> {
        if host_gid <= 0 {
            return Err(Error::Business("账号尚未登录，暂时无法召唤雷雨".into()));
        }
        let _guard = self.mutation_lock.lock().await;
        let board = self.board().await?;
        if !board.active {
            return Err(Error::Business("雨落成诗活动尚未开放或已经结束".into()));
        }
        if board.weather.active {
            let message = if board.weather.thunderstorm {
                "雷雨正在进行中"
            } else {
                "当前已有其他特殊天气"
            };
            return Err(Error::Business(message.into()));
        }
        let bag = WarehouseService::get_bag_via(&self.gateway).await?;
        let items = get_bag_items(&bag);
        let Some(stack) = soonest_stack(&items, SUMMON_BOTTLE_ID) else {
            return Err(Error::Business("背包中没有可用的雷雨召唤瓶".into()));
        };
        let request = UseRequest {
            item: Some(CoreItem {
                id: SUMMON_BOTTLE_ID,
                count: 1,
                uid: stack.uid,
                ..Default::default()
            }),
            target: Some(UseTarget { host_gid, land_ids: Vec::new(), use_config_id: 0 }),
        };
        self.gateway.request(ITEM_SERVICE, "Use", &request.encode_to_vec()).await?;
        Ok(board_json(&self.board().await?))
    }

    pub async fn inspect(
        &self,
        friends: &FriendApi,
        host_gid: i64,
        self_gid: i64,
    ) -> Result<Value> {
        ensure_other_farm(host_gid, self_gid)?;
        let _guard = self.mutation_lock.lock().await;
        let weather = read_friend_weather(friends, host_gid).await?;
        Ok(visit_json(host_gid, weather.as_ref()))
    }

    pub async fn collect(
        &self,
        friends: &FriendApi,
        host_gid: i64,
        self_gid: i64,
    ) -> Result<Value> {
        ensure_other_farm(host_gid, self_gid)?;
        let _guard = self.mutation_lock.lock().await;
        let bag = WarehouseService::get_bag_via(&self.gateway).await?;
        if !has_item(&get_bag_items(&bag), COLLECTOR_BOTTLE_ID) {
            return Err(Error::Business("背包中没有可用的天气采集瓶".into()));
        }
        let entered = friends.enter_farm(host_gid).await?;
        let operated = self.collect_ready(host_gid, entered.weather.as_ref()).await;
        let _ = friends.leave_farm(host_gid).await;
        operated?;
        let weather = read_friend_weather(friends, host_gid).await?;
        Ok(visit_json(host_gid, weather.as_ref()))
    }

    async fn collect_ready(&self, host_gid: i64, weather: Option<&WeatherStatus>) -> Result<()> {
        let now = crate::utils::time::get_server_time_secs();
        match visit_state(weather, now) {
            "ready" => {}
            "collected" => {
                return Err(Error::Business("当前这轮雷雨已经采过，下轮雷雨可再次采集".into()));
            }
            "ended" => return Err(Error::Business("这场雷雨已经结束".into())),
            _ => return Err(Error::Business("该好友农场当前不是雷雨天气".into())),
        }
        let request = CollectWeatherRequest {
            activity_id: BOTTLE_ACTIVITY_ID,
            operate_type: COLLECT_OPERATE_TYPE,
            weather_collect_operate: Some(WeatherCollectOperateParams { host_gid }),
        };
        match self.gateway.request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec()).await {
            Ok(_) => Ok(()),
            Err(NetworkError::Gateway { code, .. }) if code == ALREADY_COLLECTED => {
                Err(Error::Business("当前这轮雷雨已经采过，下轮雷雨可再次采集".into()))
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn board(&self) -> Result<WeatherBoard> {
        let body = GetGroupRequest { group_id: GROUP_ID }.encode_to_vec();
        let reply_bytes = self.gateway.request(ACTIVITY_SERVICE, "GetGroup", &body).await?;
        let reply = GetGroupReply::decode(&reply_bytes[..])?;
        let group =
            reply.group.ok_or_else(|| Error::Business("活动进度未返回，请稍后刷新".into()))?;
        let activity_id = group.activity.as_ref().map(|activity| activity.activity_id).unwrap_or(0);
        if activity_id != GROUP_ID {
            return Err(Error::Business("服务端未发现雨落成诗活动".into()));
        }
        let now = crate::utils::time::get_server_time_secs();
        let active = activity_open(group.activity.as_ref(), now);
        let bag = WarehouseService::get_bag_via(&self.gateway).await?;
        let items = get_bag_items(&bag);
        let balances = item_balances(&items);
        let weather_bytes = self
            .gateway
            .request(
                WEATHER_SERVICE,
                "GetWeatherStatus",
                &GetWeatherStatusRequest {}.encode_to_vec(),
            )
            .await?;
        let weather_reply = GetWeatherStatusReply::decode(&weather_bytes[..])?;
        let weather = describe_weather(weather_reply.weather.as_ref(), now);
        let shop = find_child(&group, SHOP_ACTIVITY_ID)
            .and_then(|child| child.catalog.as_ref())
            .and_then(|catalog| shop_offer(&catalog.goods, &balances, active));
        let tasks = find_child(&group, TASK_ACTIVITY_ID)
            .and_then(|child| child.weather_tasks.as_ref())
            .map(|tasks| weather_tasks(&tasks.tasks))
            .unwrap_or_default();
        let nodes = find_child(&group, RESEARCH_ACTIVITY_ID)
            .and_then(|child| child.weather_research.as_ref())
            .and_then(|research| research.track.as_ref())
            .map(|track| research_nodes(&track.nodes, *balances.get(&BADGE_ID).unwrap_or(&0)))
            .unwrap_or_default();
        let stage = find_child(&group, RESEARCH_ACTIVITY_ID)
            .and_then(|child| child.weather_research.as_ref())
            .and_then(|research| research.track.as_ref())
            .map(|track| track.current_stage)
            .unwrap_or(0);
        let summon_count = *balances.get(&SUMMON_BOTTLE_ID).unwrap_or(&0);
        let collector_count = *balances.get(&COLLECTOR_BOTTLE_ID).unwrap_or(&0);
        Ok(WeatherBoard {
            active,
            stage,
            badge: *balances.get(&BADGE_ID).unwrap_or(&0),
            nodes,
            weather,
            shop,
            tasks,
            summon_count,
            summon_available: can_summon(active, summon_count, weather.active),
            collector_count,
        })
    }
}

struct WeatherBoard {
    active: bool,
    stage: i64,
    badge: i64,
    nodes: Vec<ResearchNode>,
    weather: OwnWeather,
    shop: Option<ShopOffer>,
    tasks: Vec<WeatherTaskView>,
    summon_count: i64,
    summon_available: bool,
    collector_count: i64,
}

fn research_target(board: &WeatherBoard, node_id: i64) -> Result<&ResearchNode> {
    if !board.active {
        return Err(Error::Business("雨落成诗活动尚未开放或已经结束".into()));
    }
    board
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| Error::Business("气象研究节点不存在".into()))
}

pub fn research_nodes(nodes: &[WeatherResearchNode], badge: i64) -> Vec<ResearchNode> {
    nodes
        .iter()
        .filter(|node| node.node_id > 0)
        .map(|node| {
            let completed = node.status == STATUS_COMPLETED || node.claimed;
            let available = node.status == STATUS_AVAILABLE;
            let (cost_id, cost_count) =
                node.cost.as_ref().map(|cost| (cost.item_id, cost.count.max(0))).unwrap_or((0, 0));
            let affordable = cost_id == BADGE_ID && badge >= cost_count;
            ResearchNode {
                id: node.node_id,
                completed,
                available,
                locked: !completed && !available,
                cost_id,
                cost_count,
                affordable,
            }
        })
        .collect()
}

pub fn describe_weather(status: Option<&WeatherStatus>, now: i64) -> OwnWeather {
    let Some(status) = status else {
        return OwnWeather {
            weather_type: 0,
            active: false,
            thunderstorm: false,
            remaining_sec: 0,
        };
    };
    let active = status.weather_type > 0
        && status.status > 0
        && (status.end_time <= 0 || status.end_time > now);
    OwnWeather {
        weather_type: status.weather_type,
        active,
        thunderstorm: active && status.weather_type == THUNDERSTORM_TYPE,
        remaining_sec: if active && status.end_time > 0 {
            (status.end_time - now).max(0)
        } else {
            0
        },
    }
}

pub fn shop_offer(
    goods: &[StarSandGoods],
    balances: &HashMap<i64, i64>,
    active: bool,
) -> Option<ShopOffer> {
    let entry = goods
        .iter()
        .find(|goods| goods.goods_id == COLLECTOR_GOODS_ID)
        .or_else(|| goods.first())?;
    let (item_id, item_count) = item_pair(entry.item.as_ref());
    let (cost_id, cost_count) = item_pair(entry.cost.as_ref());
    let balance = balances.get(&cost_id).copied().unwrap_or(0);
    let owned = entry.owned;
    let open = entry.status != GOODS_CLOSED;
    let enough = balance >= cost_count;
    let available = active && !owned && open && enough;
    let reason = if !active {
        "雨落成诗活动尚未开放或已经结束"
    } else if owned {
        "今日已经兑换过天气采集瓶"
    } else if !enough {
        "金豆豆不足"
    } else if !open {
        "天气采集瓶当前不可兑换"
    } else {
        ""
    };
    Some(ShopOffer {
        goods_id: entry.goods_id,
        item_id,
        item_count,
        cost_id,
        cost_count,
        balance,
        owned,
        available,
        reason: reason.to_owned(),
    })
}

pub fn weather_tasks(tasks: &[WeatherActivityTask]) -> Vec<WeatherTaskView> {
    tasks
        .iter()
        .filter(|task| task.task_id > 0)
        .map(|task| {
            let (reward_id, reward_count) = item_pair(task.reward.as_ref());
            WeatherTaskView {
                id: task.task_id,
                title: task.title.clone(),
                current: task.current.max(0),
                limit: task.daily_limit.max(0),
                reward_id,
                reward_count,
            }
        })
        .collect()
}

pub fn can_summon(active: bool, bottle_count: i64, weather_active: bool) -> bool {
    active && bottle_count > 0 && !weather_active
}

pub fn visit_state(status: Option<&WeatherStatus>, now: i64) -> &'static str {
    let weather = describe_weather(status, now);
    let collected = status.is_some_and(|status| status.field_9 == COLLECTED_MARKER);
    if weather.thunderstorm && collected {
        "collected"
    } else if weather.thunderstorm {
        "ready"
    } else if status.is_some_and(|status| status.weather_type == THUNDERSTORM_TYPE)
        && !weather.active
    {
        "ended"
    } else {
        "unavailable"
    }
}

fn ensure_other_farm(host_gid: i64, self_gid: i64) -> Result<()> {
    if host_gid <= 0 {
        return Err(Error::Business("请先选择需要检查现场天气的好友".into()));
    }
    if self_gid > 0 && host_gid == self_gid {
        return Err(Error::Business("天气采集瓶只能在好友农场使用".into()));
    }
    Ok(())
}

fn has_item(items: &[BagItemLite], item_id: i64) -> bool {
    items.iter().any(|item| item.id == item_id && item.count > 0)
}

async fn read_friend_weather(friends: &FriendApi, host_gid: i64) -> Result<Option<WeatherStatus>> {
    let reply = friends.enter_farm(host_gid).await?;
    let weather = reply.weather;
    let _ = friends.leave_farm(host_gid).await;
    Ok(weather)
}

fn visit_json(host_gid: i64, weather: Option<&WeatherStatus>) -> Value {
    let now = crate::utils::time::get_server_time_secs();
    json!({
        "gid": host_gid,
        "state": visit_state(weather, now),
    })
}

pub fn soonest_stack(items: &[BagItemLite], item_id: i64) -> Option<&BagItemLite> {
    items
        .iter()
        .filter(|item| item.id == item_id && item.count > 0 && !item.locked)
        .min_by_key(|item| if item.expire_time > 0 { item.expire_time } else { i64::MAX })
}

fn item_pair(
    item: Option<&crate::proto::generated::gamepb::activitypb::ActivityItem>,
) -> (i64, i64) {
    item.map(|item| (item.item_id, item.count.max(0))).unwrap_or((0, 0))
}

fn item_balances(items: &[BagItemLite]) -> HashMap<i64, i64> {
    let mut totals = HashMap::new();
    for item in items {
        if item.id <= 0 {
            continue;
        }
        *totals.entry(item.id).or_insert(0) += item.count;
    }
    totals
}

fn board_json(board: &WeatherBoard) -> Value {
    json!({
        "active": board.active,
        "stage": board.stage,
        "badge": board.badge,
        "nodes": board.nodes.iter().map(node_json).collect::<Vec<_>>(),
        "weather": {
            "type": board.weather.weather_type,
            "active": board.weather.active,
            "thunderstorm": board.weather.thunderstorm,
            "remainingSec": board.weather.remaining_sec,
        },
        "shop": board.shop.as_ref().map(shop_json),
        "tasks": board.tasks.iter().map(task_json).collect::<Vec<_>>(),
        "summonCount": board.summon_count,
        "summonAvailable": board.summon_available,
        "collectorCount": board.collector_count,
    })
}

fn node_json(node: &ResearchNode) -> Value {
    json!({
        "id": node.id,
        "completed": node.completed,
        "available": node.available,
        "locked": node.locked,
        "affordable": node.affordable,
        "costId": node.cost_id,
        "costCount": node.cost_count,
    })
}

fn shop_json(shop: &ShopOffer) -> Value {
    json!({
        "goodsId": shop.goods_id,
        "itemId": shop.item_id,
        "itemCount": shop.item_count,
        "costId": shop.cost_id,
        "costCount": shop.cost_count,
        "balance": shop.balance,
        "owned": shop.owned,
        "available": shop.available,
    })
}

fn task_json(task: &WeatherTaskView) -> Value {
    json!({
        "id": task.id,
        "title": task.title,
        "current": task.current,
        "limit": task.limit,
        "rewardId": task.reward_id,
        "rewardCount": task.reward_count,
    })
}

fn activity_open(activity: Option<&ActivityContent>, now: i64) -> bool {
    let Some(activity) = activity else {
        return true;
    };
    let started = activity.begin_time <= 0 || now >= activity.begin_time;
    let open = activity.end_time <= 0 || now <= activity.end_time;
    started && open
}

fn find_child(group: &ActivityData, activity_id: i64) -> Option<&ActivityData> {
    group.children.iter().find(|child| {
        child.activity.as_ref().is_some_and(|activity| activity.activity_id == activity_id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::generated::gamepb::activitypb::ActivityItem;

    fn node(
        id: i64,
        status: i64,
        claimed: bool,
        cost_id: i64,
        cost_count: i64,
    ) -> WeatherResearchNode {
        WeatherResearchNode {
            node_id: id,
            status,
            claimed,
            cost: (cost_id > 0).then_some(ActivityItem { item_id: cost_id, count: cost_count }),
            ..WeatherResearchNode::default()
        }
    }

    fn goods(id: i64, status: i64, owned: bool, cost_count: i64) -> StarSandGoods {
        StarSandGoods {
            goods_id: id,
            item: Some(ActivityItem { item_id: 5001, count: 1 }),
            cost: Some(ActivityItem { item_id: 1005, count: cost_count }),
            status,
            owned,
            ..StarSandGoods::default()
        }
    }

    fn stack(id: i64, count: i64, uid: i64, expire: i64, locked: bool) -> BagItemLite {
        BagItemLite {
            id,
            count,
            uid,
            mutant_types: Vec::new(),
            expire_time: expire,
            locked,
            source: None,
            protocol_price: None,
        }
    }

    #[test]
    fn research_nodes_mark_ready_done_and_unaffordable() {
        let nodes = research_nodes(
            &[
                node(1, STATUS_COMPLETED, false, BADGE_ID, 1),
                node(2, STATUS_AVAILABLE, false, BADGE_ID, 3),
                node(3, 1, false, BADGE_ID, 1),
                node(4, STATUS_AVAILABLE, true, BADGE_ID, 1),
                node(5, STATUS_AVAILABLE, false, 1001, 1),
            ],
            3,
        );
        assert!(nodes[0].completed);
        assert!(nodes[1].available && nodes[1].affordable && !nodes[1].locked);
        assert!(nodes[2].locked);
        assert!(nodes[3].completed);
        assert!(nodes[4].available && !nodes[4].affordable);
    }

    #[test]
    fn thunderstorm_stays_active_until_it_ends() {
        let storm = describe_weather(
            Some(&WeatherStatus {
                weather_type: THUNDERSTORM_TYPE,
                status: 1,
                end_time: 200,
                ..WeatherStatus::default()
            }),
            80,
        );
        assert!(storm.thunderstorm);
        assert_eq!(storm.remaining_sec, 120);
        let ended = describe_weather(
            Some(&WeatherStatus {
                weather_type: THUNDERSTORM_TYPE,
                status: 1,
                end_time: 200,
                ..WeatherStatus::default()
            }),
            200,
        );
        assert!(!ended.active);
        assert_eq!(ended.remaining_sec, 0);
    }

    #[test]
    fn collector_shop_uses_goods_200_and_blocks_repeat_or_short_balance() {
        let balances = HashMap::from([(1005, 10)]);
        let ready = shop_offer(
            &[goods(9, 1, false, 30), goods(COLLECTOR_GOODS_ID, 1, false, 10)],
            &balances,
            true,
        )
        .expect("offer");
        assert_eq!(ready.goods_id, COLLECTOR_GOODS_ID);
        assert!(ready.available);
        let owned =
            shop_offer(&[goods(COLLECTOR_GOODS_ID, 1, true, 10)], &balances, true).expect("owned");
        assert!(!owned.available);
        let short =
            shop_offer(&[goods(COLLECTOR_GOODS_ID, 1, false, 11)], &balances, true).expect("short");
        assert!(!short.available);
        assert!(short.reason.contains("金豆豆"));
    }

    #[test]
    fn summon_uses_the_soonest_unlocked_bottle_only_in_calm_weather() {
        let items = vec![
            stack(SUMMON_BOTTLE_ID, 1, 2, 50, true),
            stack(SUMMON_BOTTLE_ID, 1, 8, 0, false),
            stack(SUMMON_BOTTLE_ID, 1, 4, 20, false),
        ];
        assert_eq!(soonest_stack(&items, SUMMON_BOTTLE_ID).map(|item| item.uid), Some(4));
        assert!(can_summon(true, 1, false));
        assert!(!can_summon(true, 1, true));
        assert!(!can_summon(false, 1, false));
    }

    #[test]
    fn friend_thunderstorm_can_be_collected_once_per_cycle() {
        let storm = WeatherStatus {
            weather_type: THUNDERSTORM_TYPE,
            status: 1,
            end_time: 200,
            field_9: 1,
            ..WeatherStatus::default()
        };
        assert_eq!(visit_state(Some(&storm), 80), "ready");
        let collected = WeatherStatus { field_9: COLLECTED_MARKER, ..storm };
        assert_eq!(visit_state(Some(&collected), 80), "collected");
        assert_eq!(visit_state(Some(&storm), 200), "ended");
        assert_eq!(
            visit_state(
                Some(&WeatherStatus { weather_type: 2, status: 1, ..WeatherStatus::default() }),
                80
            ),
            "unavailable"
        );
        assert!(ensure_other_farm(0, 1).is_err());
        assert!(ensure_other_farm(7, 7).is_err());
        assert!(ensure_other_farm(8, 7).is_ok());
    }
}
