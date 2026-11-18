//! 七夕鹊桥寄情（bot `activity-center/qixi.ts`）。
//!
//! 读取 `ActivityService.GetGroup` 组 `2026081800`。
//! 领鹊桥是活动 `2026081801` 的 `Operate` 25，`step` 固定为 0。
//! 送香囊是活动 `2026081802` 的 `Operate` 26，目标好友和文案号 15，协议里没有数量字段。

use std::sync::Arc;

use prost::Message as _;

use crate::config::game_config::global as global_game_config;
use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::activitypb::{
    claim_qixi_bridge_rewards_request, gift_qixi_sachet_request, ActivityData,
    ActivityOperateReply, ClaimQixiBridgeRewardsRequest, GetGroupReply, GetGroupRequest,
    GiftQixiSachetRequest, QixiBridgeConfig, QixiBridgeStage, QixiGiftExchange, QixiGiftProgress,
};
use crate::proto::generated::gamepb::itempb::{BagReply, BagRequest};
use crate::services::activity_text::strip_html;
use crate::utils::time::get_server_time_secs;

const SERVICE: &str = "gamepb.activitypb.ActivityService";
const ITEM_SERVICE: &str = "gamepb.itempb.ItemService";
const GROUP_ID: i64 = 2_026_081_800;
const BRIDGE_ACTIVITY: i64 = 2_026_081_801;
const GIFT_ACTIVITY: i64 = 2_026_081_802;
const CLAIM: i64 = 25;
const GIFT: i64 = 26;
const FEATHER: i64 = 1024;
const SACHET: i64 = 1025;
const RECEIVED_SACHET: i64 = 1026;
const DEW: i64 = 301_103;
const MESSAGE_TEXT_ID: i64 = 15;
const SESSION_MOVED: &str = "账号会话已变化，未发送操作";
const ACCOUNT_INVALID: &str = "账号身份无效，未发送操作";

pub struct QixiService {
    gateway: Arc<Gateway>,
    mutation_lock: tokio::sync::Mutex<()>,
}

