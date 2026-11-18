//! 雨落成诗：自己农场的天气、采集瓶兑换、雷雨召唤、气象研究和好友天气扫描。

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

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
use crate::proto::generated::gamepb::plantpb::LandInfo;
use crate::proto::generated::gamepb::weatherpb::{
    GetWeatherStatusReply, GetWeatherStatusRequest, WeatherStatus,
};
use crate::services::farm::land_analysis::{
    build_land_map, current_phase, is_occupied_slave_land_with_map, PlantPhase,
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
const FROG_BOTTLE_ID: i64 = 5005;
const CLOUD_BOTTLE_ID: i64 = 5006;
const LIGHTNING_MUTATION_ID: i64 = 5003;
const LIGHTNING_ATTRACT_ID: i64 = 5004;
const SENSE_PRIMARY_ID: i64 = 4002;
const SENSE_SECONDARY_ID: i64 = 4003;
const SENSE_PERCENT_EACH: i64 = 2;
const BOTTLE_ACTIVITY_ID: i64 = 2_026_070_303;
const COLLECT_OPERATE_TYPE: i64 = 9;
const COLLECTED_MARKER: i64 = 4;
const ALREADY_COLLECTED: i64 = 1_034_040;
const SCAN_BATCH_LIMIT: usize = 5;
const SCAN_CACHE_TTL_SEC: i64 = 600;
const SCAN_GAP: Duration = Duration::from_millis(300);
const SCAN_WAIT: Duration = Duration::from_secs(10);
const SCAN_POLL: Duration = Duration::from_millis(250);
const SCAN_VISIT_FAILED: &str = "现场天气检查失败";
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
    visits: std::sync::Mutex<HashMap<i64, CachedVisit>>,
}

