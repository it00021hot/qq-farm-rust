//! 青梅酿（bot `activity-center/qingmei.ts`）。
//!
//! 读取先看 `ActivityService.List` 里 `2026081200`、`2026081201`、`2026081202`
//! 是否有一条满足 `activityWindowIsActive`。都没有就返回空页面，不继续查询。
//! 查询是酿造活动 `2026081202` 的 `Operate` 7。
//! 领种子、开始、继续、出售分别是 `Operate` 4、14、15、16。这四个写函数本身不再看时间窗。

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use prost::Message as _;

use crate::config::game_config::global as global_game_config;
use crate::error::{Error, Result};
use crate::network::error::NetworkError;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::activitypb::{
    claim_qing_mei_daily_seed_request, continue_qing_mei_brew_request,
    settle_qing_mei_brew_request, start_qing_mei_brew_request, ActivityListReply,
    ActivityListRequest, ActivityOperateReply, ActivityWindow, ClaimQingMeiDailySeedRequest,
    ContinueQingMeiBrewRequest, QueryActivityRequest, SettleQingMeiBrewRequest,
    StartQingMeiBrewRequest,
};
use crate::proto::generated::gamepb::itempb::{BagReply, BagRequest};
use crate::proto::generated::gamepb::sharepb::ReportShareRequest;
use crate::services::activity_text::strip_html;
use crate::utils::time::{get_server_time_secs, today_system_date_key};

const ACTIVITY_SERVICE: &str = "gamepb.activitypb.ActivityService";
const ITEM_SERVICE: &str = "gamepb.itempb.ItemService";
const SHARE_SERVICE: &str = "gamepb.sharepb.ShareService";
const WINDOW_IDS: [i64; 3] = [2_026_081_200, 2_026_081_201, 2_026_081_202];
const DAILY_ACTIVITY: i64 = 2_026_081_201;
const BREW_ACTIVITY: i64 = 2_026_081_202;
const QINGMEI_ITEM: i64 = 41_221;
const GRANT_ID: i64 = 3;
const QUERY: i64 = 7;
const CLAIM_SEED: i64 = 4;
const START: i64 = 14;
const CONTINUE: i64 = 15;
const SETTLE: i64 = 16;
const SHARE_SOURCE: i32 = 11;
const SHARE_SCENE: i32 = 215;
const SETTLEMENT_MODE: i64 = 2;
const ALREADY_CLAIMED: i64 = 1_034_014;
const SESSION_MOVED: &str = "账号会话已变化，未发送操作";
const ACCOUNT_INVALID: &str = "账号身份无效，未发送操作";

pub struct QingMeiService {
    gateway: Arc<Gateway>,
    mutation_lock: tokio::sync::Mutex<()>,
    claimed_date: Mutex<String>,
}

