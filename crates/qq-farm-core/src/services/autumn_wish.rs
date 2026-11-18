//! 秋祈良愿（bot `autumn-activities.ts` 的 `wish`）。
//!
//! `ActivityService.GetGroup` 组 `2026092400`，活动 `2026092401`。
//! 抽签和领取是 `Operate` 51、52。快乐不独享不走这里。

use std::sync::{Arc, OnceLock};

use prost::Message as _;
use serde::Deserialize;

use crate::config::game_config::global as global_game_config;
use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item;
use crate::proto::generated::gamepb::activitypb::{
    ActivityData, ActivityOperateReply, AutumnOperateRequest, GetGroupReply, GetGroupRequest,
    WishSignClaimReq, WishSignDrawReq, WishSignPending,
};
use crate::services::activity_text::strip_tags;

const SERVICE: &str = "gamepb.activitypb.ActivityService";
const GROUP_ID: i64 = 2_026_092_400;
const ACTIVITY_ID: i64 = 2_026_092_401;
const DRAW_OPERATE: i64 = 51;
const CLAIM_OPERATE: i64 = 52;
const TITLE: &str = "秋祈良愿";

const STATE_MISSING: &str = "活动动态状态未返回，请稍后刷新";
const WINDOW_CLOSED: &str = "活动尚未开放或已经结束";
const DRAW_UNAVAILABLE: &str = "请先领取待领取奖励，或等待明日祈愿";
const CHOICE_INVALID: &str = "请选择有效的祈愿方向";
const CLAIM_UNAVAILABLE: &str = "当前没有待领取的祈愿奖励";
const REPLY_INVALID: &str = "操作回包不完整，请刷新确认结果，勿重复提交";
const ACTION_INVALID: &str = "活动操作不匹配";

#[derive(Debug, Deserialize)]
struct AutumnFile {
    choices: Vec<ChoiceRow>,
    texts: Vec<TextRow>,
    rewards: Vec<RewardRow>,
}

#[derive(Debug, Deserialize)]
struct ChoiceRow {
    choose_id: i64,
    desc: String,
}

#[derive(Debug, Deserialize)]
struct TextRow {
    choose_id: i64,
    text_id: i64,
    desc: String,
}

#[derive(Debug, Deserialize)]
struct RewardRow {
    day_id: i64,
    reward: String,
}

struct Catalog {
    choices: Vec<ChoiceRow>,
    texts: Vec<TextRow>,
    rewards: Vec<RewardRow>,
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let file: AutumnFile = serde_json::from_str(include_str!(
            "../../../../assets/activity-data/autumn-20260924.json"
        ))
        .expect("autumn-20260924.json 解析失败");
        Catalog { choices: file.choices, texts: file.texts, rewards: file.rewards }
    })
}

/// 基准是 `begin <= now && end > now`。结束时刻本身不开放。0 不另判成过期。
#[must_use]
pub fn wish_window_is_open(begin: i64, end: i64, now: i64) -> bool {
    begin <= now && end > now
}

fn known_choice(choose_id: i64) -> bool {
    catalog().choices.iter().any(|choice| choice.choose_id == choose_id)
}

fn slip_text(choose_id: i64, text_id: i64) -> String {
    catalog()
        .texts
        .iter()
        .find(|text| text.choose_id == choose_id && text.text_id == text_id)
        .map(|text| text.desc.clone())
        .unwrap_or_default()
}

fn find_entry(entry: &ActivityData) -> Option<&ActivityData> {
    let matches =
        entry.activity.as_ref().is_some_and(|activity| activity.activity_id == ACTIVITY_ID);
    if matches {
        return Some(entry);
    }
    entry.children.iter().find_map(find_entry)
}

