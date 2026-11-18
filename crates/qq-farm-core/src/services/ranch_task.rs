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
        TaskAction::Claim => {
            if let Err(error) =
                super::ranch::report_client_progress_tasks(gateway, account_id).await
            {
                tracing::warn!(account_id, error = %error, "牧场成长任务进度上报失败");
                crate::services::panel_log::log_warn(
                    account_id,
                    "牧场",
                    format!("牧场成长任务进度上报失败：{error}"),
                    crate::constants::PanelEvent::RanchTaskClaim,
                    Some(serde_json::json!({ "module": "ranch" })),
                );
            }
            match super::ranch::claim_ready_tasks(gateway, account_id).await {
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
            }
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

    #[tokio::test]
    async fn claim_schedule_reports_the_friend_panel_task_before_claiming() {
        use prost::Message;

        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let seen = std::sync::Arc::clone(&calls);
        let gateway = Arc::new(crate::network::gateway::Gateway::new(
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
        gateway.mark_online();
        gateway.set_unlocked_scenes(vec![1, RANCH_SCENE]);
        gateway.install_reply_script(move |_service, method, body| {
            seen.lock().expect("calls").push(method.to_owned());
            if method == "TaskInfo" {
                let reply = crate::proto::generated::gamepb::taskpb::TaskInfoReply {
                    task_info: Some(crate::proto::generated::gamepb::taskpb::TaskInfo {
                        growth_tasks: vec![
                            crate::proto::generated::gamepb::taskpb::Task {
                                id: 300008,
                                task_type: 4,
                                progress: 0,
                                total_progress: 1,
                                is_unlocked: true,
                                ..Default::default()
                            },
                            crate::proto::generated::gamepb::taskpb::Task {
                                id: 300009,
                                task_type: 4,
                                progress: 0,
                                total_progress: 1,
                                is_unlocked: true,
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                return Ok(reply.encode_to_vec());
            }
            if method == "ClientReportProgress" {
                let request =
                    crate::proto::generated::gamepb::taskpb::ClientReportProgressRequest::decode(
                        body,
                    )
                    .expect("上报请求");
                assert_eq!(request.task_id, 300008);
                assert_eq!(request.progress, 1);
                return Err(crate::network::error::NetworkError::Phase("上报失败".into()));
            }
            Err(crate::network::error::NetworkError::Phase(format!("未安排的请求 {method}")))
        });
        let account = "ranch-report-friend-panel";
        let error = claim_tasks_if_due(&gateway, account).await.expect_err("领取阶段没有场景");
        assert!(
            error.to_string().contains("未安排的请求") || error.to_string().contains("上报失败")
        );
        let names = calls.lock().expect("calls").clone();
        let report_at = names.iter().position(|name| name == "ClientReportProgress").expect("上报");
        assert!(names[..report_at].iter().any(|name| name == "TaskInfo"));
        let scene_at = names.iter().position(|name| name == "PlayerSceneLogin").expect("继续领取");
        assert!(report_at < scene_at);
        assert_eq!(names.iter().filter(|name| name.as_str() == "ClientReportProgress").count(), 1);
        assert!(!names.iter().any(|name| name == "ClaimTaskReward"));
    }
}