impl QingMeiService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            mutation_lock: tokio::sync::Mutex::new(()),
            claimed_date: Mutex::new(String::new()),
        }
    }

    pub async fn snapshot(&self) -> Result<serde_json::Value> {
        if !self.window_is_open().await? {
            return Ok(serde_json::json!({ "available": false }));
        }
        let reply = self.query().await?;
        let ingredients = self.ingredients().await.ok();
        Ok(dto(&reply, ingredients.as_deref(), self.claimed_today()))
    }

    pub async fn claim_seed(&self) -> Result<serde_json::Value> {
        self.claim_seed_if(|| true).await
    }

    pub async fn claim_seed_if(
        &self,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let _window_open = self.window_is_open().await?;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        if self.claimed_today() {
            return self.claimed_payload(Vec::new(), true).await;
        }
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let request = ClaimQingMeiDailySeedRequest {
            activity_id: DAILY_ACTIVITY,
            operate_type: CLAIM_SEED,
            params: Some(claim_qing_mei_daily_seed_request::Params { grant_id: GRANT_ID }),
        };
        let sent =
            self.gateway.request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec()).await;
        let (rewards, already) = match sent {
            Ok(bytes) => {
                let reply = ActivityOperateReply::decode(&bytes[..])?;
                if reply.activity_id != DAILY_ACTIVITY || reply.operate_type != CLAIM_SEED {
                    return Err(Error::Business(
                        "青梅种子回包不匹配，请刷新确认结果，勿重复提交".into(),
                    ));
                }
                (reply.rewards, false)
            }
            Err(NetworkError::Gateway { code, .. }) if code == ALREADY_CLAIMED => {
                (Vec::new(), true)
            }
            Err(error) => return Err(Error::Network(error)),
        };
        *self.claimed_date.lock().expect("claimed") = today_system_date_key();
        self.claimed_payload(rewards, already).await
    }

    pub async fn start(&self, ingredients: &[(i64, i64)]) -> Result<serde_json::Value> {
        self.start_if(ingredients, || true).await
    }

    pub async fn start_if(
        &self,
        ingredients: &[(i64, i64)],
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if ingredients.is_empty() {
            return Err(Error::Business("至少选择一组青梅".into()));
        }
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let owned = self.ingredients().await?;
        let mut seen = HashSet::new();
        let mut selected = Vec::new();
        for (uid, count) in ingredients {
            if *uid <= 0 {
                return Err(Error::Business("uid 必须是正十进制整数".into()));
            }
            if *count <= 0 {
                return Err(Error::Business("count 必须是正十进制整数".into()));
            }
            if !seen.insert(*uid) {
                return Err(Error::Business(format!("青梅 UID {uid} 重复")));
            }
            let have =
                owned.iter().find(|item| item.uid == *uid).map(|item| item.count).unwrap_or(0);
            if have < *count {
                return Err(Error::Business(format!("青梅 UID {uid} 数量不足")));
            }
            selected.push(start_qing_mei_brew_request::Ingredient { uid: *uid, count: *count });
        }
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let request = StartQingMeiBrewRequest {
            activity_id: BREW_ACTIVITY,
            operate_type: START,
            params: Some(start_qing_mei_brew_request::Params { ingredients: selected }),
        };
        let bytes = self
            .gateway
            .request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        let reply = ActivityOperateReply::decode(&bytes[..])?;
        if reply.activity_id != BREW_ACTIVITY || reply.operate_type != START {
            return Err(Error::Business("青梅酿造回包不匹配，请刷新确认结果，勿重复提交".into()));
        }
        let mut payload = serde_json::json!({ "started": true });
        attach_snapshot(&mut payload, self.snapshot().await);
        Ok(payload)
    }

    pub async fn continue_brew(&self) -> Result<serde_json::Value> {
        self.continue_if(|| true).await
    }

    pub async fn continue_if(&self, still_current: impl Fn() -> bool) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let current = self.query().await?;
        if !continue_enabled(&current) {
            return Err(Error::Business("当前不能继续酿造".into()));
        }
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let request = ContinueQingMeiBrewRequest {
            activity_id: BREW_ACTIVITY,
            operate_type: CONTINUE,
            params: Some(continue_qing_mei_brew_request::Empty {}),
        };
        let bytes = self
            .gateway
            .request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        let reply = ActivityOperateReply::decode(&bytes[..])?;
        if reply.activity_id != BREW_ACTIVITY || reply.operate_type != CONTINUE {
            return Err(Error::Business(
                "青梅继续酿造回包不匹配，请刷新确认结果，勿重复提交".into(),
            ));
        }
        let mut payload = serde_json::json!({ "quote": quote_value(reply.qingmei_quote.as_ref()) });
        attach_snapshot(&mut payload, self.snapshot().await);
        Ok(payload)
    }

    pub async fn settle(&self) -> Result<serde_json::Value> {
        self.settle_if(|| true).await
    }

    pub async fn settle_if(&self, still_current: impl Fn() -> bool) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let current = self.query().await?;
        if !settle_enabled(&current) {
            return Err(Error::Business("当前不能出售青梅酿".into()));
        }
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let report = ReportShareRequest { field_1: SHARE_SOURCE, field_4: SHARE_SCENE };
        self.gateway
            .request(SHARE_SERVICE, "ReportShare", &report.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        let request = SettleQingMeiBrewRequest {
            activity_id: BREW_ACTIVITY,
            operate_type: SETTLE,
            params: Some(settle_qing_mei_brew_request::Params { settlement_mode: SETTLEMENT_MODE }),
        };
        let bytes = self
            .gateway
            .request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        let reply = ActivityOperateReply::decode(&bytes[..])?;
        if reply.activity_id != BREW_ACTIVITY || reply.operate_type != SETTLE {
            return Err(Error::Business("青梅出售回包不匹配，请刷新确认结果，勿重复提交".into()));
        }
        let rewards = reply
            .qingmei_settlement
            .as_ref()
            .and_then(|settlement| settlement.reward.clone())
            .map(|item| vec![reward_value(&item)])
            .unwrap_or_else(|| reply.rewards.iter().map(reward_value).collect());
        let settlement = reply.qingmei_settlement.as_ref().map(|item| {
            serde_json::json!({ "mode": item.settlement_mode, "totalGold": item.total_gold.to_string() })
        });
        let mut payload = serde_json::json!({ "rewards": rewards, "settlement": settlement });
        attach_snapshot(&mut payload, self.snapshot().await);
        Ok(payload)
    }

    async fn claimed_payload(
        &self,
        rewards: Vec<crate::proto::generated::corepb::Item>,
        already: bool,
    ) -> Result<serde_json::Value> {
        let mut payload = serde_json::json!({
            "alreadyClaimed": already,
            "rewards": rewards.iter().map(reward_value).collect::<Vec<_>>(),
        });
        attach_snapshot(&mut payload, self.snapshot().await);
        Ok(payload)
    }

    fn claimed_today(&self) -> bool {
        self.claimed_date.lock().expect("claimed").as_str() == today_system_date_key()
    }

    async fn window_is_open(&self) -> Result<bool> {
        let body = ActivityListRequest { game_scene_id: 1 }.encode_to_vec();
        let bytes =
            self.gateway.request(ACTIVITY_SERVICE, "List", &body).await.map_err(Error::Network)?;
        let reply = ActivityListReply::decode(&bytes[..])?;
        if reply.activity_windows.is_empty() {
            return Err(Error::Business("活动列表回包未包含时间配置".into()));
        }
        let now = get_server_time_secs();
        Ok(reply
            .activity_windows
            .iter()
            .any(|window| WINDOW_IDS.contains(&window.id) && window_is_active(window, now)))
    }

    async fn query(&self) -> Result<ActivityOperateReply> {
        let request = QueryActivityRequest { activity_id: BREW_ACTIVITY, operate_type: QUERY };
        let bytes = self
            .gateway
            .request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        Ok(ActivityOperateReply::decode(&bytes[..])?)
    }

    async fn ingredients(&self) -> Result<Vec<IngredientRow>> {
        let bytes = self
            .gateway
            .request(ITEM_SERVICE, "Bag", &BagRequest {}.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        let reply = BagReply::decode(&bytes[..])?;
        let items = reply.item_bag.map(|bag| bag.items).unwrap_or_default();
        Ok(items
            .into_iter()
            .filter(|item| item.id == QINGMEI_ITEM && item.count > 0)
            .map(|item| {
                let mutants =
                    item.mutant_types.into_iter().filter(|id| *id != 0).collect::<Vec<_>>();
                IngredientRow {
                    uid: item.uid,
                    count: item.count,
                    mutant_types: mutants.clone(),
                    mutant_names: global_game_config().get_mutant_type_names(&mutants),
                }
            })
            .collect())
    }
}

