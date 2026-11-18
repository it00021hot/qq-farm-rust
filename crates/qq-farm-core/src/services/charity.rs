//! 公益小红花（bot `activity-center/charity.ts`）。
//!
//! 读取走 `ActivityService.List`。没有 `2026090901` 的小红花状态就返回空页面。
//! 领种子、捐赠、进度奖励、每日礼包分别是 `Operate` 35、36、37、38。
//! 页面不发送协议里的 39。进度领取记录按账号写入活动状态文件。

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use prost::Message as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::game_config::global as global_game_config;
use crate::error::{Error, Result};
use crate::network::error::NetworkError;
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::activitypb::{
    charity_red_flower_operate_request, ActivityData, ActivityItem, ActivityListReply,
    ActivityListRequest, ActivityOperateReply, ActivityWindow, CharityRedFlowerData,
    CharityRedFlowerOperateRequest,
};
use crate::utils::time::{get_server_time_secs, today_system_date_key};

const ACTIVITY_SERVICE: &str = "gamepb.activitypb.ActivityService";
const GROUP_ID: i64 = 2_026_090_900;
const ACTIVITY_ID: i64 = 2_026_090_901;
const CLAIM_SEED: i64 = 35;
const DONATE: i64 = 36;
const CLAIM_PROGRESS: i64 = 37;
const CLAIM_DAILY: i64 = 38;
const ALREADY_CLAIMED: i64 = 1_034_087;
const FLOW_HARVESTED: i64 = 2;
const FLOW_DAILY_CLAIMED: i64 = 3;
const SEED_CLAIMABLE: i64 = 2;
const SEED_CLAIMED: i64 = 3;
const PROGRESS_UNLOCKED: i64 = 1;
const SESSION_MOVED: &str = "账号会话已变化，未发送操作";
const ACCOUNT_INVALID: &str = "账号身份无效，未发送操作";
const PERSIST_WARNING: &str = "已确认进度未能保存";

pub struct CharityService {
    gateway: Arc<Gateway>,
    account_id: String,
    mutation_lock: tokio::sync::Mutex<()>,
    progress: Mutex<ProgressState>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ProgressState {
    initialized: bool,
    claimed: BTreeSet<String>,
    pending: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ProgressRecord {
    #[serde(rename = "activityId")]
    activity_id: String,
    initialized: bool,
    #[serde(rename = "claimedProgressTargets")]
    claimed_progress_targets: Vec<String>,
    #[serde(rename = "pendingProgressTargets")]
    pending_progress_targets: Vec<String>,
}

impl CharityService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>, account_id: impl Into<String>) -> Self {
        Self {
            gateway,
            account_id: account_id.into(),
            mutation_lock: tokio::sync::Mutex::new(()),
            progress: Mutex::new(ProgressState::default()),
        }
    }

    pub async fn snapshot(&self) -> Result<serde_json::Value> {
        self.snapshot_on(&compact_today()).await
    }

    pub async fn claim_seeds(&self) -> Result<serde_json::Value> {
        self.claim_seeds_if(|| true).await
    }

    pub async fn claim_seeds_if(
        &self,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        self.write(
            still_current,
            |board| board.seed_enabled,
            CLAIM_SEED,
            seed_request(),
            |reply| {
                reward_payload(
                    reply.charity_seed_result.as_ref().and_then(|item| item.reward.clone()),
                    &reply.rewards,
                    "小红花种子领取成功",
                )
            },
        )
        .await
    }

    pub async fn donate(&self) -> Result<serde_json::Value> {
        self.donate_if(|| true).await
    }

    pub async fn donate_if(&self, still_current: impl Fn() -> bool) -> Result<serde_json::Value> {
        self.write(still_current, |board| board.donate_enabled, DONATE, donate_request(), |reply| {
            let donated = reply
                .charity_donate_result
                .as_ref()
                .map(|item| if item.donated != 0 { item.donated } else { item.count })
                .unwrap_or(0);
            serde_json::json!({
                "donated": donated.to_string(),
                "globalDonated": reply.charity_donate_result.as_ref().map(|item| item.global_donated).unwrap_or(0).to_string(),
                "message": format!("已捐赠全部 {donated} 份爱心"),
            })
        })
        .await
    }

    pub async fn claim_daily_gift(&self) -> Result<serde_json::Value> {
        self.claim_daily_gift_if(|| true).await
    }

    pub async fn claim_daily_gift_if(
        &self,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        self.write(
            still_current,
            |board| board.daily_enabled,
            CLAIM_DAILY,
            daily_request(),
            |reply| {
                let mut payload = reward_payload(
                    reply.charity_public_fund_result.as_ref().and_then(|item| item.reward.clone()),
                    &reply.rewards,
                    "今日公益礼包领取成功",
                );
                if let Some(result) = reply.charity_public_fund_result.as_ref() {
                    payload["publicFund"] =
                        serde_json::json!({ "statusCode": result.status.to_string() });
                }
                payload
            },
        )
        .await
    }

    pub async fn claim_progress(&self, target: i64) -> Result<serde_json::Value> {
        self.claim_progress_if(target, || true).await
    }

