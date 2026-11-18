//! 青梅酿门面。写操作使用该账号当前 worker 上的同一服务。

use std::sync::Arc;

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

fn require_account(account_id: &str) -> AppResult<()> {
    qq_farm_core::services::qingmei::require_qingmei_account(account_id)
        .map_err(AppError::from_core)
}

pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    require_account(account_id)?;
    let worker = require_worker_loop(ctx, account_id)?;
    worker.qingmei().snapshot().await.map_err(AppError::from_core)
}

pub async fn claim_seed(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    require_account(account_id)?;
    let worker = require_worker_loop(ctx, account_id)?;
    let service = worker.qingmei();
    let engine = Arc::clone(&ctx.engine);
    let account = account_id.to_owned();
    service
        .claim_seed_if(move || {
            engine.worker_loop(&account).is_some_and(|found| Arc::ptr_eq(&found, &worker))
        })
        .await
        .map_err(AppError::from_core)
}

pub async fn start(
    ctx: &AppContext,
    account_id: &str,
    ingredients: &[(i64, i64)],
) -> AppResult<Value> {
    require_account(account_id)?;
    let worker = require_worker_loop(ctx, account_id)?;
    let service = worker.qingmei();
    let engine = Arc::clone(&ctx.engine);
    let account = account_id.to_owned();
    service
        .start_if(ingredients, move || {
            engine.worker_loop(&account).is_some_and(|found| Arc::ptr_eq(&found, &worker))
        })
        .await
        .map_err(AppError::from_core)
}

pub async fn continue_brew(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    require_account(account_id)?;
    let worker = require_worker_loop(ctx, account_id)?;
    let service = worker.qingmei();
    let engine = Arc::clone(&ctx.engine);
    let account = account_id.to_owned();
    service
        .continue_if(move || {
            engine.worker_loop(&account).is_some_and(|found| Arc::ptr_eq(&found, &worker))
        })
        .await
        .map_err(AppError::from_core)
}