struct IngredientRow {
    uid: i64,
    count: i64,
    mutant_types: Vec<i64>,
    mutant_names: Vec<String>,
}

pub fn require_qingmei_account(account_id: &str) -> Result<()> {
    if account_id.is_empty() || account_id.chars().all(char::is_whitespace) {
        Err(Error::Business(ACCOUNT_INVALID.into()))
    } else {
        Ok(())
    }
}

/// 只用于读取是否返回空页面。与 `activityWindowIsActive` 相同：开始为 0 或已到，结束为 0 或含结束时刻。
#[must_use]
pub fn qingmei_window_is_open(begin: i64, end: i64, now: i64) -> bool {
    (begin <= 0 || now >= begin) && (end <= 0 || now <= end)
}

fn window_is_active(window: &ActivityWindow, now: i64) -> bool {
    qingmei_window_is_open(window.begin_time, window.end_time, now)
}

fn continue_enabled(reply: &ActivityOperateReply) -> bool {
    let brew = reply.data.as_ref().and_then(|data| data.qingmei_brew.clone()).unwrap_or_default();
    let max_rounds = if brew.max_rounds <= 0 { 3 } else { brew.max_rounds };
    brew.current_round < max_rounds && !brew.finished && brew.base_gold > 0
}

fn settle_enabled(reply: &ActivityOperateReply) -> bool {
    let brew = reply.data.as_ref().and_then(|data| data.qingmei_brew.clone()).unwrap_or_default();
    !brew.quote_totals.is_empty() || brew.finished
}

