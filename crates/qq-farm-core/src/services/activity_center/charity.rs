//! 公益小红花 — 活动状态 + 写操作（对齐 bot `activity-center.ts` charity 部分）。
//!
//! - 状态查询走 `ActivityService.List`，BFS（含 children）定位 2026090901 的
//!   `charity_red_flower`
//! - 领种子 op=35（`claim_seed`）、捐爱心 op=36（`donate_love`，一次捐全部）、
//!   每日公益礼包 op=38（`send_public_fund`）
//! - 进度奖励 op=37（`claim_progress_reward`）

use prost::Message;

use crate::constants::{
    ACTIVITY_SERVICE, CHARITY_PROGRESS_ALREADY_CLAIMED_CODE, CHARITY_RED_FLOWER_ACTIVITY_ID,
    CHARITY_RED_FLOWER_GROUP_ID, CLAIM_CHARITY_DAILY_GIFT_OPERATE_TYPE,
    CLAIM_CHARITY_PROGRESS_REWARD_OPERATE_TYPE, CLAIM_CHARITY_SEED_OPERATE_TYPE,
    DONATE_CHARITY_LOVE_OPERATE_TYPE,
};
use crate::error::Result;
use crate::proto::generated::gamepb::activitypb::{
    ActivityData, ActivityListReply, ActivityListRequest, ActivityOperateReply,
    CharityRedFlowerOperateRequest,
};

use crate::services::activity_center_state::{
    load_charity_red_flower_state, merge_charity_red_flower_states,
    persist_charity_red_flower_state, CharityRedFlowerState, StateFileOptions,
};

use super::dto::{activity_item_dto, beijing_date_key, item_dto, item_from_id, text_content};
use super::error::{ActivityError, ActivityErrorCode};
use super::ActivityCenterService;

fn charity_err(code: ActivityErrorCode, message: &str) -> ActivityError {
    ActivityError { code, message: message.to_string() }
}

/// 在 List 回包的活动树（含 children）里定位目标活动。
fn find_activity_data(entries: &[ActivityData], activity_id: i64) -> Option<&ActivityData> {
    let mut queue: Vec<&ActivityData> = entries.iter().collect();
    while let Some(entry) = queue.pop() {
        if entry.activity.as_ref().is_some_and(|a| a.activity_id == activity_id) {
            return Some(entry);
        }
        queue.extend(entry.children.iter());
    }
    None
}

fn charity_active(begin_time: i64, end_time: i64, server_time: i64) -> bool {
    if begin_time > 0 && server_time < begin_time {
        return false;
    }
    if end_time > 0 && server_time > end_time {
        return false;
    }
    true
}

impl ActivityCenterService {
    #[cfg(test)]
    fn charity_dto(&self, entry: &ActivityData) -> Result<serde_json::Value> {
        self.charity_dto_with_state(entry, None)
    }