impl QixiService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway, mutation_lock: tokio::sync::Mutex::new(()) }
    }

    pub async fn snapshot(&self) -> Result<serde_json::Value> {
        let reply = self.group().await?;
        let balances = self.balances().await.ok();
        qixi_value(&reply, balances.as_ref(), get_server_time_secs())
    }

    pub async fn claim(&self) -> Result<serde_json::Value> {
        self.claim_if(|| true).await
    }

    pub async fn claim_if(&self, still_current: impl Fn() -> bool) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let current = self.snapshot().await?;
        if current["actions"]["bridge"]["enabled"] != true {
            return Err(Error::Business("当前没有可领取的鹊桥奖励".into()));
        }
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let request = ClaimQixiBridgeRewardsRequest {
            activity_id: BRIDGE_ACTIVITY,
            operate_type: CLAIM,
            params: Some(claim_qixi_bridge_rewards_request::Params { step: 0 }),
        };
        let bytes = self
            .gateway
            .request(SERVICE, "Operate", &request.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        let reply = ActivityOperateReply::decode(&bytes[..])?;
        if reply.activity_id != BRIDGE_ACTIVITY || reply.operate_type != CLAIM {
            return Err(Error::Business("鹊桥奖励回包不匹配，请刷新确认结果，勿重复提交".into()));
        }
        let rewards = reply
            .qixi_bridge_result
            .as_ref()
            .map(|result| result.awards.clone())
            .filter(|awards| !awards.is_empty())
            .unwrap_or(reply.rewards.clone());
        let stages = reply
            .qixi_bridge_result
            .as_ref()
            .map(|result| result.unlocked_steps.clone())
            .unwrap_or_default();
        let mut payload = serde_json::json!({
            "claimedStages": stages,
            "rewards": rewards.iter().map(reward_value).collect::<Vec<_>>(),
            "completed": reply.qixi_bridge_result.as_ref().is_some_and(|result| result.completed),
        });
        attach_snapshot(&mut payload, self.snapshot().await);
        Ok(payload)
    }

    pub async fn gift(&self, friend_gid: i64) -> Result<serde_json::Value> {
        self.gift_if(friend_gid, || true).await
    }

    pub async fn gift_if(
        &self,
        friend_gid: i64,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if friend_gid <= 0 {
            return Err(Error::Business("friendGid 必须是正十进制整数".into()));
        }
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let current = self.snapshot().await?;
        if current["balances"]["known"] == true
            && current["balances"]["sachet"].as_i64().unwrap_or(0) < 1
        {
            return Err(Error::Business("鹊羽香囊数量不足".into()));
        }
        if current["actions"]["gift"]["enabled"] != true {
            return Err(Error::Business("当前无法赠送鹊羽香囊".into()));
        }
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let request = GiftQixiSachetRequest {
            activity_id: GIFT_ACTIVITY,
            operate_type: GIFT,
            params: Some(gift_qixi_sachet_request::Params {
                target_gid: friend_gid,
                msg_text_id: MESSAGE_TEXT_ID,
            }),
        };
        let bytes = self
            .gateway
            .request(SERVICE, "Operate", &request.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        let reply = ActivityOperateReply::decode(&bytes[..])?;
        if reply.activity_id != GIFT_ACTIVITY || reply.operate_type != GIFT {
            return Err(Error::Business("鹊羽香囊回包不匹配，请刷新确认结果，勿重复提交".into()));
        }
        let mut payload = serde_json::json!({
            "friendGid": friend_gid.to_string(),
            "count": 1,
            "messageTextId": MESSAGE_TEXT_ID,
            "totalSendCount": reply.qixi_gift_result.as_ref().map(|result| result.total_send_count),
        });
        attach_snapshot(&mut payload, self.snapshot().await);
        Ok(payload)
    }

    async fn group(&self) -> Result<GetGroupReply> {
        let body = GetGroupRequest { group_id: GROUP_ID }.encode_to_vec();
        let bytes =
            self.gateway.request(SERVICE, "GetGroup", &body).await.map_err(Error::Network)?;
        Ok(GetGroupReply::decode(&bytes[..])?)
    }

    async fn balances(&self) -> Result<std::collections::HashMap<i64, i64>> {
        let bytes = self
            .gateway
            .request(ITEM_SERVICE, "Bag", &BagRequest {}.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        let reply = BagReply::decode(&bytes[..])?;
        let mut totals = std::collections::HashMap::new();
        let Some(bag) = reply.item_bag else {
            return Ok(totals);
        };
        for item in bag.items {
            if matches!(item.id, FEATHER | SACHET | RECEIVED_SACHET | DEW) {
                *totals.entry(item.id).or_insert(0) += item.count;
            }
        }
        Ok(totals)
    }
}

pub fn require_qixi_account(account_id: &str) -> Result<()> {
    if account_id.is_empty() || account_id.chars().all(char::is_whitespace) {
        Err(Error::Business(ACCOUNT_INVALID.into()))
    } else {
        Ok(())
    }
}

/// 基准 `activityWindowIsActive`。开始为 0 或不晚于现在，并且结束为 0 或不早于现在。
#[must_use]
pub fn qixi_window_is_open(begin: i64, end: i64, now: i64) -> bool {
    (begin <= 0 || now >= begin) && (end <= 0 || now <= end)
}

fn attach_snapshot(payload: &mut serde_json::Value, snapshot: Result<serde_json::Value>) {
    match snapshot {
        Ok(snapshot) => payload["snapshot"] = snapshot,
        Err(_) => payload["refreshRequired"] = serde_json::json!(true),
    }
}

fn qixi_value(
    reply: &GetGroupReply,
    balances: Option<&std::collections::HashMap<i64, i64>>,
    now: i64,
) -> Result<serde_json::Value> {
    let bridge = find_child(reply, BRIDGE_ACTIVITY)
        .ok_or_else(|| Error::Business("服务端未发现鹊桥寄情活动".into()))?;
    let gift = find_child(reply, GIFT_ACTIVITY)
        .ok_or_else(|| Error::Business("服务端未发现鹊桥寄情活动".into()))?;
    let bridge_activity = bridge
        .activity
        .as_ref()
        .ok_or_else(|| Error::Business("服务端未发现鹊桥寄情活动".into()))?;
    let config = bridge.qixi_bridge.clone().unwrap_or_default();
    let progress = gift.qixi_gift.clone().unwrap_or_default();
    let known = balances.is_some();
    let amount = |id: i64| balances.map(|map| map.get(&id).copied().unwrap_or(0));
    let sachet = amount(SACHET);
    let active = qixi_window_is_open(bridge_activity.begin_time, bridge_activity.end_time, now);
    let claimable = bridge_activity.field_23 != 0;
    let (title, paragraphs) = rules_from(&bridge_activity.extra);
    let name = if bridge_activity.name.is_empty() {
        "鹊桥寄情".to_owned()
    } else {
        bridge_activity.name.clone()
    };
    Ok(serde_json::json!({
        "groupId": GROUP_ID.to_string(),
        "bridgeActivityId": BRIDGE_ACTIVITY.to_string(),
        "giftActivityId": GIFT_ACTIVITY.to_string(),
        "name": name,
        "startTime": bridge_activity.begin_time,
        "endTime": bridge_activity.end_time,
        "serverTime": now,
        "active": active,
        "window": window_label(bridge_activity.begin_time, bridge_activity.end_time, now),
        "rules": { "title": title, "paragraphs": paragraphs },
        "balances": {
            "feather": amount(FEATHER),
            "sachet": sachet,
            "receivedSachet": amount(RECEIVED_SACHET),
            "dew": amount(DEW),
            "known": known,
        },
        "feather": item_value(FEATHER, amount(FEATHER).unwrap_or(0)),
        "sachet": item_value(SACHET, sachet.unwrap_or(0)),
        "receivedSachet": item_value(RECEIVED_SACHET, amount(RECEIVED_SACHET).unwrap_or(0)),
        "dew": item_value(DEW, amount(DEW).unwrap_or(0)),
        "dewHint": "鹊羽灵露在土地上使用：好友土地前往「好友」页，自己的土地前往「农场」页。",
        "bridge": bridge_value(&config, claimable),
        "gift": gift_value(&progress),
        "actions": {
            "bridge": { "enabled": active && claimable },
            "gift": { "enabled": active && (!known || sachet.unwrap_or(0) > 0) },
        },
    }))
}

fn find_child(reply: &GetGroupReply, activity_id: i64) -> Option<ActivityData> {
    reply
        .group
        .as_ref()?
        .children
        .iter()
        .find(|child| {
            child.activity.as_ref().is_some_and(|activity| activity.activity_id == activity_id)
        })
        .cloned()
}

fn bridge_value(config: &QixiBridgeConfig, claimable: bool) -> serde_json::Value {
    let current = config.current_stage;
    let stages = config
        .stages
        .iter()
        .map(|stage| stage_value(stage, current, claimable))
        .collect::<Vec<_>>();
    serde_json::json!({
        "currentStage": current,
        "claimable": claimable,
        "stages": stages,
        "displayItems": config.display_items.iter().map(|item| item_value(item.item_id, item.count)).collect::<Vec<_>>(),
    })
}

fn stage_value(stage: &QixiBridgeStage, current: i64, bridge_claimable: bool) -> serde_json::Value {
    let number = stage.stage;
    let completed = stage.status == 2 || (current > 0 && number > 0 && number <= current);
    let claimable = bridge_claimable && number == current;
    let state = if claimable {
        "待领取"
    } else if completed && !claimable {
        "已完成"
    } else if number == current {
        "建设中"
    } else if completed {
        "已达成"
    } else {
        "未达成"
    };
    serde_json::json!({
        "id": number.to_string(),
        "stage": number,
        "statusCode": stage.status.to_string(),
        "completed": completed,
        "claimed": completed && !claimable,
        "claimable": claimable,
        "current": number == current,
        "state": state,
        "cost": stage.cost.as_ref().map(|item| item_value(item.item_id, item.count)),
        "rewards": stage.rewards.iter().map(|item| item_value(item.item_id, item.count)).collect::<Vec<_>>(),
    })
}

fn gift_value(progress: &QixiGiftProgress) -> serde_json::Value {
    serde_json::json!({
        "sentCount": progress.total_send_count,
        "sendLimit": progress.total_send_limit,
        "receiveLimit": progress.total_receive_limit,
        "messageTextId": MESSAGE_TEXT_ID,
        "fixedCount": 1,
        "exchanges": progress.gifts.iter().map(exchange_value).collect::<Vec<_>>(),
    })
}

fn exchange_value(entry: &QixiGiftExchange) -> serde_json::Value {
    serde_json::json!({
        "giftType": entry.gift_type.to_string(),
        "content": entry.content.to_string(),
        "costItems": entry.cost_items.iter().map(|item| item_value(item.item_id, item.count)).collect::<Vec<_>>(),
        "receiveItems": entry.receive_items.iter().map(|item| item_value(item.item_id, item.count)).collect::<Vec<_>>(),
    })
}

fn window_label(begin: i64, end: i64, now: i64) -> &'static str {
    if begin > 0 && now < begin {
        "upcoming"
    } else if end > 0 && now > end {
        "ended"
    } else {
        "open"
    }
}