fn attach_snapshot(payload: &mut serde_json::Value, snapshot: Result<serde_json::Value>) {
    match snapshot {
        Ok(snapshot) => payload["snapshot"] = snapshot,
        Err(_) => payload["refreshRequired"] = serde_json::json!(true),
    }
}

fn dto(
    reply: &ActivityOperateReply,
    ingredients: Option<&[IngredientRow]>,
    claimed_today: bool,
) -> serde_json::Value {
    let activity = reply.data.as_ref().and_then(|data| data.activity.clone()).unwrap_or_default();
    let brew = reply.data.as_ref().and_then(|data| data.qingmei_brew.clone()).unwrap_or_default();
    let daily = reply.data.as_ref().and_then(|data| data.qingmei_daily_seed.clone());
    let quote = reply
        .qingmei_quote
        .clone()
        .or_else(|| reply.data.as_ref().and_then(|data| data.qingmei_quote.clone()));
    let max_rounds = if brew.max_rounds <= 0 { 3 } else { brew.max_rounds.max(1) };
    let balance = ingredients.map(|rows| rows.iter().map(|row| row.count).sum::<i64>());
    let claimed = claimed_today || daily.as_ref().is_some_and(|seed| seed.claimed);
    let (title, paragraphs) = rules_from(&activity.extra);
    let name =
        if activity.name.is_empty() { "青酿换万金".to_owned() } else { activity.name.clone() };
    let activity_id = if activity.activity_id == 0 { BREW_ACTIVITY } else { activity.activity_id };
    serde_json::json!({
        "available": true,
        "activityId": activity_id.to_string(),
        "dailyActivityId": DAILY_ACTIVITY.to_string(),
        "name": name,
        "startTime": activity.begin_time,
        "endTime": activity.end_time,
        "rules": { "title": title, "paragraphs": paragraphs },
        "balance": balance,
        "balanceKnown": ingredients.is_some(),
        "ingredients": ingredients.unwrap_or(&[]).iter().map(ingredient_value).collect::<Vec<_>>(),
        "baseGold": brew.base_gold.to_string(),
        "basePrice": brew.base_price.to_string(),
        "guaranteedPrice": brew.guaranteed_price.to_string(),
        "currentRound": brew.current_round,
        "started": brew.base_gold > 0,
        "maxRounds": max_rounds,
        "finished": brew.finished,
        "quotePrices": brew.quote_prices.iter().map(|price| price.to_string()).collect::<Vec<_>>(),
        "quoteTotals": brew.quote_totals.iter().map(|total| total.to_string()).collect::<Vec<_>>(),
        "quote": quote_value(quote.as_ref()),
        "dailySeed": {
            "claimed": claimed,
            "grantId": daily.as_ref().and_then(|seed| seed.grant.as_ref()).map(|grant| grant.grant_id).unwrap_or(GRANT_ID).to_string(),
            "reward": daily.as_ref().and_then(|seed| seed.grant.as_ref()).and_then(|grant| grant.item.clone()).map(|item| item_value(item.item_id, item.count)),
        },
        "actions": {
            "claimSeed": { "enabled": !claimed },
            "start": { "enabled": ingredients.is_none_or(|rows| !rows.is_empty()) },
            "continue": { "enabled": continue_enabled(reply) },
            "settle": { "enabled": settle_enabled(reply) },
        },
    })
}

