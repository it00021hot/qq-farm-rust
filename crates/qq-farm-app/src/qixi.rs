//! 七夕鹊桥寄情门面。写操作使用该账号当前 worker 上的同一服务。

use std::sync::Arc;

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

fn require_account(account_id: &str) -> AppResult<()> {
    qq_farm_core::services::qixi::require_qixi_account(account_id).map_err(AppError::from_core)
}

pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    require_account(account_id)?;
    let worker = require_worker_loop(ctx, account_id)?;
    worker.qixi().snapshot().await.map_err(AppError::from_core)
}

pub async fn claim(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    require_account(account_id)?;
    let worker = require_worker_loop(ctx, account_id)?;
    let service = worker.qixi();
    let engine = Arc::clone(&ctx.engine);
    let account = account_id.to_owned();
    service
        .claim_if(move || {
            engine.worker_loop(&account).is_some_and(|found| Arc::ptr_eq(&found, &worker))
        })
        .await
        .map_err(AppError::from_core)
}

pub async fn gift(ctx: &AppContext, account_id: &str, friend_gid: i64) -> AppResult<Value> {
    require_account(account_id)?;
    let worker = require_worker_loop(ctx, account_id)?;
    let service = worker.qixi();
    let engine = Arc::clone(&ctx.engine);
    let account = account_id.to_owned();
    service
        .gift_if(friend_gid, move || {
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
    use qq_farm_core::proto::generated::corepb::{Item as CoreItem, ItemBag};
    use qq_farm_core::proto::generated::gamepb::activitypb::{
        ActivityContent, ActivityData, ActivityItem, ActivityOperateReply, GetGroupReply,
        QixiBridgeConfig, QixiBridgeStage, QixiGiftProgress,
    };
    use qq_farm_core::proto::generated::gamepb::itempb::BagReply;
    use qq_farm_core::runtime::engine::{EngineConfig, RuntimeEngine};
    use qq_farm_core::runtime::runtime_state::{AccountStoreLike, RuntimeState};
    use qq_farm_core::runtime::worker_loop::{WorkerLoop, WorkerLoopConfig};
    use qq_farm_core::services::{
        email::EmailService, farm::FarmService, friend::scheduler::FriendService,
        mall::MallService, monthcard::MonthCardService, mystery_shop::MysteryShopService,
        qqvip::QQVipService, share::ShareService, task::TaskService, warehouse::WarehouseService,
    };
    use qq_farm_core::utils::time::get_server_time_secs;

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

    fn group(claim_flag: i64) -> Vec<u8> {
        let now = get_server_time_secs();
        GetGroupReply {
            group: Some(ActivityData {
                children: vec![
                    ActivityData {
                        activity: Some(ActivityContent {
                            activity_id: 2_026_081_801,
                            begin_time: 0,
                            end_time: 0,
                            field_23: claim_flag,
                            name: "鹊桥寄情".into(),
                            ..ActivityContent::default()
                        }),
                        qixi_bridge: Some(QixiBridgeConfig {
                            current_stage: 1,
                            stages: vec![QixiBridgeStage {
                                stage: 1,
                                status: 1,
                                cost: Some(ActivityItem { item_id: 1024, count: 1 }),
                                rewards: vec![ActivityItem { item_id: 8, count: 1 }],
                            }],
                            ..QixiBridgeConfig::default()
                        }),
                        ..ActivityData::default()
                    },
                    ActivityData {
                        activity: Some(ActivityContent {
                            activity_id: 2_026_081_802,
                            begin_time: now - 5,
                            end_time: now + 60,
                            ..ActivityContent::default()
                        }),
                        qixi_gift: Some(QixiGiftProgress::default()),
                        ..ActivityData::default()
                    },
                ],
                ..ActivityData::default()
            }),
        }
        .encode_to_vec()
    }

    fn context(id: &str, gateway: Arc<Gateway>) -> (AppContext, Arc<RuntimeEngine>) {
        let runtime = Arc::new(RuntimeEngine::assemble_with(
            EngineConfig::default(),
            Arc::new(RuntimeState::new(Arc::new(EmptyStore), Vec::new())),
            None,
        ));
        runtime.register_worker_loop(id, worker(id, gateway));
        (AppContext::new(Arc::clone(&runtime)), runtime)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn two_claims_share_one_worker_and_a_blank_account_sends_nothing() {
        let _data = TestDataDir::enter();
        let claims = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&claims);
        let done = Arc::new(AtomicBool::new(false));
        let done_flag = Arc::clone(&done);
        let gateway = gateway();
        gateway.install_reply_script(move |_service, method, _body| {
            if method == "Operate" {
                count.fetch_add(1, Ordering::SeqCst);
                done_flag.store(true, Ordering::SeqCst);
                return Ok(ActivityOperateReply {
                    activity_id: 2_026_081_801,
                    operate_type: 25,
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            if method == "Bag" {
                return Ok(BagReply {
                    item_bag: Some(ItemBag {
                        items: vec![CoreItem { id: 1025, count: 1, ..CoreItem::default() }],
                        ..ItemBag::default()
                    }),
                }
                .encode_to_vec());
            }
            let flag = if done_flag.load(Ordering::SeqCst) { 0 } else { 1 };
            Ok(group(flag))
        });
        let (ctx, _runtime) = context("acc", gateway);
        let (left, right) = tokio::join!(claim(&ctx, "acc"), claim(&ctx, "acc"));
        assert_eq!([left.is_ok(), right.is_ok()].iter().filter(|ok| **ok).count(), 1);
        assert_eq!(claims.load(Ordering::SeqCst), 1);
        let error = gift(&ctx, " ", 8).await.unwrap_err();
        assert!(error.to_string().contains("账号身份无效"), "{error}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn replacing_the_worker_drops_the_queued_claim_and_another_account_is_not_blocked() {
        let _data = TestDataDir::enter();
        let (tx, rx) = std::sync::mpsc::channel();
        let rx = Arc::new(std::sync::Mutex::new(Some(rx)));
        let entered = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&entered);
        let waiter = Arc::clone(&rx);
        let gateway = gateway();
        gateway.install_reply_script(move |_service, method, _body| {
            if method == "GetGroup" && !flag.swap(true, Ordering::AcqRel) {
                if let Some(rx) = waiter.lock().expect("wait").take() {
                    let _ = rx.recv();
                }
            }
            if method == "Bag" {
                return Ok(BagReply::default().encode_to_vec());
            }
            Ok(group(1))
        });
        let (ctx, runtime) = context("acc", gateway);
        let other = {
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
        };
        let other_claims = Arc::new(AtomicUsize::new(0));
        let other_count = Arc::clone(&other_claims);
        other.install_reply_script(move |_service, method, _body| {
            if method == "Operate" {
                other_count.fetch_add(1, Ordering::SeqCst);
                return Ok(ActivityOperateReply {
                    activity_id: 2_026_081_801,
                    operate_type: 25,
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            if method == "Bag" {
                return Ok(BagReply::default().encode_to_vec());
            }
            Ok(group(1))
        });
        runtime.register_worker_loop("beta", worker("beta", other));
        let queued = {
            let ctx = ctx.clone();
            tokio::spawn(async move { claim(&ctx, "acc").await })
        };
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        claim(&ctx, "beta").await.unwrap();
        assert_eq!(other_claims.load(Ordering::SeqCst), 1);
        runtime.unregister_worker_loop("acc");
        tx.send(()).unwrap();
        let error = queued.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("账号会话已变化"), "{error}");
    }
}
