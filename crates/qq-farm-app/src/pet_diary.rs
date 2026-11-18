//! 萌宠成长日记门面。写操作使用该账号当前 worker 上的同一服务。

use std::sync::Arc;

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

fn require_account(account_id: &str) -> AppResult<()> {
    qq_farm_core::services::pet_diary::require_pet_account(account_id).map_err(AppError::from_core)
}

pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    require_account(account_id)?;
    require_worker_loop(ctx, account_id)?.pet_diary().snapshot().await.map_err(AppError::from_core)
}

pub async fn feed(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.feed_if(still).await }).await
}

pub async fn draw(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.draw_if(still).await }).await
}

pub async fn initialize(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.initialize_if(still).await }).await
}

pub async fn claim_dog(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.claim_dog_if(still).await }).await
}

pub async fn claim_story(ctx: &AppContext, account_id: &str, order: i64) -> AppResult<Value> {
    call(ctx, account_id, move |service, still| async move {
        service.claim_story_if(order, still).await
    })
    .await
}

pub async fn claim_seeds(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.claim_seeds_if(still).await }).await
}

pub async fn refresh_charm(
    ctx: &AppContext,
    account_id: &str,
    payment: &str,
    expected_paid: Option<i64>,
) -> AppResult<Value> {
    let payment = payment.to_owned();
    call(ctx, account_id, move |service, still| async move {
        service.refresh_charm_if(&payment, expected_paid, still).await
    })
    .await
}

pub async fn equip_charm(ctx: &AppContext, account_id: &str, charm_id: i64) -> AppResult<Value> {
    call(ctx, account_id, move |service, still| async move {
        service.equip_charm_if(charm_id, still).await
    })
    .await
}

pub async fn open_treasure(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.open_treasure_if(still).await })
        .await
}

pub async fn claim_compensation(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(
        ctx,
        account_id,
        |service, still| async move { service.claim_compensation_if(still).await },
    )
    .await
}

pub async fn exchange(
    ctx: &AppContext,
    account_id: &str,
    goods_id: i64,
    count: i64,
) -> AppResult<Value> {
    call(ctx, account_id, move |service, still| async move {
        service.exchange_if(goods_id, count, still).await
    })
    .await
}

pub async fn claim_solar(ctx: &AppContext, account_id: &str, term_id: i64) -> AppResult<Value> {
    call(ctx, account_id, move |service, still| async move {
        service.claim_solar_if(term_id, still).await
    })
    .await
}

pub async fn friend(ctx: &AppContext, account_id: &str, gid: i64) -> AppResult<Value> {
    call(ctx, account_id, move |service, still| async move { service.friend_if(gid, still).await })
        .await
}

pub async fn interact_log(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.interact_log_if(still).await })
        .await
}

pub async fn plunder_log(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.plunder_log_if(still).await }).await
}

pub async fn battle(
    ctx: &AppContext,
    account_id: &str,
    gid: i64,
    treasure_id: &str,
    challenge_id: i64,
) -> AppResult<Value> {
    let treasure_id = treasure_id.to_owned();
    call(ctx, account_id, move |service, still| async move {
        service.battle_if(gid, &treasure_id, challenge_id, still).await
    })
    .await
}