pub async fn settle(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    require_account(account_id)?;
    let worker = require_worker_loop(ctx, account_id)?;
    let service = worker.qingmei();
    let engine = Arc::clone(&ctx.engine);
    let account = account_id.to_owned();
    service
        .settle_if(move || {
            engine.worker_loop(&account).is_some_and(|found| Arc::ptr_eq(&found, &worker))
        })
        .await
        .map_err(AppError::from_core)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    use prost::Message as _;
    use qq_farm_core::config::paths::TestDataDir;
    use qq_farm_core::models::account::AccountSession;
    use qq_farm_core::models::types::AccountConfigSnapshot;
    use qq_farm_core::network::encryptor::NoopEncryptor;
    use qq_farm_core::network::gateway::{Gateway, GatewayConfig};
    use qq_farm_core::proto::generated::corepb::Item as CoreItem;
    use qq_farm_core::proto::generated::gamepb::activitypb::{
        ActivityListReply, ActivityOperateReply, ActivityWindow, ClaimQingMeiDailySeedRequest,
        QueryActivityRequest,
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

    fn worker(id: &str, gateway: Arc<Gateway>) -> Arc<WorkerLoop> {
        let (tx, _) = tokio::sync::broadcast::channel(8);
        Arc::new(WorkerLoop::new(
            AccountSession::new(id, "code", "name"),
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
            Arc::new(MysteryShopService::new(gateway)),
        ))
    }

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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn two_seed_claims_share_the_worker_and_a_blank_account_sends_nothing() {
        let _data = TestDataDir::enter();
        let claims = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&claims);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, body| {
            if method == "Operate"
                && QueryActivityRequest::decode(body)
                    .map(|request| request.operate_type)
                    .unwrap_or(0)
                    == 4
            {
                count.fetch_add(1, Ordering::SeqCst);
                let request = ClaimQingMeiDailySeedRequest::decode(body).unwrap();
                assert_eq!(request.params.unwrap().grant_id, 3);
                return Ok(ActivityOperateReply {
                    activity_id: 2_026_081_201,
                    operate_type: 4,
                    rewards: vec![CoreItem { id: 1, count: 1, ..CoreItem::default() }],
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            Ok(ActivityListReply {
                activity_windows: vec![ActivityWindow {
                    id: 2_026_081_202,
                    begin_time: 0,
                    end_time: 0,
                    ..ActivityWindow::default()
                }],
                ..ActivityListReply::default()
            }
            .encode_to_vec())
        });
        let runtime = Arc::new(RuntimeEngine::assemble_with(
            EngineConfig::default(),
            Arc::new(RuntimeState::new(Arc::new(EmptyStore), Vec::new())),
            None,
        ));
        runtime.register_worker_loop("acc", worker("acc", gate));
        let ctx = AppContext::new(runtime);
        let (left, right) = tokio::join!(claim_seed(&ctx, "acc"), claim_seed(&ctx, "acc"));
        assert!(left.is_ok() && right.is_ok(), "{left:?} {right:?}");
        assert_eq!(claims.load(Ordering::SeqCst), 1);
        let error = start(&ctx, " ", &[(1, 1)]).await.unwrap_err();
        assert!(error.to_string().contains("账号身份无效"), "{error}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn replacing_the_worker_drops_the_queued_claim() {
        let _data = TestDataDir::enter();
        let (tx, rx) = std::sync::mpsc::channel();
        let rx = Arc::new(std::sync::Mutex::new(Some(rx)));
        let entered = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&entered);
        let waiter = Arc::clone(&rx);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, _body| {
            if method == "List" && !flag.swap(true, Ordering::AcqRel) {
                if let Some(rx) = waiter.lock().expect("wait").take() {
                    let _ = rx.recv();
                }
            }
            Ok(ActivityListReply {
                activity_windows: vec![ActivityWindow {
                    id: 2_026_081_202,
                    begin_time: 0,
                    end_time: 0,
                    ..ActivityWindow::default()
                }],
                ..ActivityListReply::default()
            }
            .encode_to_vec())
        });
        let runtime = Arc::new(RuntimeEngine::assemble_with(
            EngineConfig::default(),
            Arc::new(RuntimeState::new(Arc::new(EmptyStore), Vec::new())),
            None,
        ));
        runtime.register_worker_loop("acc", worker("acc", gate));
        let ctx = AppContext::new(Arc::clone(&runtime));
        let queued = {
            let ctx = ctx.clone();
            tokio::spawn(async move { claim_seed(&ctx, "acc").await })
        };
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        runtime.unregister_worker_loop("acc");
        tx.send(()).unwrap();
        let error = queued.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("账号会话已变化"), "{error}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn another_account_claims_while_the_first_holds_its_lock() {
        let _data = TestDataDir::enter();
        let (tx, rx) = std::sync::mpsc::channel();
        let rx = Arc::new(std::sync::Mutex::new(Some(rx)));
        let entered = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&entered);
        let waiter = Arc::clone(&rx);
        let first = gateway();
        first.install_reply_script(move |_service, method, _body| {
            if method == "List" && !flag.swap(true, Ordering::AcqRel) {
                if let Some(rx) = waiter.lock().expect("wait").take() {
                    let _ = rx.recv();
                }
            }
            if method == "Operate" {
                return Ok(ActivityOperateReply {
                    activity_id: 2_026_081_201,
                    operate_type: 4,
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            Ok(ActivityListReply {
                activity_windows: vec![ActivityWindow {
                    id: 2_026_081_202,
                    begin_time: 0,
                    end_time: 0,
                    ..ActivityWindow::default()
                }],
                ..ActivityListReply::default()
            }
            .encode_to_vec())
        });
        let second = gateway();
        let other_claims = Arc::new(AtomicUsize::new(0));
        let other_count = Arc::clone(&other_claims);
        second.install_reply_script(move |_service, method, body| {
            if method == "Operate"
                && QueryActivityRequest::decode(body).map(|item| item.operate_type).unwrap_or(0)
                    == 4
            {
                other_count.fetch_add(1, Ordering::SeqCst);
                return Ok(ActivityOperateReply {
                    activity_id: 2_026_081_201,
                    operate_type: 4,
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            Ok(ActivityListReply {
                activity_windows: vec![ActivityWindow {
                    id: 2_026_081_202,
                    begin_time: 0,
                    end_time: 0,
                    ..ActivityWindow::default()
                }],
                ..ActivityListReply::default()
            }
            .encode_to_vec())
        });
        let runtime = Arc::new(RuntimeEngine::assemble_with(
            EngineConfig::default(),
            Arc::new(RuntimeState::new(Arc::new(EmptyStore), Vec::new())),
            None,
        ));
        runtime.register_worker_loop("acc", worker("acc", first));
        runtime.register_worker_loop("beta", worker("beta", second));
        let ctx = AppContext::new(runtime);
        let queued = {
            let ctx = ctx.clone();
            tokio::spawn(async move { claim_seed(&ctx, "acc").await })
        };
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        claim_seed(&ctx, "beta").await.unwrap();
        assert_eq!(other_claims.load(Ordering::SeqCst), 1);
        tx.send(()).unwrap();
        queued.await.unwrap().unwrap();
    }
}