fn rules_from_extra(extra: &[u8]) -> Vec<String> {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(extra) else {
        return Vec::new();
    };
    value
        .pointer("/tips/txt")
        .and_then(serde_json::Value::as_array)
        .map(|lines| {
            lines.iter().filter_map(|line| line.as_str()).map(strip_tags).collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn reward_dto(item: &Item) -> serde_json::Value {
    let name = global_game_config()
        .get_item_by_id(item.id)
        .map(|entry| entry.name)
        .unwrap_or_else(|| format!("物品 #{}", item.id));
    let image = global_game_config().get_item_image_by_id(item.id).unwrap_or_default();
    serde_json::json!({
        "id": item.id,
        "count": item.count,
        "name": name,
        "image": image,
    })
}

fn rewards_dto(items: &[Item]) -> Vec<serde_json::Value> {
    items.iter().map(reward_dto).collect()
}

fn reward_spec(raw: &str) -> Option<(i64, i64)> {
    let (id, count) = raw.split_once(':')?;
    let id = id.parse().ok()?;
    let count = count.parse().ok()?;
    (id > 0 && count >= 0).then_some((id, count))
}

fn day_rewards() -> Vec<serde_json::Value> {
    catalog()
        .rewards
        .iter()
        .filter_map(|row| {
            let (id, count) = reward_spec(&row.reward)?;
            Some(serde_json::json!({
                "day": row.day_id,
                "reward": reward_dto(&Item { id, count, ..Item::default() }),
            }))
        })
        .collect()
}

fn pending_dto(pending: &WishSignPending) -> Option<serde_json::Value> {
    if pending.text_id <= 0 {
        return None;
    }
    Some(serde_json::json!({
        "chooseId": pending.choose_id,
        "textId": pending.text_id,
        "day": pending.day_id,
        "text": slip_text(pending.choose_id, pending.text_id),
        "rewards": rewards_dto(&pending.rewards),
    }))
}

fn snapshot_value(entry: &ActivityData, now: i64) -> Result<serde_json::Value> {
    let head = entry.activity.as_ref().ok_or_else(|| Error::Business(STATE_MISSING.into()))?;
    let state = entry.wish_sign.as_ref().ok_or_else(|| Error::Business(STATE_MISSING.into()))?;
    let active = wish_window_is_open(head.begin_time, head.end_time, now);
    let pending = state.pending.as_ref().and_then(pending_dto);
    let can_draw = active && pending.is_none() && state.remaining_count > 0;
    let can_claim = active && pending.is_some();
    let choices = catalog()
        .choices
        .iter()
        .map(|choice| serde_json::json!({ "id": choice.choose_id, "name": choice.desc }))
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "key": "wish",
        "id": ACTIVITY_ID.to_string(),
        "title": TITLE,
        "serverTime": now * 1000,
        "startTime": head.begin_time * 1000,
        "endTime": head.end_time * 1000,
        "active": active,
        "rules": rules_from_extra(&head.extra),
        "remaining": state.remaining_count,
        "day": state.activity_day,
        "choices": choices,
        "rewardDays": day_rewards(),
        "pending": pending,
        "canDraw": can_draw,
        "canClaim": can_claim,
    }))
}

const SESSION_MOVED: &str = "账号会话已变化，未发送操作";

pub struct AutumnWishService {
    gateway: Arc<Gateway>,
    mutation_lock: tokio::sync::Mutex<()>,
}