    pub async fn claim_progress_if(
        &self,
        target: i64,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if target <= 0 {
            return Err(Error::Business("target 必须是正十进制整数".into()));
        }
        let target_text = target.to_string();
        let _guard = self.mutation_lock.lock().await;
        self.ensure_account()?;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let board = self.current_board().await?;
        if !board.claimable_targets.iter().any(|item| item == &target_text) {
            return Err(Error::Business("当前不能领取该进度奖励".into()));
        }
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let request = CharityRedFlowerOperateRequest {
            activity_id: ACTIVITY_ID,
            operate_type: CLAIM_PROGRESS,
            claim_progress_reward: Some(charity_red_flower_operate_request::ProgressRewardParams {
                target,
            }),
            ..CharityRedFlowerOperateRequest::default()
        };
        match self.gateway.request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec()).await {
            Ok(bytes) => {
                let reply = ActivityOperateReply::decode(bytes.as_slice())?;
                expect_reply(&reply, CLAIM_PROGRESS)?;
                let warning = self.remember_claimed(&target_text);
                let mut payload = reward_payload(
                    reply
                        .charity_progress_reward_result
                        .as_ref()
                        .and_then(|item| item.reward.clone()),
                    &reply.rewards,
                    &format!("公益进度奖励领取成功（{target_text} 份爱心）"),
                );
                payload["target"] = serde_json::json!(target_text);
                payload["claimed"] = serde_json::json!(true);
                payload["alreadyClaimed"] = serde_json::json!(false);
                self.finish(payload, &reply, warning).await
            }
            Err(NetworkError::Gateway { code, .. }) if code == ALREADY_CLAIMED => {
                let warning = self.remember_claimed(&target_text);
                let payload = serde_json::json!({
                    "target": target_text,
                    "rewards": [],
                    "claimed": true,
                    "alreadyClaimed": true,
                    "message": format!("公益进度奖励已领取（{target_text} 份爱心）"),
                });
                self.finish(payload, &ActivityOperateReply::default(), warning).await
            }
            Err(error) => Err(Error::Network(error)),
        }
    }

    async fn write(
        &self,
        still_current: impl Fn() -> bool,
        enabled: impl Fn(&BoardGate) -> bool,
        operate_type: i64,
        request: CharityRedFlowerOperateRequest,
        payload_from: impl Fn(&ActivityOperateReply) -> serde_json::Value,
    ) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        self.ensure_account()?;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let board = self.current_board().await?;
        if !enabled(&board) {
            return Err(Error::Business("当前不能执行该公益操作".into()));
        }
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let bytes = self
            .gateway
            .request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        let reply = ActivityOperateReply::decode(bytes.as_slice())?;
        expect_reply(&reply, operate_type)?;
        self.finish(payload_from(&reply), &reply, None).await
    }

    async fn finish(
        &self,
        mut payload: serde_json::Value,
        reply: &ActivityOperateReply,
        warning: Option<String>,
    ) -> Result<serde_json::Value> {
        if let Some(warning) = warning {
            payload["persistenceWarning"] = serde_json::json!(warning);
        }
        match self.snapshot_after(reply).await {
            Ok(snapshot) => payload["snapshot"] = snapshot,
            Err(_) => payload["refreshRequired"] = serde_json::json!(true),
        }
        Ok(payload)
    }

    async fn snapshot_on(&self, today: &str) -> Result<serde_json::Value> {
        let reply = self.list().await?;
        Ok(self.dto_from_list(&reply, today))
    }

    async fn current_board(&self) -> Result<BoardGate> {
        let value = self.snapshot_on(&compact_today()).await?;
        if value.get("available").and_then(serde_json::Value::as_bool) != Some(true) {
            return Err(Error::Business("当前账号暂未发现公益小红花活动".into()));
        }
        let claimable = value
            .get("progressRewards")
            .and_then(serde_json::Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter(|row| {
                        row.get("claimable").and_then(serde_json::Value::as_bool) == Some(true)
                    })
                    .filter_map(|row| row.get("target").and_then(serde_json::Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        Ok(BoardGate {
            seed_enabled: flag(&value, "/actions/claimSeeds/enabled"),
            donate_enabled: flag(&value, "/actions/donateLove/enabled"),
            daily_enabled: flag(&value, "/actions/claimDailyGift/enabled"),
            claimable_targets: claimable,
        })
    }

    async fn snapshot_after(&self, reply: &ActivityOperateReply) -> Result<serde_json::Value> {
        if reply.data.as_ref().is_some_and(|entry| entry.charity_red_flower.is_some()) {
            return Ok(self.dto_from_entry(reply.data.as_ref().unwrap(), None, &compact_today()));
        }
        self.snapshot_on(&compact_today()).await
    }

    async fn list(&self) -> Result<ActivityListReply> {
        let body = ActivityListRequest { game_scene_id: 1 }.encode_to_vec();
        let bytes =
            self.gateway.request(ACTIVITY_SERVICE, "List", &body).await.map_err(Error::Network)?;
        Ok(ActivityListReply::decode(bytes.as_slice())?)
    }

    fn dto_from_list(&self, reply: &ActivityListReply, today: &str) -> serde_json::Value {
        let Some(entry) = find_activity(&reply.activities, ACTIVITY_ID) else {
            return serde_json::json!({ "available": false });
        };
        if entry.charity_red_flower.is_none() {
            return serde_json::json!({ "available": false });
        }
        self.dto_from_entry(entry, find_window(&reply.activity_windows), today)
    }

    fn dto_from_entry(
        &self,
        entry: &ActivityData,
        window: Option<&ActivityWindow>,
        today: &str,
    ) -> serde_json::Value {
        let Some(state) = entry.charity_red_flower.clone() else {
            return serde_json::json!({ "available": false });
        };
        let activity = entry.activity.clone().unwrap_or_default();
        let progress = self.reconcile(&state);
        let now = get_server_time_secs();
        let start = nonzero(window.map(|item| item.begin_time).unwrap_or(0), activity.begin_time);
        let activity_end =
            nonzero(window.map(|item| item.end_time).unwrap_or(0), activity.end_time);
        let end = if state.end_time > 0 { state.end_time } else { activity_end };
        let active = charity_window_is_open(start, end, now);
        let reached = reached_targets(&state);
        let claimed = progress.claimed.clone();
        let pending = progress.pending.clone();
        let daily_claimed = state.flow_status == FLOW_DAILY_CLAIMED
            || state
                .public_fund
                .as_ref()
                .is_some_and(|fund| fund.date != 0 && fund.date.to_string() == today);
        let harvested =
            state.flow_status == FLOW_HARVESTED || state.flow_status == FLOW_DAILY_CLAIMED;
        let personal_reached = state.donated_love >= state.settlement_required_love;
        let reward_target = if state.global_reward.as_ref().is_some_and(|item| item.target != 0) {
            state.global_reward.as_ref().map(|item| item.target).unwrap_or(0)
        } else {
            state.global_target_love
        };
        let settlement_target =
            if reward_target != 0 { reward_target } else { state.global_target_love };
        let global_reached =
            settlement_target != 0 && state.global_donated_love >= settlement_target;
        let eligible = global_reached && personal_reached;
        let short = (state.settlement_required_love - state.donated_love).max(0);
        let settlement_status = if eligible {
            "已获得结算资格".to_owned()
        } else if !personal_reached {
            format!("还差 {short} 份爱心")
        } else if !global_reached {
            "等待全服爱心目标达成".to_owned()
        } else {
            "等待活动结束结算".to_owned()
        };
        let seed_status = if state.seed_reward_status == SEED_CLAIMED {
            "今日已领取"
        } else if active && state.seed_reward_status == SEED_CLAIMABLE {
            "可以领取"
        } else {
            "完成今日任务后可领取"
        };
        let daily_status = if daily_claimed {
            "今日已领取"
        } else if !harvested {
            "今日收获小红花后可领取"
        } else {
            "可以领取"
        };
        let name =
            if activity.name.is_empty() { "公益小红花".to_owned() } else { activity.name };
        let (title, paragraphs) = crate::services::activity_text::text_content(&activity.extra);
        let progress_rows = state
            .progress_rewards
            .iter()
            .map(|reward| {
                let target = reward.target.to_string();
                let is_reached = reached.iter().any(|item| item == &target);
                let is_claimed = claimed.contains(&target);
                serde_json::json!({
                    "target": target,
                    "reward": item_value(reward.reward.as_ref()),
                    "statusCode": reward.status.to_string(),
                    "reached": is_reached,
                    "claimed": is_claimed,
                    "claimable": active && is_reached && reward.status == PROGRESS_UNLOCKED && !is_claimed && pending.contains(&target),
                    "claimSupported": true,
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "available": true,
            "groupId": GROUP_ID.to_string(),
            "activityId": ACTIVITY_ID.to_string(),
            "name": name,
            "title": name,
            "startTime": start.to_string(),
            "endTime": end.to_string(),
            "serverTime": now.to_string(),
            "active": active,
            "rules": { "title": title, "paragraphs": paragraphs },
            "loveBalance": state.love_balance.to_string(),
            "loveBalanceText": grouped(state.love_balance),
            "donatedLove": state.donated_love.to_string(),
            "donatedLoveText": grouped(state.donated_love),
            "flowStatus": state.flow_status.to_string(),
            "agreementStatus": state.agreement_status.to_string(),
            "seedStatus": seed_status,
            "seedReward": {
                "statusCode": state.seed_reward_status.to_string(),
                "claimable": active && state.seed_reward_status == SEED_CLAIMABLE,
                "claimed": state.seed_reward_status == SEED_CLAIMED,
                "reward": item_value(state.seed_reward.as_ref()),
            },
            "dailyStatus": daily_status,
            "dailyGift": {
                "statusCode": state.daily_reward_status.to_string(),
                "claimed": daily_claimed,
                "harvestedToday": harvested,
                "reward": item_value(state.daily_reward.as_ref()),
                "publicFund": state.public_fund.as_ref().filter(|fund| fund.date != 0).map(|fund| serde_json::json!({
                    "date": fund.date.to_string(),
                    "dateText": fund_date(fund.date),
                    "statusCode": fund.status.to_string(),
                })),
            },
            "progressRewards": progress_rows,
            "globalPercent": percent(state.global_donated_love, state.global_target_love),
            "globalProgress": {
                "donated": state.global_donated_love.to_string(),
                "donatedText": grouped(state.global_donated_love),
                "target": state.global_target_love.to_string(),
                "targetText": grouped(state.global_target_love),
                "reached": state.global_target_love > 0 && state.global_donated_love >= state.global_target_love,
                "rewardTarget": reward_target.to_string(),
                "reward": item_value(state.global_reward.as_ref().and_then(|item| item.reward.as_ref())),
            },
            "settlementStatus": settlement_status,
            "settlement": {
                "requiredLove": state.settlement_required_love.to_string(),
                "requiredLoveText": grouped(state.settlement_required_love),
                "eligible": eligible,
                "globalReached": global_reached,
                "personalReached": personal_reached,
                "reward": item_value(state.settlement_reward.as_ref()),
            },
            "actions": {
                "claimSeeds": { "enabled": active && state.seed_reward_status == SEED_CLAIMABLE },
                "donateLove": { "enabled": active && state.love_balance > 0, "count": state.love_balance },
                "claimDailyGift": { "enabled": active && harvested && !daily_claimed },
            },
        })
    }

    fn reconcile(&self, state: &CharityRedFlowerData) -> ProgressState {
        crate::services::activity_state_file::with_account_state(&self.account_id, || {
            let stored = load_progress_unlocked(&self.account_id);
            let mut merged =
                merge_progress(&stored, &self.progress.lock().expect("progress").clone());
            let reached = reached_targets(state);
            if !merged.initialized {
                if reached.len() > 1 {
                    merged.claimed.extend(reached[..reached.len() - 1].iter().cloned());
                }
                if let Some(last) = reached.last() {
                    merged.pending.insert(last.clone());
                }
            } else {
                for target in &reached {
                    if !merged.claimed.contains(target) && !merged.pending.contains(target) {
                        merged.pending.insert(target.clone());
                    }
                }
            }
            merged.initialized = true;
            for target in &merged.claimed.clone() {
                merged.pending.remove(target);
            }
            *self.progress.lock().expect("progress") = merged.clone();
            let _warning = save_progress_unlocked(&self.account_id, &merged);
            merged
        })
    }

    fn remember_claimed(&self, target: &str) -> Option<String> {
        crate::services::activity_state_file::with_account_state(&self.account_id, || {
            let mut current = merge_progress(
                &load_progress_unlocked(&self.account_id),
                &self.progress.lock().expect("progress").clone(),
            );
            current.initialized = true;
            current.claimed.insert(target.to_owned());
            current.pending.remove(target);
            *self.progress.lock().expect("progress") = current.clone();
            save_progress_unlocked(&self.account_id, &current).err()
        })
    }

    fn ensure_account(&self) -> Result<()> {
        if account_is_usable(&self.account_id) {
            Ok(())
        } else {
            Err(Error::Business(ACCOUNT_INVALID.into()))
        }
    }
}

struct BoardGate {
    seed_enabled: bool,
    donate_enabled: bool,
    daily_enabled: bool,
    claimable_targets: Vec<String>,
}

pub fn require_charity_account(account_id: &str) -> Result<()> {
    if account_is_usable(account_id) {
        Ok(())
    } else {
        Err(Error::Business(ACCOUNT_INVALID.into()))
    }
}

/// 与 `activityWindowIsActive` 相同：开始为 0 或已到，结束为 0 或含结束时刻。
#[must_use]
pub fn charity_window_is_open(begin: i64, end: i64, now: i64) -> bool {
    (begin <= 0 || now >= begin) && (end <= 0 || now <= end)
}

fn account_is_usable(account_id: &str) -> bool {
    !account_id.is_empty() && !account_id.chars().all(char::is_whitespace)
}

fn compact_today() -> String {
    today_system_date_key().replace('-', "")
}

fn nonzero(preferred: i64, fallback: i64) -> i64 {
    if preferred != 0 {
        preferred
    } else {
        fallback
    }
}

fn flag(value: &serde_json::Value, pointer: &str) -> bool {
    value.pointer(pointer).and_then(serde_json::Value::as_bool) == Some(true)
}

fn expect_reply(reply: &ActivityOperateReply, operate_type: i64) -> Result<()> {
    if reply.activity_id != ACTIVITY_ID || reply.operate_type != operate_type {
        Err(Error::Business("公益小红花回包不匹配，请刷新确认结果，勿重复提交".into()))
    } else {
        Ok(())
    }
}

fn find_activity(entries: &[ActivityData], id: i64) -> Option<&ActivityData> {
    for entry in entries {
        if entry.activity.as_ref().is_some_and(|item| item.activity_id == id) {
            return Some(entry);
        }
        if let Some(found) = find_activity(&entry.children, id) {
            return Some(found);
        }
    }
    None
}

fn find_window(windows: &[ActivityWindow]) -> Option<&ActivityWindow> {
    windows
        .iter()
        .find(|item| item.id == ACTIVITY_ID)
        .or_else(|| windows.iter().find(|item| item.id == GROUP_ID))
}

fn reached_targets(state: &CharityRedFlowerData) -> Vec<String> {
    state
        .progress_rewards
        .iter()
        .filter(|reward| {
            reward.status == PROGRESS_UNLOCKED
                && reward.target != 0
                && state.donated_love >= reward.target
        })
        .map(|reward| reward.target.to_string())
        .collect()
}

fn reward_payload(
    primary: Option<CoreItem>,
    fallback: &[CoreItem],
    message: &str,
) -> serde_json::Value {
    let rewards = primary
        .map(|item| vec![reward_value(&item)])
        .unwrap_or_else(|| fallback.iter().map(reward_value).collect());
    serde_json::json!({ "rewards": rewards, "message": message })
}

fn reward_value(item: &CoreItem) -> serde_json::Value {
    serde_json::json!({
        "id": item.id.to_string(),
        "count": item.count,
        "name": item_name(item.id),
    })
}

fn item_value(item: Option<&ActivityItem>) -> serde_json::Value {
    match item {
        Some(item) if item.item_id != 0 => serde_json::json!({
            "id": item.item_id.to_string(),
            "count": item.count,
            "name": item_name(item.item_id),
        }),
        _ => serde_json::json!({ "id": "", "count": 0, "name": "奖励未下发" }),
    }
}

fn item_name(id: i64) -> String {
    global_game_config()
        .get_item_by_id(id)
        .map(|item| item.name)
        .unwrap_or_else(|| format!("物品 #{id}"))
}

fn grouped(value: i64) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let digits = value.unsigned_abs().to_string();
    let mut reversed = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            reversed.push(',');
        }
        reversed.push(ch);
    }
    format!("{sign}{}", reversed.chars().rev().collect::<String>())
}

fn percent(donated: i64, target: i64) -> String {
    if target <= 0 || donated < 0 {
        return "0.00".to_owned();
    }
    let ratio = (i128::from(donated).saturating_mul(10_000) / i128::from(target)).clamp(0, 10_000);
    format!("{}.{:02}", ratio / 100, ratio % 100)
}

fn fund_date(value: i64) -> String {
    let text = value.to_string();
    if text.len() == 8 && text.chars().all(|ch| ch.is_ascii_digit()) {
        format!("{}-{}-{}", &text[0..4], &text[4..6], &text[6..8])
    } else {
        text
    }
}

fn seed_request() -> CharityRedFlowerOperateRequest {
    CharityRedFlowerOperateRequest {
        activity_id: ACTIVITY_ID,
        operate_type: CLAIM_SEED,
        claim_seed: Some(charity_red_flower_operate_request::Empty {}),
        ..CharityRedFlowerOperateRequest::default()
    }
}

fn donate_request() -> CharityRedFlowerOperateRequest {
    CharityRedFlowerOperateRequest {
        activity_id: ACTIVITY_ID,
        operate_type: DONATE,
        donate_love: Some(charity_red_flower_operate_request::Empty {}),
        ..CharityRedFlowerOperateRequest::default()
    }
}

fn daily_request() -> CharityRedFlowerOperateRequest {
    CharityRedFlowerOperateRequest {
        activity_id: ACTIVITY_ID,
        operate_type: CLAIM_DAILY,
        send_public_fund: Some(charity_red_flower_operate_request::Empty {}),
        ..CharityRedFlowerOperateRequest::default()
    }
}

fn merge_progress(left: &ProgressState, right: &ProgressState) -> ProgressState {
    ProgressState {
        initialized: left.initialized || right.initialized,
        claimed: left.claimed.union(&right.claimed).cloned().collect(),
        pending: left
            .pending
            .union(&right.pending)
            .filter(|target| !left.claimed.contains(*target) && !right.claimed.contains(*target))
            .cloned()
            .collect(),
    }
}

fn state_path(account_id: &str) -> Option<std::path::PathBuf> {
    #[cfg(any(test, feature = "test-isolation"))]
    if crate::config::paths::test_data_dir_override().is_none() {
        return None;
    }
    let digest = Sha256::digest(account_id.as_bytes());
    let token = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    let path = crate::config::paths::get_data_file(&format!("activity-center-state-{token}.json"));
    #[cfg(any(test, feature = "test-isolation"))]
    crate::config::paths::refuse_default_data_io(&path);
    Some(path)
}

#[cfg(test)]
fn load_progress(account_id: &str) -> ProgressState {
    crate::services::activity_state_file::with_account_state(account_id, || {
        load_progress_unlocked(account_id)
    })
}

fn load_progress_unlocked(account_id: &str) -> ProgressState {
    if !account_is_usable(account_id) {
        return ProgressState::default();
    }
    let Some(path) = state_path(account_id) else {
        return ProgressState::default();
    };
    if !path.exists() {
        return ProgressState::default();
    }
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return ProgressState::default();
    };
    let Ok(file) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return ProgressState::default();
    };
    if file.get("version").and_then(serde_json::Value::as_i64) != Some(1) {
        return ProgressState::default();
    }
    record_to_state(
        file.pointer(&format!("/charityRecords/{ACTIVITY_ID}"))
            .and_then(|value| serde_json::from_value::<ProgressRecord>(value.clone()).ok())
            .filter(|record| record.activity_id == ACTIVITY_ID.to_string()),
    )
}