    fn charity_dto_with_state(
        &self,
        entry: &ActivityData,
        progress_state: Option<&CharityRedFlowerState>,
    ) -> Result<serde_json::Value> {
        let activity = entry.activity.as_ref().ok_or_else(|| {
            charity_err(
                ActivityErrorCode::CharityRedFlowerUnavailable,
                "服务端未发现公益小红花活动状态",
            )
        })?;
        let state = entry.charity_red_flower.as_ref().ok_or_else(|| {
            charity_err(
                ActivityErrorCode::CharityRedFlowerUnavailable,
                "服务端未发现公益小红花活动状态",
            )
        })?;

        let server_time = crate::utils::time::get_server_time_secs();
        // 活动结束时间以状态里的 end_time 优先
        let end_time = if state.end_time > 0 { state.end_time } else { activity.end_time };
        let active = charity_active(activity.begin_time, end_time, server_time);
        let love_balance = state.love_balance;
        let donated_love = state.donated_love;
        let seed_status = state.seed_reward_status;
        let public_fund = state.public_fund.as_ref();
        let public_fund_status = public_fund.map(|p| p.status).unwrap_or(0);
        let public_fund_date = public_fund.map(|p| p.date).unwrap_or(0);
        // public_fund 是历史记录，可能仍保留昨天的订单；只有今天的记录才算今日已领。
        let today_key = beijing_date_key().replace('-', "");
        let daily_gift_claimed = state.flow_status == 3
            || (public_fund_date != 0 && public_fund_date.to_string() == today_key);
        let daily_gift_harvested_today = state.flow_status == 2 || state.flow_status == 3;
        let progress_state = progress_state
            .cloned()
            .unwrap_or_else(|| reconcile_charity_progress_state(entry, None));
        let claimed_targets: std::collections::BTreeSet<String> =
            progress_state.claimed_progress_targets.iter().cloned().collect();
        let pending_targets: std::collections::BTreeSet<String> =
            progress_state.pending_progress_targets.iter().cloned().collect();
        let progress_rewards: Vec<serde_json::Value> = state
            .progress_rewards
            .iter()
            .map(|reward| {
                let target = reward.target.to_string();
                let reached = donated_love >= reward.target && reward.target > 0;
                let claimed = claimed_targets.contains(&target);
                serde_json::json!({
                    "target": target,
                    "reward": activity_item_or_default(&reward.reward),
                    "statusCode": reward.status.to_string(),
                    "reached": reached,
                    "claimed": claimed,
                    "claimable": reached && reward.status == 1 && !claimed && pending_targets.contains(&target),
                    "claimSupported": true,
                })
            })
            .collect();
        let global_reward = state.global_reward.as_ref();
        let global_reward_target =
            merge_reward_target(global_reward.map(|g| g.target), state.global_target_love);

        let name = if activity.name.trim().is_empty() {
            "公益小红花".to_string()
        } else {
            activity.name.clone()
        };
        Ok(serde_json::json!({
            "groupId": CHARITY_RED_FLOWER_GROUP_ID.to_string(),
            "activityId": CHARITY_RED_FLOWER_ACTIVITY_ID.to_string(),
            "name": name,
            "title": name,
            "startTime": activity.begin_time.to_string(),
            "endTime": end_time.to_string(),
            "serverTime": server_time.to_string(),
            "active": active,
            "rules": text_content(&activity.extra),
            "love": item_from_id(state.love_item_id, love_balance),
            "loveBalance": love_balance.to_string(),
            "donatedLove": donated_love.to_string(),
            "flowStatus": state.flow_status.to_string(),
            "seedReward": {
                "statusCode": seed_status.to_string(),
                "claimable": seed_status == 2,
                "claimed": seed_status == 3,
                "reward": activity_item_or_default(&state.seed_reward),
            },
            "dailyGift": {
                "statusCode": state.daily_reward_status.to_string(),
                "claimed": daily_gift_claimed,
                "harvestedToday": daily_gift_harvested_today,
                "reward": activity_item_or_default(&state.daily_reward),
                "publicFund": if public_fund_date != 0 {
                    serde_json::json!({
                        "date": public_fund_date.to_string(),
                        "statusCode": public_fund_status.to_string(),
                    })
                } else {
                    serde_json::Value::Null
                },
            },
            "progressRewards": progress_rewards,
            "globalProgress": {
                "donated": state.global_donated_love.to_string(),
                "target": state.global_target_love.to_string(),
                "reached": state.global_donated_love >= state.global_target_love && state.global_target_love > 0,
                "rewardTarget": global_reward_target.to_string(),
                "reward": global_reward.map(|g| activity_item_or_default(&g.reward)).unwrap_or_else(|| item_from_id(0, 0)),
            },
            "settlement": {
                "requiredLove": state.settlement_required_love.to_string(),
                "eligible": donated_love >= state.settlement_required_love,
                "reward": activity_item_or_default(&state.settlement_reward),
            },
            "actions": {
                "claimSeeds": {
                    "enabled": active && seed_status == 2,
                    "available": active && seed_status == 2,
                    "availabilityKnown": true,
                },
                "donateLove": {
                    "enabled": active && love_balance > 0,
                    "available": active && love_balance > 0,
                    "availabilityKnown": true,
                    "count": love_balance,
                },
                "claimDailyGift": {
                    "enabled": active && daily_gift_harvested_today && !daily_gift_claimed,
                    "available": active && daily_gift_harvested_today && !daily_gift_claimed,
                    "attemptable": active && daily_gift_harvested_today && !daily_gift_claimed,
                    "availabilityKnown": true,
                },
            },
        }))
    }