impl AutumnWishService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway, mutation_lock: tokio::sync::Mutex::new(()) }
    }

    pub async fn snapshot(&self) -> Result<serde_json::Value> {
        let now = crate::utils::time::get_server_time_secs();
        let entry = self.load_entry().await?;
        snapshot_value(&entry, now)
    }

    pub async fn operate(&self, action: &str, choose_id: i64) -> Result<serde_json::Value> {
        self.operate_if_current(action, choose_id, || true).await
    }

    /// 持有这把锁的调用会重新读取资格。`still_current` 在读取后、发送前再查一次。
    pub async fn operate_if_current(
        &self,
        action: &str,
        choose_id: i64,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        let operate_type = match action {
            "draw" => DRAW_OPERATE,
            "claim" => CLAIM_OPERATE,
            _ => return Err(Error::Business(ACTION_INVALID.into())),
        };
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let state = self.snapshot().await?;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let request = self.write_request(action, operate_type, choose_id, &state)?;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let reply_bytes = self
            .gateway
            .request(SERVICE, "Operate", &request.encode_to_vec())
            .await
            .map_err(Error::from)?;
        let reply = ActivityOperateReply::decode(&reply_bytes[..])?;
        let (result, rewards) = read_reply(action, operate_type, &reply)?;
        let mut payload = serde_json::json!({
            "result": result,
            "rewards": rewards_dto(&rewards),
        });
        match self.snapshot().await {
            Ok(activity) => payload["activity"] = activity,
            Err(_) => {
                payload["activity"] = serde_json::Value::Null;
                payload["refreshRequired"] = serde_json::json!(true);
            }
        }
        Ok(payload)
    }

    async fn load_entry(&self) -> Result<ActivityData> {
        let body = GetGroupRequest { group_id: GROUP_ID }.encode_to_vec();
        let reply_bytes =
            self.gateway.request(SERVICE, "GetGroup", &body).await.map_err(Error::from)?;
        let reply = GetGroupReply::decode(&reply_bytes[..])?;
        let group = reply.group.ok_or_else(|| Error::Business(STATE_MISSING.into()))?;
        find_entry(&group).cloned().ok_or_else(|| Error::Business(STATE_MISSING.into()))
    }

    fn write_request(
        &self,
        action: &str,
        operate_type: i64,
        choose_id: i64,
        state: &serde_json::Value,
    ) -> Result<AutumnOperateRequest> {
        if state.get("active").and_then(serde_json::Value::as_bool) != Some(true) {
            return Err(Error::Business(WINDOW_CLOSED.into()));
        }
        let chosen = match action {
            "draw" => {
                if state.get("canDraw").and_then(serde_json::Value::as_bool) != Some(true) {
                    return Err(Error::Business(DRAW_UNAVAILABLE.into()));
                }
                if !known_choice(choose_id) {
                    return Err(Error::Business(CHOICE_INVALID.into()));
                }
                choose_id
            }
            "claim" => {
                let pending = state.get("pending").filter(|value| !value.is_null());
                let Some(pending) = pending else {
                    return Err(Error::Business(CLAIM_UNAVAILABLE.into()));
                };
                if state.get("canClaim").and_then(serde_json::Value::as_bool) != Some(true) {
                    return Err(Error::Business(CLAIM_UNAVAILABLE.into()));
                }
                pending
                    .get("chooseId")
                    .and_then(serde_json::Value::as_i64)
                    .ok_or_else(|| Error::Business(CLAIM_UNAVAILABLE.into()))?
            }
            _ => return Err(Error::Business(ACTION_INVALID.into())),
        };
        Ok(AutumnOperateRequest {
            activity_id: ACTIVITY_ID,
            operate_type,
            wish_sign_draw: (action == "draw").then_some(WishSignDrawReq { choose_id: chosen }),
            wish_sign_claim: (action == "claim").then_some(WishSignClaimReq { choose_id: chosen }),
            ..AutumnOperateRequest::default()
        })
    }
}

