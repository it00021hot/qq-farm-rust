//! 秋日活动 — 秋祈良愿（wish）+ 快乐不独享（happy）。
//!
//! 1:1 翻译原 `core/src/services/autumn-activities.ts`（160 行，bot 2026-09-24）。
//!
//! ## 协议
//!
//! - 查询走 `ActivityService.GetGroup`（wish 状态在 ActivityData field 119 `wish_sign`、
//!   happy 在 field 120 `share_reward`）
//! - 写走 `ActivityService.Operate` + `AutumnOperateRequest{activity_id, operate_type,
//!   <field 151-157>}`：抽签 51 / 领奖 52 / 分享 69 / 档位领奖 70 / 日志 71 / 每日 73
//! - 回包三重校验（activity_id / operate_type / 对应 field 一致才算成功）
//! - 写成功后读状态失败降级为 `refreshRequired`，不整体报错（bot 149-151 注释）
//! - happy 分享刻意只调 op 69（回包带 Ark 上下文的 op 72 需要转发上下文，页面用不到）
//!
//! ## 数值配置
//!
//! 镜像 bot `activity-data/autumn-20260924.json`（祈愿方向 / 祈愿文案 / 每日奖励）。

use std::sync::OnceLock;

use prost::Message;
use serde::Deserialize;

use crate::constants::{
    ACTIVITY_SERVICE, AUTUMN_HAPPY_ACTIVITY_ID, AUTUMN_HAPPY_CLAIM_DAILY_OPERATE_TYPE,
    AUTUMN_HAPPY_CLAIM_MILESTONES_OPERATE_TYPE, AUTUMN_HAPPY_GET_LOGS_OPERATE_TYPE,
    AUTUMN_HAPPY_GROUP_ID, AUTUMN_HAPPY_SHARE_OPERATE_TYPE, AUTUMN_WISH_ACTIVITY_ID,
    AUTUMN_WISH_CLAIM_OPERATE_TYPE, AUTUMN_WISH_DRAW_OPERATE_TYPE, AUTUMN_WISH_GROUP_ID,
};
use crate::error::Result;
use crate::proto::generated::gamepb::activitypb::{
    ActivityData, ActivityOperateReply, AutumnOperateRequest, GetGroupReply, GetGroupRequest,
    ShareRewardClaimDailyReq, ShareRewardClaimMilestonesReq, ShareRewardGetLogsReq,
    ShareRewardShareReq, WishSignClaimReq, WishSignDrawReq,
};

use super::dto::text_content;
use super::error::{ActivityError, ActivityErrorCode};
use super::ActivityCenterService;

fn autumn_err(code: ActivityErrorCode, message: &str) -> ActivityError {
    ActivityError { code, message: message.to_string() }
}

// ============ 数值配置（镜像 bot activity-data/autumn-20260924.json） ============

#[derive(Deserialize)]
struct AutumnConfig {
    choices: Vec<AutumnChoice>,
    texts: Vec<AutumnText>,
    rewards: Vec<AutumnReward>,
}

#[derive(Deserialize)]
struct AutumnChoice {
    #[allow(dead_code)]
    activity_id: i64,
    choose_id: i64,
    desc: String,
}

#[derive(Deserialize)]
struct AutumnText {
    choose_id: i64,
    text_id: i64,
    desc: String,
}

#[derive(Deserialize)]
struct AutumnReward {
    #[allow(dead_code)]
    activity_id: i64,
    day_id: i64,
    /// 形如 `"6001:20"`（道具 ID:数量）
    reward: String,
    #[allow(dead_code)]
    display: i32,
}

fn autumn_config() -> &'static AutumnConfig {
    static CONFIG: OnceLock<AutumnConfig> = OnceLock::new();
    CONFIG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../assets/activity-data/autumn-20260924.json"
        ))
        .expect("解析秋日活动数值配置失败")
    })
}

// ============ 活动定义 ============

/// 两个子活动（bot `EVENTS`）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutumnEvent {
    Wish,
    Happy,
}

impl AutumnEvent {
    fn parse(key: &str) -> Result<Self> {
        match key {
            "wish" => Ok(Self::Wish),
            "happy" => Ok(Self::Happy),
            _ => Err(autumn_err(ActivityErrorCode::InvalidAutumnActivity, "未知活动").into()),
        }
    }