impl WeatherService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            mutation_lock: tokio::sync::Mutex::new(()),
            visits: std::sync::Mutex::new(HashMap::new()),
        }
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
        match read_friend_visit(friends, host_gid).await {
            Ok(visit) => {
                self.save_visit(host_gid, &visit);
                Ok(visit_value(host_gid, &visit))
            }
            Err(error) => {
                self.save_new_failure(host_gid, &visit_error_text(&error));
                Err(error)
            }
        }
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
        self.forget_visit(host_gid);
        let entered = friends.enter_farm(host_gid).await?;
        let operated = self.collect_ready(host_gid, entered.weather.as_ref()).await;
        let _ = friends.leave_farm(host_gid).await;
        operated?;
        let visit = read_friend_visit(friends, host_gid).await?;
        self.save_visit(host_gid, &visit);
        Ok(visit_value(host_gid, &visit))
    }

    pub async fn frog(&self, friends: &FriendApi, host_gid: i64, self_gid: i64) -> Result<Value> {
        ensure_other_farm(host_gid, self_gid)?;
        let _guard = self.mutation_lock.lock().await;
        let uid = bottle_uid(&self.gateway, FROG_BOTTLE_ID, "背包中没有可用的青蛙使坏瓶").await?;
        self.forget_visit(host_gid);
        let _entered = friends.enter_farm(host_gid).await?;
        let sent = self.use_bottle(FROG_BOTTLE_ID, uid, host_gid, Vec::new()).await;
        let _ = friends.leave_farm(host_gid).await;
        sent?;
        let visit = read_friend_visit(friends, host_gid).await?;
        self.save_visit(host_gid, &visit);
        Ok(visit_value(host_gid, &visit))
    }

    pub async fn cloud(
        &self,
        friends: &FriendApi,
        host_gid: i64,
        self_gid: i64,
        land_id: Option<i64>,
    ) -> Result<Value> {
        ensure_other_farm(host_gid, self_gid)?;
        let _guard = self.mutation_lock.lock().await;
        let uid = bottle_uid(&self.gateway, CLOUD_BOTTLE_ID, "背包中没有可用的乌云使坏瓶").await?;
        self.forget_visit(host_gid);
        let reply = friends.enter_farm(host_gid).await?;
        let sent = match choose_cloud_land(&cloud_land_ids(&reply.lands), land_id) {
            Ok(land_id) => self.use_bottle(CLOUD_BOTTLE_ID, uid, host_gid, vec![land_id]).await,
            Err(error) => Err(error),
        };
        let _ = friends.leave_farm(host_gid).await;
        sent?;
        let visit = read_friend_visit(friends, host_gid).await?;
        self.save_visit(host_gid, &visit);
        Ok(visit_value(host_gid, &visit))
    }

    pub async fn scan(
        &self,
        friends: &FriendApi,
        account_id: &str,
        gids: &[i64],
        self_gid: i64,
    ) -> Result<Value> {
        let chosen = prepare_scan(gids, self_gid)?;
        let _guard = self.mutation_lock.lock().await;
        let mut cache = self.visit_cache();
        let mut reader = LowPriorityVisit { friends };
        let value = scan_friends(
            &chosen,
            &mut cache,
            SCAN_PACE,
            || crate::infra::friend_task_flag::is_friend_checking(account_id),
            &mut reader,
        )
        .await;
        self.replace_visits(cache);
        Ok(value)
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

    async fn use_bottle(
        &self,
        item_id: i64,
        uid: i64,
        host_gid: i64,
        land_ids: Vec<i64>,
    ) -> Result<()> {
        let request = UseRequest {
            item: Some(CoreItem { id: item_id, count: 1, uid, ..Default::default() }),
            target: Some(UseTarget { host_gid, land_ids, use_config_id: 0 }),
        };
        self.gateway.request(ITEM_SERVICE, "Use", &request.encode_to_vec()).await?;
        Ok(())
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
        let summon_count = stock(&balances, SUMMON_BOTTLE_ID);
        let collector_count = stock(&balances, COLLECTOR_BOTTLE_ID);
        let frog_count = stock(&balances, FROG_BOTTLE_ID);
        let cloud_count = stock(&balances, CLOUD_BOTTLE_ID);
        let lightning_mutation_count = stock(&balances, LIGHTNING_MUTATION_ID);
        let lightning_attract_count = stock(&balances, LIGHTNING_ATTRACT_ID);
        let (sense_count, sense_percent) =
            sense_bonus(stock(&balances, SENSE_PRIMARY_ID), stock(&balances, SENSE_SECONDARY_ID));
        Ok(WeatherBoard {
            active,
            stage,
            badge: stock(&balances, BADGE_ID),
            nodes,
            weather,
            shop,
            tasks,
            summon_count,
            summon_available: can_summon(active, summon_count, weather.active),
            collector_count,
            frog_count,
            cloud_count,
            lightning_mutation_count,
            lightning_attract_count,
            sense_count,
            sense_percent,
        })
    }

    fn visit_cache(&self) -> HashMap<i64, CachedVisit> {
        self.visits.lock().expect("weather visit cache").clone()
    }

    fn replace_visits(&self, cache: HashMap<i64, CachedVisit>) {
        *self.visits.lock().expect("weather visit cache") = cache;
    }

    fn save_visit(&self, gid: i64, visit: &FriendVisit) {
        let now = crate::utils::time::get_server_time_secs();
        self.visits.lock().expect("weather visit cache").insert(
            gid,
            CachedVisit { inspected_at: now, visit: Some(visit.clone()), error: None },
        );
    }

    fn save_new_failure(&self, gid: i64, message: &str) {
        let mut cache = self.visits.lock().expect("weather visit cache");
        if cache.contains_key(&gid) {
            return;
        }
        let now = crate::utils::time::get_server_time_secs();
        cache.insert(
            gid,
            CachedVisit { inspected_at: now, visit: None, error: Some(message.to_owned()) },
        );
    }

    fn forget_visit(&self, gid: i64) {
        self.visits.lock().expect("weather visit cache").remove(&gid);
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
    frog_count: i64,
    cloud_count: i64,
    lightning_mutation_count: i64,
    lightning_attract_count: i64,
    sense_count: i64,
    sense_percent: i64,
}

fn stock(balances: &HashMap<i64, i64>, id: i64) -> i64 {
    balances.get(&id).copied().unwrap_or(0).max(0)
}

fn sense_bonus(primary: i64, secondary: i64) -> (i64, i64) {
    let count = primary.max(0).saturating_add(secondary.max(0));
    (count, count.saturating_mul(SENSE_PERCENT_EACH))
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

pub fn prepare_scan(gids: &[i64], self_gid: i64) -> Result<Vec<i64>> {
    let mut chosen = Vec::new();
    for gid in gids {
        if *gid <= 0 || (self_gid > 0 && *gid == self_gid) || chosen.contains(gid) {
            continue;
        }
        chosen.push(*gid);
    }
    if chosen.is_empty() {
        return Err(Error::Business("请先选择需要检查现场天气的好友".into()));
    }
    if chosen.len() > SCAN_BATCH_LIMIT {
        return Err(Error::Business(format!("单次最多检查 {SCAN_BATCH_LIMIT} 位好友，请分批发起")));
    }
    Ok(chosen)
}

async fn wait_until_idle(busy: &mut impl FnMut() -> bool, wait: Duration, poll: Duration) -> bool {
    if !busy() {
        return true;
    }
    let deadline = std::time::Instant::now() + wait;
    loop {
        if !busy() {
            return true;
        }
        if std::time::Instant::now() >= deadline || poll.is_zero() {
            return false;
        }
        tokio::time::sleep(poll).await;
    }
}

fn has_item(items: &[BagItemLite], item_id: i64) -> bool {
    items.iter().any(|item| item.id == item_id && item.count > 0)
}

async fn read_friend_visit(friends: &FriendApi, host_gid: i64) -> Result<FriendVisit> {
    let reply = friends.enter_farm(host_gid).await?;
    let visit = FriendVisit { weather: reply.weather, cloud_lands: cloud_land_ids(&reply.lands) };
    let _ = friends.leave_farm(host_gid).await;
    Ok(visit)
}

async fn read_friend_visit_low(friends: &FriendApi, host_gid: i64) -> Result<FriendVisit> {
    crate::network::gateway::background_scope(read_friend_visit(friends, host_gid)).await
}

#[derive(Clone)]
struct FriendVisit {
    weather: Option<WeatherStatus>,
    cloud_lands: Vec<i64>,
}

#[derive(Clone)]
struct CachedVisit {
    inspected_at: i64,
    visit: Option<FriendVisit>,
    error: Option<String>,
}

struct ScanPace {
    wait: Duration,
    poll: Duration,
    gap: Duration,
}

const SCAN_PACE: ScanPace = ScanPace { wait: SCAN_WAIT, poll: SCAN_POLL, gap: SCAN_GAP };

trait VisitReader {
    fn read(
        &mut self,
        gid: i64,
    ) -> impl Future<Output = std::result::Result<FriendVisit, String>> + Send + '_;
}

struct LowPriorityVisit<'a> {
    friends: &'a FriendApi,
}

impl VisitReader for LowPriorityVisit<'_> {
    async fn read(&mut self, gid: i64) -> std::result::Result<FriendVisit, String> {
        read_friend_visit_low(self.friends, gid).await.map_err(|error| visit_error_text(&error))
    }
}