fn read_reply(
    action: &str,
    operate_type: i64,
    reply: &ActivityOperateReply,
) -> Result<(serde_json::Value, Vec<Item>)> {
    if reply.activity_id != ACTIVITY_ID || reply.operate_type != operate_type {
        return Err(Error::Business(REPLY_INVALID.into()));
    }
    match action {
        "draw" => {
            let draw = reply
                .wish_sign_draw
                .as_ref()
                .ok_or_else(|| Error::Business(REPLY_INVALID.into()))?;
            Ok((
                serde_json::json!({
                    "textId": draw.text_id,
                    "dayId": draw.day_id,
                    "rewards": rewards_dto(&draw.rewards),
                }),
                draw.rewards.clone(),
            ))
        }
        "claim" => {
            let claim = reply
                .wish_sign_claim
                .as_ref()
                .ok_or_else(|| Error::Business(REPLY_INVALID.into()))?;
            Ok((serde_json::json!({ "awards": rewards_dto(&claim.awards) }), claim.awards.clone()))
        }
        _ => Err(Error::Business(ACTION_INVALID.into())),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::network::encryptor::NoopEncryptor;
    use crate::network::error::NetworkError;
    use crate::network::gateway::GatewayConfig;
    use crate::proto::generated::corepb::Item;
    use crate::proto::generated::gamepb::activitypb::{ActivityBodyWishSign, ActivityContent};

    fn gateway() -> Arc<Gateway> {
        let gateway = Arc::new(Gateway::new(
            GatewayConfig {
                server_url: "wss://gate.example.com/ws".into(),
                platform: "qq".into(),
                os: "Windows".into(),
                client_version: "1.15.0.2_20261001".into(),
                auth_code: "test".into(),
                headers: std::collections::HashMap::new(),
            },
            Arc::new(NoopEncryptor),
        ));
        gateway.mark_online();
        gateway
    }

    fn item(id: i64, count: i64) -> Item {
        Item { id, count, ..Item::default() }
    }

    fn entry(
        begin: i64,
        end: i64,
        remaining: i64,
        day: i64,
        pending: Option<WishSignPending>,
    ) -> ActivityData {
        ActivityData {
            activity: Some(ActivityContent {
                activity_id: ACTIVITY_ID,
                begin_time: begin,
                end_time: end,
                extra: r#"{"tips":{"txt":["<b>daily</b>"]}}"#.as_bytes().to_vec().into(),
                ..ActivityContent::default()
            }),
            wish_sign: Some(ActivityBodyWishSign {
                remaining_count: remaining,
                activity_day: day,
                pending,
            }),
            ..ActivityData::default()
        }
    }

    fn grouped(child: ActivityData) -> Vec<u8> {
        GetGroupReply {
            group: Some(ActivityData { children: vec![child], ..ActivityData::default() }),
        }
        .encode_to_vec()
    }

    struct Script {
        calls: Mutex<Vec<(String, String, Vec<u8>)>>,
        gets: Mutex<Vec<Vec<u8>>>,
        operate: Mutex<Option<Vec<u8>>>,
        fail_operate: Mutex<bool>,
        fail_refresh: Mutex<bool>,
    }

    fn install(gateway: &Gateway, script: Arc<Script>) {
        gateway.install_reply_script(move |service, method, body| {
            script.calls.lock().expect("calls").push((
                service.to_owned(),
                method.to_owned(),
                body.to_vec(),
            ));
            match method {
                "GetGroup" => {
                    let gets = script.gets.lock().expect("gets");
                    let index = script
                        .calls
                        .lock()
                        .expect("calls")
                        .iter()
                        .filter(|call| call.1 == "GetGroup")
                        .count()
                        - 1;
                    if *script.fail_refresh.lock().expect("refresh") && index > 0 {
                        return Err(NetworkError::Phase("refresh failed".into()));
                    }
                    Ok(gets.get(index).or_else(|| gets.last()).cloned().unwrap_or_default())
                }
                "Operate" => {
                    if *script.fail_operate.lock().expect("operate") {
                        return Err(NetworkError::Timeout {
                            client_seq: 1,
                            service_name: service.to_owned(),
                            method_name: method.to_owned(),
                            pending: 1,
                        });
                    }
                    script
                        .operate
                        .lock()
                        .expect("body")
                        .clone()
                        .ok_or_else(|| NetworkError::Phase("missing operate reply".into()))
                }
                _ => Err(NetworkError::Phase(method.into())),
            }
        });
    }

    fn script_with(gets: Vec<Vec<u8>>, operate: Option<Vec<u8>>) -> Arc<Script> {
        Arc::new(Script {
            calls: Mutex::new(Vec::new()),
            gets: Mutex::new(gets),
            operate: Mutex::new(operate),
            fail_operate: Mutex::new(false),
            fail_refresh: Mutex::new(false),
        })
    }

    fn now() -> i64 {
        crate::utils::time::get_server_time_secs()
    }

    #[test]
    fn window_matches_the_source_comparison() {
        assert!(wish_window_is_open(10, 20, 10));
        assert!(wish_window_is_open(10, 20, 19));
        assert!(!wish_window_is_open(10, 20, 20));
        assert!(!wish_window_is_open(10, 20, 9));
        assert!(!wish_window_is_open(0, 0, 1_700_000_000));
        assert!(wish_window_is_open(0, 20, 10));
        assert!(!wish_window_is_open(10, 0, 15));
    }

    #[tokio::test]
    async fn snapshot_reads_the_group_and_keeps_zero_times_closed() {
        let _data = crate::config::paths::TestDataDir::enter();
        let cases = [
            (now() - 86_400, now() + 86_400, true, true),
            (now() + 86_400, now() + 172_800, false, false),
            (now() - 86_400, now(), false, false),
            (now() - 172_800, now() - 86_400, false, false),
            (0, 0, false, false),
            (0, now() + 86_400, true, true),
        ];
        for (begin, end, active, can_draw) in cases {
            let gateway = gateway();
            let script = script_with(vec![grouped(entry(begin, end, 1, 3, None))], None);
            install(&gateway, Arc::clone(&script));
            let value = AutumnWishService::new(gateway).snapshot().await.unwrap();
            let calls = script.calls.lock().expect("calls");
            assert_eq!(calls[0].0, SERVICE);
            assert_eq!(calls[0].1, "GetGroup");
            let request = GetGroupRequest::decode(calls[0].2.as_slice()).unwrap();
            assert_eq!(request.group_id, GROUP_ID);
            assert_eq!(value["active"], active);
            assert_eq!(value["canDraw"], can_draw);
            assert_eq!(value["id"], ACTIVITY_ID.to_string());
            assert_eq!(value["day"], 3);
            assert_eq!(value["remaining"], 1);
            assert_eq!(value["rules"][0], "daily");
            assert!(value["pending"].is_null());
            assert_eq!(value["choices"][0]["name"], "财运");
            assert_eq!(value["rewardDays"][0]["reward"]["id"], 6001);
            assert_eq!(value["rewardDays"][0]["reward"]["count"], 20);
        }
    }

    #[tokio::test]
    async fn pending_slip_uses_the_catalog_text_and_blocks_another_draw() {
        let _data = crate::config::paths::TestDataDir::enter();
        let gateway = gateway();
        let pending =
            WishSignPending { choose_id: 1, text_id: 1, day_id: 3, rewards: vec![item(1002, 5)] };
        let script = script_with(
            vec![grouped(entry(now() - 10, now() + 86_400, 1, 3, Some(pending)))],
            None,
        );
        install(&gateway, Arc::clone(&script));
        let value = AutumnWishService::new(gateway).snapshot().await.unwrap();
        assert_eq!(value["canDraw"], false);
        assert_eq!(value["canClaim"], true);
        assert_eq!(value["pending"]["text"], "鸿运将至\n所行皆顺");
        assert_eq!(value["pending"]["rewards"][0]["id"], 1002);
        assert_eq!(value["pending"]["rewards"][0]["count"], 5);
    }

    #[tokio::test]
    async fn zero_text_pending_is_not_a_slip() {
        let _data = crate::config::paths::TestDataDir::enter();
        let gateway = gateway();
        let pending = WishSignPending { choose_id: 1, text_id: 0, day_id: 1, rewards: vec![] };
        let script =
            script_with(vec![grouped(entry(now() - 10, now() + 100, 1, 1, Some(pending)))], None);
        install(&gateway, script);
        let value = AutumnWishService::new(gateway).snapshot().await.unwrap();
        assert!(value["pending"].is_null());
        assert_eq!(value["canDraw"], true);
        assert_eq!(value["canClaim"], false);
    }

    #[tokio::test]
    async fn missing_state_does_not_become_a_drawable_board() {
        let _data = crate::config::paths::TestDataDir::enter();
        let gateway = gateway();
        let bare = ActivityData {
            activity: Some(ActivityContent {
                activity_id: ACTIVITY_ID,
                begin_time: now() - 10,
                end_time: now() + 100,
                ..ActivityContent::default()
            }),
            ..ActivityData::default()
        };
        let script = script_with(vec![grouped(bare)], None);
        install(&gateway, Arc::clone(&script));
        let error = AutumnWishService::new(gateway).snapshot().await.unwrap_err();
        assert!(error.to_string().contains(STATE_MISSING), "{error}");
        assert!(script.calls.lock().expect("calls").iter().all(|call| call.1 != "Operate"));
    }

    #[tokio::test]
    async fn draw_sends_the_choice_and_refreshes_from_the_reply() {
        let _data = crate::config::paths::TestDataDir::enter();
        let open = entry(now() - 10, now() + 86_400, 1, 3, None);
        let pending =
            WishSignPending { choose_id: 5, text_id: 1, day_id: 3, rewards: vec![item(1002, 5)] };
        let after = entry(now() - 10, now() + 86_400, 0, 3, Some(pending));
        let reply = ActivityOperateReply {
            activity_id: ACTIVITY_ID,
            operate_type: DRAW_OPERATE,
            wish_sign_draw: Some(crate::proto::generated::gamepb::activitypb::WishSignDrawRsp {
                text_id: 1,
                day_id: 3,
                rewards: vec![item(1002, 5)],
            }),
            ..ActivityOperateReply::default()
        };
        let gateway = gateway();
        let script = script_with(vec![grouped(open), grouped(after)], Some(reply.encode_to_vec()));
        install(&gateway, Arc::clone(&script));
        let value = AutumnWishService::new(gateway).operate("draw", 5).await.unwrap();
        let calls = script.calls.lock().expect("calls");
        assert_eq!(
            calls.iter().map(|call| call.1.as_str()).collect::<Vec<_>>(),
            ["GetGroup", "Operate", "GetGroup"]
        );
        let request = AutumnOperateRequest::decode(calls[1].2.as_slice()).unwrap();
        assert_eq!(request.activity_id, ACTIVITY_ID);
        assert_eq!(request.operate_type, DRAW_OPERATE);
        assert_eq!(request.wish_sign_draw.unwrap().choose_id, 5);
        assert!(request.wish_sign_claim.is_none());
        assert_eq!(value["rewards"][0]["count"], 5);
        assert_eq!(value["activity"]["canClaim"], true);
        assert_eq!(value["activity"]["pending"]["text"], "顺应天时");
        assert!(value.get("refreshRequired").is_none());
    }

    #[tokio::test]
    async fn ineligible_draws_do_not_send_operate() {
        let _data = crate::config::paths::TestDataDir::enter();
        let samples = [
            entry(now() + 100, now() + 200, 1, 1, None),
            entry(now() - 100, now(), 1, 1, None),
            entry(0, 0, 1, 1, None),
            entry(now() - 100, now() + 100, 0, 1, None),
        ];
        for sample in samples {
            let gateway = gateway();
            let script = script_with(vec![grouped(sample)], None);
            install(&gateway, Arc::clone(&script));
            let error = AutumnWishService::new(gateway).operate("draw", 1).await.unwrap_err();
            assert!(
                error.to_string().contains(WINDOW_CLOSED)
                    || error.to_string().contains(DRAW_UNAVAILABLE),
                "{error}"
            );
            assert!(script.calls.lock().expect("calls").iter().all(|call| call.1 != "Operate"));
        }
        let choice_gateway = gateway();
        let script = script_with(vec![grouped(entry(now() - 10, now() + 100, 1, 1, None))], None);
        install(&choice_gateway, Arc::clone(&script));
        let error = AutumnWishService::new(choice_gateway).operate("draw", 99).await.unwrap_err();
        assert!(error.to_string().contains(CHOICE_INVALID), "{error}");
        assert!(script.calls.lock().expect("calls").iter().all(|call| call.1 != "Operate"));
    }

    #[tokio::test]
    async fn claim_sends_the_pending_choice() {
        let _data = crate::config::paths::TestDataDir::enter();
        let pending =
            WishSignPending { choose_id: 2, text_id: 4, day_id: 2, rewards: vec![item(6001, 20)] };
        let open = entry(now() - 10, now() + 100, 0, 2, Some(pending));
        let reply = ActivityOperateReply {
            activity_id: ACTIVITY_ID,
            operate_type: CLAIM_OPERATE,
            wish_sign_claim: Some(crate::proto::generated::gamepb::activitypb::WishSignClaimRsp {
                awards: vec![item(6001, 20)],
            }),
            ..ActivityOperateReply::default()
        };
        let gateway = gateway();
        let script = script_with(
            vec![grouped(open.clone()), grouped(entry(now() - 10, now() + 100, 0, 2, None))],
            Some(reply.encode_to_vec()),
        );
        install(&gateway, Arc::clone(&script));
        let value = AutumnWishService::new(gateway).operate("claim", 1).await.unwrap();
        let calls = script.calls.lock().expect("calls");
        let request = AutumnOperateRequest::decode(calls[1].2.as_slice()).unwrap();
        assert_eq!(request.operate_type, CLAIM_OPERATE);
        assert_eq!(request.wish_sign_claim.unwrap().choose_id, 2);
        assert!(request.wish_sign_draw.is_none());
        assert_eq!(value["rewards"][0]["id"], 6001);
        assert_eq!(value["activity"]["canDraw"], false);
    }

    fn claim_reply() -> Vec<u8> {
        ActivityOperateReply {
            activity_id: ACTIVITY_ID,
            operate_type: CLAIM_OPERATE,
            wish_sign_claim: Some(crate::proto::generated::gamepb::activitypb::WishSignClaimRsp {
                awards: vec![item(6001, 20)],
            }),
            ..ActivityOperateReply::default()
        }
        .encode_to_vec()
    }

    async fn claim_choice(choose_id: i64, caller: i64) -> i64 {
        let pending =
            WishSignPending { choose_id, text_id: 1, day_id: 1, rewards: vec![item(6001, 20)] };
        let open = entry(now() - 10, now() + 100, 0, 1, Some(pending));
        let gateway = gateway();
        let script = script_with(vec![grouped(open.clone()), grouped(open)], Some(claim_reply()));
        install(&gateway, Arc::clone(&script));
        let value = AutumnWishService::new(gateway).operate("claim", caller).await.unwrap();
        assert_eq!(value["rewards"][0]["count"], 20);
        let calls = script.calls.lock().expect("calls");
        let request = AutumnOperateRequest::decode(calls[1].2.as_slice()).unwrap();
        request.wish_sign_claim.unwrap().choose_id
    }

    #[tokio::test]
    async fn claim_sends_the_server_choice_including_zero_and_unknown_ids() {
        let _data = crate::config::paths::TestDataDir::enter();
        assert_eq!(claim_choice(5, 1).await, 5);
        assert_eq!(claim_choice(0, 1).await, 0);
        assert_eq!(claim_choice(99, 4).await, 99);
    }

    #[tokio::test]
    async fn claim_without_a_slip_or_outside_the_window_does_not_send() {
        let _data = crate::config::paths::TestDataDir::enter();
        let open_gateway = gateway();
        let script = script_with(vec![grouped(entry(now() - 10, now() + 100, 1, 1, None))], None);
        install(&open_gateway, Arc::clone(&script));
        let error = AutumnWishService::new(open_gateway).operate("claim", 1).await.unwrap_err();
        assert!(error.to_string().contains(CLAIM_UNAVAILABLE), "{error}");
        assert!(script.calls.lock().expect("calls").iter().all(|call| call.1 != "Operate"));

        let pending = WishSignPending { choose_id: 5, text_id: 1, day_id: 1, rewards: vec![] };
        let closed_gateway = gateway();
        let script = script_with(vec![grouped(entry(0, 0, 0, 1, Some(pending)))], None);
        install(&closed_gateway, Arc::clone(&script));
        let error = AutumnWishService::new(closed_gateway).operate("claim", 5).await.unwrap_err();
        assert!(error.to_string().contains(WINDOW_CLOSED), "{error}");
        assert!(script.calls.lock().expect("calls").iter().all(|call| call.1 != "Operate"));
    }

    #[tokio::test]
    async fn failed_or_incomplete_writes_do_not_retry_or_invent_rewards() {
        let _data = crate::config::paths::TestDataDir::enter();
        let open = || entry(now() - 10, now() + 100, 1, 1, None);
        let first = gateway();
        let script = script_with(vec![grouped(open())], None);
        *script.fail_operate.lock().expect("flag") = true;
        install(&first, Arc::clone(&script));
        let error = AutumnWishService::new(first).operate("draw", 1).await.unwrap_err();
        assert!(error.to_string().contains("请求超时"), "{error}");
        assert_eq!(
            script.calls.lock().expect("calls").iter().filter(|call| call.1 == "Operate").count(),
            1
        );

        let second = gateway();
        let reply = ActivityOperateReply {
            activity_id: ACTIVITY_ID,
            operate_type: DRAW_OPERATE,
            ..ActivityOperateReply::default()
        };
        let script = script_with(vec![grouped(open())], Some(reply.encode_to_vec()));
        install(&second, Arc::clone(&script));
        let error = AutumnWishService::new(second).operate("draw", 1).await.unwrap_err();
        assert!(error.to_string().contains(REPLY_INVALID), "{error}");
        assert_eq!(
            script.calls.lock().expect("calls").iter().filter(|call| call.1 == "GetGroup").count(),
            1
        );

        let third = gateway();
        let reply = ActivityOperateReply {
            activity_id: ACTIVITY_ID,
            operate_type: DRAW_OPERATE,
            wish_sign_draw: Some(crate::proto::generated::gamepb::activitypb::WishSignDrawRsp {
                text_id: 1,
                day_id: 1,
                rewards: vec![item(1002, 1)],
            }),
            ..ActivityOperateReply::default()
        };
        let script = script_with(vec![grouped(open())], Some(reply.encode_to_vec()));
        *script.fail_refresh.lock().expect("flag") = true;
        install(&third, Arc::clone(&script));
        let value = AutumnWishService::new(third).operate("draw", 1).await.unwrap();
        assert_eq!(value["refreshRequired"], true);
        assert!(value["activity"].is_null());
        assert_eq!(value["rewards"][0]["count"], 1);
        assert_eq!(
            script.calls.lock().expect("calls").iter().filter(|call| call.1 == "Operate").count(),
            1
        );
    }

    #[tokio::test]
    async fn a_second_draw_waits_and_does_not_send_another_operate() {
        let _data = crate::config::paths::TestDataDir::enter();
        let pending =
            WishSignPending { choose_id: 1, text_id: 1, day_id: 1, rewards: vec![item(1002, 1)] };
        let reply = ActivityOperateReply {
            activity_id: ACTIVITY_ID,
            operate_type: DRAW_OPERATE,
            wish_sign_draw: Some(crate::proto::generated::gamepb::activitypb::WishSignDrawRsp {
                text_id: 1,
                day_id: 1,
                rewards: vec![item(1002, 1)],
            }),
            ..ActivityOperateReply::default()
        };
        let gateway = gateway();
        let script = script_with(
            vec![
                grouped(entry(now() - 10, now() + 100, 1, 1, None)),
                grouped(entry(now() - 10, now() + 100, 0, 1, Some(pending))),
            ],
            Some(reply.encode_to_vec()),
        );
        install(&gateway, Arc::clone(&script));
        let service = Arc::new(AutumnWishService::new(gateway));
        let first = service.clone();
        let second = service.clone();
        let (left, right) = tokio::join!(first.operate("draw", 1), second.operate("draw", 1));
        let results = [left.is_ok(), right.is_ok()];
        assert_eq!(results.iter().filter(|ok| **ok).count(), 1);
        assert_eq!(
            script.calls.lock().expect("calls").iter().filter(|call| call.1 == "Operate").count(),
            1
        );
    }

    #[test]
    fn official_wish_vectors_round_trip_operate_bodies() {
        let vectors: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("fixtures/autumn-official-vectors.json")).unwrap();
        let mut wish = 0;
        for vector in vectors {
            if vector["expected"]["activity_id"] != "2026092401" {
                continue;
            }
            wish += 1;
            let bytes = decode_hex(vector["hex"].as_str().unwrap());
            if vector["direction"] == "send" {
                let decoded = AutumnOperateRequest::decode(bytes.as_slice()).unwrap();
                assert_eq!(decoded.encode_to_vec(), bytes);
                assert_eq!(decoded.activity_id, ACTIVITY_ID);
            } else {
                let decoded = ActivityOperateReply::decode(bytes.as_slice()).unwrap();
                assert_eq!(decoded.encode_to_vec(), bytes);
                assert_eq!(decoded.activity_id, ACTIVITY_ID);
            }
        }
        assert_eq!(wish, 4);
    }

    fn decode_hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect()
    }

    #[tokio::test]
    async fn unknown_action_does_not_touch_the_gateway() {
        let gateway = gateway();
        let script = script_with(vec![], None);
        install(&gateway, Arc::clone(&script));
        let error = AutumnWishService::new(gateway).operate("share", 1).await.unwrap_err();
        assert!(error.to_string().contains(ACTION_INVALID), "{error}");
        assert!(script.calls.lock().expect("calls").is_empty());
    }
}
