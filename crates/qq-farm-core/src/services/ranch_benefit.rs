//! 牧场每日福利的自动领取。没有礼包或已经领过则当天停止，回包对不上也不再重试。

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;

const RANCH_SCENE: i32 = 2;
const BENEFIT_RETRY_MS: i64 = 10 * 60 * 1000;

struct BenefitRoutine {
    date: String,
    done: bool,
    next_check_at: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BenefitAction {
    Skip,
    Claim,
}

fn routines() -> &'static Mutex<HashMap<String, BenefitRoutine>> {
    static ROUTINES: std::sync::OnceLock<Mutex<HashMap<String, BenefitRoutine>>> =
        std::sync::OnceLock::new();
    ROUTINES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 自动领取牧场每日福利。开关关闭、当天已结束、牧场未解锁或还没到重试时间时直接返回。
pub async fn claim_benefit_if_due(gateway: &Arc<Gateway>, account_id: &str) -> Result<()> {
    let today = crate::utils::time::today_system_date_key();
    let now = crate::utils::time::now_ms();
    let enabled =
        crate::models::store::account_config::get_automation(Some(account_id)).ranch_daily_benefit;
    let (done, next_check_at) = stored_routine(account_id, &today);
    if benefit_action(enabled, gateway.scene_unlocked(RANCH_SCENE), done, next_check_at, now)
        == BenefitAction::Skip
    {
        return Ok(());
    }
    match super::ranch::claim_daily_benefit(gateway, account_id).await {
        Ok(claimed) => {
            write_routine(account_id, &today, true, 0);
            if claimed {
                tracing::info!(account_id, "牧场每日福利已领取");
            }
            Ok(())
        }
        Err(error) => {
            let unknown = matches!(error, Error::Protocol(_) | Error::Protobuf(_));
            write_routine(
                account_id,
                &today,
                unknown,
                if unknown { 0 } else { now.saturating_add(BENEFIT_RETRY_MS) },
            );
            if unknown {
                tracing::error!(account_id, error = %error, "牧场每日福利领取结果未能确认");
            } else {
                tracing::warn!(account_id, error = %error, "牧场每日福利领取失败，稍后重试");
            }
            Err(error)
        }
    }
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
        BenefitRoutine { date: today.to_owned(), done, next_check_at },
    );
}

fn benefit_action(
    enabled: bool,
    unlocked: bool,
    done: bool,
    next_check_at: i64,
    now: i64,
) -> BenefitAction {
    if !enabled || !unlocked || done || now < next_check_at {
        BenefitAction::Skip
    } else {
        BenefitAction::Claim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benefit_waits_until_the_ranch_is_open_and_stops_after_it_is_settled() {
        let now = 1_000_000;
        assert_eq!(benefit_action(false, true, false, 0, now), BenefitAction::Skip);
        assert_eq!(benefit_action(true, false, false, 0, now), BenefitAction::Skip);
        assert_eq!(benefit_action(true, true, true, 0, now), BenefitAction::Skip);
        assert_eq!(benefit_action(true, true, false, now + 1, now), BenefitAction::Skip);
        assert_eq!(benefit_action(true, true, false, now, now), BenefitAction::Claim);

        let account = "ranch-benefit-routine-test";
        let today = "2026-11-17";
        write_routine(account, today, false, now + BENEFIT_RETRY_MS);
        assert_eq!(stored_routine(account, today), (false, now + BENEFIT_RETRY_MS));
        assert_eq!(stored_routine(account, "2026-11-18"), (false, 0));
        write_routine(account, today, true, 0);
        assert_eq!(stored_routine(account, today), (true, 0));
    }
}
