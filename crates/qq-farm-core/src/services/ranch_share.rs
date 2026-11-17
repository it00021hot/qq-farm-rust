//! 牧场每日分享。分享编号 102，奖励是 5 个普通罐头。

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use prost::Message;
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::ranchpb::{
    ClaimShareRewardReply, ClaimShareRewardRequest, ReportShareReply, ReportShareRequest,
};
use crate::proto::generated::gamepb::sharepb::{CheckCanShareReply, CheckCanShareRequest};
use crate::proto::generated::gamepb::userpb::ShareInfo;

use super::ranch::{self, id_count_signature};

const SHARE_ID: i64 = 102;
const DOC_ID: i64 = 2101;
const REWARD_ITEM: i64 = 80021;
const REWARD_COUNT: i64 = 5;
const RANCH_SCENE: i32 = 2;
const LOCKED_REASON: &str = "牧场尚未解锁，请先在游戏中完成开启条件";
const SHARE_SERVICE: &str = "gamepb.sharepb.ShareService";
const SHARE_RETRY_MS: i64 = 10 * 60 * 1000;
pub const SHARE_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);
pub const SHARE_INITIAL_DELAY: std::time::Duration = std::time::Duration::from_secs(5);

struct DayShare {
    date: String,
    count: i64,
    claimed: bool,
}

enum ShareStep {
    Report,
    Claim,
    Done,
}

struct ShareRoutine {
    date: String,
    done: bool,
    next_check_at: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ShareAction {
    Skip,
    Settle,
    Claim,
}

fn routines() -> &'static Mutex<HashMap<String, ShareRoutine>> {
    static ROUTINES: std::sync::OnceLock<Mutex<HashMap<String, ShareRoutine>>> =
        std::sync::OnceLock::new();
    ROUTINES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn stored_routine(account_id: &str, today: &str) -> (bool, i64) {
    routines()
        .lock()
        .get(account_id)
        .filter(|state| state.date == today)
        .map(|state| (state.done, state.next_check_at))
        .unwrap_or((false, 0))
}

fn write_routine(account_id: &str, today: &str, done: bool, next_check_at: i64) {
    routines().lock().insert(
        account_id.to_owned(),
        ShareRoutine { date: today.to_owned(), done, next_check_at },
    );
}

fn share_action(
    enabled: bool,
    unlocked: bool,
    claimed: bool,
    done: bool,
    next_check_at: i64,
    now: i64,
) -> ShareAction {
    if !enabled || done || now < next_check_at {
        ShareAction::Skip
    } else if !unlocked || claimed {
        ShareAction::Settle
    } else {
        ShareAction::Claim
    }
}

fn shares() -> &'static Mutex<HashMap<String, DayShare>> {
    static SHARES: std::sync::OnceLock<Mutex<HashMap<String, DayShare>>> =
        std::sync::OnceLock::new();
    SHARES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 记下登录或分享回包里的牧场每日分享状态。
pub fn remember(account_id: &str, infos: &HashMap<i64, ShareInfo>) {
    let Some(info) = infos.get(&SHARE_ID) else {
        return;
    };
    shares().lock().insert(
        account_id.to_owned(),
        DayShare {
            date: crate::utils::time::today_system_date_key(),
            count: info.share_count,
            claimed: info.is_claimed,
        },
    );
}

/// 今天是否还能领牧场每日分享。
pub async fn daily_share(gateway: &Arc<Gateway>, account_id: &str) -> Result<Value> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Ok(json!({ "canClaim": false }));
    }
    let state = current(account_id);
    if state.claimed || state.count > 0 {
        let can_claim =
            next_step(state.count, state.claimed, true).is_ok_and(ShareStep::is_claimable);
        return Ok(json!({ "canClaim": can_claim }));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    ranch::in_ranch(gateway, account_id, move |_cells, _bag| async move {
        let can_share = check_share(&owned).await?;
        let state = current(&account);
        let can_claim =
            next_step(state.count, state.claimed, can_share).is_ok_and(ShareStep::is_claimable);
        Ok(json!({ "canClaim": can_claim }))
    })
    .await
}

