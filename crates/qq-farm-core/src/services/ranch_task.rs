//! 牧场任务的自动领取。没有可领任务时按巡查间隔再看，回包对不上则当天停止。

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;

const RANCH_SCENE: i32 = 2;
const TASK_RETRY_MS: i64 = 10 * 60 * 1000;
const TASK_CHECK_MIN_MINUTES: i64 = 1;
const TASK_CHECK_MAX_MINUTES: i64 = 60;
const TASK_CHECK_DEFAULT_MINUTES: i64 = 5;

struct TaskRoutine {
    date: String,
    done: bool,
    next_check_at: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TaskAction {
    Skip,
    Settle,
    Claim,
}

fn routines() -> &'static Mutex<HashMap<String, TaskRoutine>> {
    static ROUTINES: std::sync::OnceLock<Mutex<HashMap<String, TaskRoutine>>> =
        std::sync::OnceLock::new();
    ROUTINES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 自动领取已完成的牧场任务。开关关闭、当天已停止或还没到复查时间时直接返回。
pub async fn claim_tasks_if_due(gateway: &Arc<Gateway>, account_id: &str) -> Result<()> {
    let today = crate::utils::time::today_system_date_key();
    let now = crate::utils::time::now_ms();
    let automation = crate::models::store::account_config::get_automation(Some(account_id));
    let (done, next_check_at) = stored_routine(account_id, &today);
    match task_action(
        automation.ranch_task,
        gateway.scene_unlocked(RANCH_SCENE),
        done,
        next_check_at,
        now,
    ) {
        TaskAction::Skip => Ok(()),
        TaskAction::Settle => {
            write_routine(account_id, &today, true, 0);
            Ok(())
        }
        TaskAction::Claim => match super::ranch::claim_ready_tasks(gateway, account_id).await {
            Ok(claimed) => {
                write_routine(
                    account_id,
                    &today,
                    false,
                    now.saturating_add(check_delay_ms(automation.ranch_check_minutes)),
                );
                if claimed {
                    tracing::info!(account_id, "牧场任务奖励已领取");
                    crate::services::panel_log::log(
                        account_id,
                        "牧场",
                        "牧场任务奖励已领取",
                        crate::constants::PanelEvent::RanchTaskClaim,
                        Some(serde_json::json!({ "module": "ranch" })),
                    );
                }
                Ok(())
            }
            Err(error) => {
                let unknown = matches!(error, Error::Protocol(_) | Error::Protobuf(_));
                write_routine(
                    account_id,
                    &today,
                    unknown,
                    if unknown { 0 } else { now.saturating_add(TASK_RETRY_MS) },
                );
                if unknown {
                    tracing::error!(account_id, error = %error, "牧场任务奖励领取结果未能确认");
                } else {
                    tracing::warn!(account_id, error = %error, "牧场任务奖励领取失败，稍后重试");
                }
                crate::services::panel_log::log_warn(
                    account_id,
                    "牧场",
                    format!("牧场任务奖励领取失败：{error}"),
                    crate::constants::PanelEvent::RanchTaskClaim,
                    Some(serde_json::json!({ "module": "ranch" })),
                );
                Err(error)
            }
        },
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
    routines()
        .lock()
        .insert(account_id.to_owned(), TaskRoutine { date: today.to_owned(), done, next_check_at });
}

fn task_action(
    enabled: bool,
    unlocked: bool,
    done: bool,
    next_check_at: i64,
    now: i64,
) -> TaskAction {
    if !enabled || done || now < next_check_at {
        TaskAction::Skip
    } else if !unlocked {
        TaskAction::Settle
    } else {
        TaskAction::Claim
    }
}

fn check_delay_ms(minutes: i64) -> i64 {
    let minutes = if (TASK_CHECK_MIN_MINUTES..=TASK_CHECK_MAX_MINUTES).contains(&minutes) {
        minutes
    } else {
        TASK_CHECK_DEFAULT_MINUTES
    };
    minutes.saturating_mul(60_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_claims_wait_for_the_next_patrol_and_stop_after_an_unknown_result() {
        let now = 1_000_000;
        assert_eq!(task_action(false, true, false, 0, now), TaskAction::Skip);
        assert_eq!(task_action(true, true, true, 0, now), TaskAction::Skip);
        assert_eq!(task_action(true, true, false, now + 1, now), TaskAction::Skip);
        assert_eq!(task_action(true, true, false, now, now), TaskAction::Claim);
        assert_eq!(task_action(true, false, false, 0, now), TaskAction::Settle);
        assert_eq!(check_delay_ms(5), 300_000);
        assert_eq!(check_delay_ms(0), 300_000);
        assert_eq!(check_delay_ms(61), 300_000);

        let account = "ranch-task-routine-test";
        let today = "2026-11-17";
        write_routine(account, today, false, now + TASK_RETRY_MS);
        assert_eq!(stored_routine(account, today), (false, now + TASK_RETRY_MS));
        assert_eq!(stored_routine(account, "2026-11-18"), (false, 0));
        write_routine(account, today, true, 0);
        assert_eq!(stored_routine(account, today), (true, 0));
    }
}
