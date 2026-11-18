//! 秋祈良愿门面。写操作使用该账号当前 worker 上的同一服务，不另建锁。

use std::sync::Arc;

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    worker.autumn_wish().snapshot().await.map_err(AppError::from_core)
}

pub async fn operate(
    ctx: &AppContext,
    account_id: &str,
    action: &str,
    choose_id: i64,
) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    let service = worker.autumn_wish();
    let engine = Arc::clone(&ctx.engine);
    let account = account_id.to_owned();
    service
        .operate_if_current(action, choose_id, move || {
            engine.worker_loop(&account).is_some_and(|found| Arc::ptr_eq(&found, &worker))
        })
        .await
        .map_err(AppError::from_core)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use prost::Message as _;
    use qq_farm_core::config::paths::TestDataDir;
    use qq_farm_core::models::account::AccountSession;
    use qq_farm_core::models::types::AccountConfigSnapshot;
    use qq_farm_core::network::encryptor::NoopEncryptor;
    use qq_farm_core::network::error::NetworkError;
    use qq_farm_core::network::gateway::{Gateway, GatewayConfig};
    use qq_farm_core::proto::generated::corepb::Item;
    use qq_farm_core::proto::generated::gamepb::activitypb::{
        ActivityBodyWishSign, ActivityContent, ActivityData, ActivityOperateReply,
        AutumnOperateRequest, GetGroupReply, WishSignDrawRsp, WishSignPending,
    };
    use qq_farm_core::runtime::engine::{EngineConfig, RuntimeEngine};
    use qq_farm_core::runtime::runtime_state::{AccountStoreLike, RuntimeState};
    use qq_farm_core::runtime::worker_loop::{WorkerLoop, WorkerLoopConfig};
    use qq_farm_core::services::{
        email::EmailService, farm::FarmService, friend::scheduler::FriendService,
        mall::MallService, monthcard::MonthCardService, mystery_shop::MysteryShopService,
        qqvip::QQVipService, share::ShareService, task::TaskService, warehouse::WarehouseService,
    };

    use super::*;

    struct EmptyStore;
    impl AccountStoreLike for EmptyStore {
        fn get_config_snapshot(&self, _id: &str) -> AccountConfigSnapshot {
            AccountConfigSnapshot::default()
        }
        fn get_automation(&self, _id: &str) -> Value {
            Value::Null
        }
        fn get_preferred_seed(&self, _id: &str) -> i64 {
            0
        }
    }

    fn engine() -> Arc<RuntimeEngine> {
        Arc::new(RuntimeEngine::assemble_with(
            EngineConfig::default(),
            Arc::new(RuntimeState::new(Arc::new(EmptyStore), Vec::new())),
            None,
        ))
    }

    fn gateway() -> Arc<Gateway> {
        let gateway = Arc::new(Gateway::new(
            GatewayConfig {
                server_url: "wss://gate.example.com/ws".into(),
                platform: "qq".into(),
                os: "Windows".into(),
                client_version: "1.15.0.2_20261001".into(),
                auth_code: "test".into(),
                headers: std::collections::HashMap::new(),
            },
            Arc::new(NoopEncryptor),
        ));
        gateway.mark_online();
        gateway
    }

    fn worker(id: &str, gateway: Arc<Gateway>) -> Arc<WorkerLoop> {
        let (tx, _) = tokio::sync::broadcast::channel(8);
        let account = AccountSession::new(id, "code", "name");
        Arc::new(WorkerLoop::new(
            account,
            WorkerLoopConfig::default(),
            Arc::clone(&gateway),
            tx,
            Arc::new(FarmService::new(Arc::clone(&gateway))),
            Arc::new(FriendService::new(Arc::clone(&gateway), 5)),
            Arc::new(EmailService::new(Arc::clone(&gateway), id)),
            Arc::new(ShareService::new(Arc::clone(&gateway))),
            Arc::new(MonthCardService::new(Arc::clone(&gateway))),
            Arc::new(QQVipService::new(
                Arc::clone(&gateway),
                Arc::new(MallService::new(Arc::clone(&gateway))),
            )),
            Arc::new(MallService::new(Arc::clone(&gateway))),
            Arc::new(TaskService::new(Arc::clone(&gateway))),
            Arc::new(WarehouseService::new(Arc::clone(&gateway))),
            Arc::new(MysteryShopService::new(Arc::clone(&gateway))),
        ))
    }

    fn grouped(child: ActivityData) -> Vec<u8> {
        GetGroupReply {
            group: Some(ActivityData { children: vec![child], ..ActivityData::default() }),
        }
        .encode_to_vec()
    }

    fn entry(
        begin: i64,
        end: i64,
        remaining: i64,
        pending: Option<WishSignPending>,
    ) -> ActivityData {
        ActivityData {
            activity: Some(ActivityContent {
                activity_id: 2_026_092_401,
                begin_time: begin,
                end_time: end,
                ..ActivityContent::default()
            }),
            wish_sign: Some(ActivityBodyWishSign {
                remaining_count: remaining,
                activity_day: 1,
                pending,
            }),
            ..ActivityData::default()
        }
    }

    fn now() -> i64 {
        qq_farm_core::utils::time::get_server_time_secs()
    }

    fn draw_reply() -> Vec<u8> {
        ActivityOperateReply {
            activity_id: 2_026_092_401,
            operate_type: 51,
            wish_sign_draw: Some(WishSignDrawRsp {
                text_id: 1,
                day_id: 1,
                rewards: vec![Item { id: 6001, count: 20, ..Item::default() }],
            }),
            ..ActivityOperateReply::default()
        }
        .encode_to_vec()
    }

    struct Calls {
        methods: Mutex<Vec<String>>,
        choices: Mutex<Vec<i64>>,
    }

    fn install_draw(gateway: &Gateway, calls: Arc<Calls>) {
        let seen = Arc::new(AtomicUsize::new(0));
        gateway.install_reply_script(move |_service, method, body| {
            calls.methods.lock().expect("methods").push(method.to_owned());
            match method {
                "GetGroup" => {
                    let index = seen.fetch_add(1, Ordering::SeqCst);
                    let pending = (index > 0).then(|| WishSignPending {
                        choose_id: 5,
                        text_id: 1,
                        day_id: 1,
                        rewards: vec![Item { id: 6001, count: 20, ..Item::default() }],
                    });
                    Ok(grouped(entry(
                        now() - 10,
                        now() + 100,
                        if index == 0 { 1 } else { 0 },
                        pending,
                    )))
                }
                "Operate" => {
                    let request = AutumnOperateRequest::decode(body)
                        .map_err(|error| NetworkError::Phase(error.to_string()))?;
                    calls.choices.lock().expect("choices").push(
                        request.wish_sign_draw.as_ref().map(|draw| draw.choose_id).unwrap_or(-1),
                    );
                    Ok(draw_reply())
                }
                _ => Err(NetworkError::Phase(method.into())),
            }
        });
    }

    fn context(runtime: Arc<RuntimeEngine>) -> AppContext {
        AppContext::new(runtime)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn two_app_calls_share_the_worker_and_the_second_does_not_draw_again() {
        let _data = TestDataDir::enter();
        let runtime = engine();
        let calls =
            Arc::new(Calls { methods: Mutex::new(Vec::new()), choices: Mutex::new(Vec::new()) });
        let gate = gateway();
        install_draw(&gate, Arc::clone(&calls));
        runtime.register_worker_loop("acc", worker("acc", gate));
        let ctx = context(Arc::clone(&runtime));
        let left = operate(&ctx, "acc", "draw", 1);
        let right = operate(&ctx, "acc", "draw", 5);
        let (left, right) = tokio::join!(left, right);
        assert_eq!([left.is_ok(), right.is_ok()].iter().filter(|ok| **ok).count(), 1);
        assert_eq!(
            calls.methods.lock().expect("methods").iter().filter(|m| *m == "Operate").count(),
            1
        );
        assert!(Arc::ptr_eq(
            &runtime.worker_loop("acc").unwrap().autumn_wish(),
            &runtime.worker_loop("acc").unwrap().autumn_wish()
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn another_account_is_not_blocked_by_the_first_accounts_lock() {
        let _data = TestDataDir::enter();
        let runtime = engine();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release_rx = Arc::new(std::sync::Mutex::new(Some(release_rx)));
        let entered = Arc::new(AtomicBool::new(false));
        let a_calls =
            Arc::new(Calls { methods: Mutex::new(Vec::new()), choices: Mutex::new(Vec::new()) });
        let b_calls =
            Arc::new(Calls { methods: Mutex::new(Vec::new()), choices: Mutex::new(Vec::new()) });
        let gate_a = gateway();
        let entered_flag = Arc::clone(&entered);
        let waiter = Arc::clone(&release_rx);
        let a_log = Arc::clone(&a_calls);
        gate_a.install_reply_script(move |_service, method, _body| {
            a_log.methods.lock().expect("methods").push(method.to_owned());
            if method == "GetGroup" && !entered_flag.swap(true, Ordering::SeqCst) {
                if let Some(rx) = waiter.lock().expect("wait").take() {
                    let _ = rx.recv();
                }
            }
            if method == "Operate" {
                return Ok(draw_reply());
            }
            Ok(grouped(entry(now() - 10, now() + 100, 1, None)))
        });
        let gate_b = gateway();
        install_draw(&gate_b, Arc::clone(&b_calls));
        runtime.register_worker_loop("a", worker("a", gate_a));
        runtime.register_worker_loop("b", worker("b", gate_b));
        let ctx = context(Arc::clone(&runtime));
        let first = tokio::spawn({
            let ctx = ctx.clone();
            async move { operate(&ctx, "a", "draw", 1).await }
        });
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        operate(&ctx, "b", "draw", 5).await.unwrap();
        assert_eq!(
            b_calls.methods.lock().expect("methods").iter().filter(|m| *m == "Operate").count(),
            1
        );
        release_tx.send(()).unwrap();
        let _ = first.await.unwrap();
        assert!(
            a_calls.methods.lock().expect("methods").iter().filter(|m| *m == "Operate").count()
                <= 1
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_replaced_session_does_not_send_the_queued_write() {
        let _data = TestDataDir::enter();
        let runtime = engine();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release_rx = Arc::new(std::sync::Mutex::new(Some(release_rx)));
        let entered = Arc::new(AtomicBool::new(false));
        let calls =
            Arc::new(Calls { methods: Mutex::new(Vec::new()), choices: Mutex::new(Vec::new()) });
        let gate = gateway();
        let entered_flag = Arc::clone(&entered);
        let waiter = Arc::clone(&release_rx);
        let call_log = Arc::clone(&calls);
        gate.install_reply_script(move |_service, method, _body| {
            call_log.methods.lock().expect("methods").push(method.to_owned());
            if method == "GetGroup" {
                entered_flag.store(true, Ordering::Release);
                if let Some(rx) = waiter.lock().expect("wait").take() {
                    let _ = rx.recv();
                }
            }
            Ok(grouped(entry(now() - 10, now() + 100, 1, None)))
        });
        runtime.register_worker_loop("acc", worker("acc", gate));
        let ctx = context(Arc::clone(&runtime));
        let task = tokio::spawn({
            let ctx = ctx.clone();
            async move { operate(&ctx, "acc", "draw", 1).await }
        });
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        runtime.unregister_worker_loop("acc");
        release_tx.send(()).unwrap();
        let error = task.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("账号会话已变化"), "{error}");
        assert!(calls.methods.lock().expect("methods").iter().all(|method| method != "Operate"));
    }
}