/// 自动领取今天的牧场分享。开关关闭、已经领过或还没到重试时间时直接返回。
pub async fn claim_share_if_due(gateway: &Arc<Gateway>, account_id: &str) -> Result<()> {
    let today = crate::utils::time::today_system_date_key();
    let now = crate::utils::time::now_ms();
    let enabled =
        crate::models::store::account_config::get_automation(Some(account_id)).ranch_share;
    let (done, next_check_at) = stored_routine(account_id, &today);
    let action = share_action(
        enabled,
        gateway.scene_unlocked(RANCH_SCENE),
        current(account_id).claimed,
        done,
        next_check_at,
        now,
    );
    match action {
        ShareAction::Skip => Ok(()),
        ShareAction::Settle => {
            write_routine(account_id, &today, true, 0);
            Ok(())
        }
        ShareAction::Claim => match claim_daily_share(gateway, account_id).await {
            Ok(()) => {
                write_routine(account_id, &today, true, 0);
                tracing::info!(account_id, "牧场每日分享已领取");
                crate::services::panel_log::log(
                    account_id,
                    "牧场",
                    "牧场每日分享已领取",
                    crate::constants::PanelEvent::RanchDailyShare,
                    Some(serde_json::json!({ "module": "ranch" })),
                );
                Ok(())
            }
            Err(error) => {
                let unknown = matches!(error, Error::Protocol(_) | Error::Protobuf(_));
                write_routine(
                    account_id,
                    &today,
                    unknown,
                    if unknown { 0 } else { now.saturating_add(SHARE_RETRY_MS) },
                );
                if unknown {
                    tracing::error!(account_id, error = %error, "牧场每日分享领取结果未能确认");
                } else {
                    tracing::warn!(account_id, error = %error, "牧场每日分享领取失败，稍后重试");
                }
                crate::services::panel_log::log_warn(
                    account_id,
                    "牧场",
                    format!("牧场每日分享领取失败：{error}"),
                    crate::constants::PanelEvent::RanchDailyShare,
                    Some(serde_json::json!({ "module": "ranch" })),
                );
                Err(error)
            }
        },
    }
}

/// 上报牧场每日分享并领取 5 个普通罐头。已经领过则直接返回。
pub async fn claim_daily_share(gateway: &Arc<Gateway>, account_id: &str) -> Result<()> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    ranch::in_ranch(gateway, account_id, move |_cells, _bag| async move {
        let state = current(&account);
        if state.claimed {
            return Ok(());
        }
        let plan = super::ranch::guard_plan(
            "ranch:claimDailyShare:all".to_owned(),
            "领取每日首次分享礼包",
        );
        let tracked = account.clone();
        crate::services::ranch_ops::spend(&account, plan, || async move {
            if state.count <= 0 {
                if !check_share(&owned).await? {
                    return Err(Error::Business("当前分享入口不可用".to_owned()));
                }
                let request = ReportShareRequest {
                    share_id: SHARE_ID,
                    doc_id: DOC_ID,
                    ..ReportShareRequest::default()
                };
                let bytes =
                    owned.request(SHARE_SERVICE, "ReportShare", &request.encode_to_vec()).await?;
                let reply = ReportShareReply::decode(bytes.as_slice())?;
                remember(&tracked, &reply.share_infos);
            }
            let state = current(&tracked);
            if state.claimed {
                return Ok(());
            }
            if state.count <= 0 {
                return Err(Error::Protocol("分享上报未能确认，请刷新核对，勿重复提交".to_owned()));
            }
            let request = ClaimShareRewardRequest { share_id: SHARE_ID };
            let bytes =
                owned.request(SHARE_SERVICE, "ClaimShareReward", &request.encode_to_vec()).await?;
            let reply = ClaimShareRewardReply::decode(bytes.as_slice())?;
            remember(&tracked, &reply.share_infos);
            let state = current(&tracked);
            let expected = id_count_signature([(REWARD_ITEM, REWARD_COUNT)]);
            if state.claimed
                && id_count_signature(reply.rewards.iter().map(|item| (item.id, item.count)))
                    == expected
            {
                Ok(())
            } else {
                Err(Error::Protocol("每日分享奖励回包未能确认，请刷新核对，勿重复提交".to_owned()))
            }
        })
        .await
    })
    .await
}