#[cfg(test)]
pub(crate) fn confirm_progress_target(
    account_id: &str,
    target: &str,
) -> std::result::Result<(), String> {
    crate::services::activity_state_file::with_account_state(account_id, || {
        let mut state = load_progress_unlocked(account_id);
        state.initialized = true;
        state.claimed.insert(target.to_owned());
        state.pending.remove(target);
        save_progress_unlocked(account_id, &state)
    })
}

fn save_progress_unlocked(
    account_id: &str,
    state: &ProgressState,
) -> std::result::Result<(), String> {
    if !account_is_usable(account_id) {
        return Err(ACCOUNT_INVALID.to_owned());
    }
    let Some(path) = state_path(account_id) else {
        return Err(PERSIST_WARNING.to_owned());
    };
    let mut file = if path.exists() {
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        {
            Some(value) if value.get("version").and_then(serde_json::Value::as_i64) == Some(1) => {
                value
            }
            Some(_) => return Err(PERSIST_WARNING.to_owned()),
            None => return Err(PERSIST_WARNING.to_owned()),
        }
    } else {
        serde_json::json!({ "version": 1, "records": {}, "charityRecords": {} })
    };
    if !file.get("records").is_some() {
        file["records"] = serde_json::json!({});
    }
    if !file.get("charityRecords").and_then(serde_json::Value::as_object).is_some() {
        file["charityRecords"] = serde_json::json!({});
    }
    let existing = record_to_state(
        file.pointer(&format!("/charityRecords/{ACTIVITY_ID}"))
            .and_then(|value| serde_json::from_value::<ProgressRecord>(value.clone()).ok())
            .filter(|record| record.activity_id == ACTIVITY_ID.to_string()),
    );
    let merged = merge_progress(&existing, state);
    let record = ProgressRecord {
        activity_id: ACTIVITY_ID.to_string(),
        initialized: merged.initialized,
        claimed_progress_targets: sorted_targets(&merged.claimed),
        pending_progress_targets: sorted_targets(&merged.pending),
    };
    file["charityRecords"][ACTIVITY_ID.to_string()] =
        serde_json::to_value(record).unwrap_or(serde_json::Value::Null);
    crate::infra::json_db::write_json_file_atomic(path, &file)
        .map_err(|_| PERSIST_WARNING.to_owned())
}

