//! 千星游记门面。写操作使用该账号当前 worker 上的同一服务。

use std::sync::Arc;

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    worker.stellar().snapshot().await.map_err(AppError::from_core)
}

pub async fn claim_pass(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    let service = worker.stellar();
    let engine = Arc::clone(&ctx.engine);
    let account = account_id.to_owned();
    service
        .claim_pass_if(move || {
            engine.worker_loop(&account).is_some_and(|found| Arc::ptr_eq(&found, &worker))
        })
        .await
        .map_err(AppError::from_core)
}

pub async fn exchange(
    ctx: &AppContext,
    account_id: &str,
    goods_id: i64,
    count: i64,
) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    let service = worker.stellar();
    let engine = Arc::clone(&ctx.engine);
    let account = account_id.to_owned();
    service
        .exchange_if(goods_id, count, move || {
            engine.worker_loop(&account).is_some_and(|found| Arc::ptr_eq(&found, &worker))
        })
        .await
        .map_err(AppError::from_core)
}

pub async fn light(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    let service = worker.stellar();
    let engine = Arc::clone(&ctx.engine);
    let account = account_id.to_owned();
    service
        .light_if(move || {
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
    use qq_farm_core::proto::generated::gamepb::seasonpb::{
        ClaimBattlePassRewardsReply, GetSeasonInfoReply, SeasonInfo, SeasonPass, SeasonRewardNode,
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

    fn season(claimed: bool) -> Vec<u8> {
        GetSeasonInfoReply {
            season_info: Some(SeasonInfo {
                season_id: 1,
                server_time: 1_700_000_000,
                pass: Some(SeasonPass {
                    current_level: 1,
                    claimed_through_level: if claimed { 1 } else { 0 },
                    nodes: vec![SeasonRewardNode { node_id: 1, ..SeasonRewardNode::default() }],
                    ..SeasonPass::default()
                }),
                ..SeasonInfo::default()
            }),
        }
        .encode_to_vec()
    }

    fn context() -> (AppContext, Arc<RuntimeEngine>, Arc<Gateway>) {
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
        let runtime = Arc::new(RuntimeEngine::assemble_with(
            EngineConfig::default(),
            Arc::new(RuntimeState::new(Arc::new(EmptyStore), Vec::new())),
            None,
        ));
        runtime.register_worker_loop("acc", worker("acc", Arc::clone(&gateway)));
        (AppContext::new(Arc::clone(&runtime)), runtime, gateway)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn two_claim_calls_share_the_worker_gateway() {
        let _data = TestDataDir::enter();
        let claimed = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&claimed);
        let claims = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&claims);
        let (ctx, _runtime, gateway) = context();
        gateway.install_reply_script(move |_service, method, _body| {
            if method == "ClaimBattlePassRewards" {
                count.fetch_add(1, Ordering::SeqCst);
                flag.store(true, Ordering::SeqCst);
                return Ok(ClaimBattlePassRewardsReply::default().encode_to_vec());
            }
            Ok(season(flag.load(Ordering::SeqCst)))
        });
        let (left, right) = tokio::join!(claim_pass(&ctx, "acc"), claim_pass(&ctx, "acc"));
        assert_eq!([left.is_ok(), right.is_ok()].iter().filter(|ok| **ok).count(), 1);
        assert_eq!(claims.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn replacing_the_worker_drops_the_queued_claim() {
        let _data = TestDataDir::enter();
        let (tx, rx) = std::sync::mpsc::channel();
        let rx = Arc::new(std::sync::Mutex::new(Some(rx)));
        let entered = Arc::new(AtomicBool::new(false));
        let entered_flag = Arc::clone(&entered);
        let waiter = Arc::clone(&rx);
        let claims = Arc::new(AtomicBool::new(false));
        let claimed = Arc::clone(&claims);
        let (ctx, runtime, gateway) = context();
        gateway.install_reply_script(move |_service, method, _body| {
            if method == "ClaimBattlePassRewards" {
                claimed.store(true, Ordering::SeqCst);
            }
            if method == "GetSeasonInfo" {
                entered_flag.store(true, Ordering::Release);
                if let Some(rx) = waiter.lock().expect("wait").take() {
                    let _ = rx.recv();
                }
            }
            Ok(season(false))
        });
        let task = tokio::spawn(async move { claim_pass(&ctx, "acc").await });
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        runtime.unregister_worker_loop("acc");
        tx.send(()).unwrap();
        let error = task.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("账号会话已变化"), "{error}");
        assert!(!claims.load(Ordering::SeqCst));
    }
}