    async fn query_charity_list(&self) -> Result<ActivityListReply> {
        let body = self
            .gateway
            .request(ACTIVITY_SERVICE, "List", &ActivityListRequest {}.encode_to_vec())
            .await?;
        Ok(ActivityListReply::decode(&body[..])?)
    }

    /// 当前公益小红花；活动不存在时返回 `Ok(None)`（对齐 bot 返回 null）。
    pub async fn get_current_charity_red_flower_activity(
        &self,
    ) -> Result<Option<serde_json::Value>> {
        let reply = self.query_charity_list().await?;
        match find_activity_data(&reply.activities, CHARITY_RED_FLOWER_ACTIVITY_ID) {
            Some(entry) if entry.charity_red_flower.is_some() => {
                let progress_state = self.resolve_charity_progress_state(entry);
                Ok(Some(self.charity_dto_with_state(entry, Some(&progress_state))?))
            }
            _ => Ok(None),
        }
    }

    async fn operate_charity_red_flower(
        &self,
        operate_type: i64,
        request: CharityRedFlowerOperateRequest,
    ) -> Result<ActivityOperateReply> {
        let body =
            self.gateway.request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec()).await?;
        let reply = ActivityOperateReply::decode(&body[..])?;
        if reply.activity_id != CHARITY_RED_FLOWER_ACTIVITY_ID || reply.operate_type != operate_type
        {
            return Err(charity_err(
                ActivityErrorCode::CharityResponseInvalid,
                "公益小红花回包不匹配",
            )
            .into());
        }
        Ok(reply)
    }

    /// 领取小红花种子（op=35）。
    pub async fn claim_charity_red_flower_seeds(&self) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let activity = self.require_charity_activity().await?;
        if !action_enabled(&activity, "claimSeeds") {
            return Err(charity_err(
                ActivityErrorCode::CharitySeedsUnavailable,
                "当前没有可领取的小红花种子",
            )
            .into());
        }
        let reply = self
            .operate_charity_red_flower(
                CLAIM_CHARITY_SEED_OPERATE_TYPE,
                CharityRedFlowerOperateRequest {
                    activity_id: CHARITY_RED_FLOWER_ACTIVITY_ID,
                    operate_type: CLAIM_CHARITY_SEED_OPERATE_TYPE,
                    claim_seed: Some(Default::default()),
                    ..Default::default()
                },
            )
            .await?;
        let mut rewards: Vec<serde_json::Value> = Vec::new();
        if let Some(result) = reply.charity_seed_result.as_ref() {
            if let Some(reward) = result.reward.as_ref() {
                rewards.push(item_dto_json(reward));
            }
        }
        if rewards.is_empty() {
            rewards.extend(reply.rewards.iter().map(item_dto_json));
        }
        let snapshot = self.snapshot_with_shop(None).await.ok();
        Ok(serde_json::json!({
            "rewards": rewards,
            "message": "小红花种子领取成功",
            "snapshot": snapshot,
        }))
    }

    /// 捐赠全部爱心（op=36）。
    pub async fn donate_charity_red_flower_love(&self) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let activity = self.require_charity_activity().await?;
        if !action_enabled(&activity, "donateLove") {
            return Err(charity_err(
                ActivityErrorCode::InsufficientCharityLove,
                "当前没有可捐赠的爱心",
            )
            .into());
        }
        let love_balance = activity
            .get("loveBalance")
            .and_then(serde_json::Value::as_str)
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let reply = self
            .operate_charity_red_flower(
                DONATE_CHARITY_LOVE_OPERATE_TYPE,
                CharityRedFlowerOperateRequest {
                    activity_id: CHARITY_RED_FLOWER_ACTIVITY_ID,
                    operate_type: DONATE_CHARITY_LOVE_OPERATE_TYPE,
                    donate_love: Some(Default::default()),
                    ..Default::default()
                },
            )
            .await?;
        let donated = reply
            .charity_donate_result
            .as_ref()
            .map(|r| r.donated)
            .filter(|v| *v != 0)
            .unwrap_or(love_balance);
        let global_donated = reply
            .charity_donate_result
            .as_ref()
            .map(|r| r.global_donated.to_string())
            .unwrap_or_default();
        let snapshot = self.snapshot_with_shop(None).await.ok();
        Ok(serde_json::json!({
            "donated": donated.to_string(),
            "globalDonated": global_donated,
            "message": format!("已捐赠全部 {donated} 份爱心"),
            "snapshot": snapshot,
        }))
    }

    /// 领取今日公益礼包（op=38）。
    pub async fn claim_charity_red_flower_daily_gift(&self) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let activity = self.require_charity_activity().await?;
        let claimed = activity
            .pointer("/dailyGift/claimed")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if claimed {
            return Err(charity_err(
                ActivityErrorCode::CharityDailyGiftUnavailable,
                "今日公益礼包已经领取",
            )
            .into());
        }
        let harvested_today = activity
            .pointer("/dailyGift/harvestedToday")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !harvested_today {
            return Err(charity_err(
                ActivityErrorCode::CharityDailyGiftNotHarvested,
                "今天还没有收获小红花，暂时无法领取公益礼包",
            )
            .into());
        }
        let active = activity.get("active").and_then(serde_json::Value::as_bool).unwrap_or(false);
        if !active {
            return Err(charity_err(
                ActivityErrorCode::CharityRedFlowerUnavailable,
                "公益小红花活动暂未开放或已经结束",
            )
            .into());
        }
        let reply = self
            .operate_charity_red_flower(
                CLAIM_CHARITY_DAILY_GIFT_OPERATE_TYPE,
                CharityRedFlowerOperateRequest {
                    activity_id: CHARITY_RED_FLOWER_ACTIVITY_ID,
                    operate_type: CLAIM_CHARITY_DAILY_GIFT_OPERATE_TYPE,
                    send_public_fund: Some(Default::default()),
                    ..Default::default()
                },
            )
            .await?;
        let mut rewards: Vec<serde_json::Value> = Vec::new();
        if let Some(result) = reply.charity_public_fund_result.as_ref() {
            if let Some(reward) = result.reward.as_ref() {
                rewards.push(item_dto_json(reward));
            }
        }
        if rewards.is_empty() {
            rewards.extend(reply.rewards.iter().map(item_dto_json));
        }
        let public_fund_status = reply
            .charity_public_fund_result
            .as_ref()
            .map(|r| r.status.to_string())
            .unwrap_or_default();
        let snapshot = self.snapshot_with_shop(None).await.ok();
        Ok(serde_json::json!({
            "rewards": rewards,
            "publicFund": {
                "statusCode": public_fund_status,
            },
            "message": "今日公益礼包领取成功",
            "snapshot": snapshot,
        }))
    }

    /// 领取公益小红花个人进度奖励（op=37）。
    pub async fn claim_charity_red_flower_progress_reward(
        &self,
        target: &str,
    ) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let activity = self.require_charity_activity().await?;
        let target = super::dto::positive_decimal(
            target,
            ActivityErrorCode::CharityProgressRewardUnavailable,
            "target",
        )?;
        let target_text = target.to_string();
        let progress = activity
            .get("progressRewards")
            .and_then(serde_json::Value::as_array)
            .and_then(|rewards| {
                rewards.iter().find(|reward| {
                    reward.get("target").and_then(serde_json::Value::as_str)
                        == Some(target_text.as_str())
                })
            })
            .ok_or_else(|| {
                charity_err(
                    ActivityErrorCode::CharityProgressRewardUnavailable,
                    "当前没有可领取的公益进度奖励",
                )
            })?;
        if progress.get("claimed").and_then(serde_json::Value::as_bool).unwrap_or(false) {
            return Err(charity_err(
                ActivityErrorCode::CharityProgressRewardAlreadyClaimed,
                "该公益进度奖励档位已经领取",
            )
            .into());
        }
        if !progress.get("claimable").and_then(serde_json::Value::as_bool).unwrap_or(false) {
            return Err(charity_err(
                ActivityErrorCode::CharityProgressRewardUnavailable,
                "当前没有可领取的公益进度奖励",
            )
            .into());
        }

        let request = CharityRedFlowerOperateRequest {
            activity_id: CHARITY_RED_FLOWER_ACTIVITY_ID,
            operate_type: CLAIM_CHARITY_PROGRESS_REWARD_OPERATE_TYPE,
            claim_progress_reward: Some(
                crate::proto::generated::gamepb::activitypb::charity_red_flower_operate_request::ProgressRewardParams {
                    target,
                },
            ),
            ..Default::default()
        };
        let (reply, already_claimed) = match self
            .operate_charity_red_flower(CLAIM_CHARITY_PROGRESS_REWARD_OPERATE_TYPE, request)
            .await
        {
            Ok(reply) => (Some(reply), false),
            Err(crate::error::Error::Network(crate::network::error::NetworkError::Gateway {
                code,
                ..
            })) if code == CHARITY_PROGRESS_ALREADY_CLAIMED_CODE => (None, true),
            Err(error) => return Err(error),
        };

        self.remember_claimed_charity_progress_target(target);
        let mut rewards = Vec::new();
        if let Some(result) = reply.as_ref().and_then(|r| r.charity_progress_reward_result.as_ref())
        {
            if let Some(reward) = result.reward.as_ref() {
                rewards.push(item_dto_json(reward));
            }
        }
        if rewards.is_empty() {
            if let Some(reply) = reply.as_ref() {
                rewards.extend(reply.rewards.iter().map(item_dto_json));
            }
        }
        let snapshot = self.snapshot_with_shop(None).await.ok();
        Ok(serde_json::json!({
            "target": target_text,
            "rewards": rewards,
            "claimed": true,
            "alreadyClaimed": already_claimed,
            "message": if already_claimed {
                format!("公益进度奖励已领取（{target_text} 份爱心）")
            } else {
                format!("公益进度奖励领取成功（{target_text} 份爱心）")
            },
            "snapshot": snapshot,
        }))
    }

    /// 写操作前拉取当前活动；不存在直接报「暂未开放或已经结束」。
    async fn require_charity_activity(&self) -> Result<serde_json::Value> {
        self.get_current_charity_red_flower_activity().await?.ok_or_else(|| {
            charity_err(
                ActivityErrorCode::CharityRedFlowerUnavailable,
                "公益小红花活动暂未开放或已经结束",
            )
            .into()
        })
    }

    fn resolve_charity_progress_state(&self, entry: &ActivityData) -> CharityRedFlowerState {
        let account_id = self.account_id.lock().clone();
        let memory =
            self.last_charity_red_flower_state.lock().get(&CHARITY_RED_FLOWER_ACTIVITY_ID).cloned();
        let file = load_charity_red_flower_state(
            CHARITY_RED_FLOWER_ACTIVITY_ID,
            Some(account_id.as_str()).filter(|id| !id.is_empty()),
            &StateFileOptions::default(),
        );
        let merged = merge_charity_red_flower_states(
            CHARITY_RED_FLOWER_ACTIVITY_ID,
            &[
                serde_json::to_value(file).unwrap_or(serde_json::Value::Null),
                serde_json::to_value(memory).unwrap_or(serde_json::Value::Null),
            ],
        );
        let reconciled = reconcile_charity_progress_state(entry, Some(&merged));
        self.last_charity_red_flower_state
            .lock()
            .insert(CHARITY_RED_FLOWER_ACTIVITY_ID, reconciled.clone());
        if !account_id.is_empty() {
            let _ = persist_charity_red_flower_state(
                serde_json::to_value(&reconciled).unwrap_or(serde_json::Value::Null),
                CHARITY_RED_FLOWER_ACTIVITY_ID,
                Some(&account_id),
                &StateFileOptions::default(),
            );
        }
        reconciled
    }

    fn remember_claimed_charity_progress_target(&self, target: i64) {
        let account_id = self.account_id.lock().clone();
        let memory =
            self.last_charity_red_flower_state.lock().get(&CHARITY_RED_FLOWER_ACTIVITY_ID).cloned();
        let file = load_charity_red_flower_state(
            CHARITY_RED_FLOWER_ACTIVITY_ID,
            Some(account_id.as_str()).filter(|id| !id.is_empty()),
            &StateFileOptions::default(),
        );
        let claimed = CharityRedFlowerState {
            activity_id: CHARITY_RED_FLOWER_ACTIVITY_ID.to_string(),
            initialized: true,
            claimed_progress_targets: vec![target.to_string()],
            pending_progress_targets: Vec::new(),
        };
        let merged = merge_charity_red_flower_states(
            CHARITY_RED_FLOWER_ACTIVITY_ID,
            &[
                serde_json::to_value(file).unwrap_or(serde_json::Value::Null),
                serde_json::to_value(memory).unwrap_or(serde_json::Value::Null),
                serde_json::to_value(claimed).unwrap_or(serde_json::Value::Null),
            ],
        );
        self.last_charity_red_flower_state
            .lock()
            .insert(CHARITY_RED_FLOWER_ACTIVITY_ID, merged.clone());
        if !account_id.is_empty() {
            let _ = persist_charity_red_flower_state(
                serde_json::to_value(&merged).unwrap_or(serde_json::Value::Null),
                CHARITY_RED_FLOWER_ACTIVITY_ID,
                Some(&account_id),
                &StateFileOptions::default(),
            );
        }
    }
}

