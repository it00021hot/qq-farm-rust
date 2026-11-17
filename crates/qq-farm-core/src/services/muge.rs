//! 牧歌初启（bot `muge-activity.ts`，活动组 2026111700）。

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use prost::Message as _;
use serde_json::{json, Value};

use crate::config::game_config::global as global_game_config;
use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item;
use crate::proto::generated::gamepb::activitypb::{
    ActivityListRequest, MugeActivity, MugeEmpty, MugeListReply, MugeOperateReply,
    MugeOperateRequest, MugeSurveySaveRequest, MugeSurveyShareRequest, MugeTaskClaimRequest,
    MugeTier, MugeTierClaimReply, MugeTierClaimRequest,
};

const SERVICE: &str = "gamepb.activitypb.ActivityService";
const GROUP_ID: i64 = 2026111700;
const CELEBRATION: i64 = 2026111702;
const FRIENDS: i64 = 2026111703;
const HANDBOOK: i64 = 2026111704;
const PERSONALITY: i64 = 2026111705;

pub struct MugeService {
    gateway: Arc<Gateway>,
}

impl MugeService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway }
    }

    pub async fn snapshot(&self) -> Result<Value> {
        normalize(&self.read().await?, crate::utils::time::get_server_time_secs())
    }

    pub async fn operate(&self, action: &str, input: &Value) -> Result<Value> {
        let Some(operation) = operation(action) else {
            return Err(Error::Business("未知的牧歌初启操作".into()));
        };
        let state = self.snapshot().await?;
        if state.get("active").and_then(Value::as_bool) != Some(true) {
            return Err(Error::Business("牧歌初启尚未开始或已经结束".into()));
        }
        let request = build_request(action, operation, &state, input)?;
        let body = self.gateway.request(SERVICE, "Operate", &request.encode_to_vec()).await?;
        let reply = MugeOperateReply::decode(body.as_slice())?;
        if !reply_confirmed(&reply, operation, action, input) {
            return Err(Error::Business("操作回包未能确认，请刷新核对，勿重复提交".into()));
        }
        let rewards = reward_rows(&reply_awards(&reply, action));
        match self.snapshot().await {
            Ok(activity) => Ok(json!({ "activity": activity, "rewards": rewards })),
            Err(_) => Ok(json!({ "activity": null, "rewards": rewards, "refreshRequired": true })),
        }
    }

    async fn read(&self) -> Result<MugeListReply> {
        let body = self
            .gateway
            .request(SERVICE, "List", &ActivityListRequest { game_scene_id: 1 }.encode_to_vec())
            .await?;
        Ok(MugeListReply::decode(body.as_slice())?)
    }
}

#[derive(Clone, Copy)]
struct Operation {
    id: i64,
    cmd: i64,
}

fn operation(action: &str) -> Option<Operation> {
    Some(match action {
        "claimCelebration" => Operation { id: CELEBRATION, cmd: 85 },
        "claimFriends" => Operation { id: FRIENDS, cmd: 84 },
        "claimTask" => Operation { id: HANDBOOK, cmd: 82 },
        "claimHandbook" => Operation { id: HANDBOOK, cmd: 83 },
        "saveAnswers" => Operation { id: PERSONALITY, cmd: 86 },
        "settle" => Operation { id: PERSONALITY, cmd: 87 },
        "claimShare" => Operation { id: PERSONALITY, cmd: 89 },
        _ => return None,
    })
}