fn current(account_id: &str) -> DayShare {
    let today = crate::utils::time::today_system_date_key();
    shares()
        .lock()
        .get(account_id)
        .filter(|state| state.date == today)
        .map(|state| DayShare {
            date: state.date.clone(),
            count: state.count,
            claimed: state.claimed,
        })
        .unwrap_or(DayShare { date: today, count: 0, claimed: false })
}

fn next_step(count: i64, claimed: bool, can_share: bool) -> Result<ShareStep> {
    if claimed {
        Ok(ShareStep::Done)
    } else if count > 0 {
        Ok(ShareStep::Claim)
    } else if can_share {
        Ok(ShareStep::Report)
    } else {
        Err(Error::Business("当前分享入口不可用".to_owned()))
    }
}

impl ShareStep {
    fn is_claimable(self) -> bool {
        !matches!(self, Self::Done)
    }
}

async fn check_share(gateway: &Gateway) -> Result<bool> {
    let bytes = gateway
        .request(SHARE_SERVICE, "CheckCanShare", &CheckCanShareRequest {}.encode_to_vec())
        .await?;
    Ok(CheckCanShareReply::decode(bytes.as_slice())?.can_share)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_share_claims_once_after_it_is_reported() {
        assert!(matches!(next_step(0, false, true).unwrap(), ShareStep::Report));
        assert!(matches!(next_step(1, false, false).unwrap(), ShareStep::Claim));
        assert!(matches!(next_step(1, true, true).unwrap(), ShareStep::Done));
        assert!(next_step(0, false, false).is_err());
        let account = "ranch-share-test";
        remember(
            account,
            &HashMap::from([(SHARE_ID, ShareInfo { share_count: 1, is_claimed: false })]),
        );
        let state = current(account);
        assert_eq!(state.count, 1);
        assert!(!state.claimed);
        remember(
            account,
            &HashMap::from([(SHARE_ID, ShareInfo { share_count: 1, is_claimed: true })]),
        );
        assert!(current(account).claimed);
        assert_eq!(id_count_signature([(REWARD_ITEM, REWARD_COUNT)]), "80021:5");
    }

    #[test]
    fn automatic_share_waits_out_a_retry_and_stops_after_an_unknown_result() {
        let now = 1_000_000;
        assert_eq!(share_action(false, true, false, false, 0, now), ShareAction::Skip);
        assert_eq!(share_action(true, true, false, true, 0, now), ShareAction::Skip);
        assert_eq!(share_action(true, true, false, false, now + 1, now), ShareAction::Skip);
        assert_eq!(share_action(true, true, false, false, now, now), ShareAction::Claim);
        assert_eq!(share_action(true, false, false, false, 0, now), ShareAction::Settle);
        assert_eq!(share_action(true, true, true, false, 0, now), ShareAction::Settle);

        let account = "ranch-share-routine-test";
        let today = "2026-11-17";
        write_routine(account, today, false, now + SHARE_RETRY_MS);
        assert_eq!(stored_routine(account, today), (false, now + SHARE_RETRY_MS));
        assert_eq!(stored_routine(account, "2026-11-18"), (false, 0));
        write_routine(account, today, true, 0);
        assert_eq!(stored_routine(account, today), (true, 0));
    }
}