/// 根据服务端快照和本地状态恢复公益进度奖励的“已领取 / 待领取”边界。
fn reconcile_charity_progress_state(
    entry: &ActivityData,
    state_value: Option<&CharityRedFlowerState>,
) -> CharityRedFlowerState {
    let mut state = state_value.cloned().unwrap_or_else(|| {
        crate::services::activity_center_state::create_empty_charity_red_flower_state(
            CHARITY_RED_FLOWER_ACTIVITY_ID,
        )
    });
    let Some(data) = entry.charity_red_flower.as_ref() else { return state };
    let donated_love = data.donated_love;
    let reached_targets: Vec<String> = data
        .progress_rewards
        .iter()
        .filter(|reward| reward.status == 1 && reward.target > 0 && donated_love >= reward.target)
        .map(|reward| reward.target.to_string())
        .collect();
    let mut claimed: std::collections::BTreeSet<String> =
        state.claimed_progress_targets.into_iter().collect();
    let mut pending: std::collections::BTreeSet<String> =
        state.pending_progress_targets.into_iter().collect();
    if !state.initialized {
        if reached_targets.len() > 1 {
            claimed.extend(reached_targets[..reached_targets.len() - 1].iter().cloned());
        }
        if let Some(last) = reached_targets.last() {
            pending.insert(last.clone());
        }
    } else {
        for target in reached_targets {
            if !claimed.contains(&target) && !pending.contains(&target) {
                pending.insert(target);
            }
        }
    }
    for target in &claimed {
        pending.remove(target);
    }
    state.activity_id = CHARITY_RED_FLOWER_ACTIVITY_ID.to_string();
    state.initialized = true;
    state.claimed_progress_targets = claimed.into_iter().collect();
    state.pending_progress_targets = pending.into_iter().collect();
    state
}

