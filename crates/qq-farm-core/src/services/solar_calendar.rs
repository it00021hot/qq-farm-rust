//! 节令日历（bot `core/src/services/solar-calendar.ts`，0533264）。
//!
//! 协议：`gamepb.solartermspb.SolarTermsV2Service` 的
//! `GetSolarTermsV2` / `ClaimSolarTermsV2` / `AnswerSolarTerms` / `ClaimSolarSeason`。
//! 数值配置镜像 bot `activity-data/solar-calendar.json`（节令文案 / 题目 / 季度）。

use std::sync::{Arc, OnceLock};

use prost::Message as _;
use serde::Deserialize;

use crate::config::game_config::global as global_game_config;
use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item;
use crate::proto::generated::gamepb::solartermspb::{
    AnswerSolarTermsReply, AnswerSolarTermsRequest, ClaimSolarSeasonReply, ClaimSolarSeasonRequest,
    ClaimSolarTermsV2Reply, ClaimSolarTermsV2Request, GetSolarTermsV2Reply, GetSolarTermsV2Request,
    SolarSeasonInfo, SolarTermsEventInfo,
};
use crate::services::activity_text::strip_html;

const SERVICE: &str = "gamepb.solartermspb.SolarTermsV2Service";

#[derive(Debug, Deserialize)]
struct SolarCalendarConfig {
    events: Vec<SolarEventConfig>,
    seasons: Vec<SolarSeasonConfig>,
}

#[derive(Debug, Deserialize)]
struct SolarEventConfig {
    id: i64,
    name: Option<String>,
    question: Option<String>,
    answer: Option<String>,
    error_tips: Option<String>,
    help: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SolarSeasonConfig {
    id: i64,
    year: Option<String>,
    name: Option<String>,
    description: Option<String>,
}

fn solar_config() -> &'static SolarCalendarConfig {
    static CONFIG: OnceLock<SolarCalendarConfig> = OnceLock::new();
    CONFIG.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../assets/activity-data/solar-calendar.json"))
            .expect("solar-calendar.json 解析失败")
    })
}

