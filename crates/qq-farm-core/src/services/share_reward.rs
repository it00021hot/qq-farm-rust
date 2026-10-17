//! 快乐不独享（分享奖励，bot `core/src/services/share-reward-activity.ts`）。
//!
//! 通过 `gamepb.activitypb.ActivityService` 的 `GetGroup` / `Operate`（操作码 70-74）
//! 读实例进度并执行分享 / 领奖；实例表镜像 bot `activity-data/share-reward-instances.json`。
//! 分享凭据（ark）不下发到面板，也不用于发送消息（对齐 bot 注释）。

use std::sync::{Arc, OnceLock};

use prost::Message as _;
use serde::Deserialize;

use crate::config::game_config::global as global_game_config;
use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item;
use crate::proto::generated::gamepb::activitypb::{
    ActivityData, ActivityOperateReply, AutumnOperateRequest, GetGroupReply, GetGroupRequest,
    ShareRewardClaimDailyReq, ShareRewardClaimDailyRsp, ShareRewardClaimMilestonesReq,
    ShareRewardClaimMilestonesRsp, ShareRewardGetLogsReq, ShareRewardGetLogsRsp,
    ShareRewardShareReq, ShareRewardShareRsp,
};
use crate::services::activity_text::strip_tags;

const SERVICE: &str = "gamepb.activitypb.ActivityService";

#[derive(Debug, Deserialize)]
struct ShareInstance {
    #[serde(rename = "activityId")]
    activity_id: String,
    #[serde(rename = "groupId")]
    group_id: String,
    title: String,
    icon: String,
}

fn instances() -> &'static Vec<ShareInstance> {
    static CONFIG: OnceLock<Vec<ShareInstance>> = OnceLock::new();
    CONFIG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../assets/activity-data/share-reward-instances.json"
        ))
        .expect("share-reward-instances.json 解析失败")
    })
}

fn instance_for(activity_id: &str) -> Result<&'static ShareInstance> {
    instances()
        .iter()
        .find(|instance| instance.activity_id == activity_id)
        .ok_or_else(|| Error::Business("未知的快乐不独享活动".into()))
}

fn operation(action: &str) -> Option<(i64, &'static str)> {
    Some(match action {
        "share" => (70, "share"),
        "milestones" => (71, "milestones"),
        "logs" => (72, "logs"),
        "daily" => (74, "daily"),
        _ => return None,
    })
}

fn reward_dto(item: &Item) -> serde_json::Value {
    let gc = global_game_config();
    serde_json::json!({
        "id": item.id,
        "count": item.count,
        "name": gc.get_item_by_id(item.id).map(|entry| entry.name).unwrap_or_else(|| format!("物品 #{}", item.id)),
        "image": gc.get_item_image_by_id(item.id).unwrap_or_default(),
    })
}

fn rewards_dto(items: &[Item]) -> Vec<serde_json::Value> {
    items.iter().map(reward_dto).collect()
}

fn find_entry<'a>(entry: &'a ActivityData, activity_id: &str) -> Option<&'a ActivityData> {
    let matches = entry
        .activity
        .as_ref()
        .is_some_and(|activity| activity.activity_id.to_string() == activity_id);
    if matches {
        return Some(entry);
    }
    entry.children.iter().find_map(|child| find_entry(child, activity_id))
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

pub struct ShareRewardService {
    gateway: Arc<Gateway>,
    /// 写操作串行化（对齐 bot `mutationTail`）
    mutation_lock: tokio::sync::Mutex<()>,
}