fn ingredient_value(row: &IngredientRow) -> serde_json::Value {
    serde_json::json!({
        "uid": row.uid.to_string(),
        "count": row.count,
        "name": item_name(QINGMEI_ITEM),
        "mutantTypes": row.mutant_types.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
        "mutantTypeNames": row.mutant_names,
    })
}

fn quote_value(
    quote: Option<&crate::proto::generated::gamepb::activitypb::QingMeiQuote>,
) -> serde_json::Value {
    quote
        .map(|quote| {
            serde_json::json!({
                "round": quote.round,
                "unitPrice": quote.unit_price.to_string(),
                "totalGold": quote.total_gold.to_string(),
                "doubled": quote.doubled,
            })
        })
        .unwrap_or(serde_json::Value::Null)
}

fn item_value(id: i64, count: i64) -> serde_json::Value {
    serde_json::json!({ "id": id.to_string(), "count": count, "name": item_name(id) })
}

fn reward_value(item: &crate::proto::generated::corepb::Item) -> serde_json::Value {
    item_value(item.id, item.count)
}

fn item_name(id: i64) -> String {
    global_game_config()
        .get_item_by_id(id)
        .map(|item| item.name)
        .unwrap_or_else(|| format!("物品 #{id}"))
}

fn rules_from(extra: &[u8]) -> (String, Vec<String>) {
    let text = String::from_utf8_lossy(extra).trim().to_owned();
    if text.is_empty() {
        return (String::new(), Vec::new());
    }
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
        let plain = strip_html(&text).trim().to_owned();
        return (String::new(), if plain.is_empty() { Vec::new() } else { vec![plain] });
    };
    let paragraphs = parsed
        .pointer("/tips/txt")
        .and_then(serde_json::Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(serde_json::Value::as_str)
                .map(|line| strip_html(line).trim().to_owned())
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !paragraphs.is_empty() {
        let title = parsed
            .pointer("/tips/title")
            .and_then(serde_json::Value::as_str)
            .map(|line| strip_html(line).trim().to_owned())
            .unwrap_or_default();
        return (title, paragraphs);
    }
    (String::new(), Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::encryptor::NoopEncryptor;
    use crate::network::gateway::{Gateway, GatewayConfig};
    use crate::proto::generated::corepb::{Item as CoreItem, ItemBag};
    use crate::proto::generated::gamepb::activitypb::{
        ActivityContent, ActivityData, QingMeiBrewData, QingMeiDailySeedData, QingMeiQuote,
        QingMeiSettlement,
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

    fn windows(id: i64, begin: i64, end: i64) -> Vec<u8> {
        ActivityListReply {
            activity_windows: vec![ActivityWindow {
                id,
                name: "青梅".into(),
                begin_time: begin,
                end_time: end,
            }],
            ..ActivityListReply::default()
        }
        .encode_to_vec()
    }

    fn brew(base_gold: i64, round: i64, finished: bool) -> ActivityOperateReply {
        ActivityOperateReply {
            activity_id: BREW_ACTIVITY,
            operate_type: QUERY,
            data: Some(ActivityData {
                activity: Some(ActivityContent {
                    activity_id: BREW_ACTIVITY,
                    name: "青酿换万金".into(),
                    extra: r#"{"tips":{"title":"说明","txt":["<b>酿造</b>"]}}"#.into(),
                    ..ActivityContent::default()
                }),
                qingmei_brew: Some(QingMeiBrewData {
                    base_gold,
                    current_round: round,
                    max_rounds: 3,
                    quote_prices: vec![10],
                    quote_totals: if base_gold > 0 { vec![20] } else { Vec::new() },
                    finished,
                    base_price: 8,
                    guaranteed_price: 6,
                    ..QingMeiBrewData::default()
                }),
                qingmei_daily_seed: Some(QingMeiDailySeedData {
                    claimed: false,
                    ..QingMeiDailySeedData::default()
                }),
                ..ActivityData::default()
            }),
            ..ActivityOperateReply::default()
        }
    }

    fn bag(uid: i64, count: i64) -> Vec<u8> {
        BagReply {
            item_bag: Some(ItemBag {
                items: vec![CoreItem {
                    id: QINGMEI_ITEM,
                    count,
                    uid,
                    mutant_types: vec![0, 2],
                    ..CoreItem::default()
                }],
                ..ItemBag::default()
            }),
        }
        .encode_to_vec()
    }

    #[test]
    fn read_window_matches_the_shared_activity_window() {
        assert!(qingmei_window_is_open(0, 0, 100));
        assert!(qingmei_window_is_open(100, 200, 200));
        assert!(!qingmei_window_is_open(100, 200, 201));
        assert!(!qingmei_window_is_open(100, 200, 99));
    }

    #[tokio::test]
    async fn a_closed_window_returns_empty_but_claim_still_sends_without_a_time_gate() {
        let _data = crate::config::paths::TestDataDir::enter();
        let writes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&writes);
        let now = get_server_time_secs();
        let gateway = gateway();
        gateway.install_reply_script(move |_service, method, body| {
            if method == "List" {
                return Ok(windows(BREW_ACTIVITY, now + 100, now + 200));
            }
            if method == "Operate" {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let request = ClaimQingMeiDailySeedRequest::decode(body).unwrap();
                assert_eq!(request.activity_id, DAILY_ACTIVITY);
                assert_eq!(request.operate_type, CLAIM_SEED);
                assert_eq!(request.params.unwrap().grant_id, GRANT_ID);
                return Ok(ActivityOperateReply {
                    activity_id: DAILY_ACTIVITY,
                    operate_type: CLAIM_SEED,
                    rewards: vec![CoreItem { id: 1, count: 2, ..CoreItem::default() }],
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            Err(NetworkError::Phase(method.into()))
        });
        let service = QingMeiService::new(gateway);
        let empty = service.snapshot().await.unwrap();
        assert_eq!(empty["available"], false);
        let claimed = service.claim_seed().await.unwrap();
        assert_eq!(claimed["rewards"][0]["count"], 2);
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
        let again = service.claim_seed().await.unwrap();
        assert_eq!(again["alreadyClaimed"], true);
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn start_sends_the_selected_count_and_rejects_unknown_or_excess() {
        let _data = crate::config::paths::TestDataDir::enter();
        let writes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&writes);
        let gateway = gateway();
        gateway.install_reply_script(move |_service, method, body| match method {
            "List" => Ok(windows(2_026_081_200, 0, 0)),
            "Bag" => Ok(bag(9, 2)),
            "Operate"
                if QueryActivityRequest::decode(body)
                    .map(|request| request.operate_type)
                    .unwrap_or(0)
                    == QUERY =>
            {
                Ok(brew(0, 0, false).encode_to_vec())
            }
            "Operate" => {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let request = StartQingMeiBrewRequest::decode(body).unwrap();
                assert_eq!(request.activity_id, BREW_ACTIVITY);
                assert_eq!(request.operate_type, START);
                let picked = &request.params.unwrap().ingredients;
                assert_eq!(picked.len(), 1);
                assert_eq!(picked[0].uid, 9);
                assert_eq!(picked[0].count, 2);
                Ok(ActivityOperateReply {
                    activity_id: BREW_ACTIVITY,
                    operate_type: START,
                    data: Some(ActivityData {
                        qingmei_brew: Some(QingMeiBrewData {
                            base_gold: 5,
                            current_round: 0,
                            max_rounds: 3,
                            ..QingMeiBrewData::default()
                        }),
                        ..ActivityData::default()
                    }),
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec())
            }
            _ => Err(NetworkError::Phase(method.into())),
        });
        let service = QingMeiService::new(gateway);
        let snapshot = service.snapshot().await.unwrap();
        assert_eq!(snapshot["available"], true);
        assert_eq!(snapshot["balance"], 2);
        assert_eq!(snapshot["ingredients"][0]["uid"], "9");
        assert_eq!(snapshot["rules"]["paragraphs"][0], "酿造");
        assert!(service.start(&[]).await.is_err());
        assert!(service.start(&[(9, 0)]).await.is_err());
        assert!(service.start(&[(9, 3)]).await.is_err());
        assert!(service.start(&[(9, 1), (9, 1)]).await.is_err());
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 0);
        service.start(&[(9, 2)]).await.unwrap();
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn continue_and_settle_decode_fields_and_a_timeout_is_not_replayed() {
        let _data = crate::config::paths::TestDataDir::enter();
        let settles = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let reports = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let settle_count = std::sync::Arc::clone(&settles);
        let report_count = std::sync::Arc::clone(&reports);
        let now = get_server_time_secs();
        let gateway = gateway();
        gateway.install_reply_script(move |_service, method, body| {
            if method == "List" {
                return Ok(windows(DAILY_ACTIVITY, now - 10, now));
            }
            if method == "ReportShare" {
                report_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let request = ReportShareRequest::decode(body).unwrap();
                assert_eq!(request.field_1, SHARE_SOURCE);
                assert_eq!(request.field_4, SHARE_SCENE);
                return Ok(Vec::new());
            }
            if method == "Operate"
                && QueryActivityRequest::decode(body).map(|item| item.operate_type).unwrap_or(0)
                    == CONTINUE
            {
                return Ok(ActivityOperateReply {
                    activity_id: BREW_ACTIVITY,
                    operate_type: CONTINUE,
                    qingmei_quote: Some(QingMeiQuote {
                        round: 1,
                        unit_price: 10,
                        total_gold: 20,
                        doubled: false,
                    }),
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            if method == "Operate"
                && QueryActivityRequest::decode(body).map(|item| item.operate_type).unwrap_or(0)
                    == SETTLE
            {
                settle_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let request = SettleQingMeiBrewRequest::decode(body).unwrap();
                assert_eq!(request.params.unwrap().settlement_mode, SETTLEMENT_MODE);
                return Err(NetworkError::Timeout {
                    client_seq: 1,
                    service_name: ACTIVITY_SERVICE.into(),
                    method_name: "Operate".into(),
                    pending: 1,
                });
            }
            if method == "Bag" {
                return Err(NetworkError::Phase("bag".into()));
            }
            Ok(brew(5, 1, false).encode_to_vec())
        });
        let service = QingMeiService::new(gateway);
        let continued = service.continue_brew().await.unwrap();
        assert_eq!(continued["quote"]["totalGold"], "20");
        let error = service.settle().await.unwrap_err();
        assert!(
            error.to_string().contains("超时") || error.to_string().contains("请求"),
            "{error}"
        );
        assert_eq!(reports.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(settles.load(std::sync::atomic::Ordering::SeqCst), 1);
        let _ = now;
    }
}