async fn call<F, Fut>(ctx: &AppContext, account_id: &str, invoke: F) -> AppResult<Value>
where
    F: FnOnce(
        Arc<qq_farm_core::services::pet_diary::PetDiaryService>,
        Box<dyn Fn() -> bool + Send>,
    ) -> Fut,
    Fut: std::future::Future<Output = qq_farm_core::error::Result<Value>>,
{
    require_account(account_id)?;
    let worker = require_worker_loop(ctx, account_id)?;
    let service = worker.pet_diary();
    let engine = Arc::clone(&ctx.engine);
    let account = account_id.to_owned();
    let worker = Arc::clone(&worker);
    invoke(
        service,
        Box::new(move || {
            engine.worker_loop(&account).is_some_and(|found| Arc::ptr_eq(&found, &worker))
        }),
    )
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
        ActivityBodyPetTreasureHunt, PetDiaryActivityData, PetDiaryActivityGroup,
        PetDiaryActivityHead, PetDiaryGetGroupReply, PetDiaryOperateReply, PetDiaryOperateRequest,
        PetDiaryShopGoodsInfo, PetDiaryShopItemInfo, PetTreasureHuntFeed, PetTreasureHuntFeedRsp,
        PetTreasureHuntHunt, PetTreasureHuntNurture,
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
        let gate = Arc::new(Gateway::new(
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
        gate.mark_online();
        gate
    }

    fn group_bytes() -> Vec<u8> {
        let end = qq_farm_core::utils::time::get_server_time_secs() + 3600;
        let head = |id| PetDiaryActivityHead {
            id,
            start_time: 1,
            end_time: end,
            ..PetDiaryActivityHead::default()
        };
        PetDiaryGetGroupReply {
            group: Some(PetDiaryActivityGroup {
                head: Some(head(2_026_090_100)),
                children: vec![PetDiaryActivityData {
                    head: Some(head(2_026_090_101)),
                    pet_treasure_hunt: Some(ActivityBodyPetTreasureHunt {
                        nurture: Some(PetTreasureHuntNurture { stage: 1, ..Default::default() }),
                        feed: Some(PetTreasureHuntFeed { feed_count: 0 }),
                        hunt: Some(PetTreasureHuntHunt {
                            can_play_plunder: true,
                            ..PetTreasureHuntHunt::default()
                        }),
                        ..ActivityBodyPetTreasureHunt::default()
                    }),
                    ..PetDiaryActivityData::default()
                }],
            }),
        }
        .encode_to_vec()
    }

    fn bag_bytes() -> Vec<u8> {
        BagReply {
            item_bag: Some(ItemBag {
                items: vec![CoreItem { id: 1028, count: 800, ..CoreItem::default() }],
                ..ItemBag::default()
            }),
        }
        .encode_to_vec()
    }

    fn reply_for(body: &[u8]) -> Vec<u8> {
        let request = PetDiaryOperateRequest::decode(body).unwrap_or_default();
        let mut reply = PetDiaryOperateReply {
            activity_id: request.activity_id,
            operate_type: request.operate_type,
            ..PetDiaryOperateReply::default()
        };
        if request.operate_type == 29 {
            reply.pet_treasure_hunt_feed = Some(PetTreasureHuntFeedRsp::default());
        }
        if request.operate_type == 7 {
            reply.data = Some(PetDiaryActivityData {
                shop: Some(
                    qq_farm_core::proto::generated::gamepb::activitypb::PetDiaryActivityBodyShop {
                        goods: vec![PetDiaryShopGoodsInfo {
                            id: 8,
                            cost: vec![PetDiaryShopItemInfo { id: 1028, count: 1 }],
                            purchase_limit: 3,
                            ..PetDiaryShopGoodsInfo::default()
                        }],
                    },
                ),
                ..PetDiaryActivityData::default()
            });
        }
        reply.encode_to_vec()
    }

    fn runtime() -> Arc<RuntimeEngine> {
        Arc::new(RuntimeEngine::assemble_with(
            EngineConfig::default(),
            Arc::new(RuntimeState::new(Arc::new(EmptyStore), Vec::new())),
            None,
        ))
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn the_same_worker_serializes_feeds_and_a_blank_account_sends_nothing() {
        let _data = TestDataDir::enter();
        let (tx, rx) = std::sync::mpsc::channel();
        let rx = Arc::new(std::sync::Mutex::new(Some(rx)));
        let entered = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&entered);
        let waiter = Arc::clone(&rx);
        let writes = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&writes);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, body| {
            if method == "GetGroup" && !flag.swap(true, Ordering::AcqRel) {
                if let Some(rx) = waiter.lock().expect("wait").take() {
                    let _ = rx.recv();
                }
            }
            if method == "Operate" {
                let request = PetDiaryOperateRequest::decode(body).unwrap_or_default();
                if request.operate_type == 29 {
                    count.fetch_add(1, Ordering::SeqCst);
                }
                return Ok(reply_for(body));
            }
            if method == "Bag" {
                return Ok(bag_bytes());
            }
            Ok(group_bytes())
        });
        let runtime = runtime();
        runtime.register_worker_loop("acc", worker("acc", gate));
        let ctx = AppContext::new(runtime);
        let first = {
            let ctx = ctx.clone();
            tokio::spawn(async move { feed(&ctx, "acc").await })
        };
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        let second = {
            let ctx = ctx.clone();
            tokio::spawn(async move { feed(&ctx, "acc").await })
        };
        tokio::task::yield_now().await;
        assert_eq!(writes.load(Ordering::SeqCst), 0);
        tx.send(()).unwrap();
        let (left, right) = tokio::join!(first, second);
        assert!(left.unwrap().is_ok());
        assert!(right.unwrap().is_ok());
        assert_eq!(writes.load(Ordering::SeqCst), 2);
        let missed = gateway();
        let missed_writes = Arc::new(AtomicUsize::new(0));
        let missed_count = Arc::clone(&missed_writes);
        missed.install_reply_script(move |_service, method, _body| {
            if method == "Operate" {
                missed_count.fetch_add(1, Ordering::SeqCst);
            }
            Ok(group_bytes())
        });
        ctx.engine.register_worker_loop(" ", worker(" ", missed));
        assert!(feed(&ctx, " ").await.unwrap_err().to_string().contains("账号身份无效"));
        assert_eq!(missed_writes.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn replacing_the_worker_drops_the_queued_feed() {
        let _data = TestDataDir::enter();
        let (tx, rx) = std::sync::mpsc::channel();
        let rx = Arc::new(std::sync::Mutex::new(Some(rx)));
        let entered = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&entered);
        let waiter = Arc::clone(&rx);
        let writes = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&writes);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, body| {
            if method == "GetGroup" && !flag.swap(true, Ordering::AcqRel) {
                if let Some(rx) = waiter.lock().expect("wait").take() {
                    let _ = rx.recv();
                }
            }
            if method == "Operate" {
                count.fetch_add(1, Ordering::SeqCst);
                return Ok(reply_for(body));
            }
            if method == "Bag" {
                return Ok(bag_bytes());
            }
            Ok(group_bytes())
        });
        let runtime = runtime();
        runtime.register_worker_loop("acc", worker("acc", gate));
        let ctx = AppContext::new(Arc::clone(&runtime));
        let queued = {
            let ctx = ctx.clone();
            tokio::spawn(async move { feed(&ctx, "acc").await })
        };
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        runtime.unregister_worker_loop("acc");
        tx.send(()).unwrap();
        let error = queued.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("账号会话已变化"), "{error}");
        assert_eq!(writes.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn another_account_can_feed_while_the_first_holds_its_lock() {
        let _data = TestDataDir::enter();
        let (tx, rx) = std::sync::mpsc::channel();
        let rx = Arc::new(std::sync::Mutex::new(Some(rx)));
        let entered = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&entered);
        let waiter = Arc::clone(&rx);
        let first = gateway();
        first.install_reply_script(move |_service, method, body| {
            if method == "GetGroup" && !flag.swap(true, Ordering::AcqRel) {
                if let Some(rx) = waiter.lock().expect("wait").take() {
                    let _ = rx.recv();
                }
            }
            if method == "Operate" {
                return Ok(reply_for(body));
            }
            if method == "Bag" {
                return Ok(bag_bytes());
            }
            Ok(group_bytes())
        });
        let second_writes = Arc::new(AtomicUsize::new(0));
        let second_count = Arc::clone(&second_writes);
        let second = gateway();
        second.install_reply_script(move |_service, method, body| {
            if method == "Operate" {
                let request = PetDiaryOperateRequest::decode(body).unwrap_or_default();
                if request.operate_type == 29 {
                    second_count.fetch_add(1, Ordering::SeqCst);
                }
                return Ok(reply_for(body));
            }
            if method == "Bag" {
                return Ok(bag_bytes());
            }
            Ok(group_bytes())
        });
        let runtime = runtime();
        runtime.register_worker_loop("acc", worker("acc", first));
        runtime.register_worker_loop("other", worker("other", second));
        let ctx = AppContext::new(runtime);
        let held = {
            let ctx = ctx.clone();
            tokio::spawn(async move { feed(&ctx, "acc").await })
        };
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        assert!(feed(&ctx, "other").await.is_ok());
        assert_eq!(second_writes.load(Ordering::SeqCst), 1);
        tx.send(()).unwrap();
        assert!(held.await.unwrap().is_ok());
    }
}