fn build_request(
    action: &str,
    operation: Operation,
    state: &Value,
    input: &Value,
) -> Result<MugeOperateRequest> {
    let mut request =
        MugeOperateRequest { id: operation.id, cmd: operation.cmd, ..Default::default() };
    match action {
        "claimCelebration" | "claimFriends" | "claimHandbook" => {
            let section = match action {
                "claimCelebration" => "celebration",
                "claimFriends" => "friends",
                _ => "handbook",
            };
            if !section_claimable(state.get(section)) {
                return Err(Error::Business("当前没有可领取的档位奖励".into()));
            }
            let claim = Some(MugeTierClaimRequest { tier_id: 0 });
            match action {
                "claimCelebration" => request.global_count_claim_tier = claim,
                "claimFriends" => request.invite_friend_tier_claim = claim,
                _ => request.task_tier_claim = claim,
            }
        }
        "claimTask" => {
            if !task_claimable(state.pointer("/handbook/tasks")) {
                return Err(Error::Business("任务未完成或奖励已领取".into()));
            }
            request.task_claim = Some(MugeTaskClaimRequest { task_id: 0 });
        }
        "saveAnswers" => {
            if state.get("personality").is_none_or(Value::is_null) {
                return Err(Error::Business("牧场人格数据暂不可用".into()));
            }
            let answers = checked_answers(input)?;
            request.survey_save_progress = Some(MugeSurveySaveRequest {
                answers,
                clear: input.get("clear").and_then(Value::as_bool) == Some(true),
            });
        }
        "settle" => {
            let personality = state.get("personality").filter(|value| !value.is_null());
            let Some(personality) = personality else {
                return Err(Error::Business("牧场人格数据暂不可用".into()));
            };
            if !survey_complete(personality.get("answers")) {
                return Err(Error::Business("请先完成全部题目".into()));
            }
            request.survey_settle = Some(MugeEmpty {});
        }
        "claimShare" => {
            let personality = state.get("personality").filter(|value| !value.is_null());
            let ready = personality.is_some_and(|row| {
                row.get("personalityId").and_then(Value::as_i64).unwrap_or(0) > 0
                    && row.get("shareRewardClaimed").and_then(Value::as_bool) != Some(true)
            });
            if !ready {
                return Err(Error::Business("请先完成人格测试，首次分享奖励仅可领取一次".into()));
            }
            request.survey_claim_share_reward =
                Some(MugeSurveyShareRequest { share_id: String::new() });
        }
        _ => return Err(Error::Business("未知的牧歌初启操作".into())),
    }
    Ok(request)
}

fn checked_answers(input: &Value) -> Result<HashMap<i64, i64>> {
    let Some(answers) = input.get("answers").and_then(Value::as_object) else {
        return Err(Error::Business("请选择题目答案".into()));
    };
    if let Some(clear) = input.get("clear") {
        if !clear.is_boolean() {
            return Err(Error::Business("重答标记无效".into()));
        }
    }
    let clear = input.get("clear").and_then(Value::as_bool) == Some(true);
    if clear && !answers.is_empty() {
        return Err(Error::Business("清空进度和保存答案需要分开提交，请刷新页面重新测试".into()));
    }
    if !clear && answers.is_empty() {
        return Err(Error::Business("请选择题目答案".into()));
    }
    let mut parsed = HashMap::new();
    for (key, value) in answers {
        let question_id = key.parse::<i64>().unwrap_or(0);
        let chosen = value.as_i64().unwrap_or(0);
        if !option_exists(question_id, chosen) {
            return Err(Error::Business("题目或答案无效".into()));
        }
        parsed.insert(question_id, chosen);
    }
    Ok(parsed)
}

fn option_exists(question_id: i64, value: i64) -> bool {
    config().get("questions").and_then(Value::as_array).is_some_and(|rows| {
        rows.iter().any(|question| {
            question.get("id").and_then(Value::as_i64) == Some(question_id)
                && question.get("options").and_then(Value::as_array).is_some_and(|options| {
                    options
                        .iter()
                        .any(|option| option.get("value").and_then(Value::as_i64) == Some(value))
                })
        })
    })
}

fn survey_complete(answers: Option<&Value>) -> bool {
    let Some(answers) = answers.and_then(Value::as_object) else {
        return false;
    };
    config().get("questions").and_then(Value::as_array).is_some_and(|rows| {
        rows.iter().all(|question| {
            let id = question.get("id").and_then(Value::as_i64).unwrap_or(0).to_string();
            let chosen = answers.get(&id).and_then(Value::as_i64).unwrap_or(0);
            option_exists(question.get("id").and_then(Value::as_i64).unwrap_or(0), chosen)
        })
    })
}

fn section_claimable(section: Option<&Value>) -> bool {
    section.and_then(|row| row.get("tiers")).and_then(Value::as_array).is_some_and(|rows| {
        rows.iter().any(|tier| tier.get("canClaim").and_then(Value::as_bool) == Some(true))
    })
}