fn rules_from(extra: &[u8]) -> (String, Vec<String>) {
    let text = String::from_utf8_lossy(extra).trim().to_owned();
    if text.is_empty() {
        return (String::new(), Vec::new());
    }
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
        return (String::new(), vec![strip_html(&text)]);
    };
    let paragraphs = parsed
        .pointer("/tips/txt")
        .and_then(serde_json::Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(serde_json::Value::as_str)
                .map(strip_html)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if paragraphs.is_empty() {
        return (String::new(), vec![strip_html(&text)]);
    }
    let title = parsed
        .pointer("/tips/title")
        .and_then(serde_json::Value::as_str)
        .map(strip_html)
        .unwrap_or_default();
    (title, paragraphs)
}

fn item_value(id: i64, count: i64) -> serde_json::Value {
    serde_json::json!({ "id": id.to_string(), "count": count, "name": item_name(id) })
}

fn reward_value(item: &crate::proto::generated::corepb::Item) -> serde_json::Value {
    serde_json::json!({ "id": item.id.to_string(), "count": item.count, "name": item_name(item.id) })
}

fn item_name(id: i64) -> String {
    global_game_config()
        .get_item_by_id(id)
        .map(|item| item.name)
        .unwrap_or_else(|| format!("物品 #{id}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::encryptor::NoopEncryptor;
    use crate::network::error::NetworkError;
    use crate::network::gateway::{Gateway, GatewayConfig};
    use crate::proto::generated::corepb::{Item as CoreItem, ItemBag};
    use crate::proto::generated::gamepb::activitypb::{
        ActivityContent, ActivityItem, QixiBridgeRewardResult,
    };

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

    fn activity(id: i64, begin: i64, end: i64, claim_flag: i64) -> ActivityContent {
        ActivityContent {
            activity_id: id,
            begin_time: begin,
            end_time: end,
            field_23: claim_flag,
            name: "鹊桥寄情".into(),
            extra: r#"{"tips":{"title":"<b>说明</b>","txt":["<b>筑桥</b>","香囊"]}}"#.into(),
            ..ActivityContent::default()
        }
    }

    fn group(begin: i64, end: i64, claim_flag: i64) -> GetGroupReply {
        GetGroupReply {
            group: Some(ActivityData {
                children: vec![
                    ActivityData {
                        activity: Some(activity(BRIDGE_ACTIVITY, begin, end, claim_flag)),
                        qixi_bridge: Some(QixiBridgeConfig {
                            current_stage: 2,
                            display_items: vec![ActivityItem { item_id: FEATHER, count: 1 }],
                            stages: vec![
                                QixiBridgeStage {
                                    stage: 1,
                                    status: 2,
                                    cost: Some(ActivityItem { item_id: FEATHER, count: 10 }),
                                    rewards: vec![ActivityItem { item_id: DEW, count: 1 }],
                                },
                                QixiBridgeStage {
                                    stage: 2,
                                    status: 1,
                                    cost: Some(ActivityItem { item_id: FEATHER, count: 20 }),
                                    rewards: vec![
                                        ActivityItem { item_id: SACHET, count: 1 },
                                        ActivityItem { item_id: DEW, count: 2 },
                                    ],
                                },
                                QixiBridgeStage {
                                    stage: 3,
                                    status: 0,
                                    cost: Some(ActivityItem { item_id: FEATHER, count: 30 }),
                                    rewards: vec![ActivityItem { item_id: 8, count: 3 }],
                                },
                            ],
                        }),
                        ..ActivityData::default()
                    },
                    ActivityData {
                        activity: Some(activity(GIFT_ACTIVITY, begin, end, 0)),
                        qixi_gift: Some(QixiGiftProgress {
                            total_send_count: 2,
                            total_send_limit: 5,
                            total_receive_limit: 7,
                            gifts: vec![QixiGiftExchange {
                                gift_type: 1,
                                content: MESSAGE_TEXT_ID,
                                cost_items: vec![ActivityItem { item_id: SACHET, count: 1 }],
                                receive_items: vec![ActivityItem {
                                    item_id: RECEIVED_SACHET,
                                    count: 1,
                                }],
                            }],
                        }),
                        ..ActivityData::default()
                    },
                ],
                ..ActivityData::default()
            }),
        }
    }

    fn bag(count: i64) -> Vec<u8> {
        BagReply {
            item_bag: Some(ItemBag {
                items: vec![
                    CoreItem { id: FEATHER, count: 40, ..CoreItem::default() },
                    CoreItem { id: SACHET, count, ..CoreItem::default() },
                    CoreItem { id: RECEIVED_SACHET, count: 3, ..CoreItem::default() },
                    CoreItem { id: DEW, count: 4, ..CoreItem::default() },
                ],
                ..ItemBag::default()
            }),
        }
        .encode_to_vec()
    }

    #[test]
    fn window_keeps_zero_open_and_includes_the_end() {
        assert!(qixi_window_is_open(0, 0, 100));
        assert!(qixi_window_is_open(0, 50, 50));
        assert!(!qixi_window_is_open(0, 50, 51));
        assert!(!qixi_window_is_open(100, 0, 50));
        assert!(qixi_window_is_open(100, 0, 100));
        assert!(!qixi_window_is_open(100, 200, 99));
        assert!(qixi_window_is_open(100, 200, 100));
        assert!(qixi_window_is_open(100, 200, 200));
    }

    #[tokio::test]
    async fn claim_sends_step_zero_and_keeps_rewards_when_refresh_fails() {
        let _data = crate::config::paths::TestDataDir::enter();
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(String, Vec<u8>)>::new()));
        let log = std::sync::Arc::clone(&calls);
        let gateway = gateway();
        let now = get_server_time_secs();
        gateway.install_reply_script(move |_service, method, body| {
            log.lock().expect("log").push((method.to_owned(), body.to_vec()));
            if method == "Operate" {
                return Ok(ActivityOperateReply {
                    activity_id: BRIDGE_ACTIVITY,
                    operate_type: CLAIM,
                    qixi_bridge_result: Some(QixiBridgeRewardResult {
                        unlocked_steps: vec![2],
                        awards: vec![CoreItem { id: DEW, count: 1, ..CoreItem::default() }],
                        completed: false,
                    }),
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            if method == "GetGroup"
                && log
                    .lock()
                    .expect("log")
                    .iter()
                    .filter(|(method, _)| method == "GetGroup")
                    .count()
                    > 1
            {
                return Err(NetworkError::Phase("refresh".into()));
            }
            if method == "Bag" {
                return Ok(bag(2));
            }
            Ok(group(now - 10, now + 10, 1).encode_to_vec())
        });
        let value = QixiService::new(gateway).claim().await.unwrap();
        let saved = calls.lock().expect("log");
        let body = &saved.iter().find(|(method, _)| method == "Operate").unwrap().1;
        let request = ClaimQixiBridgeRewardsRequest::decode(body.as_slice()).unwrap();
        assert_eq!(request.activity_id, BRIDGE_ACTIVITY);
        assert_eq!(request.operate_type, CLAIM);
        assert_eq!(request.params.unwrap().step, 0);
        assert_eq!(saved.iter().filter(|(method, _)| method == "Operate").count(), 1);
        assert_eq!(value["rewards"][0]["count"], 1);
        assert_eq!(value["claimedStages"][0], 2);
        assert_eq!(value["refreshRequired"], true);
        assert!(value.get("snapshot").is_none());
    }

    #[tokio::test]
    async fn gift_sends_the_friend_and_text_once_and_stops_when_the_bag_is_empty() {
        let _data = crate::config::paths::TestDataDir::enter();
        let writes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&writes);
        let gateway = gateway();
        let now = get_server_time_secs();
        gateway.install_reply_script(move |_service, method, body| {
            if method == "Operate" {
                let seen = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let request = GiftQixiSachetRequest::decode(body).unwrap();
                assert_eq!(request.activity_id, GIFT_ACTIVITY);
                assert_eq!(request.operate_type, GIFT);
                assert_eq!(request.params.as_ref().unwrap().target_gid, 88);
                assert_eq!(request.params.as_ref().unwrap().msg_text_id, MESSAGE_TEXT_ID);
                if seen == 0 {
                    return Ok(ActivityOperateReply {
                        activity_id: GIFT_ACTIVITY,
                        operate_type: GIFT,
                        qixi_gift_result: Some(
                            crate::proto::generated::gamepb::activitypb::QixiGiftResult {
                                total_send_count: 3,
                            },
                        ),
                        ..ActivityOperateReply::default()
                    }
                    .encode_to_vec());
                }
                return Err(NetworkError::Timeout {
                    client_seq: 1,
                    service_name: SERVICE.into(),
                    method_name: "Operate".into(),
                    pending: 1,
                });
            }
            if method == "Bag" {
                let left = if count.load(std::sync::atomic::Ordering::SeqCst) > 0 { 0 } else { 2 };
                return Ok(bag(left));
            }
            Ok(group(now - 5, now + 60, 1).encode_to_vec())
        });
        let service = QixiService::new(gateway);
        assert!(service.gift(0).await.is_err());
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 0);
        let value = service.gift(88).await.unwrap();
        assert_eq!(value["count"], 1);
        assert_eq!(value["snapshot"]["balances"]["sachet"], 0);
        assert_eq!(value["snapshot"]["bridge"]["stages"].as_array().unwrap().len(), 3);
        assert!(value["snapshot"]["rules"]["paragraphs"][0].as_str().unwrap().contains("筑桥"));
        assert_ne!(value["snapshot"]["rules"]["title"], value["snapshot"]["name"]);
        let error = service.gift(88).await.unwrap_err();
        assert!(error.to_string().contains("数量不足"), "{error}");
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_closed_window_and_a_mismatched_reply_do_not_invent_rewards() {
        let _data = crate::config::paths::TestDataDir::enter();
        let writes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&writes);
        let closed_gateway = gateway();
        let now = get_server_time_secs();
        closed_gateway.install_reply_script(move |_service, method, _body| {
            if method == "Operate" {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                return Ok(ActivityOperateReply {
                    activity_id: 1,
                    operate_type: CLAIM,
                    rewards: vec![CoreItem { id: 9, count: 9, ..CoreItem::default() }],
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            if method == "Bag" {
                return Err(NetworkError::Phase("bag".into()));
            }
            Ok(group(now + 100, now + 200, 0).encode_to_vec())
        });
        let service = QixiService::new(Arc::clone(&closed_gateway));
        let closed = service.snapshot().await.unwrap();
        assert_eq!(closed["window"], "upcoming");
        assert_eq!(closed["actions"]["bridge"]["enabled"], false);
        assert_eq!(closed["balances"]["known"], false);
        assert!(service.claim().await.is_err());
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 0);
        let open_gateway = gateway();
        let writes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&writes);
        open_gateway.install_reply_script(move |_service, method, _body| {
            if method == "Operate" {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                return Ok(ActivityOperateReply {
                    activity_id: 1,
                    operate_type: 1,
                    rewards: vec![CoreItem { id: 9, count: 9, ..CoreItem::default() }],
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            if method == "Bag" {
                return Ok(bag(1));
            }
            Ok(group(0, 0, 1).encode_to_vec())
        });
        let error = QixiService::new(open_gateway).claim().await.unwrap_err();
        assert!(error.to_string().contains("不匹配"), "{error}");
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