impl ShareRewardService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway, mutation_lock: tokio::sync::Mutex::new(()) }
    }

    /// 实例快照（对齐 bot `getShareRewardActivity`）。
    pub async fn get_activity(&self, activity_id: &str) -> Result<serde_json::Value> {
        let instance = instance_for(activity_id)?;
        let body = GetGroupRequest {
            group_id: instance
                .group_id
                .parse()
                .map_err(|_| Error::Business("未知的快乐不独享活动".into()))?,
        }
        .encode_to_vec();
        let reply_bytes =
            self.gateway.request(SERVICE, "GetGroup", &body).await.map_err(Error::from)?;
        let reply = GetGroupReply::decode(&reply_bytes[..])?;
        let group =
            reply.group.ok_or_else(|| Error::Business("活动进度未返回，请稍后刷新".into()))?;
        let entry = find_entry(&group, activity_id)
            .ok_or_else(|| Error::Business("活动进度未返回，请稍后刷新".into()))?;
        let summary = entry
            .share_reward
            .as_ref()
            .and_then(|share_reward| share_reward.summary.as_ref())
            .ok_or_else(|| Error::Business("活动进度未返回，请稍后刷新".into()))?;
        let head = entry
            .activity
            .as_ref()
            .ok_or_else(|| Error::Business("活动进度未返回，请稍后刷新".into()))?;
        let now = crate::utils::time::get_server_time_secs();
        let active = head.begin_time <= now && head.end_time > now;
        let can_claim_milestones =
            active && summary.milestones.iter().any(|milestone| milestone.state == 2);
        let milestones: Vec<serde_json::Value> = summary
            .milestones
            .iter()
            .map(|milestone| {
                serde_json::json!({
                    "id": milestone.tier_id.to_string(),
                    "threshold": milestone.threshold,
                    "state": milestone.state,
                    "rewards": rewards_dto(&milestone.rewards),
                })
            })
            .collect();
        Ok(serde_json::json!({
            "id": instance.activity_id,
            "title": instance.title,
            "icon": instance.icon,
            "serverTime": now * 1000,
            "startTime": head.begin_time * 1000,
            "endTime": head.end_time * 1000,
            "active": active,
            "rules": rules_from_extra(&head.extra),
            "score": summary.current_score,
            "scoreItemId": summary.score_item_id,
            "dailyReward": summary.daily_reward,
            "firstShareReward": summary.first_share_reward,
            "claimedCount": summary.daily.as_ref().map(|daily| daily.claimed_count).unwrap_or(0),
            "claimLimit": summary.daily.as_ref().map(|daily| daily.claim_limit).unwrap_or(0),
            "poolClaimedCount": summary.my_pool.as_ref().map(|pool| pool.claimed_count).unwrap_or(0),
            "poolClaimLimit": summary.my_pool.as_ref().map(|pool| pool.claim_limit).unwrap_or(0),
            "canClaimDaily": active
                && summary.daily.as_ref().is_some_and(|daily| !daily.daily_reward_claimed),
            "canShare": active
                && summary.daily.as_ref().is_some_and(|daily| !daily.first_share_awarded),
            "firstShareAwarded": summary
                .daily
                .as_ref()
                .is_some_and(|daily| daily.first_share_awarded),
            "canClaimMilestones": can_claim_milestones,
            "milestones": milestones,
        }))
    }

    /// 执行操作（对齐 bot `operateShareRewardActivity` → `performOperation`）。
    pub async fn operate(
        &self,
        activity_id: &str,
        action: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let instance = instance_for(activity_id)?;
        let Some((operate_type, _field)) = operation(action) else {
            return Err(Error::Business("未知活动操作".into()));
        };
        let _guard = self.mutation_lock.lock().await;
        let state = self.get_activity(instance.activity_id.as_str()).await?;
        if state.get("active").and_then(serde_json::Value::as_bool) != Some(true) {
            return Err(Error::Business("活动尚未开放或已经结束".into()));
        }
        if action == "daily"
            && state.get("canClaimDaily").and_then(serde_json::Value::as_bool) != Some(true)
        {
            return Err(Error::Business("今日快乐值已领取".into()));
        }
        if action == "share"
            && state.get("canShare").and_then(serde_json::Value::as_bool) != Some(true)
        {
            return Err(Error::Business("今日分享奖励已领取".into()));
        }
        if action == "milestones"
            && state.get("canClaimMilestones").and_then(serde_json::Value::as_bool) != Some(true)
        {
            return Err(Error::Business("当前没有可领取的档位奖励".into()));
        }
        let activity_number: i64 = instance
            .activity_id
            .parse()
            .map_err(|_| Error::Business("未知的快乐不独享活动".into()))?;
        let request = AutumnOperateRequest {
            activity_id: activity_number,
            operate_type,
            share_reward_share: (action == "share").then(ShareRewardShareReq::default),
            share_reward_claim_milestones: (action == "milestones")
                .then(ShareRewardClaimMilestonesReq::default),
            share_reward_get_logs: (action == "logs").then(|| ShareRewardGetLogsReq {
                page: -1,
                page_size: 100,
                tab: if input.get("tab").and_then(serde_json::Value::as_i64) == Some(0) {
                    0
                } else {
                    1
                },
            }),
            share_reward_claim_daily: (action == "daily").then(ShareRewardClaimDailyReq::default),
            ..Default::default()
        };
        let body = request.encode_to_vec();
        let reply_bytes =
            self.gateway.request(SERVICE, "Operate", &body).await.map_err(Error::from)?;
        let reply = ActivityOperateReply::decode(&reply_bytes[..])?;
        if reply.activity_id != activity_number || reply.operate_type != operate_type {
            return Err(Error::Business("操作回包不完整，请刷新确认结果，勿重复提交".into()));
        }
        let (result, rewards) = match action {
            "share" => {
                let rsp: ShareRewardShareRsp =
                    reply.share_reward_share.clone().ok_or_else(|| {
                        Error::Business("操作回包不完整，请刷新确认结果，勿重复提交".into())
                    })?;
                // 分享凭据不下发：只保留积分
                (serde_json::json!({ "grantedScore": rsp.granted_score }), Vec::new())
            }
            "milestones" => {
                let rsp: ShareRewardClaimMilestonesRsp =
                    reply.share_reward_claim_milestones.clone().ok_or_else(|| {
                        Error::Business("操作回包不完整，请刷新确认结果，勿重复提交".into())
                    })?;
                (
                    serde_json::json!({
                        "claimedTierIds": rsp.claimed_tier_ids,
                        "rewards": rewards_dto(&rsp.rewards),
                    }),
                    rsp.rewards,
                )
            }
            "logs" => {
                let rsp: ShareRewardGetLogsRsp =
                    reply.share_reward_get_logs.clone().ok_or_else(|| {
                        Error::Business("操作回包不完整，请刷新确认结果，勿重复提交".into())
                    })?;
                let logs: Vec<serde_json::Value> = rsp
                    .logs
                    .iter()
                    .map(|entry| {
                        serde_json::json!({
                            "seq": entry.seq,
                            "kind": entry.kind,
                            "score": entry.score,
                            "createdAt": entry.created_at,
                            "actor": entry.actor.as_ref().map(|actor| serde_json::json!({ "name": actor.name })),
                        })
                    })
                    .collect();
                (serde_json::json!({ "total": rsp.total, "logs": logs }), Vec::new())
            }
            _ => {
                let rsp: ShareRewardClaimDailyRsp =
                    reply.share_reward_claim_daily.clone().ok_or_else(|| {
                        Error::Business("操作回包不完整，请刷新确认结果，勿重复提交".into())
                    })?;
                (
                    serde_json::json!({
                        "rewards": rewards_dto(&rsp.rewards),
                        "grantedScore": rsp.granted_score,
                    }),
                    rsp.rewards,
                )
            }
        };
        let mut payload = serde_json::json!({
            "result": result,
            "rewards": rewards_dto(&rewards),
        });
        match self.get_activity(instance.activity_id.as_str()).await {
            Ok(activity) => {
                payload["activity"] = activity;
            }
            Err(_) => {
                payload["activity"] = serde_json::Value::Null;
                payload["refreshRequired"] = serde_json::json!(true);
            }
        }
        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instances_match_bot_config() {
        let autumn = instance_for("2026092501").unwrap();
        assert_eq!(autumn.group_id, "2026092500");
        assert_eq!(autumn.title, "快乐不独享");
        let milk = instance_for("2026110104").unwrap();
        assert_eq!(milk.group_id, "2026110100");
        assert!(instance_for("999").is_err());
    }

    #[test]
    fn operation_numbers_match_bot_encoder() {
        assert_eq!(operation("share"), Some((70, "share")));
        assert_eq!(operation("milestones"), Some((71, "milestones")));
        assert_eq!(operation("logs"), Some((72, "logs")));
        assert_eq!(operation("daily"), Some((74, "daily")));
        assert_eq!(operation("unknown"), None);
    }

    #[test]
    fn find_entry_walks_children() {
        let mut child = ActivityData::default();
        child.activity = Some(crate::proto::generated::gamepb::activitypb::ActivityContent {
            activity_id: 2_026_110_104,
            ..Default::default()
        });
        let root = ActivityData { children: vec![child], ..Default::default() };
        assert!(find_entry(&root, "2026110104").is_some());
        assert!(find_entry(&root, "2026110101").is_none());
    }

    #[test]
    fn operate_rejects_unknown_instance_before_network() {
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
        let service = ShareRewardService::new(gateway);
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let error = service.operate("999", "share", &serde_json::json!({})).await.unwrap_err();
            assert!(error.to_string().contains("未知的快乐不独享活动"), "{error}");
            let error =
                service.operate("2026110104", "nope", &serde_json::json!({})).await.unwrap_err();
            assert!(error.to_string().contains("未知活动操作"), "{error}");
        });
    }
}