fn task_claimable(tasks: Option<&Value>) -> bool {
    tasks.and_then(Value::as_array).is_some_and(|rows| {
        rows.iter().any(|task| task.get("canClaim").and_then(Value::as_bool) == Some(true))
    })
}

fn reply_confirmed(
    reply: &MugeOperateReply,
    operation: Operation,
    action: &str,
    input: &Value,
) -> bool {
    let head_id =
        reply.activity.as_ref().and_then(|activity| activity.head.as_ref()).map(|head| head.id);
    if reply.id != operation.id || reply.cmd != operation.cmd || head_id != Some(operation.id) {
        return false;
    }
    match action {
        "claimTask" => reply.task_claim.as_ref().is_some_and(|row| !row.claimed_ids.is_empty()),
        "claimHandbook" => {
            reply.task_tier_claim.as_ref().is_some_and(|row| !row.claimed_ids.is_empty())
        }
        "claimFriends" => {
            reply.invite_friend_tier_claim.as_ref().is_some_and(|row| !row.claimed_ids.is_empty())
        }
        "claimCelebration" => {
            reply.global_count_claim_tier.as_ref().is_some_and(|row| !row.claimed_ids.is_empty())
        }
        "claimShare" => reply
            .activity
            .as_ref()
            .and_then(|activity| activity.survey.as_ref())
            .is_some_and(|survey| survey.share_reward_claimed),
        "settle" => reply
            .activity
            .as_ref()
            .and_then(|activity| activity.survey.as_ref())
            .is_some_and(|survey| survey.personality_id > 0),
        "saveAnswers" => save_confirmed(reply.activity.as_ref(), input),
        _ => false,
    }
}

fn save_confirmed(activity: Option<&MugeActivity>, input: &Value) -> bool {
    let Some(survey) = activity.and_then(|activity| activity.survey.as_ref()) else {
        return false;
    };
    let clear = input.get("clear").and_then(Value::as_bool) == Some(true);
    if clear {
        return survey.answers.values().all(|value| *value <= 0);
    }
    let Some(answers) = input.get("answers").and_then(Value::as_object) else {
        return false;
    };
    answers.iter().all(|(key, value)| {
        let id = key.parse::<i64>().unwrap_or(0);
        survey.answers.get(&id).copied() == value.as_i64()
    })
}

fn reply_awards(reply: &MugeOperateReply, action: &str) -> Vec<Item> {
    let claim = match action {
        "claimTask" => reply.task_claim.as_ref(),
        "claimHandbook" => reply.task_tier_claim.as_ref(),
        "claimFriends" => reply.invite_friend_tier_claim.as_ref(),
        "claimCelebration" => reply.global_count_claim_tier.as_ref(),
        _ => None,
    };
    if let Some(MugeTierClaimReply { awards, .. }) = claim {
        return awards.clone();
    }
    reply.survey_claim_share_reward.as_ref().map(|row| row.awards.clone()).unwrap_or_default()
}