fn record_to_state(record: Option<ProgressRecord>) -> ProgressState {
    let Some(record) = record else {
        return ProgressState::default();
    };
    let claimed = record
        .claimed_progress_targets
        .into_iter()
        .filter(|item| !item.is_empty())
        .collect::<BTreeSet<_>>();
    ProgressState {
        initialized: record.initialized,
        pending: record
            .pending_progress_targets
            .into_iter()
            .filter(|item| !item.is_empty() && !claimed.contains(item))
            .collect(),
        claimed,
    }
}

fn sorted_targets(values: &BTreeSet<String>) -> Vec<String> {
    let mut rows = values.iter().cloned().collect::<Vec<_>>();
    rows.sort_by_key(|item| item.parse::<i64>().unwrap_or(0));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::encryptor::NoopEncryptor;
    use crate::network::gateway::{Gateway, GatewayConfig};
    use crate::proto::generated::gamepb::activitypb::{
        ActivityContent, CharityRedFlowerGlobalReward, CharityRedFlowerProgressReward,
        CharityRedFlowerPublicFund,
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

    fn flower(balance: i64, donated: i64, seed: i64, flow: i64) -> CharityRedFlowerData {
        CharityRedFlowerData {
            love_item_id: 1040,
            love_balance: balance,
            donated_love: donated,
            global_donated_love: 1,
            global_target_love: 100,
            seed_reward_status: seed,
            seed_reward: Some(ActivityItem { item_id: 20883, count: 6 }),
            progress_rewards: vec![
                CharityRedFlowerProgressReward {
                    target: 30,
                    status: 1,
                    reward: Some(ActivityItem { item_id: 80013, count: 1 }),
                },
                CharityRedFlowerProgressReward {
                    target: 60,
                    status: 1,
                    reward: Some(ActivityItem { item_id: 1002, count: 50 }),
                },
                CharityRedFlowerProgressReward {
                    target: 90,
                    status: 0,
                    reward: Some(ActivityItem { item_id: 80013, count: 2 }),
                },
            ],
            global_reward: Some(CharityRedFlowerGlobalReward {
                target: 100,
                reward: Some(ActivityItem { item_id: 80001, count: 1 }),
            }),
            settlement_required_love: 30,
            settlement_reward: Some(ActivityItem { item_id: 90001, count: 1 }),
            flow_status: flow,
            daily_reward: Some(ActivityItem { item_id: 80001, count: 2 }),
            ..CharityRedFlowerData::default()
        }
    }

    fn listed(state: CharityRedFlowerData, begin: i64, end: i64, extra: &[u8]) -> Vec<u8> {
        ActivityListReply {
            activities: vec![ActivityData {
                activity: Some(ActivityContent {
                    activity_id: ACTIVITY_ID,
                    name: "公益小红花".into(),
                    extra: extra.to_vec().into(),
                    begin_time: begin,
                    end_time: end,
                    ..ActivityContent::default()
                }),
                charity_red_flower: Some(state),
                ..ActivityData::default()
            }],
            activity_windows: vec![ActivityWindow {
                id: ACTIVITY_ID,
                name: "公益".into(),
                begin_time: begin,
                end_time: end,
            }],
            ..ActivityListReply::default()
        }
        .encode_to_vec()
    }

    #[test]
    fn the_read_window_matches_the_shared_activity_window() {
        assert!(charity_window_is_open(0, 0, 10));
        assert!(charity_window_is_open(5, 10, 10));
        assert!(!charity_window_is_open(11, 20, 10));
        assert!(!charity_window_is_open(0, 9, 10));
    }

    #[test]
    fn rules_reuse_text_content_for_tips_and_nested_strings() {
        let (title, lines) = crate::services::activity_text::text_content(
            r#"{"title":"不要当规则","note":{"body":"<b>甲</b>","more":["乙"]}}"#.as_bytes(),
        );
        assert!(title.is_empty());
        assert_eq!(lines, vec!["不要当规则".to_string(), "甲".to_string(), "乙".to_string()]);
    }

    #[tokio::test]
    async fn an_inactive_window_stays_visible_and_does_not_send() {
        let _data = crate::config::paths::TestDataDir::enter();
        let writes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&writes);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, _body| {
            if method == "Operate" {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            Ok(listed(
                flower(4, 62, SEED_CLAIMABLE, FLOW_HARVESTED),
                4_000_000_000,
                4_100_000_000,
                r#"{"tips":{"title":"说明","txt":["捐赠"]}}"#.as_bytes(),
            ))
        });
        let service = CharityService::new(gate, "acc");
        let snapshot = service.snapshot().await.unwrap();
        assert_eq!(snapshot["available"], true);
        assert_eq!(snapshot["active"], false);
        assert_eq!(snapshot["seedStatus"], "完成今日任务后可领取");
        assert_eq!(snapshot["actions"]["claimSeeds"]["enabled"], false);
        assert_eq!(snapshot["actions"]["donateLove"]["enabled"], false);
        assert_eq!(snapshot["rules"]["title"], "说明");
        assert_eq!(snapshot["rules"]["paragraphs"][0], "捐赠");
        assert_eq!(snapshot["progressRewards"][0]["claimed"], true);
        assert_eq!(snapshot["progressRewards"][1]["claimable"], false);
        assert!(service.claim_seeds().await.is_err());
        assert!(service.donate().await.is_err());
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn missing_activity_and_bad_counts_do_not_send() {
        let _data = crate::config::paths::TestDataDir::enter();
        let writes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&writes);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, _body| {
            if method == "Operate" {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            Ok(ActivityListReply::default().encode_to_vec())
        });
        let service = CharityService::new(gate, "acc");
        assert_eq!(service.snapshot().await.unwrap()["available"], false);
        assert!(service.claim_seeds().await.is_err());
        assert!(service.donate().await.is_err());
        assert!(service.claim_progress(0).await.is_err());
        assert!(service.claim_progress(-1).await.is_err());
        assert!(CharityService::new(gateway(), " ").claim_seeds().await.is_err());
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn capture_vectors_round_trip_and_seed_mismatch_is_not_a_reward() {
        let vectors: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/charity-capture-vectors.json")).unwrap();
        assert_eq!(hex::encode(seed_request().encode_to_vec()), vectors["requests"]["claimSeed"]);
        assert_eq!(
            hex::encode(donate_request().encode_to_vec()),
            vectors["requests"]["donateLove"]
        );
        let progress = CharityRedFlowerOperateRequest {
            activity_id: ACTIVITY_ID,
            operate_type: CLAIM_PROGRESS,
            claim_progress_reward: Some(charity_red_flower_operate_request::ProgressRewardParams {
                target: 30,
            }),
            ..CharityRedFlowerOperateRequest::default()
        };
        assert_eq!(hex::encode(progress.encode_to_vec()), vectors["requests"]["claimProgress30"]);
        assert_eq!(
            hex::encode(daily_request().encode_to_vec()),
            vectors["requests"]["sendPublicFund"]
        );
        let seed = ActivityOperateReply::decode(
            hex::decode(vectors["replies"]["seed"].as_str().unwrap()).unwrap().as_slice(),
        )
        .unwrap();
        assert_eq!(seed.charity_seed_result.unwrap().reward.unwrap().count, 6);
        let progress = ActivityOperateReply::decode(
            hex::decode(vectors["replies"]["progress"].as_str().unwrap()).unwrap().as_slice(),
        )
        .unwrap();
        let progress = progress.charity_progress_reward_result.unwrap();
        assert_eq!((progress.target, progress.reward.unwrap().id), (30, 80013));
        let fund_bytes = hex::decode(vectors["replies"]["fund"].as_str().unwrap()).unwrap();
        assert!(fund_bytes.windows(18).any(|window| window == b"QQNC2699MW20260901"));
        assert!(ActivityOperateReply::decode(fund_bytes.as_slice()).is_err());

        let _data = crate::config::paths::TestDataDir::enter();
        let writes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&writes);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, body| {
            if method == "List" {
                return Ok(listed(flower(4, 10, SEED_CLAIMABLE, 1), 0, 0, b"plain"));
            }
            if method == "Operate" {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let request = CharityRedFlowerOperateRequest::decode(body).unwrap();
                assert_eq!(request.operate_type, CLAIM_SEED);
                assert!(request.claim_seed.is_some());
                return Ok(ActivityOperateReply {
                    activity_id: 1,
                    operate_type: 1,
                    rewards: vec![CoreItem { id: 1, count: 9, ..CoreItem::default() }],
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            Ok(Vec::new())
        });
        let error = CharityService::new(gate, "acc").claim_seeds().await.unwrap_err();
        assert!(error.to_string().contains("不匹配"), "{error}");
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn daily_date_progress_and_timeout_follow_the_reply() {
        let _data = crate::config::paths::TestDataDir::enter();
        let mut state = flower(0, 91, SEED_CLAIMED, 1);
        state.progress_rewards[2].status = 1;
        state.public_fund = Some(CharityRedFlowerPublicFund {
            date: 20260901,
            status: 1,
            ..CharityRedFlowerPublicFund::default()
        });
        let gate = gateway();
        let listed_state = state.clone();
        gate.install_reply_script(move |_service, method, _body| {
            if method == "List" {
                return Ok(listed(listed_state.clone(), 0, 0, b""));
            }
            Ok(Vec::new())
        });
        let service = CharityService::new(gate, "date-account");
        let yesterday = service.dto_from_list(
            &ActivityListReply::decode(listed(state.clone(), 0, 0, b"").as_slice()).unwrap(),
            "20260902",
        );
        assert_eq!(yesterday["dailyGift"]["claimed"], false);
        let today = service.dto_from_list(
            &ActivityListReply::decode(listed(state, 0, 0, b"").as_slice()).unwrap(),
            "20260901",
        );
        assert_eq!(today["dailyGift"]["claimed"], true);
        assert_eq!(today["dailyGift"]["publicFund"]["dateText"], "2026-09-01");
        assert_eq!(today["progressRewards"][2]["claimable"], true);
        assert!(service.donate().await.is_err());

        let writes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&writes);
        let daily_gate = gateway();
        daily_gate.install_reply_script(move |_service, method, body| {
            if method == "List" {
                return Ok(listed(flower(3, 62, SEED_CLAIMED, FLOW_HARVESTED), 0, 0, b""));
            }
            if method == "Operate" {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let request = CharityRedFlowerOperateRequest::decode(body).unwrap();
                assert_eq!(request.operate_type, CLAIM_DAILY);
                assert!(request.send_public_fund.is_some());
                return Err(NetworkError::Timeout {
                    client_seq: 1,
                    service_name: ACTIVITY_SERVICE.into(),
                    method_name: "Operate".into(),
                    pending: 1,
                });
            }
            Ok(Vec::new())
        });
        let error = CharityService::new(daily_gate, "daily").claim_daily_gift().await.unwrap_err();
        assert!(
            error.to_string().contains("请求") || error.to_string().contains("超时"),
            "{error}"
        );
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn progress_claim_sends_one_target_and_the_same_target_is_not_repeated() {
        let _data = crate::config::paths::TestDataDir::enter();
        let bodies = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<u8>>::new()));
        let log = std::sync::Arc::clone(&bodies);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, body| {
            if method == "List" {
                return Ok(listed(flower(3, 62, SEED_CLAIMED, FLOW_HARVESTED), 0, 0, b""));
            }
            if method == "Operate" {
                log.lock().expect("log").push(body.to_vec());
                return Err(NetworkError::Gateway {
                    code: ALREADY_CLAIMED,
                    service_name: ACTIVITY_SERVICE.into(),
                    method_name: "Operate".into(),
                    error_message: "claimed".into(),
                    client_seq: 1,
                });
            }
            Ok(Vec::new())
        });
        let service = CharityService::new(Arc::clone(&gate), "progress-account");
        assert!(service.claim_progress(90).await.is_err());
        let claimed = service.claim_progress(60).await.unwrap();
        assert_eq!(claimed["alreadyClaimed"], true);
        assert_eq!(claimed["message"], "公益进度奖励已领取（60 份爱心）");
        assert!(service.claim_progress(60).await.is_err());
        assert_eq!(bodies.lock().expect("log").len(), 1);
        let request =
            CharityRedFlowerOperateRequest::decode(bodies.lock().expect("log")[0].as_slice())
                .unwrap();
        assert_eq!(request.claim_progress_reward.unwrap().target, 60);
        let again = CharityService::new(gate, "progress-account");
        let snapshot = again.snapshot().await.unwrap();
        assert_eq!(snapshot["progressRewards"][1]["claimed"], true);
        assert_eq!(snapshot["progressRewards"][1]["claimable"], false);
    }

    fn write_raw(account: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = state_path(account).expect("test root");
        std::fs::create_dir_all(path.parent().expect("parent")).unwrap();
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn charity_save_keeps_stellar_records_and_an_old_file_without_charity_records() {
        let _data = crate::config::paths::TestDataDir::enter();
        let raw = r#"{"version":1,"records":{"7:2026072701:v1":{"seasonId":"7","activityId":"2026072701","catalogVersion":1,"confirmedOpenedNodeIds":["9"],"confirmedLitNodeIds":["9"],"noClaimableDays":{}}}}"#;
        write_raw("shared", raw.as_bytes());
        assert!(load_progress("shared").claimed.is_empty());
        confirm_progress_target("shared", "30").unwrap();
        let saved = std::fs::read_to_string(state_path("shared").unwrap()).unwrap();
        let file: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(file["records"]["7:2026072701:v1"]["confirmedLitNodeIds"][0], "9");
        assert_eq!(
            file["charityRecords"][ACTIVITY_ID.to_string()]["claimedProgressTargets"][0],
            "30"
        );
        confirm_progress_target("other", "90").unwrap();
        let other = std::fs::read_to_string(state_path("other").unwrap()).unwrap();
        assert!(!other.contains("\"30\""));
        assert!(!saved.contains("\"90\""));
    }

    #[test]
    fn bad_json_and_a_foreign_version_load_empty_and_save_does_not_overwrite() {
        let _data = crate::config::paths::TestDataDir::enter();
        let broken = b"not-json{";
        let broken_path = write_raw("broken", broken);
        assert!(load_progress("broken").claimed.is_empty());
        assert!(confirm_progress_target("broken", "30").is_err());
        assert_eq!(std::fs::read(&broken_path).unwrap(), broken);
        let foreign = br#"{"version":2,"records":{"keep":true},"secret":"synthetic"}"#;
        let foreign_path = write_raw("foreign", foreign);
        assert!(load_progress("foreign").claimed.is_empty());
        assert!(confirm_progress_target("foreign", "30").is_err());
        assert_eq!(std::fs::read(&foreign_path).unwrap(), foreign);
    }

    #[tokio::test]
    async fn a_failed_save_keeps_the_confirmed_target_without_another_send() {
        let _data = crate::config::paths::TestDataDir::enter();
        let foreign = br#"{"version":2,"records":{"keep":true}}"#;
        let path = write_raw("save-fail", foreign);
        let writes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&writes);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, body| {
            if method == "List" {
                return Ok(listed(flower(3, 62, SEED_CLAIMED, FLOW_HARVESTED), 0, 0, b""));
            }
            if method == "Operate" {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let request = CharityRedFlowerOperateRequest::decode(body).unwrap();
                assert_eq!(request.claim_progress_reward.unwrap().target, 60);
                return Err(NetworkError::Gateway {
                    code: ALREADY_CLAIMED,
                    service_name: ACTIVITY_SERVICE.into(),
                    method_name: "Operate".into(),
                    error_message: "claimed".into(),
                    client_seq: 1,
                });
            }
            Ok(Vec::new())
        });
        let service = CharityService::new(gate, "save-fail");
        let claimed = service.claim_progress(60).await.unwrap();
        assert_eq!(claimed["persistenceWarning"], PERSIST_WARNING);
        assert_eq!(claimed["alreadyClaimed"], true);
        assert!(service.claim_progress(60).await.is_err());
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(std::fs::read(&path).unwrap(), foreign);
        let mut state = flower(3, 62, SEED_CLAIMED, FLOW_HARVESTED);
        state.progress_rewards[1].status = 0;
        let shown = service.dto_from_entry(
            &ActivityData {
                activity: Some(ActivityContent {
                    activity_id: ACTIVITY_ID,
                    name: "公益小红花".into(),
                    ..ActivityContent::default()
                }),
                charity_red_flower: Some(state),
                ..ActivityData::default()
            },
            None,
            "20260901",
        );
        assert_eq!(shown["progressRewards"][1]["target"], "60");
        assert_eq!(shown["progressRewards"][1]["claimed"], true);
    }
}