    const fn group_id(self) -> i64 {
        match self {
            Self::Wish => AUTUMN_WISH_GROUP_ID,
            Self::Happy => AUTUMN_HAPPY_GROUP_ID,
        }
    }

    const fn activity_id(self) -> i64 {
        match self {
            Self::Wish => AUTUMN_WISH_ACTIVITY_ID,
            Self::Happy => AUTUMN_HAPPY_ACTIVITY_ID,
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::Wish => "秋祈良愿",
            Self::Happy => "快乐不独享",
        }
    }
}

/// 操作定义（bot `OPERATIONS`）：action → (所属活动，操作码)
fn parse_operation(action: &str) -> Option<(AutumnEvent, i64)> {
    match action {
        "draw" => Some((AutumnEvent::Wish, AUTUMN_WISH_DRAW_OPERATE_TYPE)),
        "claim" => Some((AutumnEvent::Wish, AUTUMN_WISH_CLAIM_OPERATE_TYPE)),
        "daily" => Some((AutumnEvent::Happy, AUTUMN_HAPPY_CLAIM_DAILY_OPERATE_TYPE)),
        "milestones" => Some((AutumnEvent::Happy, AUTUMN_HAPPY_CLAIM_MILESTONES_OPERATE_TYPE)),
        "share" => Some((AutumnEvent::Happy, AUTUMN_HAPPY_SHARE_OPERATE_TYPE)),
        "logs" => Some((AutumnEvent::Happy, AUTUMN_HAPPY_GET_LOGS_OPERATE_TYPE)),
        _ => None,
    }
}

/// 递归找 activity_id 对应的活动节点（bot `findEntry`）
fn find_autumn_entry(entry: &ActivityData, activity_id: i64) -> Option<&ActivityData> {
    if entry.activity.as_ref().is_some_and(|a| a.activity_id == activity_id) {
        return Some(entry);
    }
    for child in &entry.children {
        if let Some(found) = find_autumn_entry(child, activity_id) {
            return Some(found);
        }
    }
    None
}