fn action_enabled(activity: &serde_json::Value, key: &str) -> bool {
    activity
        .pointer(&format!("/actions/{key}/enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn item_dto_json(item: &crate::proto::generated::corepb::Item) -> serde_json::Value {
    serde_json::to_value(item_dto(item)).unwrap_or(serde_json::Value::Null)
}

/// `Option<ActivityItem>` → DTO；缺省时给全 0（对齐 bot `itemDto(undefined)` 的零值输出）。
fn activity_item_or_default(
    item: &Option<crate::proto::generated::gamepb::activitypb::ActivityItem>,
) -> super::ItemDto {
    match item {
        Some(i) => activity_item_dto(i),
        None => item_from_id(0, 0),
    }
}

/// 全局奖励目标：global_reward.target 非 0 优先，否则回退 global_target_love。
fn merge_reward_target(reward_target: Option<i64>, fallback: i64) -> i64 {
    match reward_target {
        Some(t) if t != 0 => t,
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        state: crate::proto::generated::gamepb::activitypb::CharityRedFlowerData,
    ) -> ActivityData {
        use crate::proto::generated::gamepb::activitypb::ActivityContent;
        ActivityData {
            activity: Some(ActivityContent {
                activity_id: CHARITY_RED_FLOWER_ACTIVITY_ID,
                name: "公益小红花".to_string(),
                begin_time: 1_000,
                end_time: 9_999_999_999,
                ..Default::default()
            }),
            charity_red_flower: Some(state),
            ..Default::default()
        }
    }

    fn service() -> ActivityCenterService {
        use crate::network::encryptor::NoopEncryptor;
        use crate::network::gateway::{Gateway, GatewayConfig};
        let gateway = Gateway::new(
            GatewayConfig {
                server_url: "wss://gate.example.com/ws".to_string(),
                platform: "qq".to_string(),
                os: "Windows".to_string(),
                client_version: "1.13.3.16_20260826".to_string(),
                auth_code: "test".to_string(),
                headers: std::collections::HashMap::new(),
            },
            std::sync::Arc::new(NoopEncryptor),
        );
        ActivityCenterService::new(std::sync::Arc::new(gateway))
    }

    #[test]
    fn find_activity_data_walks_children() {
        use crate::proto::generated::gamepb::activitypb::ActivityContent;
        let leaf = ActivityData {
            activity: Some(ActivityContent {
                activity_id: CHARITY_RED_FLOWER_ACTIVITY_ID,
                ..Default::default()
            }),
            ..Default::default()
        };
        let root = ActivityData {
            activity: Some(ActivityContent { activity_id: 1, ..Default::default() }),
            children: vec![leaf],
            ..Default::default()
        };
        assert!(find_activity_data(&[root], CHARITY_RED_FLOWER_ACTIVITY_ID).is_some());
        assert!(find_activity_data(&[], CHARITY_RED_FLOWER_ACTIVITY_ID).is_none());
    }

    #[test]
    fn dto_maps_seed_and_daily_status() {
        let svc = service();
        let dto = svc
            .charity_dto(&entry(
                crate::proto::generated::gamepb::activitypb::CharityRedFlowerData {
                    love_balance: 5,
                    donated_love: 12,
                    seed_reward_status: 2,
                    flow_status: 2,
                    ..Default::default()
                },
            ))
            .expect("dto");
        assert_eq!(dto["seedReward"]["claimable"], serde_json::json!(true));
        assert_eq!(dto["seedReward"]["claimed"], serde_json::json!(false));
        assert_eq!(dto["dailyGift"]["claimed"], serde_json::json!(false));
        assert_eq!(dto["actions"]["claimSeeds"]["enabled"], serde_json::json!(true));
        assert_eq!(dto["actions"]["donateLove"]["enabled"], serde_json::json!(true));
        assert_eq!(dto["actions"]["donateLove"]["count"], serde_json::json!(5));
        assert_eq!(dto["actions"]["claimDailyGift"]["enabled"], serde_json::json!(true));
        assert_eq!(dto["progressRewards"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn dto_daily_gift_claimed_via_public_fund() {
        use crate::proto::generated::gamepb::activitypb::CharityRedFlowerPublicFund;
        let svc = service();
        let dto = svc
            .charity_dto(&entry(
                crate::proto::generated::gamepb::activitypb::CharityRedFlowerData {
                    public_fund: Some(CharityRedFlowerPublicFund {
                        date: 20_260_901,
                        order_id: "ORDER".to_string(),
                        status: 1,
                        ..Default::default()
                    }),
                    flow_status: 3,
                    ..Default::default()
                },
            ))
            .expect("dto");
        assert_eq!(dto["dailyGift"]["claimed"], serde_json::json!(true));
        assert_eq!(dto["dailyGift"]["publicFund"]["statusCode"], serde_json::json!("1"));
        assert_eq!(dto["actions"]["claimDailyGift"]["enabled"], serde_json::json!(false));
    }

    #[test]
    fn dto_does_not_use_yesterday_public_fund_as_today_claimed() {
        use crate::proto::generated::gamepb::activitypb::CharityRedFlowerPublicFund;
        let svc = service();
        let dto = svc
            .charity_dto(&entry(
                crate::proto::generated::gamepb::activitypb::CharityRedFlowerData {
                    public_fund: Some(CharityRedFlowerPublicFund {
                        date: 20_260_901,
                        order_id: "OLD_ORDER".to_string(),
                        status: 1,
                        ..Default::default()
                    }),
                    flow_status: 1,
                    ..Default::default()
                },
            ))
            .expect("dto");
        assert_eq!(dto["dailyGift"]["claimed"], serde_json::json!(false));
        assert_eq!(dto["dailyGift"]["harvestedToday"], serde_json::json!(false));
        assert_eq!(dto["actions"]["claimDailyGift"]["enabled"], serde_json::json!(false));
    }

    #[test]
    fn dto_reconciles_charity_progress_frontier() {
        use crate::proto::generated::gamepb::activitypb::{
            ActivityItem, CharityRedFlowerProgressReward,
        };
        let svc = service();
        let dto = svc
            .charity_dto(&entry(
                crate::proto::generated::gamepb::activitypb::CharityRedFlowerData {
                    donated_love: 62,
                    progress_rewards: vec![
                        CharityRedFlowerProgressReward {
                            target: 30,
                            reward: Some(ActivityItem { item_id: 80013, count: 1 }),
                            status: 1,
                        },
                        CharityRedFlowerProgressReward {
                            target: 60,
                            reward: Some(ActivityItem { item_id: 1002, count: 50 }),
                            status: 1,
                        },
                        CharityRedFlowerProgressReward {
                            target: 90,
                            reward: Some(ActivityItem { item_id: 80013, count: 2 }),
                            status: 0,
                        },
                    ],
                    ..Default::default()
                },
            ))
            .expect("dto");
        let progress = dto["progressRewards"].as_array().expect("progress rewards");
        assert_eq!(progress[0]["claimed"], serde_json::json!(true));
        assert_eq!(progress[0]["claimable"], serde_json::json!(false));
        assert_eq!(progress[1]["claimed"], serde_json::json!(false));
        assert_eq!(progress[1]["claimable"], serde_json::json!(true));
        assert_eq!(progress[2]["claimable"], serde_json::json!(false));
    }

    #[test]
    fn dto_seed_claimed_status_disables_action() {
        let svc = service();
        let dto = svc
            .charity_dto(&entry(
                crate::proto::generated::gamepb::activitypb::CharityRedFlowerData {
                    seed_reward_status: 3,
                    ..Default::default()
                },
            ))
            .expect("dto");
        assert_eq!(dto["seedReward"]["claimed"], serde_json::json!(true));
        assert_eq!(dto["actions"]["claimSeeds"]["enabled"], serde_json::json!(false));
    }

    #[test]
    fn dto_settlement_and_global_progress() {
        use crate::proto::generated::gamepb::activitypb::{
            ActivityItem, CharityRedFlowerGlobalReward,
        };
        let svc = service();
        let dto = svc
            .charity_dto(&entry(
                crate::proto::generated::gamepb::activitypb::CharityRedFlowerData {
                    donated_love: 100,
                    settlement_required_love: 80,
                    global_donated_love: 500,
                    global_target_love: 1000,
                    global_reward: Some(CharityRedFlowerGlobalReward {
                        target: 0,
                        reward: Some(ActivityItem { item_id: 9, count: 1 }),
                    }),
                    ..Default::default()
                },
            ))
            .expect("dto");
        assert_eq!(dto["settlement"]["eligible"], serde_json::json!(true));
        assert_eq!(dto["globalProgress"]["reached"], serde_json::json!(false));
        // global_reward.target == 0 → 回退 global_target_love
        assert_eq!(dto["globalProgress"]["rewardTarget"], serde_json::json!("1000"));
    }

    #[test]
    fn merge_reward_target_prefers_nonzero() {
        assert_eq!(merge_reward_target(Some(7), 9), 7);
        assert_eq!(merge_reward_target(Some(0), 9), 9);
        assert_eq!(merge_reward_target(None, 9), 9);
    }

    #[test]
    fn charity_active_window() {
        assert!(charity_active(0, 0, 100));
        assert!(charity_active(50, 150, 100));
        assert!(!charity_active(101, 0, 100));
        assert!(!charity_active(0, 99, 100));
    }
}
