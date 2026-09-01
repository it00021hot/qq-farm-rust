//! 天气活动「雨落成诗」服务。
//!
//! 对应 bot `core/src/services/weather-activity.ts`（2026-08-26 活动，
//! 2026-08-28 终态）。核心语义：
//!
//! - 活动组 `2026070300`；子活动：`01` 兑换商店、`02` 闪电变异、`03` 采雨、
//!   `04` 气象研究、`05` 气象任务。
//! - 采雨 = `ActivityService.Operate(activity=2026070303, type=9, field107.host_gid)`，
//!   **不走 ItemService.Use**；重复采集服务端回 `1034040`，转业务错幂等提示。
//! - 好友现场天气以 `VisitService.Enter` 回包 `weather`（field 13）为权威来源；
//!   好友列表摘要里的 weather 不可靠。
//! - `field_9 == 4` 表示本轮雷雨已采（field_8 不是采集标记）；作用于当前这轮，
//!   下一轮复位，不建本地每日记录。
//! - 快照构建严格串行（GetGroup → GetBag → GetWeatherStatus）：网关对活动
//!   读取的并发敏感。
//! - 好友扫描批上限 5、批内间隔 300ms、缓存 TTL 600s；扫描让位好友巡查，
//!   等不到空闲把剩余好友放入 `deferredGids` 回传。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use prost::Message as _;
use tokio::sync::Mutex as AsyncMutex;

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::activitypb::{
    ActivityData, ActivityOperateReply, AdvanceWeatherResearchRequest, CollectWeatherRequest,
    ExchangeShopOperateParams, ExchangeShopRequest, GetGroupReply, GetGroupRequest,
    WeatherCollectOperateParams, WeatherResearchOperateParams,
};
use crate::proto::generated::corepb;
use crate::proto::generated::gamepb::itempb::{UseReply, UseRequest, UseTarget};
use crate::proto::generated::gamepb::plantpb::LandInfo;
use crate::proto::generated::gamepb::weatherpb::{
    GetWeatherStatusReply, GetWeatherStatusRequest, WeatherStatus,
};
use crate::services::friend::api::FriendApi;
use crate::services::warehouse::WarehouseService;

const ACTIVITY_SERVICE: &str = "gamepb.activitypb.ActivityService";
const WEATHER_SERVICE: &str = "gamepb.weatherpb.WeatherService";
const ITEM_SERVICE: &str = "gamepb.itempb.ItemService";

/// 活动组 ID（2026-08-26 ~ 2026-09-08）
pub const WEATHER_GROUP_ID: i64 = 2_026_070_300;
/// 兑换商店 + 活动说明 + 基础变异概率
pub const WEATHER_SHOP_ACTIVITY_ID: i64 = 2_026_070_301;
/// 闪电变异（售价 ×4，排除 1/2 品作物）
pub const WEATHER_MUTATION_ACTIVITY_ID: i64 = 2_026_070_302;
/// 采雨操作
pub const WEATHER_BOTTLE_ACTIVITY_ID: i64 = 2_026_070_303;
/// 气象研究
pub const WEATHER_RESEARCH_ACTIVITY_ID: i64 = 2_026_070_304;
/// 气象任务
pub const WEATHER_TASK_ACTIVITY_ID: i64 = 2_026_070_305;

/// 雷雨 weather_type
const THUNDERSTORM_TYPE: i64 = 1;
/// 闪电变异 MutantEffect ID
pub const LIGHTNING_MUTANT_CONFIG_ID: i64 = 12;
/// 采集瓶 / 雷雨召唤瓶 / 青蛙使坏瓶 / 乌云使坏瓶
pub const COLLECTOR_BOTTLE_ID: i64 = 5001;
pub const SUMMON_BOTTLE_ID: i64 = 5002;
pub const FROG_MISCHIEF_BOTTLE_ID: i64 = 5005;
pub const CLOUD_MISCHIEF_BOTTLE_ID: i64 = 5006;
/// 雷电徽章（气象研究消耗）
pub const LIGHTNING_BADGE_ID: i64 = 1027;
/// 天气相关物品展示集
const WEATHER_ITEM_IDS: &[i64] = &[4002, 4003, 5001, 5002, 5003, 5004, 5005, 5006, 5007, 5008];

/// 采雨 Operate 类型
const COLLECT_WEATHER_OPERATE_TYPE: i64 = 9;
/// 推进气象研究 Operate 类型
const ADVANCE_RESEARCH_OPERATE_TYPE: i64 = 40;
/// 兑换商店 Operate 类型
const EXCHANGE_SHOP_OPERATE_TYPE: i64 = 1;
/// 「本轮雷雨已采」标记：WeatherStatus.field_9 == 4（field_8 不是采集标记）
const COLLECTED_THIS_CYCLE_MARKER: i64 = 4;
/// 服务端「已经采过雨了」错误码，转业务错幂等提示
const WEATHER_ALREADY_COLLECTED_CODE: i64 = 1_034_040;

/// 好友天气缓存 TTL（秒）
const FRIEND_WEATHER_CACHE_TTL_SEC: i64 = 600;
/// 好友天气扫描单批上限
pub const FRIEND_WEATHER_SCAN_BATCH_LIMIT: usize = 5;
/// 扫描批内两次进农场间隔
const FRIEND_WEATHER_SCAN_GAP_MS: u64 = 300;
/// 扫描让位好友巡查的最长等待
const FRIEND_TASK_WAIT_MAX_MS: u64 = 10_000;
/// 让位轮询间隔
const FRIEND_TASK_POLL_MS: u64 = 250;
/// 账号全局每日采雨上限（协议不回已用次数，仅作面板提示）
pub const COLLECT_DAILY_LIMIT: i64 = 10;
/// 使坏（青蛙/乌云）每日上限（协议回包日限）
pub const MISCHIEF_DAILY_LIMIT: i64 = 100;

fn business_error(code: &str, message: &str) -> Error {
    Error::Business(format!("{code}：{message}"))
}

/// 从业务错误信息里抽出错误码（面板按码映射提示）。
#[must_use]
pub fn weather_error_code(error: &Error) -> Option<String> {
    let msg = error.to_string();
    msg.split('：').next().and_then(|code| {
        (code.starts_with("WEATHER_")
            || code.starts_with("INSUFFICIENT_")
            || code.starts_with("INVALID_"))
        .then(|| code.to_string())
    })
}

// =====================================================================
// 纯函数 DTO（可单测）
// =====================================================================

/// 天气状态展示（对齐 bot `weatherStatusDto`）
#[must_use]
pub fn weather_status_dto(weather: Option<&WeatherStatus>, host_gid: i64) -> serde_json::Value {
    let w = weather.copied().unwrap_or_default();
    let now = crate::utils::time::get_server_time_secs();
    let end = w.end_time;
    let active = w.weather_type > 0 && w.status > 0 && (end == 0 || end > now);
    let is_thunderstorm = active && w.weather_type == THUNDERSTORM_TYPE;
    serde_json::json!({
        "hostGid": host_gid.to_string(),
        "type": w.weather_type,
        "status": w.status,
        "beginTime": w.begin_time,
        "endTime": w.end_time,
        "source": w.source,
        "field8": w.field_8,
        "friendMarker": w.field_9,
        "active": active,
        "isThunderstorm": is_thunderstorm,
        "collectedThisCycle": w.field_9 == COLLECTED_THIS_CYCLE_MARKER,
        "remainingSec": if active && end > 0 { (end - now).max(0) } else { 0 },
    })
}