/// 从 `help` JSON 提取 `tips.txt` 文本行（缺失/损坏时为空白，对齐 bot 容错）。
fn rule_lines(help: Option<&str>) -> Vec<String> {
    let Some(help) = help else { return Vec::new() };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(help) else {
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

/// 奖励 DTO：`{ id, count, name, image }`（对齐 bot `rewards()`）。
fn rewards(items: &[Item]) -> Vec<serde_json::Value> {
    let gc = global_game_config();
    items
        .iter()
        .map(|item| {
            let name = gc
                .get_item_by_id(item.id)
                .map(|entry| entry.name)
                .unwrap_or_else(|| format!("物品{}", item.id));
            serde_json::json!({
                "id": item.id,
                "count": item.count,
                "name": name,
                "image": gc.get_item_image_by_id(item.id).unwrap_or_default(),
            })
        })
        .collect()
}

fn event_dto(value: &SolarTermsEventInfo, now: i64) -> serde_json::Value {
    let id = value.id;
    let entry = solar_config().events.iter().find(|entry| entry.id == id);
    let start_time = value.open_start_time;
    let end_time = value.open_end_time;
    let open = value.window_status == 2 && now >= start_time && now <= end_time;
    let question = entry.and_then(|entry| entry.question.clone()).unwrap_or_default();
    let options: Vec<String> = match entry.and_then(|entry| entry.answer.as_deref()) {
        Some(answer) if !answer.is_empty() => {
            answer.split(';').map(str::to_string).collect::<Vec<_>>()
        }
        _ => Vec::new(),
    };
    serde_json::json!({
        "id": id,
        "name": entry.and_then(|entry| entry.name.clone()).unwrap_or_else(|| format!("节令 {id}")),
        "description": entry.and_then(|entry| entry.description.clone()).unwrap_or_default(),
        "startTime": start_time,
        "endTime": end_time,
        "status": value.window_status,
        "directClaimed": value.direct_claimed,
        "quizClaimed": value.quiz_claimed,
        "quizWrongToday": value.quiz_wrong_today,
        "wrongOptions": value.wrong_options,
        "question": question,
        "options": options,
        "errorTips": entry.and_then(|entry| entry.error_tips.clone()).unwrap_or_default(),
        "rules": rule_lines(entry.and_then(|entry| entry.help.as_deref())),
        "rewards": rewards(&value.rewards),
        "questionRewards": rewards(&value.question_rewards),
        "canClaim": open && !value.direct_claimed,
        "canAnswer": open && !question.is_empty() && !value.quiz_claimed && !value.quiz_wrong_today,
    })
}

fn season_dto(value: &SolarSeasonInfo) -> serde_json::Value {
    let entry = solar_config().seasons.iter().find(|entry| entry.id == value.id);
    let can_claim = value.reward_claimable && !value.reward_claimed && !value.rewards.is_empty();
    serde_json::json!({
        "id": value.id,
        "name": entry.and_then(|entry| entry.name.clone()).unwrap_or_default(),
        "year": entry.and_then(|entry| entry.year.clone()).unwrap_or_default(),
        "description": entry.and_then(|entry| entry.description.clone()).unwrap_or_default(),
        "eventIds": value.event_ids,
        "litCount": value.lit_count,
        "totalCount": value.total_count,
        "claimed": value.reward_claimed,
        "canClaim": can_claim,
        "rewards": rewards(&value.rewards),
    })
}

/// JSON 数值近似 bot `Number()`：数字（含整数值浮点）与数字字符串都接受。
fn json_i64(value: Option<&serde_json::Value>) -> Option<i64> {
    match value? {
        serde_json::Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().filter(|f| f.fract() == 0.).map(|f| f as i64)),
        serde_json::Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// 节令 / 季度操作入口的纯门控（可离线测试）。
fn event_action_target(
    state: &serde_json::Value,
    id: i64,
    quiz: bool,
) -> Result<serde_json::Value> {
    let event = state
        .get("events")
        .and_then(serde_json::Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.get("id").and_then(serde_json::Value::as_i64) == Some(id))
        })
        .cloned();
    let allowed = event
        .as_ref()
        .and_then(|event| {
            event
                .get(if quiz { "canAnswer" } else { "canClaim" })
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(false);
    if !allowed {
        return Err(Error::Business("节令尚未开放、已结束或今日操作已完成".into()));
    }
    Ok(event.unwrap_or(serde_json::Value::Null))
}

fn season_action_target(state: &serde_json::Value, id: i64) -> Result<serde_json::Value> {
    let can_claim = state
        .get("seasons")
        .and_then(serde_json::Value::as_array)
        .and_then(|seasons| {
            seasons
                .iter()
                .find(|season| season.get("id").and_then(serde_json::Value::as_i64) == Some(id))
        })
        .and_then(|season| season.get("canClaim").and_then(serde_json::Value::as_bool))
        .unwrap_or(false);
    if !can_claim {
        return Err(Error::Business("当前季度奖励不可领取".into()));
    }
    Ok(serde_json::Value::Null)
}

/// 答题参数校验（对齐 bot：整数下标、未选过已答错项、题目一致）。
fn answer_option_index(input: &serde_json::Value, event: &serde_json::Value) -> Result<i64> {
    let options =
        event.get("options").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
    let index = input.get("optionIndex").and_then(serde_json::Value::as_i64);
    let wrong = event
        .get("wrongOptions")
        .and_then(serde_json::Value::as_array)
        .map(|values| values.iter().filter_map(serde_json::Value::as_i64).collect::<Vec<_>>())
        .unwrap_or_default();
    let Some(index) = index
        .filter(|index| *index >= 0 && (*index as usize) < options.len() && !wrong.contains(index))
    else {
        return Err(Error::Business("请选择有效答案".into()));
    };
    let question = event.get("question").and_then(serde_json::Value::as_str).unwrap_or("");
    let expected = input.get("expectedQuestion").and_then(serde_json::Value::as_str).unwrap_or("");
    if expected != question {
        return Err(Error::Business("题目已变化，请刷新后重新选择".into()));
    }
    Ok(index)
}

/// 写回包必须携带与请求一致的 updated 实体（对齐 bot 的「操作回包不完整」校验）。
fn validate_updated_id(updated: Option<i64>, id: i64) -> Result<()> {
    if updated != Some(id) {
        return Err(Error::Business("操作回包不完整，请刷新确认结果，勿重复提交".into()));
    }
    Ok(())
}

pub struct SolarCalendarService {
    gateway: Arc<Gateway>,
    /// 写操作串行化（对齐 bot `tail` 链）
    mutation_lock: tokio::sync::Mutex<()>,
}

impl SolarCalendarService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway, mutation_lock: tokio::sync::Mutex::new(()) }
    }

    async fn rpc(&self, method: &str, body: Vec<u8>) -> Result<Vec<u8>> {
        self.gateway.request(SERVICE, method, &body).await.map_err(Error::from)
    }

    /// 节令日历快照（对齐 bot `getSolarCalendar`）。
    pub async fn get_calendar(&self) -> Result<serde_json::Value> {
        let body = GetSolarTermsV2Request {}.encode_to_vec();
        let reply_bytes = self.rpc("GetSolarTermsV2", body).await?;
        let reply = GetSolarTermsV2Reply::decode(&reply_bytes[..])?;
        // prost 无标量 presence，无法复刻 protobufjs 的 Object.hasOwn：
        // 以 server_time > 0 近似「回包包含服务器时间」的完整性校验。
        if reply.server_time <= 0 {
            return Err(Error::Business("节令日历回包不完整".into()));
        }
        let now = reply.server_time;
        Ok(serde_json::json!({
            "serverTime": now,
            "defaultEventId": reply.default_event_id,
            "events": reply.events.iter().map(|event| event_dto(event, now)).collect::<Vec<_>>(),
            "seasons": reply.seasons.iter().map(season_dto).collect::<Vec<_>>(),
        }))
    }

    /// 节令操作：claim / answer / season（对齐 bot `operateSolarCalendar`）。
    pub async fn operate(
        &self,
        action: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        if !["claim", "answer", "season"].contains(&action) {
            return Err(Error::Business("未知的节令操作".into()));
        }
        let id = json_i64(input.get("id")).filter(|id| *id > 0);
        let Some(id) = id else {
            return Err(Error::Business("节令编号无效".into()));
        };
        let _guard = self.mutation_lock.lock().await;
        let state = self.get_calendar().await?;
        let mut result = match action {
            "season" => {
                season_action_target(&state, id)?;
                let body = ClaimSolarSeasonRequest { id }.encode_to_vec();
                let bytes = self.rpc("ClaimSolarSeason", body).await?;
                let reply = ClaimSolarSeasonReply::decode(&bytes[..])?;
                validate_updated_id(reply.updated_season.as_ref().map(|season| season.id), id)?;
                serde_json::json!({ "rewards": rewards(&reply.rewards) })
            }
            "answer" => {
                let event = event_action_target(&state, id, true)?;
                let option_index = answer_option_index(input, &event)?;
                let body = AnswerSolarTermsRequest { id, option_index: option_index as i32 }
                    .encode_to_vec();
                let bytes = self.rpc("AnswerSolarTerms", body).await?;
                let reply = AnswerSolarTermsReply::decode(&bytes[..])?;
                validate_updated_id(reply.updated_event.as_ref().map(|event| event.id), id)?;
                serde_json::json!({
                    "rewards": rewards(&reply.rewards),
                    "correct": reply.correct,
                })
            }
            _ => {
                let _event = event_action_target(&state, id, false)?;
                let body = ClaimSolarTermsV2Request { id }.encode_to_vec();
                let bytes = self.rpc("ClaimSolarTermsV2", body).await?;
                let reply = ClaimSolarTermsV2Reply::decode(&bytes[..])?;
                validate_updated_id(reply.updated_event.as_ref().map(|event| event.id), id)?;
                serde_json::json!({ "rewards": rewards(&reply.rewards) })
            }
        };
        match self.get_calendar().await {
            Ok(calendar) => {
                result["calendar"] = calendar;
            }
            Err(_) => {
                result["calendar"] = serde_json::Value::Null;
                result["refreshRequired"] = serde_json::json!(true);
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_lines_extract_tips_without_tags() {
        let rules = rule_lines(Some(
            r#"{"tips": {"title": "节令说明", "txt": ["<b>【节令奖励】</b>", "答对节令问题后领取额外奖励。<br/>", "端午：6月19日 ~ 6月25日。"]}}"#,
        ));
        assert_eq!(rules[0], "【节令奖励】");
        assert_eq!(rules[1], "答对节令问题后领取额外奖励。\n");
        assert_eq!(rules.last().unwrap(), "端午：6月19日 ~ 6月25日。");
        assert!(rule_lines(Some("not json")).is_empty());
        assert!(rule_lines(None).is_empty());
    }

    #[test]
    fn event_dto_gates_claim_and_answer_like_bot() {
        crate::config::game_config::global().load();
        let now = 1_792_252_900;
        let event = SolarTermsEventInfo {
            id: 401,
            window_status: 2,
            open_start_time: now - 100,
            open_end_time: now + 86_400,
            rewards: vec![Item { id: 1002, count: 200, ..Default::default() }],
            question_rewards: vec![Item { id: 1002, count: 100, ..Default::default() }],
            ..Default::default()
        };
        let dto = event_dto(&event, now);
        assert_eq!(dto["name"], "重阳");
        assert!(dto["description"].as_str().unwrap().contains("农历 九月初九"));
        assert_eq!(dto["options"][0], "A.敬老节");
        assert_eq!(dto["canClaim"], true);
        assert_eq!(dto["canAnswer"], true);
        assert!(dto.get("answer_index").is_none(), "不得下发答案索引");

        for status in [0, 1, 3] {
            let closed = SolarTermsEventInfo { window_status: status, ..event.clone() };
            let dto = event_dto(&closed, now);
            assert_eq!(dto["canClaim"], false, "status {status} 不可领取");
            assert_eq!(dto["canAnswer"], false, "status {status} 不可答题");
        }
        let claimed = SolarTermsEventInfo { direct_claimed: true, ..event.clone() };
        assert_eq!(event_dto(&claimed, now)["canClaim"], false);
        let wrong_today = SolarTermsEventInfo { quiz_wrong_today: true, ..event.clone() };
        assert_eq!(event_dto(&wrong_today, now)["canAnswer"], false);
        let quiz_done = SolarTermsEventInfo { quiz_claimed: true, ..event };
        assert_eq!(event_dto(&quiz_done, now)["canAnswer"], false);
    }

    #[test]
    fn event_action_target_rejects_closed_and_unknown() {
        let closed =
            serde_json::json!({"events": [{"id": 401, "canClaim": false, "canAnswer": false}]});
        assert!(event_action_target(&closed, 401, false).is_err());
        assert!(event_action_target(&closed, 999, false).is_err());
        let open =
            serde_json::json!({"events": [{"id": 401, "canClaim": true, "canAnswer": true}]});
        assert!(event_action_target(&open, 401, false).is_ok());
        assert!(event_action_target(&open, 401, true).is_ok());
    }

    #[test]
    fn answer_option_index_validates_like_bot() {
        let event = serde_json::json!({
            "question": "重阳节又被称为什么节？",
            "options": ["A", "B", "C", "D"],
            "wrongOptions": [1]
        });
        let question = "重阳节又被称为什么节？";
        let input = serde_json::json!({
            "id": 401, "expectedQuestion": question, "optionIndex": 0
        });
        assert_eq!(answer_option_index(&input, &event).unwrap(), 0);
        let stale = serde_json::json!({
            "id": 401, "expectedQuestion": "stale", "optionIndex": 0
        });
        assert!(answer_option_index(&stale, &event)
            .unwrap_err()
            .to_string()
            .contains("题目已变化"));
        for bad in [
            serde_json::json!(null),
            serde_json::json!(false),
            serde_json::json!("0"),
            serde_json::json!(0.5),
            serde_json::json!(4),
            serde_json::json!(1),
        ] {
            let input = serde_json::json!({"expectedQuestion": question, "optionIndex": bad});
            let error = answer_option_index(&input, &event).unwrap_err();
            assert!(error.to_string().contains("请选择有效答案"), "{bad}");
        }
    }

    #[test]
    fn season_action_target_requires_claimable() {
        let state = serde_json::json!({
            "seasons": [{"id": 4, "canClaim": false}, {"id": 5, "canClaim": true}]
        });
        assert!(season_action_target(&state, 4).is_err());
        assert!(season_action_target(&state, 5).is_ok());
        assert!(season_action_target(&state, 99).is_err());
    }

    /// 未知操作 / 非法编号在触网前被拒绝（对齐 bot 的入参校验顺序）。
    #[tokio::test]
    async fn operate_rejects_invalid_action_and_id_before_network() {
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
        let service = SolarCalendarService::new(gateway);
        assert!(service
            .operate("unknown", &serde_json::json!({"id": 4}))
            .await
            .unwrap_err()
            .to_string()
            .contains("未知的节令操作"));
        for bad_id in [serde_json::json!(null), serde_json::json!("abc"), serde_json::json!(-1)] {
            assert!(service
                .operate("claim", &serde_json::json!({"id": bad_id}))
                .await
                .unwrap_err()
                .to_string()
                .contains("节令编号无效"));
        }
    }
}