/// extra bytes → 活动规则数组（tips.txt 去标签；复用 `text_content` 的解析）
fn autumn_rules(extra: &[u8]) -> Vec<String> {
    text_content(extra)
        .get("paragraphs")
        .and_then(serde_json::Value::as_array)
        .map(|arr| arr.iter().filter_map(serde_json::Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

fn autumn_base(event: AutumnEvent, entry: &ActivityData) -> Result<(serde_json::Value, bool)> {
    let head = entry.activity.as_ref().ok_or_else(|| {
        autumn_err(ActivityErrorCode::AutumnStateUnavailable, "活动动态状态未返回，请稍后刷新")
    })?;
    let now = crate::utils::time::get_server_time_secs();
    let start_time = head.begin_time;
    let end_time = head.end_time;
    // bot：active = startTime <= now && endTime > now
    let active = start_time <= now && end_time > now;
    let base = serde_json::json!({
        "id": event.activity_id().to_string(),
        "title": event.title(),
        "serverTime": now * 1000,
        "startTime": start_time * 1000,
        "endTime": end_time * 1000,
        "active": active,
        "rules": autumn_rules(&head.extra),
    });
    Ok((base, active))
}

fn wish_state_dto(entry: &ActivityData) -> Result<serde_json::Value> {
    let (base, active) = autumn_base(AutumnEvent::Wish, entry)?;
    let state = entry.wish_sign.as_ref().ok_or_else(|| {
        autumn_err(ActivityErrorCode::AutumnStateUnavailable, "活动动态状态未返回，请稍后刷新")
    })?;
    let pending = state.pending.as_ref().filter(|p| p.choose_id > 0);
    let config = autumn_config();
    let pending_json = pending.map(|p| {
        let text = config
            .texts
            .iter()
            .find(|t| t.choose_id == p.choose_id && t.text_id == p.text_id)
            .map(|t| t.desc.clone())
            .unwrap_or_default();
        serde_json::json!({
            "chooseId": p.choose_id,
            "textId": p.text_id,
            "day": p.day_id,
            "text": text,
            "rewards": p.rewards.iter().map(super::dto::item_dto).collect::<Vec<_>>(),
        })
    });
    let reward_days: Vec<serde_json::Value> = config
        .rewards
        .iter()
        .map(|v| {
            let (id, count) = v
                .reward
                .split_once(':')
                .map(|(a, b)| (a.parse::<i64>().unwrap_or(0), b.parse::<i64>().unwrap_or(0)))
                .unwrap_or((0, 0));
            serde_json::json!({
                "day": v.day_id,
                "reward": super::dto::item_dto(&crate::proto::generated::corepb::Item {
                    id,
                    count,
                    ..Default::default()
                }),
            })
        })
        .collect();
    let mut dto = base;
    let obj = dto.as_object_mut().expect("base 必为对象");
    obj.insert("key".into(), serde_json::json!("wish"));
    obj.insert("remaining".into(), serde_json::json!(state.remaining_count));
    obj.insert("day".into(), serde_json::json!(state.activity_day));
    obj.insert(
        "choices".into(),
        serde_json::json!(config
            .choices
            .iter()
            .map(|v| serde_json::json!({ "id": v.choose_id, "name": v.desc }))
            .collect::<Vec<_>>()),
    );
    obj.insert("rewardDays".into(), serde_json::json!(reward_days));
    obj.insert("pending".into(), pending_json.unwrap_or(serde_json::Value::Null));
    obj.insert(
        "canDraw".into(),
        serde_json::json!(active && pending.is_none() && state.remaining_count > 0),
    );
    obj.insert("canClaim".into(), serde_json::json!(active && pending.is_some()));
    Ok(dto)
}

fn happy_state_dto(entry: &ActivityData) -> Result<serde_json::Value> {
    let (base, active) = autumn_base(AutumnEvent::Happy, entry)?;
    let state = entry.share_reward.as_ref().ok_or_else(|| {
        autumn_err(ActivityErrorCode::AutumnStateUnavailable, "活动动态状态未返回，请稍后刷新")
    })?;
    let summary = state.summary.as_ref().ok_or_else(|| {
        autumn_err(ActivityErrorCode::AutumnStateUnavailable, "快乐值进度未返回，请稍后刷新")
    })?;
    let daily = summary.daily.as_ref().ok_or_else(|| {
        autumn_err(ActivityErrorCode::AutumnStateUnavailable, "快乐值进度未返回，请稍后刷新")
    })?;
    let mut dto = base;
    let obj = dto.as_object_mut().expect("base 必为对象");
    obj.insert("key".into(), serde_json::json!("happy"));
    obj.insert("score".into(), serde_json::json!(summary.current_score));
    obj.insert("scoreItemId".into(), serde_json::json!(summary.score_item_id));
    obj.insert("dailyReward".into(), serde_json::json!(summary.daily_reward));
    obj.insert("firstShareReward".into(), serde_json::json!(summary.first_share_reward));
    obj.insert("claimedCount".into(), serde_json::json!(daily.claimed_count));
    obj.insert("claimLimit".into(), serde_json::json!(daily.claim_limit));
    obj.insert(
        "poolClaimedCount".into(),
        serde_json::json!(summary.my_pool.as_ref().map(|p| p.claimed_count).unwrap_or(0)),
    );
    obj.insert(
        "poolClaimLimit".into(),
        serde_json::json!(summary.my_pool.as_ref().map(|p| p.claim_limit).unwrap_or(0)),
    );
    obj.insert("canClaimDaily".into(), serde_json::json!(active && !daily.daily_reward_claimed));
    obj.insert("canShare".into(), serde_json::json!(active && !daily.first_share_awarded));
    obj.insert("firstShareAwarded".into(), serde_json::json!(daily.first_share_awarded));
    obj.insert(
        "canClaimMilestones".into(),
        serde_json::json!(active && summary.milestones.iter().any(|v| v.state == 2)),
    );
    obj.insert(
        "milestones".into(),
        serde_json::json!(summary
            .milestones
            .iter()
            .map(|v| serde_json::json!({
                "id": v.tier_id.to_string(),
                "threshold": v.threshold,
                "state": v.state,
                "rewards": v.rewards.iter().map(super::dto::item_dto).collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>()),
    );
    Ok(dto)
}

impl ActivityCenterService {
    async fn autumn_query(&self, event: AutumnEvent) -> Result<serde_json::Value> {
        let req = GetGroupRequest { group_id: event.group_id() };
        let body = self.gateway.request(ACTIVITY_SERVICE, "GetGroup", &req.encode_to_vec()).await?;
        let reply = GetGroupReply::decode(&body[..])?;
        let group = reply.group.as_ref().ok_or_else(|| {
            autumn_err(ActivityErrorCode::AutumnStateUnavailable, "活动动态状态未返回，请稍后刷新")
        })?;
        let entry = find_autumn_entry(group, event.activity_id()).ok_or_else(|| {
            autumn_err(ActivityErrorCode::AutumnStateUnavailable, "活动动态状态未返回，请稍后刷新")
        })?;
        match event {
            AutumnEvent::Wish => wish_state_dto(entry),
            AutumnEvent::Happy => happy_state_dto(entry),
        }
    }

    /// 秋日活动状态（`key`: `wish` / `happy`）。
    ///
    /// # Errors
    /// - 活动 key 非法 / 服务端未返回动态状态 / 网络 RPC 错误
    pub async fn get_autumn_activity(&self, key: &str) -> Result<serde_json::Value> {
        self.autumn_query(AutumnEvent::parse(key)?).await
    }

    /// 秋日活动操作（`action`: draw / claim / daily / milestones / share / logs）。
    ///
    /// 写操作经 `mutation_lock` 串行化（对齐 bot `mutationTail`）。
    ///
    /// # Errors
    /// - 活动结束 / 门控不满足 / 回包不完整 / 网络 RPC 错误
    pub async fn operate_autumn_activity(
        &self,
        key: &str,
        action: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let event = AutumnEvent::parse(key)?;
        let Some((op_event, operate_type)) = parse_operation(action) else {
            return Err(
                autumn_err(ActivityErrorCode::InvalidAutumnOperation, "活动操作不匹配").into()
            );
        };
        if op_event != event {
            return Err(
                autumn_err(ActivityErrorCode::InvalidAutumnOperation, "活动操作不匹配").into()
            );
        }
        let _guard = self.mutation_lock.lock().await;
        let state = self.autumn_query(event).await?;
        let params = autumn_operation_params(action, &state, input)?;

        let req = build_autumn_operate_request(event.activity_id(), operate_type, action, params);
        let body = self.gateway.request(ACTIVITY_SERVICE, "Operate", &req.encode_to_vec()).await?;
        let reply = ActivityOperateReply::decode(&body[..])?;
        let field_present = match action {
            "draw" => reply.wish_sign_draw.is_some(),
            "claim" => reply.wish_sign_claim.is_some(),
            "share" => reply.share_reward_share.is_some(),
            "milestones" => reply.share_reward_claim_milestones.is_some(),
            "logs" => reply.share_reward_get_logs.is_some(),
            "daily" => reply.share_reward_claim_daily.is_some(),
            _ => false,
        };
        if reply.activity_id != event.activity_id()
            || reply.operate_type != operate_type
            || !field_present
        {
            return Err(autumn_err(
                ActivityErrorCode::AutumnResponseInvalid,
                "操作回包不完整，请刷新确认结果，勿重复提交",
            )
            .into());
        }

        // 页面只需要奖励 / 日志数据（share 刻意丢弃 Ark 转发上下文）
        let (result, rewards) = autumn_operation_result(action, &reply);
        // 已完成的写操作不能因回读失败而报失败（bot 149-151 注释）
        match self.autumn_query(event).await {
            Ok(activity) => Ok(serde_json::json!({
                "activity": activity,
                "result": result,
                "rewards": rewards,
            })),
            Err(_) => Ok(serde_json::json!({
                "activity": null,
                "result": result,
                "rewards": rewards,
                "refreshRequired": true,
            })),
        }
    }
}

/// 操作前置门控（bot `parameters`）
fn autumn_operation_params(
    action: &str,
    state: &serde_json::Value,
    input: &serde_json::Value,
) -> Result<serde_json::Value> {
    let active = state.get("active").and_then(serde_json::Value::as_bool).unwrap_or(false);
    if !active {
        return Err(
            autumn_err(ActivityErrorCode::AutumnActivityEnded, "活动尚未开放或已经结束").into()
        );
    }
    match action {
        "draw" => {
            let can_draw =
                state.get("canDraw").and_then(serde_json::Value::as_bool).unwrap_or(false);
            if !can_draw {
                return Err(autumn_err(
                    ActivityErrorCode::WishDrawUnavailable,
                    "请先领取待领取奖励，或等待明日祈愿",
                )
                .into());
            }
            let choose_id = input.get("chooseId").and_then(serde_json::Value::as_i64);
            let valid = choose_id.is_some_and(|id| {
                state.get("choices").and_then(serde_json::Value::as_array).is_some_and(|choices| {
                    choices
                        .iter()
                        .any(|c| c.get("id").and_then(serde_json::Value::as_i64) == Some(id))
                })
            });
            if !valid {
                return Err(autumn_err(
                    ActivityErrorCode::InvalidWishChoice,
                    "请选择有效的祈愿方向",
                )
                .into());
            }
            Ok(serde_json::json!({ "choose_id": choose_id }))
        }
        "claim" => {
            let can_claim =
                state.get("canClaim").and_then(serde_json::Value::as_bool).unwrap_or(false);
            if !can_claim {
                return Err(autumn_err(
                    ActivityErrorCode::WishClaimUnavailable,
                    "当前没有待领取的祈愿奖励",
                )
                .into());
            }
            let choose_id =
                state.pointer("/pending/chooseId").and_then(serde_json::Value::as_i64).unwrap_or(0);
            Ok(serde_json::json!({ "choose_id": choose_id }))
        }
        "daily" => {
            let can =
                state.get("canClaimDaily").and_then(serde_json::Value::as_bool).unwrap_or(false);
            if !can {
                return Err(
                    autumn_err(ActivityErrorCode::HappyDailyClaimed, "今日快乐值已领取").into()
                );
            }
            Ok(serde_json::json!({}))
        }
        "share" => {
            let can = state.get("canShare").and_then(serde_json::Value::as_bool).unwrap_or(false);
            if !can {
                return Err(
                    autumn_err(ActivityErrorCode::HappyShareClaimed, "今日分享奖励已领取").into()
                );
            }
            Ok(serde_json::json!({}))
        }
        "milestones" => {
            let can = state
                .get("canClaimMilestones")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if !can {
                return Err(autumn_err(
                    ActivityErrorCode::HappyMilestoneUnavailable,
                    "当前没有可领取的档位奖励",
                )
                .into());
            }
            Ok(serde_json::json!({}))
        }
        // bot：logs 固定拉全量（page=-1, page_size=100），tab 0=领取我的 / 1=我领取的
        "logs" => {
            let tab =
                if input.get("tab").and_then(serde_json::Value::as_i64) == Some(0) { 0 } else { 1 };
            Ok(serde_json::json!({ "page": -1, "page_size": 100, "tab": tab }))
        }
        _ => Ok(serde_json::json!({})),
    }
}

/// 组装 `AutumnOperateRequest`（仅对应操作 field 置 Some）
#[must_use]
fn build_autumn_operate_request(
    activity_id: i64,
    operate_type: i64,
    action: &str,
    params: serde_json::Value,
) -> AutumnOperateRequest {
    let choose_id = || {
        params.get("choose_id").and_then(serde_json::Value::as_i64).unwrap_or_else(|| {
            params.get("chooseId").and_then(serde_json::Value::as_i64).unwrap_or(0)
        })
    };
    let mut req = AutumnOperateRequest { activity_id, operate_type, ..Default::default() };
    match action {
        "draw" => {
            req.wish_sign_draw = Some(WishSignDrawReq { choose_id: choose_id() });
        }
        "claim" => {
            req.wish_sign_claim = Some(WishSignClaimReq { choose_id: choose_id() });
        }
        "share" => {
            req.share_reward_share = Some(ShareRewardShareReq {});
        }
        "milestones" => {
            req.share_reward_claim_milestones = Some(ShareRewardClaimMilestonesReq {});
        }
        "logs" => {
            let get = |k: &str| params.get(k).and_then(serde_json::Value::as_i64).unwrap_or(0);
            req.share_reward_get_logs = Some(ShareRewardGetLogsReq {
                page: i32::try_from(get("page")).unwrap_or(0),
                page_size: i32::try_from(get("page_size")).unwrap_or(0),
                tab: i32::try_from(get("tab")).unwrap_or(0),
            });
        }
        "daily" => {
            req.share_reward_claim_daily = Some(ShareRewardClaimDailyReq {});
        }
        _ => {}
    }
    req
}

/// 回包 →（result JSON, 奖励列表）
fn autumn_operation_result(
    action: &str,
    reply: &ActivityOperateReply,
) -> (serde_json::Value, Vec<super::ItemDto>) {
    let item_dtos = |items: &[crate::proto::generated::corepb::Item]| {
        items.iter().map(super::dto::item_dto).collect::<Vec<_>>()
    };
    match action {
        "draw" => {
            let Some(rsp) = reply.wish_sign_draw.as_ref() else {
                return (serde_json::json!({}), Vec::new());
            };
            (
                serde_json::json!({ "textId": rsp.text_id, "dayId": rsp.day_id }),
                item_dtos(&rsp.rewards),
            )
        }
        "claim" => {
            let Some(rsp) = reply.wish_sign_claim.as_ref() else {
                return (serde_json::json!({}), Vec::new());
            };
            (serde_json::json!({}), item_dtos(&rsp.awards))
        }
        "share" => {
            let granted = reply.share_reward_share.as_ref().map(|r| r.granted_score).unwrap_or(0);
            (serde_json::json!({ "grantedScore": granted }), Vec::new())
        }
        "milestones" => {
            let Some(rsp) = reply.share_reward_claim_milestones.as_ref() else {
                return (serde_json::json!({}), Vec::new());
            };
            (
                serde_json::json!({
                    "claimedTierIds": rsp.claimed_tier_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
                }),
                item_dtos(&rsp.rewards),
            )
        }
        "daily" => {
            let Some(rsp) = reply.share_reward_claim_daily.as_ref() else {
                return (serde_json::json!({}), Vec::new());
            };
            (serde_json::json!({ "grantedScore": rsp.granted_score }), item_dtos(&rsp.rewards))
        }
        "logs" => {
            let Some(rsp) = reply.share_reward_get_logs.as_ref() else {
                return (serde_json::json!({}), Vec::new());
            };
            (
                serde_json::json!({
                    "total": rsp.total,
                    "logs": rsp.logs.iter().map(|entry| serde_json::json!({
                        "seq": entry.seq,
                        "kind": entry.kind,
                        "score": entry.score,
                        "createdAt": entry.created_at,
                        "actor": entry.actor.as_ref().map(|a| serde_json::json!({
                            "name": a.name,
                        })),
                    })).collect::<Vec<_>>(),
                }),
                Vec::new(),
            )
        }
        _ => (serde_json::json!({}), Vec::new()),
    }
}

// =====================================================================
// 单元测试（门控纯函数 + 请求组装）
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn wish_state(can_draw: bool, pending_choose: Option<i64>, active: bool) -> serde_json::Value {
        serde_json::json!({
            "active": active,
            "canDraw": can_draw,
            "canClaim": pending_choose.is_some(),
            "pending": pending_choose.map(|id| serde_json::json!({ "chooseId": id })),
            "choices": [ { "id": 1 }, { "id": 2 }, { "id": 3 } ],
        })
    }

    #[test]
    fn operation_table_matches_ts() {
        assert_eq!(
            parse_operation("draw"),
            Some((AutumnEvent::Wish, AUTUMN_WISH_DRAW_OPERATE_TYPE))
        );
        assert_eq!(
            parse_operation("claim"),
            Some((AutumnEvent::Wish, AUTUMN_WISH_CLAIM_OPERATE_TYPE))
        );
        assert_eq!(
            parse_operation("daily"),
            Some((AutumnEvent::Happy, AUTUMN_HAPPY_CLAIM_DAILY_OPERATE_TYPE))
        );
        assert_eq!(
            parse_operation("milestones"),
            Some((AutumnEvent::Happy, AUTUMN_HAPPY_CLAIM_MILESTONES_OPERATE_TYPE))
        );
        assert_eq!(
            parse_operation("share"),
            Some((AutumnEvent::Happy, AUTUMN_HAPPY_SHARE_OPERATE_TYPE))
        );
        assert_eq!(
            parse_operation("logs"),
            Some((AutumnEvent::Happy, AUTUMN_HAPPY_GET_LOGS_OPERATE_TYPE))
        );
        assert_eq!(parse_operation("other"), None);
    }

    #[test]
    fn event_ids_match_ts_constants() {
        assert_eq!(AutumnEvent::Wish.group_id(), 2_026_092_400);
        assert_eq!(AutumnEvent::Wish.activity_id(), 2_026_092_401);
        assert_eq!(AutumnEvent::Happy.group_id(), 2_026_092_500);
        assert_eq!(AutumnEvent::Happy.activity_id(), 2_026_092_501);
        assert_eq!(AutumnEvent::parse("wish").unwrap(), AutumnEvent::Wish);
        assert!(AutumnEvent::parse("nope").is_err());
    }

    #[test]
    fn params_gate_activity_ended() {
        let state = wish_state(true, None, false);
        let err = autumn_operation_params("draw", &state, &serde_json::json!({})).unwrap_err();
        assert!(err.to_string().contains("AUTUMN_ACTIVITY_ENDED"));
    }

    #[test]
    fn params_draw_requires_choice_in_list() {
        let state = wish_state(true, None, true);
        let p =
            autumn_operation_params("draw", &state, &serde_json::json!({ "chooseId": 2 })).unwrap();
        assert_eq!(p["choose_id"], 2);
        // 非法祈愿方向
        let err = autumn_operation_params("draw", &state, &serde_json::json!({ "chooseId": 9 }))
            .unwrap_err();
        assert!(err.to_string().contains("INVALID_WISH_CHOICE"));
        // 不可抽签
        let state = wish_state(false, None, true);
        let err = autumn_operation_params("draw", &state, &serde_json::json!({ "chooseId": 1 }))
            .unwrap_err();
        assert!(err.to_string().contains("WISH_DRAW_UNAVAILABLE"));
    }

    #[test]
    fn params_claim_uses_pending_choose_id() {
        let state = wish_state(false, Some(3), true);
        let p = autumn_operation_params("claim", &state, &serde_json::json!({})).unwrap();
        assert_eq!(p["choose_id"], 3);
        let state = wish_state(false, None, true);
        let err = autumn_operation_params("claim", &state, &serde_json::json!({})).unwrap_err();
        assert!(err.to_string().contains("WISH_CLAIM_UNAVAILABLE"));
    }

    #[test]
    fn params_logs_defaults_tab_one() {
        let state = serde_json::json!({ "active": true });
        let p = autumn_operation_params("logs", &state, &serde_json::json!({})).unwrap();
        assert_eq!(p["page"], -1);
        assert_eq!(p["page_size"], 100);
        assert_eq!(p["tab"], 1);
        let p = autumn_operation_params("logs", &state, &serde_json::json!({ "tab": 0 })).unwrap();
        assert_eq!(p["tab"], 0);
    }

    #[test]
    fn build_request_sets_only_matching_field() {
        let req = build_autumn_operate_request(
            AUTUMN_WISH_ACTIVITY_ID,
            AUTUMN_WISH_DRAW_OPERATE_TYPE,
            "draw",
            serde_json::json!({ "choose_id": 5 }),
        );
        assert_eq!(req.activity_id, AUTUMN_WISH_ACTIVITY_ID);
        assert_eq!(req.operate_type, AUTUMN_WISH_DRAW_OPERATE_TYPE);
        assert_eq!(req.wish_sign_draw.unwrap().choose_id, 5);
        assert!(req.share_reward_share.is_none());

        let req = build_autumn_operate_request(
            AUTUMN_HAPPY_ACTIVITY_ID,
            AUTUMN_HAPPY_GET_LOGS_OPERATE_TYPE,
            "logs",
            serde_json::json!({ "page": -1, "page_size": 100, "tab": 1 }),
        );
        let logs = req.share_reward_get_logs.unwrap();
        assert_eq!(logs.page, -1);
        assert_eq!(logs.page_size, 100);
        assert_eq!(logs.tab, 1);
    }

    #[test]
    fn config_renders_choices_and_rewards() {
        let cfg = autumn_config();
        assert_eq!(cfg.choices.len(), 6);
        assert_eq!(cfg.choices[0].choose_id, 1);
        assert_eq!(cfg.choices[0].desc, "财运");
        assert!(!cfg.rewards.is_empty());
        // "6001:20" → id 6001 count 20
        let first = &cfg.rewards[0];
        let (id, count) = first.reward.split_once(':').unwrap();
        assert_eq!(id.parse::<i64>().unwrap(), 6001);
        assert_eq!(count.parse::<i64>().unwrap(), 20);
    }
}