/// 是否雷雨进行中（纯判定）
#[must_use]
pub fn weather_is_thunderstorm(weather: Option<&WeatherStatus>) -> bool {
    let w = weather.copied().unwrap_or_default();
    let now = crate::utils::time::get_server_time_secs();
    w.weather_type > 0
        && w.status > 0
        && (w.end_time == 0 || w.end_time > now)
        && w.weather_type == THUNDERSTORM_TYPE
}

fn activity_is_active(begin_time: i64, end_time: i64) -> bool {
    let server = crate::utils::time::get_server_time_secs();
    (begin_time == 0 || server >= begin_time) && (end_time == 0 || server <= end_time)
}

fn plain_text(value: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ").replace("&amp;", "&").trim().to_string()
}

/// 活动说明：extra(tips.txt JSON) 抽段落（对齐 bot `activityRules`）
#[must_use]
pub fn activity_rules(extra: &[u8]) -> serde_json::Value {
    let source = String::from_utf8_lossy(extra).trim().to_string();
    if source.is_empty() {
        return serde_json::json!({ "title": "活动说明", "paragraphs": [] });
    }
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&source) {
        let tips = parsed.get("tips");
        let title = tips
            .and_then(|t| t.get("title"))
            .and_then(|v| v.as_str())
            .map(plain_text)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "活动说明".to_string());
        let paragraphs: Vec<String> = tips
            .and_then(|t| t.get("txt"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| plain_text(&s.replace("<br/>", "\n").replace("<br>", "\n")))
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        return serde_json::json!({ "title": title, "paragraphs": paragraphs });
    }
    let text = plain_text(&source);
    serde_json::json!({
        "title": "活动说明",
        "paragraphs": if text.is_empty() { vec![] } else { vec![text] },
    })
}

/// 物品 DTO（对齐 bot `itemDto`：本地 ItemInfo 查名图）
fn item_dto(item_id: i64, count: i64) -> serde_json::Value {
    let gc = crate::config::game_config::global();
    let meta = gc.get_item_by_id(item_id);
    let name = meta
        .as_ref()
        .map(|i| i.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            if item_id == LIGHTNING_BADGE_ID {
                "雷电徽章".to_string()
            } else {
                format!("物品 {item_id}")
            }
        });
    serde_json::json!({
        "id": item_id.to_string(),
        "count": count.to_string(),
        "name": name,
        "image": gc.get_item_image_by_id(item_id).unwrap_or_default(),
        "rarity": meta.as_ref().and_then(|i| i.rarity).unwrap_or(0),
    })
}

/// 乌云可用地块：已解锁、非占用从属块（master 有植物）、有作物、生长中
/// （>种子 <成熟）、该地无 5006 互动记录（对齐 bot `cloudEligibleLandIds`）
#[must_use]
pub fn cloud_eligible_land_ids(lands: &[LandInfo]) -> Vec<i64> {
    use crate::services::farm::land_analysis::{build_land_map, PlantPhase};
    let land_map = build_land_map(lands);
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for land in lands {
        if !land.unlocked {
            continue;
        }
        if land.master_land_id > 0 && land.master_land_id != land.id {
            if let Some(master) = land_map.get(&land.master_land_id) {
                if master.plant.is_some() {
                    continue;
                }
            }
        }
        let Some(plant) = land.plant.as_ref() else { continue };
        if plant.id <= 0 || plant.phases.is_empty() {
            continue;
        }
        // > SEED(1) 且 < MATURE(6)：发芽/生长中
        match PlantPhase::from_phases(&plant.phases) {
            Some(PlantPhase::Sprout) | Some(PlantPhase::Growing) => {}
            _ => continue,
        }
        let has_cloud = plant.interaction_uses.iter().any(|e| e.item_id == CLOUD_MISCHIEF_BOTTLE_ID)
            || plant
                .interaction_targets
                .iter()
                .any(|e| e.item_id == CLOUD_MISCHIEF_BOTTLE_ID);
        if has_cloud {
            continue;
        }
        if seen.insert(land.id) {
            result.push(land.id);
        }
    }
    result
}

/// 好友现场检查结果
#[derive(Clone)]
struct FriendInspection {
    weather: Option<WeatherStatus>,
    lands: Vec<LandInfo>,
    inspected_at: i64,
    error: String,
}

impl FriendInspection {
    fn empty() -> Self {
        Self { weather: None, lands: Vec::new(), inspected_at: 0, error: String::new() }
    }
}

struct CachedFriendWeather {
    inspection: FriendInspection,
    cached_at: i64,
}

// =====================================================================
// 服务
// =====================================================================

/// 天气活动服务（每账号一个，由 WorkerLoop 持有）
pub struct WeatherActivityService {
    gateway: Arc<Gateway>,
    friend_api: FriendApi,
    warehouse: WarehouseService,
    account_id: Mutex<String>,
    /// 自己的角色 GID（登录成功后由 WorkerLoop 写入；0 = 未登录）
    own_gid: Mutex<i64>,
    /// 写操作串行（对齐 bot serializeMutation 链）
    mutation_lock: Arc<AsyncMutex<()>>,
    /// 快照单飞（持锁期间并发调用等待同一轮构建）
    snapshot_lock: Arc<AsyncMutex<()>>,
    friend_weather_cache: Mutex<HashMap<i64, CachedFriendWeather>>,
}