fn normalize(reply: &MugeListReply, now_secs: i64) -> Result<Value> {
    let group = reply
        .groups
        .iter()
        .find(|group| group.head.as_ref().is_some_and(|head| head.id == GROUP_ID))
        .ok_or_else(|| Error::Business("牧歌初启活动数据暂不可用".into()))?;
    let head =
        group.head.as_ref().ok_or_else(|| Error::Business("牧歌初启活动数据暂不可用".into()))?;
    let child = |id: i64| {
        group.children.iter().find(|entry| entry.head.as_ref().is_some_and(|head| head.id == id))
    };
    let active = head.status == 1 && now_secs >= head.start_time && now_secs < head.end_time;
    let personality = child(PERSONALITY).and_then(|entry| entry.survey.as_ref());
    let personality_id = personality.map(|survey| survey.personality_id).unwrap_or(0);
    let personality_answers = personality.map(|survey| {
        let mut answers = serde_json::Map::new();
        for (key, value) in &survey.answers {
            answers.insert(key.to_string(), json!(value));
        }
        answers
    });
    Ok(json!({
        "activityId": GROUP_ID.to_string(),
        "title": "牧歌初启",
        "serverTime": now_secs * 1000,
        "startTime": head.start_time * 1000,
        "endTime": head.end_time * 1000,
        "active": active,
        "celebration": child(CELEBRATION).and_then(|entry| entry.global_count.as_ref()).map(|count| json!({
            "total": count.total,
            "tiers": tiers_json(&count.tiers),
            "canClaim": count.tiers.iter().any(|tier| tier.status == 2),
        })),
        "friends": child(FRIENDS).and_then(|entry| entry.invite_friend.as_ref()).map(|friends| json!({
            "count": friends.counted_friends.len(),
            "tiers": tiers_json(&friends.tiers),
            "canClaim": friends.tiers.iter().any(|tier| tier.status == 2),
        })),
        "handbook": child(HANDBOOK).and_then(|entry| entry.task.as_ref()).map(|book| json!({
            "points": book.tier_points,
            "tiers": tiers_json(&book.tiers),
            "canClaim": book.tiers.iter().any(|tier| tier.status == 2),
            "tasks": book.tasks.iter().map(|task| json!({
                "id": task.task_id.to_string(),
                "description": task.desc,
                "progress": task.progress,
                "target": task.target,
                "status": task.status,
                "canClaim": task.status == 2,
                "points": task_points(task.task_id),
            })).collect::<Vec<_>>(),
        })),
        "personality": personality.map(|survey| json!({
            "answers": personality_answers,
            "personalityId": personality_id,
            "settleCount": survey.settle_count,
            "shareRewardClaimed": survey.share_reward_claimed,
            "canShare": personality_id > 0 && !survey.share_reward_claimed,
        })),
        "questions": questions_json(),
    }))
}

fn tiers_json(tiers: &[MugeTier]) -> Vec<Value> {
    tiers
        .iter()
        .map(|tier| {
            json!({
                "id": tier.tier_id.to_string(),
                "threshold": tier.threshold,
                "status": tier.status,
                "canClaim": tier.status == 2,
                "rewards": reward_rows(&tier.reward),
            })
        })
        .collect()
}

fn reward_rows(items: &[Item]) -> Vec<Value> {
    items
        .iter()
        .map(|item| {
            let name = global_game_config()
                .get_item_by_id(item.id)
                .map(|entry| entry.name.clone())
                .unwrap_or_else(|| format!("物品 #{}", item.id));
            json!({ "id": item.id, "count": item.count, "name": name })
        })
        .collect()
}

fn task_points(id: i64) -> i64 {
    config()
        .get("tasks")
        .and_then(Value::as_array)
        .and_then(|rows| rows.iter().find(|row| row.get("id").and_then(Value::as_i64) == Some(id)))
        .and_then(|row| row.get("points").and_then(Value::as_i64))
        .unwrap_or(0)
}

fn questions_json() -> Value {
    config().get("questions").cloned().unwrap_or_else(|| json!([]))
}

fn config() -> &'static Value {
    static SLOT: OnceLock<Value> = OnceLock::new();
    SLOT.get_or_init(|| {
        serde_json::from_str(include_str!("../../assets/activity/muge-20261117.json"))
            .expect("牧歌初启配置无法读取")
    })
}

#[cfg(test)]
mod tests {
    use crate::proto::generated::gamepb::activitypb::{
        MugeActivity, MugeGlobalCount, MugeGroup, MugeHead, MugeListReply, MugeTier,
    };

    use super::{normalize, option_exists};

    #[test]
    fn open_celebration_marks_a_ready_tier() {
        let reply = MugeListReply {
            groups: vec![MugeGroup {
                head: Some(MugeHead {
                    id: 2026111700,
                    status: 1,
                    start_time: 0,
                    end_time: 5_000,
                    name: "牧歌初启".into(),
                    ..Default::default()
                }),
                children: vec![MugeActivity {
                    head: Some(MugeHead { id: 2026111702, ..Default::default() }),
                    global_count: Some(MugeGlobalCount {
                        total: 8,
                        tiers: vec![MugeTier {
                            tier_id: 1,
                            threshold: 5,
                            status: 2,
                            ..Default::default()
                        }],
                    }),
                    ..Default::default()
                }],
            }],
        };
        let state = normalize(&reply, 1_000).expect("muge");
        assert!(state["active"].as_bool().unwrap_or(false));
        assert!(state["celebration"]["canClaim"].as_bool().unwrap_or(false));
        assert!(!option_exists(1, 9));
        assert!(option_exists(1, 1));
    }
}