fn visit_error_text(error: &Error) -> String {
    let message = match error {
        Error::Business(message) => message.clone(),
        Error::Network(NetworkError::Gateway { error_message, .. }) => error_message.clone(),
        other => other.to_string(),
    };
    let trimmed = message.trim();
    if trimmed.is_empty() {
        SCAN_VISIT_FAILED.to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn fresh_visit(cache: &HashMap<i64, CachedVisit>, gid: i64, now: i64) -> Option<&CachedVisit> {
    cache.get(&gid).filter(|item| now.saturating_sub(item.inspected_at) <= SCAN_CACHE_TTL_SEC)
}

fn scan_row(gid: i64, cached: &CachedVisit, now: i64, error_override: Option<&str>) -> Value {
    let error = error_override.or(cached.error.as_deref()).unwrap_or("");
    let (state, lands) = match &cached.visit {
        Some(visit) => (visit_state(visit.weather.as_ref(), now), visit.cloud_lands.clone()),
        None => ("unknown", Vec::new()),
    };
    json!({ "gid": gid, "state": state, "cloudLands": lands, "error": error })
}

fn remember_visit(
    cache: &mut HashMap<i64, CachedVisit>,
    gid: i64,
    now: i64,
    visit: FriendVisit,
) -> Value {
    let cached = CachedVisit { inspected_at: now, visit: Some(visit), error: None };
    let row = scan_row(gid, &cached, now, None);
    cache.insert(gid, cached);
    row
}

fn remember_failure(
    cache: &mut HashMap<i64, CachedVisit>,
    gid: i64,
    now: i64,
    message: &str,
) -> Value {
    if let Some(previous) = cache.get(&gid) {
        return scan_row(gid, previous, now, Some(message));
    }
    let cached = CachedVisit { inspected_at: now, visit: None, error: Some(message.to_owned()) };
    let row = scan_row(gid, &cached, now, None);
    cache.insert(gid, cached);
    row
}

async fn scan_friends<R: VisitReader>(
    chosen: &[i64],
    cache: &mut HashMap<i64, CachedVisit>,
    pace: ScanPace,
    mut busy: impl FnMut() -> bool,
    reader: &mut R,
) -> Value {
    let mut rows = Vec::new();
    let mut deferred = Vec::new();
    let mut entered = 0_usize;
    for (index, gid) in chosen.iter().copied().enumerate() {
        let now = crate::utils::time::get_server_time_secs();
        if let Some(hit) = fresh_visit(cache, gid, now) {
            rows.push(scan_row(gid, hit, now, None));
            continue;
        }
        if !wait_until_idle(&mut busy, pace.wait, pace.poll).await {
            deferred.extend(chosen[index..].iter().copied());
            break;
        }
        if entered > 0 {
            tokio::time::sleep(pace.gap).await;
        }
        let now = crate::utils::time::get_server_time_secs();
        let result = reader.read(gid).await;
        rows.push(apply_read(cache, gid, now, result));
        entered += 1;
    }
    json!({ "friends": rows, "deferredGids": deferred })
}

fn apply_read(
    cache: &mut HashMap<i64, CachedVisit>,
    gid: i64,
    now: i64,
    result: std::result::Result<FriendVisit, String>,
) -> Value {
    match result {
        Ok(visit) => remember_visit(cache, gid, now, visit),
        Err(message) => remember_failure(cache, gid, now, &message),
    }
}

fn visit_value(host_gid: i64, visit: &FriendVisit) -> Value {
    let now = crate::utils::time::get_server_time_secs();
    json!({
        "gid": host_gid,
        "state": visit_state(visit.weather.as_ref(), now),
        "cloudLands": visit.cloud_lands,
    })
}

pub fn choose_cloud_land(ids: &[i64], requested: Option<i64>) -> Result<i64> {
    let land_id = requested.filter(|id| *id > 0).or_else(|| ids.first().copied());
    match land_id {
        Some(land_id) if ids.contains(&land_id) => Ok(land_id),
        _ => Err(Error::Business("好友当前没有可使用乌云使坏瓶的作物".into())),
    }
}

pub fn cloud_land_ids(lands: &[LandInfo]) -> Vec<i64> {
    let map = build_land_map(lands);
    let mut ids = Vec::new();
    for land in lands {
        if !land.unlocked || land.id <= 0 || is_occupied_slave_land_with_map(land, &map) {
            continue;
        }
        let Some(plant) = land.plant.as_ref() else { continue };
        if plant.id <= 0 || plant.phases.is_empty() {
            continue;
        }
        if !matches!(current_phase(land), PlantPhase::Sprout | PlantPhase::Growing) {
            continue;
        }
        if plant.interaction_uses.iter().any(|use_info| use_info.item_id == CLOUD_BOTTLE_ID)
            || plant.interaction_targets.iter().any(|target| target.item_id == CLOUD_BOTTLE_ID)
        {
            continue;
        }
        if !ids.contains(&land.id) {
            ids.push(land.id);
        }
    }
    ids
}

async fn bottle_uid(gateway: &Arc<Gateway>, item_id: i64, missing: &str) -> Result<i64> {
    let bag = WarehouseService::get_bag_via(gateway).await?;
    soonest_stack(&get_bag_items(&bag), item_id)
        .map(|item| item.uid)
        .ok_or_else(|| Error::Business(missing.to_owned()))
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
        "frogCount": board.frog_count,
        "cloudCount": board.cloud_count,
        "lightningMutationCount": board.lightning_mutation_count,
        "lightningAttractCount": board.lightning_attract_count,
        "senseCount": board.sense_count,
        "sensePercent": board.sense_percent,
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

    #[test]
    fn lightning_sense_adds_two_percent_per_copy() {
        assert_eq!(sense_bonus(0, 0), (0, 0));
        assert_eq!(sense_bonus(2, 1), (3, 6));
        assert_eq!(sense_bonus(-1, 4), (4, 8));
    }

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

    #[test]
    fn cloud_bottle_targets_only_growing_master_plots() {
        use crate::proto::generated::gamepb::plantpb::{
            PlantInfo, PlantInteractionTargetInfo, PlantPhaseInfo,
        };

        fn land(id: i64, unlocked: bool, phase: i32, master: i64, cloud: bool) -> LandInfo {
            let plant = (phase > 0).then(|| PlantInfo {
                id: 1,
                phases: vec![PlantPhaseInfo { phase, ..PlantPhaseInfo::default() }],
                interaction_targets: cloud
                    .then(|| PlantInteractionTargetInfo {
                        item_id: CLOUD_BOTTLE_ID,
                        ..Default::default()
                    })
                    .into_iter()
                    .collect(),
                ..PlantInfo::default()
            });
            LandInfo { id, unlocked, master_land_id: master, plant, ..LandInfo::default() }
        }

        let ids = cloud_land_ids(&[
            land(1, true, 3, 0, false),
            land(2, true, 3, 1, false),
            land(3, true, 6, 0, false),
            land(4, true, 3, 0, true),
            land(5, false, 3, 0, false),
        ]);
        assert_eq!(ids, vec![1]);
        assert_eq!(choose_cloud_land(&ids, None).expect("first"), 1);
        assert_eq!(choose_cloud_land(&[1, 4], Some(4)).expect("chosen"), 4);
        assert!(choose_cloud_land(&ids, Some(9)).is_err());
        assert!(choose_cloud_land(&[], None).is_err());
    }

    #[test]
    fn scan_keeps_five_distinct_friends_and_rejects_a_larger_batch() {
        assert_eq!(prepare_scan(&[0, 7, 7, 8, 9], 7).expect("chosen"), vec![8, 9]);
        let empty = prepare_scan(&[7], 7).expect_err("self").to_string();
        assert!(empty.contains("请先选择需要检查现场天气的好友"));
        let oversized = prepare_scan(&[1, 2, 3, 4, 5, 6], 0).expect_err("limit").to_string();
        assert!(oversized.contains("单次最多检查 5 位好友"));
        assert_eq!(prepare_scan(&[1, 2, 3, 4, 5], 0).expect("five").len(), 5);
    }

    fn ready_visit() -> FriendVisit {
        FriendVisit {
            weather: Some(WeatherStatus {
                weather_type: THUNDERSTORM_TYPE,
                status: 1,
                end_time: crate::utils::time::get_server_time_secs() + 3_600,
                ..WeatherStatus::default()
            }),
            cloud_lands: vec![4],
        }
    }

    struct Script {
        fail: i64,
        reads: Vec<i64>,
    }

    impl VisitReader for Script {
        async fn read(&mut self, gid: i64) -> std::result::Result<FriendVisit, String> {
            self.reads.push(gid);
            if gid == self.fail {
                Err("进入失败".to_owned())
            } else {
                Ok(ready_visit())
            }
        }
    }

    fn immediate() -> ScanPace {
        ScanPace { wait: Duration::ZERO, poll: Duration::ZERO, gap: Duration::ZERO }
    }

    #[tokio::test]
    async fn scan_continues_after_one_friend_fails_and_skips_a_fresh_cache() {
        let mut cache = HashMap::new();
        let mut script = Script { fail: 9, reads: Vec::new() };
        let first = scan_friends(&[8, 9, 10], &mut cache, immediate(), || false, &mut script).await;
        assert_eq!(script.reads, vec![8, 9, 10]);
        let rows = first.get("friends").and_then(Value::as_array).expect("rows");
        assert_eq!(rows[0].get("state").and_then(Value::as_str), Some("ready"));
        assert_eq!(rows[0].get("error").and_then(Value::as_str), Some(""));
        assert_eq!(rows[1].get("error").and_then(Value::as_str), Some("进入失败"));
        assert_eq!(rows[1].get("state").and_then(Value::as_str), Some("unknown"));
        assert_eq!(rows[2].get("cloudLands").and_then(Value::as_array).map(Vec::len), Some(1));
        script.reads.clear();
        let second =
            scan_friends(&[8, 9, 10], &mut cache, immediate(), || false, &mut script).await;
        assert!(script.reads.is_empty());
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn stale_failure_keeps_the_previous_visit_and_a_busy_patrol_defers_the_rest() {
        let now = crate::utils::time::get_server_time_secs();
        let mut cache = HashMap::from([(
            8,
            CachedVisit {
                inspected_at: now - SCAN_CACHE_TTL_SEC - 1,
                visit: Some(ready_visit()),
                error: None,
            },
        )]);
        let mut script = Script { fail: 8, reads: Vec::new() };
        let failed = scan_friends(&[8], &mut cache, immediate(), || false, &mut script).await;
        let row = &failed.get("friends").and_then(Value::as_array).expect("rows")[0];
        assert_eq!(row.get("error").and_then(Value::as_str), Some("进入失败"));
        assert_eq!(row.get("state").and_then(Value::as_str), Some("ready"));
        assert_eq!(cache.get(&8).expect("kept").inspected_at, now - SCAN_CACHE_TTL_SEC - 1);
        assert!(cache.get(&8).expect("kept").error.is_none());

        let mut cache = HashMap::new();
        let mut script = Script { fail: 0, reads: Vec::new() };
        let mut allow = 1_i64;
        let deferred = scan_friends(
            &[8, 9],
            &mut cache,
            immediate(),
            || {
                if allow > 0 {
                    allow -= 1;
                    false
                } else {
                    true
                }
            },
            &mut script,
        )
        .await;
        assert_eq!(script.reads, vec![8]);
        let deferred_ids =
            deferred.get("deferredGids").and_then(Value::as_array).expect("deferred");
        assert_eq!(deferred_ids.len(), 1);
        assert_eq!(deferred_ids[0].as_i64(), Some(9));
        assert!(cache.contains_key(&8));
        assert!(!cache.contains_key(&9));

        let mut blocked_cache = HashMap::new();
        let mut blocked = Script { fail: 0, reads: Vec::new() };
        let waiting =
            scan_friends(&[1, 2], &mut blocked_cache, immediate(), || true, &mut blocked).await;
        assert!(blocked.reads.is_empty());
        assert!(waiting.get("friends").and_then(Value::as_array).expect("none").is_empty());
        assert_eq!(waiting.get("deferredGids").and_then(Value::as_array).expect("rest").len(), 2);
    }
}