impl WeatherActivityService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            friend_api: FriendApi::new(Arc::clone(&gateway)),
            warehouse: WarehouseService::new(Arc::clone(&gateway)),
            gateway,
            account_id: Mutex::new(String::new()),
            own_gid: Mutex::new(0),
            mutation_lock: Arc::new(AsyncMutex::new(())),
            snapshot_lock: Arc::new(AsyncMutex::new(())),
            friend_weather_cache: Mutex::new(HashMap::new()),
        }
    }

    /// 绑定账号（worker 启动时）；切换账号清缓存
    pub fn set_account_id(&self, account_id: &str) {
        *self.account_id.lock() = account_id.to_string();
        self.clear_caches();
    }

    /// 写入自己的角色 GID（登录成功后由 WorkerLoop 调用）
    pub fn set_own_gid(&self, gid: i64) {
        *self.own_gid.lock() = gid;
    }

    fn own_gid(&self) -> i64 {
        *self.own_gid.lock()
    }

    /// 天气变化 / 活动变化 / 断线：清好友天气缓存
    pub fn clear_caches(&self) {
        self.friend_weather_cache.lock().clear();
    }

    fn account(&self) -> String {
        self.account_id.lock().clone()
    }

    // ----- 底层 RPC -----

    async fn query_weather_group(&self) -> Result<GetGroupReply> {
        let req = GetGroupRequest { group_id: WEATHER_GROUP_ID };
        let body =
            self.gateway.request(ACTIVITY_SERVICE, "GetGroup", &req.encode_to_vec()).await?;
        Ok(GetGroupReply::decode(&body[..])?)
    }

    async fn get_weather_status(&self) -> Result<GetWeatherStatusReply> {
        let req = GetWeatherStatusRequest {};
        let body =
            self.gateway.request(WEATHER_SERVICE, "GetWeatherStatus", &req.encode_to_vec()).await?;
        Ok(GetWeatherStatusReply::decode(&body[..])?)
    }

    async fn bag_balances(&self) -> Result<HashMap<i64, i64>> {
        let bag = self.warehouse.get_bag().await?;
        let mut balances: HashMap<i64, i64> = HashMap::new();
        if let Some(item_bag) = bag.item_bag.as_ref() {
            for item in &item_bag.items {
                if item.id > 0 {
                    *balances.entry(item.id).or_insert(0) += item.count;
                }
            }
        }
        Ok(balances)
    }

    /// 背包里最早过期的一叠指定物品（对齐 bot `availableStack`）
    async fn available_stack(&self, item_id: i64) -> Result<Option<(i64, i64)>> {
        let bag = self.warehouse.get_bag().await?;
        let mut stacks: Vec<(i64, i64, i64)> = bag
            .item_bag
            .iter()
            .flat_map(|b| b.items.iter())
            .filter(|i| i.id == item_id && i.count > 0)
            .map(|i| (i.uid, i.count, if i.expire_time > 0 { i.expire_time } else { i64::MAX }))
            .collect();
        stacks.sort_by_key(|(_, _, expire)| *expire);
        Ok(stacks.first().map(|(uid, count, _)| (*uid, *count)))
    }

    async fn send_bottle_use(
        &self,
        item_id: i64,
        uid: i64,
        target: Option<UseTarget>,
    ) -> Result<UseReply> {
        let req = UseRequest {
            item: Some(corepb::Item { id: item_id, count: 1, uid, ..Default::default() }),
            target,
        };
        let body = self.gateway.request(ITEM_SERVICE, "Use", &req.encode_to_vec()).await?;
        Ok(UseReply::decode(&body[..])?)
    }

    fn cache_inspection(&self, gid: i64, inspection: FriendInspection) {
        let now = crate::utils::time::get_server_time_secs();
        self.friend_weather_cache
            .lock()
            .insert(gid, CachedFriendWeather { inspection, cached_at: now });
    }

    // ----- 快照 -----

    /// 当前活动快照（单飞：并发调用共享同一轮构建）
    pub async fn get_current_activity(&self) -> Result<serde_json::Value> {
        let _lock = self.snapshot_lock.lock().await;
        self.build_snapshot().await
    }

    async fn build_snapshot(&self) -> Result<serde_json::Value> {
        // 网关对活动读取并发敏感，严格串行：GetGroup → GetBag → GetWeatherStatus
        let group_reply = self.query_weather_group().await?;
        let balances = self.bag_balances().await?;
        let own_weather_reply = self.get_weather_status().await?;
        let Some(group) = group_reply.group.as_ref() else {
            return Err(business_error("WEATHER_ACTIVITY_UNAVAILABLE", "服务端未发现雨落成诗活动"));
        };
        let activity = group.activity.clone().unwrap_or_default();
        if activity.activity_id != WEATHER_GROUP_ID {
            return Err(business_error("WEATHER_ACTIVITY_UNAVAILABLE", "服务端未发现雨落成诗活动"));
        }
        let active = activity_is_active(activity.begin_time, activity.end_time);
        let balance = |id: i64| balances.get(&id).copied().unwrap_or(0);

        let find_child = |id: i64| -> Option<&ActivityData> {
            group
                .children
                .iter()
                .find(|c| c.activity.as_ref().map(|a| a.activity_id) == Some(id))
        };
        let shop_child = find_child(WEATHER_SHOP_ACTIVITY_ID);
        let mutation_child = find_child(WEATHER_MUTATION_ACTIVITY_ID);
        let bottle_child = find_child(WEATHER_BOTTLE_ACTIVITY_ID);
        let research_child = find_child(WEATHER_RESEARCH_ACTIVITY_ID);
        let task_child = find_child(WEATHER_TASK_ACTIVITY_ID);

        let own_gid = self.own_gid();
        let own_weather = weather_status_dto(own_weather_reply.weather.as_ref(), own_gid);
        let own_active = own_weather.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
        let own_thunder = own_weather.get("isThunderstorm").and_then(|v| v.as_bool()).unwrap_or(false);

        // 兑换商店：goods 200（金豆豆兑换采集瓶），dailyLimit 1
        let shop = (|| -> Option<serde_json::Value> {
            let catalog = shop_child?.catalog.as_ref()?;
            let goods = catalog
                .goods
                .iter()
                .find(|g| g.goods_id == 200)
                .or_else(|| catalog.goods.first())?
                .clone();
            let item = goods.item.unwrap_or_default();
            let cost = goods.cost.unwrap_or_default();
            let cost_balance = balance(cost.item_id);
            let available =
                active && !goods.owned && goods.status != 0 && cost_balance >= cost.count.max(0);
            let reason = if !active {
                "活动尚未开放或已经结束".to_string()
            } else if goods.owned {
                "今日已经兑换过天气采集瓶".to_string()
            } else if cost_balance < cost.count.max(0) {
                "金豆豆不足".to_string()
            } else {
                String::new()
            };
            Some(serde_json::json!({
                "activityId": WEATHER_SHOP_ACTIVITY_ID,
                "goodsId": goods.goods_id,
                "item": item_dto(item.item_id, item.count),
                "cost": item_dto(cost.item_id, cost.count),
                "balance": cost_balance.to_string(),
                "owned": goods.owned,
                "statusCode": goods.status,
                "dailyLimit": 1,
                "available": available,
                "reason": reason,
            }))
        })();

        // 气象研究
        let badge_balance = balance(LIGHTNING_BADGE_ID);
        let research = (|| -> Option<serde_json::Value> {
            let track = research_child?.weather_research.as_ref()?.track.as_ref()?;
            let nodes: Vec<serde_json::Value> = track
                .nodes
                .iter()
                .map(|node| {
                    let cost = node.cost.unwrap_or_default();
                    let reward = node.reward.unwrap_or_default();
                    let status_code = node.status;
                    let available_by_status = status_code == 2;
                    let completed = status_code == 4 || node.claimed;
                    let affordable =
                        cost.item_id == LIGHTNING_BADGE_ID && badge_balance >= cost.count.max(0);
                    serde_json::json!({
                        "id": node.node_id.to_string(),
                        "prerequisiteNodeIds": node.prerequisite_node_ids.iter()
                            .map(|id| id.to_string()).collect::<Vec<_>>(),
                        "statusCode": status_code,
                        "cost": item_dto(cost.item_id, cost.count),
                        "reward": item_dto(reward.item_id, reward.count),
                        "field5": node.field_5,
                        "field8": node.field_8,
                        "field9": node.field_9,
                        "availableByStatus": available_by_status,
                        "completed": completed,
                        "locked": !completed && !available_by_status,
                        "affordable": affordable,
                    })
                })
                .collect();
            let next_node = nodes
                .iter()
                .find(|n| n.get("availableByStatus").and_then(|v| v.as_bool()).unwrap_or(false))
                .cloned();
            Some(serde_json::json!({
                "activityId": WEATHER_RESEARCH_ACTIVITY_ID,
                "currentStage": track.current_stage,
                "badgeBalance": badge_balance.to_string(),
                "nodes": nodes,
                "nextNode": next_node.unwrap_or(serde_json::Value::Null),
                "operateSupported": true,
                "operateReason": "",
            }))
        })();
        let next_research_node = research
            .as_ref()
            .and_then(|r| r.get("nextNode"))
            .filter(|n| !n.is_null())
            .cloned();

        // 采集瓶配置 / 任务列表
        let collector = bottle_child.and_then(|child| {
            let config = child.weather_bottle.as_ref()?;
            Some(serde_json::json!({
                "activityId": WEATHER_BOTTLE_ACTIVITY_ID,
                "collectorItemId": config.collector_item_id,
                "collectorItemCount": config.collector_item_count,
                "field3": config.field_3,
                "field4": config.field_4,
                "field9": config.field_9,
                "rewards": config.rewards.iter().map(|reward| {
                    let item = reward.reward.unwrap_or_default();
                    serde_json::json!({
                        "id": reward.reward_id.to_string(),
                        "reward": item_dto(item.item_id, item.count),
                        "statusCode": reward.status,
                        "probability": reward.probability,
                    })
                }).collect::<Vec<_>>(),
            }))
        });
        let tasks: Vec<serde_json::Value> = task_child
            .map(|child| {
                child
                    .weather_tasks
                    .as_ref()
                    .map(|t| &t.tasks)
                    .map(|tasks| {
                        tasks
                            .iter()
                            .map(|task| {
                                let reward = task.reward.unwrap_or_default();
                                serde_json::json!({
                                    "id": task.task_id.to_string(),
                                    "triggerItemId": task.trigger_item_id.to_string(),
                                    "title": task.title,
                                    "reward": item_dto(reward.item_id, reward.count),
                                    "dailyLimit": task.daily_limit,
                                    "current": task.current,
                                    "progressKnown": true,
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        let inventory: Vec<serde_json::Value> = WEATHER_ITEM_IDS
            .iter()
            .chain(std::iter::once(&LIGHTNING_BADGE_ID))
            .map(|id| item_dto(*id, balance(*id)))
            .collect();

        // actions 可用性
        let summon_balance = balance(SUMMON_BOTTLE_ID);
        let next_affordable = next_research_node
            .as_ref()
            .and_then(|n| n.get("affordable"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let next_available = next_research_node
            .as_ref()
            .and_then(|n| n.get("availableByStatus"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let advance_reason = if !active {
            "活动尚未开放或已经结束".to_string()
        } else if next_research_node.is_none() {
            "气象研究已经全部完成".to_string()
        } else if !next_affordable {
            "雷电徽章不足".to_string()
        } else {
            String::new()
        };
        let summon_reason = if !active {
            "活动尚未开放或已经结束".to_string()
        } else if own_active {
            if own_thunder { "雷雨正在进行中".to_string() } else { "当前已有其他特殊天气".to_string() }
        } else if summon_balance <= 0 {
            "背包中没有可用的雷雨召唤瓶".to_string()
        } else {
            String::new()
        };

        Ok(serde_json::json!({
            "groupId": WEATHER_GROUP_ID,
            "activity": {
                "id": activity.activity_id.to_string(),
                "groupId": activity.group_id.to_string(),
                "typeCode": activity.r#type,
                "name": activity.name,
                "startTime": activity.begin_time,
                "endTime": activity.end_time,
            },
            "rules": activity_rules(&activity.extra),
            "active": active,
            "serverTime": crate::utils::time::get_server_time_secs(),
            "mutation": {
                "activityId": WEATHER_MUTATION_ACTIVITY_ID,
                "active": mutation_child
                    .map(|c| {
                        c.activity
                            .as_ref()
                            .map(|a| activity_is_active(a.begin_time, a.end_time))
                            .unwrap_or(false)
                    })
                    .unwrap_or(false),
                "mutantConfigId": LIGHTNING_MUTANT_CONFIG_ID,
                "baseRatePercent": shop_child
                    .and_then(|c| c.activity.as_ref())
                    .map(|a| a.field_21)
                    .unwrap_or(0),
                "sellMultiplier": 4,
                "excludedCropQualities": [1, 2],
            },
            "ownWeather": own_weather,
            "shop": shop,
            "collector": collector,
            "tasks": tasks,
            "research": research,
            "inventory": inventory,
            "actions": {
                "exchangeCollector": {
                    "enabled": shop_child.is_some()
                        && shop.as_ref().and_then(|s| s.get("available"))
                            .and_then(|v| v.as_bool()).unwrap_or(false),
                },
                "collectWeather": {
                    "enabled": active && balance(COLLECTOR_BOTTLE_ID) > 0,
                    "dailyLimit": COLLECT_DAILY_LIMIT,
                },
                "scanFriendWeather": {
                    "enabled": active,
                    "batchSize": FRIEND_WEATHER_SCAN_BATCH_LIMIT,
                    "reason": if active { "" } else { "活动尚未开放或已经结束" },
                },
                "frogMischief": {
                    "enabled": active && balance(FROG_MISCHIEF_BOTTLE_ID) > 0,
                    "dailyLimit": MISCHIEF_DAILY_LIMIT,
                },
                "cloudMischief": {
                    "enabled": active && balance(CLOUD_MISCHIEF_BOTTLE_ID) > 0,
                    "dailyLimit": MISCHIEF_DAILY_LIMIT,
                },
                "summonThunderstorm": {
                    "enabled": active && summon_balance > 0 && !own_active,
                    "reason": summon_reason,
                },
                "advanceResearch": {
                    "enabled": active && next_available && next_affordable,
                    "nodeId": next_research_node.as_ref()
                        .and_then(|n| n.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    "reason": advance_reason,
                },
            },
        }))
    }

    // ----- 好友现场天气 -----

    /// 好友基础名单（不进任何农场；现场天气由面板点击好友时按需扫描）
    pub async fn get_weather_friends(&self) -> Result<Vec<serde_json::Value>> {
        let friends = self.friend_api.get_all_game_friends().await?;
        let own_gid = self.own_gid();
        Ok(friends
            .into_iter()
            .filter(|f| f.gid > 0 && f.gid != own_gid)
            .map(|f| {
                serde_json::json!({
                    "gid": f.gid.to_string(),
                    "name": if f.remark.is_empty() { f.name.clone() } else { f.remark.clone() },
                    "avatarUrl": f.avatar_url,
                    "level": f.level,
                })
            })
            .collect())
    }

    fn fresh_friend_weather(&self, gid: i64) -> Option<FriendInspection> {
        let cache = self.friend_weather_cache.lock();
        let cached = cache.get(&gid)?;
        let now = crate::utils::time::get_server_time_secs();
        if now - cached.cached_at <= FRIEND_WEATHER_CACHE_TTL_SEC {
            Some(cached.inspection.clone())
        } else {
            None
        }
    }

    async fn wait_for_friend_task_idle(&self) -> bool {
        let account = self.account();
        if !crate::infra::friend_task_flag::is_friend_checking(&account) {
            return true;
        }
        let deadline = crate::utils::time::now_ms() + FRIEND_TASK_WAIT_MAX_MS as i64;
        while crate::utils::time::now_ms() < deadline {
            tokio::time::sleep(Duration::from_millis(FRIEND_TASK_POLL_MS)).await;
            if !crate::infra::friend_task_flag::is_friend_checking(&account) {
                return true;
            }
        }
        false
    }

    /// 进入好友农场读取现场天气（失败保留缓存值并带 error）
    async fn inspect_friend_farm_weather(&self, gid: i64) -> FriendInspection {
        let cached_weather = {
            let cache = self.friend_weather_cache.lock();
            cache.get(&gid).and_then(|c| c.inspection.weather)
        };
        let mut entered = false;
        let result: std::result::Result<(Option<WeatherStatus>, Vec<LandInfo>), String> = async {
            let reply = self.friend_api.enter_farm(gid).await.map_err(|e| e.to_string())?;
            entered = true;
            Ok((reply.weather.clone(), reply.lands.clone()))
        }
        .await;
        if entered {
            let _ = self.friend_api.leave_farm(gid).await;
        }
        let inspection = match result {
            Ok((weather, lands)) => FriendInspection {
                weather,
                lands,
                inspected_at: crate::utils::time::get_server_time_secs(),
                error: String::new(),
            },
            Err(err) => {
                tracing::warn!(gid, error = %err, "好友现场天气检查失败");
                FriendInspection {
                    weather: cached_weather,
                    lands: Vec::new(),
                    inspected_at: crate::utils::time::get_server_time_secs(),
                    error: format!("现场天气检查失败: {err}"),
                }
            }
        };
        self.cache_inspection(gid, inspection.clone());
        inspection
    }

    fn friend_weather_dto(
        &self,
        meta: &serde_json::Value,
        inspection: &FriendInspection,
    ) -> serde_json::Value {
        let gid: i64 = meta
            .get("gid")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .or_else(|| meta.get("gid").and_then(|v| v.as_i64()))
            .unwrap_or(0);
        let scan_error = inspection.error.clone();
        let inspected = inspection.weather.is_some() && scan_error.is_empty();
        let weather = weather_status_dto(inspection.weather.as_ref(), gid);
        let is_thunder = weather.get("isThunderstorm").and_then(|v| v.as_bool()).unwrap_or(false);
        let collected =
            weather.get("collectedThisCycle").and_then(|v| v.as_bool()).unwrap_or(false);
        let w_type = weather.get("type").and_then(|v| v.as_i64()).unwrap_or(0);
        let w_active = weather.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
        let (availability, availability_reason) = if !inspected {
            ("unknown".to_string(), "尚未进入好友农场检查现场天气".to_string())
        } else if is_thunder && collected {
            ("collected".to_string(), "当前这轮雷雨已经采过，下轮雷雨可再次采集".to_string())
        } else if is_thunder {
            ("available".to_string(), String::new())
        } else if w_type == THUNDERSTORM_TYPE && !w_active {
            ("expired".to_string(), "这场雷雨已经结束".to_string())
        } else {
            ("unavailable".to_string(), "好友农场当前不是雷雨天气".to_string())
        };
        let eligible_cloud: Vec<String> = if scan_error.is_empty() {
            cloud_eligible_land_ids(&inspection.lands)
                .iter()
                .map(|id| id.to_string())
                .collect()
        } else {
            Vec::new()
        };
        serde_json::json!({
            "gid": gid.to_string(),
            "name": meta.get("name").and_then(|v| v.as_str()).unwrap_or(""),
            "avatarUrl": meta.get("avatarUrl").and_then(|v| v.as_str()).unwrap_or(""),
            "level": meta.get("level").and_then(|v| v.as_i64()).unwrap_or(0),
            "inspected": inspected,
            "inspectedAt": inspection.inspected_at,
            "scanError": scan_error,
            "availability": availability,
            "availabilityReason": availability_reason,
            "canCollect": inspection.error.is_empty() && availability == "available",
            "eligibleCloudLandIds": eligible_cloud,
            "weather": weather,
        })
    }

    /// 扫描好友现场天气（批上限 5；让位好友巡查，等不到把剩余放入 deferredGids）
    pub async fn scan_weather_friends(&self, friend_gids: &[i64]) -> Result<serde_json::Value> {
        let own_gid = self.own_gid();
        let mut gids: Vec<i64> = Vec::new();
        for gid in friend_gids {
            if *gid > 0 && *gid != own_gid && !gids.contains(gid) {
                gids.push(*gid);
            }
        }
        if gids.is_empty() {
            return Err(business_error("INVALID_WEATHER_FRIEND_GID", "请先选择需要检查现场天气的好友"));
        }
        if gids.len() > FRIEND_WEATHER_SCAN_BATCH_LIMIT {
            return Err(business_error(
                "WEATHER_SCAN_BATCH_TOO_LARGE",
                &format!("单次最多检查 {FRIEND_WEATHER_SCAN_BATCH_LIMIT} 位好友，请分批发起"),
            ));
        }
        let _mutation = self.mutation_lock.lock().await;
        let meta_by_gid: HashMap<i64, serde_json::Value> = self
            .get_weather_friends()
            .await?
            .into_iter()
            .filter_map(|f| {
                f.get("gid")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<i64>().ok())
                    .map(|g| (g, f))
            })
            .collect();
        let empty_meta = |gid: i64| serde_json::json!({ "gid": gid.to_string() });
        let mut friends = Vec::new();
        let mut deferred_gids: Vec<String> = Vec::new();
        let mut visited = 0usize;
        for (index, gid) in gids.iter().enumerate() {
            if let Some(fresh) = self.fresh_friend_weather(*gid) {
                let meta = meta_by_gid.get(gid).cloned().unwrap_or_else(|| empty_meta(*gid));
                friends.push(self.friend_weather_dto(&meta, &fresh));
                continue;
            }
            // 好友任务同样要进出好友农场，先让路；等不到空闲就交回前端稍后重试
            if !self.wait_for_friend_task_idle().await {
                deferred_gids.extend(gids[index..].iter().map(|g| g.to_string()));
                break;
            }
            if visited > 0 {
                tokio::time::sleep(Duration::from_millis(FRIEND_WEATHER_SCAN_GAP_MS)).await;
            }
            visited += 1;
            let inspection = self.inspect_friend_farm_weather(*gid).await;
            let meta = meta_by_gid.get(gid).cloned().unwrap_or_else(|| empty_meta(*gid));
            friends.push(self.friend_weather_dto(&meta, &inspection));
        }
        Ok(serde_json::json!({
            "outcome": "scanned",
            "serverTime": crate::utils::time::get_server_time_secs(),
            "friends": friends,
            "deferredGids": deferred_gids,
        }))
    }

    // ----- 写操作 -----

    fn use_reply_dto(&self, reply: &UseReply) -> serde_json::Value {
        let mut rewards: Vec<serde_json::Value> =
            reply.items.iter().map(|i| item_dto(i.id, i.count)).collect();
        if let Some(land_reward) = reply.land_reward.as_ref() {
            rewards.extend(land_reward.items.iter().map(|i| item_dto(i.id, i.count)));
        }
        if let Some(social) = reply.social_reward.as_ref() {
            rewards.extend(social.items.iter().map(|i| item_dto(i.id, i.count)));
        }
        serde_json::json!({
            "usedItems": reply.used_items.iter()
                .map(|i| item_dto(i.id, i.count)).collect::<Vec<_>>(),
            "rewards": rewards,
            "landId": reply.land.as_ref().map(|l| l.id.to_string())
                .or_else(|| reply.land_reward.as_ref().map(|r| r.land_id.to_string()))
                .unwrap_or_default(),
            "socialItemId": reply.social_reward.as_ref().map(|r| r.item_id).unwrap_or(0),
        })
    }

    /// 兑换采集瓶（goods 通常 200，金豆豆兑换，dailyLimit 1）
    pub async fn exchange_collector_bottle(&self) -> Result<serde_json::Value> {
        let _mutation = self.mutation_lock.lock().await;
        let group_reply = self.query_weather_group().await?;
        let balances = self.bag_balances().await?;
        let active = group_reply
            .group
            .as_ref()
            .and_then(|g| g.activity.as_ref())
            .map(|a| activity_is_active(a.begin_time, a.end_time))
            .unwrap_or(false);
        let shop_child = group_reply.group.as_ref().and_then(|g| {
            g.children
                .iter()
                .find(|c| c.activity.as_ref().map(|a| a.activity_id) == Some(WEATHER_SHOP_ACTIVITY_ID))
        });
        let goods_list = shop_child.and_then(|c| c.catalog.as_ref()).map(|cat| cat.goods.clone());
        let Some(goods_list) = goods_list else {
            return Err(business_error("WEATHER_SHOP_UNAVAILABLE", "天气采集瓶商店暂不可用"));
        };
        let goods = goods_list
            .iter()
            .find(|g| g.goods_id == 200)
            .or_else(|| goods_list.first())
            .ok_or_else(|| business_error("WEATHER_SHOP_UNAVAILABLE", "天气采集瓶商店暂不可用"))?;
        let cost = goods.cost.unwrap_or_default();
        let cost_balance = balances.get(&cost.item_id).copied().unwrap_or(0);
        if goods.owned {
            return Err(business_error("WEATHER_SHOP_ALREADY_EXCHANGED", "今日已经兑换过天气采集瓶"));
        }
        let available = active && goods.status != 0 && cost_balance >= cost.count.max(0);
        if !available {
            return Err(business_error("WEATHER_SHOP_UNAVAILABLE", "天气采集瓶当前不可兑换"));
        }
        let req = ExchangeShopRequest {
            activity_id: WEATHER_SHOP_ACTIVITY_ID,
            operate_type: EXCHANGE_SHOP_OPERATE_TYPE,
            exchange_shop_operate: Some(ExchangeShopOperateParams { goods_id: goods.goods_id, count: 1 }),
        };
        let body =
            self.gateway.request(ACTIVITY_SERVICE, "Operate", &req.encode_to_vec()).await?;
        let reply = ActivityOperateReply::decode(&body[..])?;
        Ok(serde_json::json!({
            "outcome": "exchanged",
            "rewards": reply.rewards.iter().map(|i| item_dto(i.id, i.count)).collect::<Vec<_>>(),
            "activityId": reply.activity_id,
            "operateType": reply.operate_type,
            "snapshot": self.build_snapshot().await?,
        }))
    }

    /// 采雨：进好友农场 → Operate(2026070303, type=9, field107) → Leave → 再进场确认
    pub async fn collect_weather(&self, friend_gid: i64) -> Result<serde_json::Value> {
        let _mutation = self.mutation_lock.lock().await;
        let own_gid = self.own_gid();
        if friend_gid <= 0 || friend_gid == own_gid {
            return Err(business_error("INVALID_WEATHER_FRIEND_GID", "天气采集瓶只能在好友农场使用"));
        }
        if self.available_stack(COLLECTOR_BOTTLE_ID).await?.is_none() {
            return Err(business_error("WEATHER_COLLECTOR_UNAVAILABLE", "背包中没有可用的天气采集瓶"));
        }

        let mut entered = false;
        let result: Result<(serde_json::Value, ActivityOperateReply)> = async {
            let enter_reply = self.friend_api.enter_farm(friend_gid).await?;
            entered = true;
            self.cache_inspection(
                friend_gid,
                FriendInspection {
                    weather: enter_reply.weather.clone(),
                    lands: Vec::new(),
                    inspected_at: crate::utils::time::get_server_time_secs(),
                    error: String::new(),
                },
            );
            let weather_before = weather_status_dto(enter_reply.weather.as_ref(), friend_gid);
            let is_thunder =
                weather_before.get("isThunderstorm").and_then(|v| v.as_bool()).unwrap_or(false);
            let collected =
                weather_before.get("collectedThisCycle").and_then(|v| v.as_bool()).unwrap_or(false);
            if !is_thunder {
                return Err(business_error(
                    "WEATHER_FRIEND_NOT_THUNDERSTORM",
                    "该好友农场当前不是雷雨天气",
                ));
            }
            if collected {
                return Err(business_error(
                    "WEATHER_ALREADY_COLLECTED",
                    "当前这轮雷雨已经采过，下轮雷雨可再次采集",
                ));
            }
            let req = CollectWeatherRequest {
                activity_id: WEATHER_BOTTLE_ACTIVITY_ID,
                operate_type: COLLECT_WEATHER_OPERATE_TYPE,
                weather_collect_operate: Some(WeatherCollectOperateParams { host_gid: friend_gid }),
            };
            let body = match self
                .gateway
                .request(ACTIVITY_SERVICE, "Operate", &req.encode_to_vec())
                .await
            {
                Ok(body) => body,
                Err(crate::network::error::NetworkError::Gateway { code, .. })
                    if code == WEATHER_ALREADY_COLLECTED_CODE =>
                {
                    return Err(business_error(
                        "WEATHER_ALREADY_COLLECTED",
                        "当前这轮雷雨已经采过，下轮雷雨可再次采集",
                    ));
                }
                Err(e) => return Err(e.into()),
            };
            Ok((weather_before, ActivityOperateReply::decode(&body[..])?))
        }
        .await;
        if entered {
            let _ = self.friend_api.leave_farm(friend_gid).await;
        }
        let (weather_before, reply) = result?;

        // 采集成功后按官方客户端方式再次进入，记录服务端更新后的现场标记
        let after = self.inspect_friend_farm_weather(friend_gid).await;
        let weather_after = weather_status_dto(after.weather.as_ref(), friend_gid);
        let meta = serde_json::json!({ "gid": friend_gid.to_string() });
        let friend = self.friend_weather_dto(&meta, &after);
        Ok(serde_json::json!({
            "outcome": "collected",
            "friendGid": friend_gid.to_string(),
            "activityId": reply.activity_id,
            "operateType": reply.operate_type,
            "rewards": reply.rewards.iter().map(|i| item_dto(i.id, i.count)).collect::<Vec<_>>(),
            "weatherBefore": weather_before,
            "weatherAfter": weather_after,
            "friend": friend,
            "snapshot": self.build_snapshot().await?,
        }))
    }

    /// 召唤雷雨（自己已有任意特殊天气时禁用）
    pub async fn summon_thunderstorm(&self) -> Result<serde_json::Value> {
        let _mutation = self.mutation_lock.lock().await;
        let own_gid = self.own_gid();
        let before = self.get_weather_status().await?;
        let before_dto = weather_status_dto(before.weather.as_ref(), own_gid);
        let active = before_dto.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
        if active {
            return Err(business_error(
                "WEATHER_ALREADY_ACTIVE",
                "自己的农场当前已有特殊天气，暂时无法召唤雷雨",
            ));
        }
        let Some((uid, _)) = self.available_stack(SUMMON_BOTTLE_ID).await? else {
            return Err(business_error("WEATHER_SUMMON_UNAVAILABLE", "背包中没有可用的雷雨召唤瓶"));
        };
        let reply = self
            .send_bottle_use(
                SUMMON_BOTTLE_ID,
                uid,
                Some(UseTarget { host_gid: own_gid, land_ids: vec![], use_config_id: 0 }),
            )
            .await?;
        let after = self.get_weather_status().await?;
        Ok(serde_json::json!({
            "outcome": "summoned",
            "use": self.use_reply_dto(&reply),
            "weather": weather_status_dto(after.weather.as_ref(), own_gid),
            "snapshot": self.build_snapshot().await?,
        }))
    }

    /// 青蛙使坏（农场级，不指定地块；回包 social_reward 经验日限 100）
    pub async fn frog_mischief(&self, friend_gid: i64) -> Result<serde_json::Value> {
        let _mutation = self.mutation_lock.lock().await;
        let own_gid = self.own_gid();
        if friend_gid <= 0 || friend_gid == own_gid {
            return Err(business_error("INVALID_WEATHER_FRIEND_GID", "青蛙使坏瓶只能在好友农场使用"));
        }
        let Some((uid, _)) = self.available_stack(FROG_MISCHIEF_BOTTLE_ID).await? else {
            return Err(business_error("WEATHER_FROG_UNAVAILABLE", "背包中没有可用的青蛙使坏瓶"));
        };
        let mut entered = false;
        let result: Result<UseReply> = async {
            let enter_reply = self.friend_api.enter_farm(friend_gid).await?;
            entered = true;
            self.cache_inspection(
                friend_gid,
                FriendInspection {
                    weather: enter_reply.weather.clone(),
                    lands: enter_reply.lands.clone(),
                    inspected_at: crate::utils::time::get_server_time_secs(),
                    error: String::new(),
                },
            );
            self.send_bottle_use(
                FROG_MISCHIEF_BOTTLE_ID,
                uid,
                Some(UseTarget { host_gid: friend_gid, land_ids: vec![], use_config_id: 0 }),
            )
            .await
        }
        .await;
        if entered {
            let _ = self.friend_api.leave_farm(friend_gid).await;
        }
        let reply = result?;
        let inspection =
            self.fresh_friend_weather(friend_gid).unwrap_or_else(FriendInspection::empty);
        let meta = serde_json::json!({ "gid": friend_gid.to_string() });
        let friend = self.friend_weather_dto(&meta, &inspection);
        Ok(serde_json::json!({
            "outcome": "frog-used",
            "friendGid": friend_gid.to_string(),
            "use": self.use_reply_dto(&reply),
            "friend": friend,
            "snapshot": self.build_snapshot().await?,
        }))
    }

    /// 乌云使坏（地块级；目标必须生长中作物且该地无 5006 记录；不发 use_config_id）
    pub async fn cloud_mischief(
        &self,
        friend_gid: i64,
        land_id: Option<i64>,
    ) -> Result<serde_json::Value> {
        let _mutation = self.mutation_lock.lock().await;
        let own_gid = self.own_gid();
        if friend_gid <= 0 || friend_gid == own_gid {
            return Err(business_error("INVALID_WEATHER_FRIEND_GID", "乌云使坏瓶只能在好友农场使用"));
        }
        let Some((uid, _)) = self.available_stack(CLOUD_MISCHIEF_BOTTLE_ID).await? else {
            return Err(business_error("WEATHER_CLOUD_UNAVAILABLE", "背包中没有可用的乌云使坏瓶"));
        };
        let mut entered = false;
        let result: Result<UseReply> = async {
            let enter_reply = self.friend_api.enter_farm(friend_gid).await?;
            entered = true;
            let eligible = cloud_eligible_land_ids(&enter_reply.lands);
            let target = match land_id {
                Some(id) if eligible.contains(&id) => id,
                Some(_) => {
                    return Err(business_error(
                        "WEATHER_CLOUD_TARGET_UNAVAILABLE",
                        "指定地块当前不能使用乌云使坏瓶",
                    ));
                }
                None => eligible.first().copied().ok_or_else(|| {
                    business_error(
                        "WEATHER_CLOUD_TARGET_UNAVAILABLE",
                        "好友当前没有可使用乌云使坏瓶的作物",
                    )
                })?,
            };
            self.send_bottle_use(
                CLOUD_MISCHIEF_BOTTLE_ID,
                uid,
                Some(UseTarget { host_gid: friend_gid, land_ids: vec![target], use_config_id: 0 }),
            )
            .await
        }
        .await;
        if entered {
            let _ = self.friend_api.leave_farm(friend_gid).await;
        }
        let reply = result?;
        let inspection =
            self.fresh_friend_weather(friend_gid).unwrap_or_else(FriendInspection::empty);
        let meta = serde_json::json!({ "gid": friend_gid.to_string() });
        let friend = self.friend_weather_dto(&meta, &inspection);
        Ok(serde_json::json!({
            "outcome": "cloud-used",
            "friendGid": friend_gid.to_string(),
            "landId": reply.land.as_ref().map(|l| l.id.to_string()).unwrap_or_default(),
            "use": self.use_reply_dto(&reply),
            "friend": friend,
            "snapshot": self.build_snapshot().await?,
        }))
    }

    /// 推进气象研究节点（消耗雷电徽章）
    pub async fn advance_research(&self, node_id: i64) -> Result<serde_json::Value> {
        let _mutation = self.mutation_lock.lock().await;
        if node_id <= 0 {
            return Err(business_error("INVALID_WEATHER_RESEARCH_NODE", "气象研究节点无效"));
        }
        let group_reply = self.query_weather_group().await?;
        let active = group_reply
            .group
            .as_ref()
            .and_then(|g| g.activity.as_ref())
            .map(|a| activity_is_active(a.begin_time, a.end_time))
            .unwrap_or(false);
        if !active {
            return Err(business_error("WEATHER_ACTIVITY_UNAVAILABLE", "雨落成诗活动尚未开放或已经结束"));
        }
        let balances = self.bag_balances().await?;
        let badge_balance = balances.get(&LIGHTNING_BADGE_ID).copied().unwrap_or(0);
        let research_child = group_reply.group.as_ref().and_then(|g| {
            g.children
                .iter()
                .find(|c| {
                    c.activity.as_ref().map(|a| a.activity_id) == Some(WEATHER_RESEARCH_ACTIVITY_ID)
                })
        });
        let track = research_child
            .and_then(|c| c.weather_research.as_ref())
            .and_then(|w| w.track.as_ref())
            .ok_or_else(|| business_error("WEATHER_RESEARCH_UNAVAILABLE", "服务端未返回气象研究数据"))?;
        let node = track
            .nodes
            .iter()
            .find(|n| n.node_id == node_id)
            .ok_or_else(|| business_error("INVALID_WEATHER_RESEARCH_NODE", "气象研究节点不存在"))?;
        if node.status == 4 || node.claimed {
            return Err(business_error("WEATHER_RESEARCH_ALREADY_COMPLETED", "该气象研究节点已经完成"));
        }
        if node.status != 2 {
            return Err(business_error("WEATHER_RESEARCH_LOCKED", "请先完成前置气象研究节点"));
        }
        let cost = node.cost.unwrap_or_default();
        if !(cost.item_id == LIGHTNING_BADGE_ID && badge_balance >= cost.count.max(0)) {
            return Err(business_error("INSUFFICIENT_LIGHTNING_BADGES", "雷电徽章不足"));
        }
        let reward = node.reward.unwrap_or_default();
        let req = AdvanceWeatherResearchRequest {
            activity_id: WEATHER_RESEARCH_ACTIVITY_ID,
            operate_type: ADVANCE_RESEARCH_OPERATE_TYPE,
            weather_research_operate: Some(WeatherResearchOperateParams { node_id }),
        };
        let body =
            self.gateway.request(ACTIVITY_SERVICE, "Operate", &req.encode_to_vec()).await?;
        let reply = ActivityOperateReply::decode(&body[..])?;
        let rewards =
            if reward.item_id > 0 { vec![item_dto(reward.item_id, reward.count)] } else { vec![] };
        Ok(serde_json::json!({
            "outcome": "advanced",
            "nodeId": node_id.to_string(),
            "activityId": reply.activity_id,
            "operateType": reply.operate_type,
            "rewards": rewards,
            "snapshot": self.build_snapshot().await?,
        }))
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn weather(t: i64, status: i64, end: i64, field9: i64) -> WeatherStatus {
        WeatherStatus {
            weather_type: t,
            status,
            begin_time: 0,
            end_time: end,
            source: 0,
            field_8: 0,
            field_9: field9,
        }
    }

    #[test]
    fn weather_status_active_rules() {
        let now = crate::utils::time::get_server_time_secs();
        let dto = weather_status_dto(Some(&weather(1, 1, now + 100, 0)), 42);
        assert!(dto["active"].as_bool().unwrap());
        assert!(dto["isThunderstorm"].as_bool().unwrap());
        assert!(!dto["collectedThisCycle"].as_bool().unwrap());
        assert_eq!(dto["hostGid"], "42");

        let dto = weather_status_dto(Some(&weather(1, 1, now - 10, 0)), 1);
        assert!(!dto["active"].as_bool().unwrap());
        let dto = weather_status_dto(Some(&weather(2, 1, now + 100, 0)), 1);
        assert!(dto["active"].as_bool().unwrap());
        assert!(!dto["isThunderstorm"].as_bool().unwrap());
    }

    #[test]
    fn collected_this_cycle_only_reads_field9() {
        let now = crate::utils::time::get_server_time_secs();
        let dto = weather_status_dto(Some(&weather(1, 1, now + 100, 4)), 1);
        assert!(dto["collectedThisCycle"].as_bool().unwrap());
        let dto = weather_status_dto(Some(&weather(1, 1, now + 100, 1)), 1);
        assert!(!dto["collectedThisCycle"].as_bool().unwrap());
    }

    #[test]
    fn is_thunderstorm_helper() {
        let now = crate::utils::time::get_server_time_secs();
        assert!(weather_is_thunderstorm(Some(&weather(1, 1, now + 100, 0))));
        assert!(!weather_is_thunderstorm(Some(&weather(2, 1, now + 100, 0))));
        // end=0 表示无结束时间 → 视为进行中（对齐 bot：!end || end > now）
        assert!(weather_is_thunderstorm(Some(&weather(1, 1, 0, 0))));
        assert!(!weather_is_thunderstorm(None));
    }

    #[test]
    fn activity_rules_parses_tips_txt() {
        let extra = r#"{"tips":{"title":"玩法说明","txt":["第一段<br/>说明","第二段 <b>加粗</b>"]}}"#.as_bytes();
        let rules = activity_rules(extra);
        assert_eq!(rules["title"], "玩法说明");
        let paragraphs = rules["paragraphs"].as_array().unwrap();
        assert_eq!(paragraphs.len(), 2);
        assert_eq!(paragraphs[0].as_str().unwrap(), "第一段\n说明");
        assert_eq!(paragraphs[1].as_str().unwrap(), "第二段 加粗");
    }

    #[test]
    fn activity_rules_plain_fallback() {
        let rules = activity_rules("纯文本说明".as_bytes());
        assert_eq!(rules["paragraphs"][0], "纯文本说明");
    }

    #[test]
    fn cloud_eligible_requires_growing_plant_without_cloud() {
        use crate::proto::generated::gamepb::plantpb::{
            LandInfo, PlantInteractionUseInfo, PlantInfo, PlantPhaseInfo,
        };
        let phases = |phase: i32| {
            vec![PlantPhaseInfo {
                phase,
                begin_time: 1,
                dry_time: 0,
                weeds_time: 0,
                insect_time: 0,
                mutants: vec![],
                ..Default::default()
            }]
        };
        let land = |id: i64, plant_phase: Option<i32>, uses: Vec<i64>| LandInfo {
            id,
            unlocked: true,
            level: 1,
            plant: plant_phase.map(|p| PlantInfo {
                id: 100,
                name: "作物".into(),
                phases: phases(p),
                interaction_uses: uses
                    .into_iter()
                    .map(|item_id| PlantInteractionUseInfo {
                        item_id,
                        count: 1,
                        effect_type: 0,
                        host_gid: 0,
                        timestamp: 0,
                    })
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let lands = vec![
            land(1, Some(3), vec![]),
            land(2, Some(6), vec![]),
            land(3, Some(1), vec![]),
            land(4, Some(3), vec![5006]),
            land(5, None, vec![]),
        ];
        assert_eq!(cloud_eligible_land_ids(&lands), vec![1]);
    }
}
