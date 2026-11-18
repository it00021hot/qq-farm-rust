//! 千星游记门面。写操作使用该账号当前 worker 上的同一服务。

use std::sync::Arc;

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

fn require_account(account_id: &str) -> AppResult<()> {
    if account_id.is_empty() || account_id.chars().all(char::is_whitespace) {
        Err(AppError::BadRequest("账号身份无效，未发送操作".into()))
    } else {
        Ok(())
    }
}

pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    require_account(account_id)?;
    let worker = require_worker_loop(ctx, account_id)?;
    worker.stellar().snapshot().await.map_err(AppError::from_core)
}

pub async fn claim_pass(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    require_account(account_id)?;
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
    require_account(account_id)?;
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

pub async fn claim_solar(ctx: &AppContext, account_id: &str, term_id: i64) -> AppResult<Value> {
    require_account(account_id)?;
    let worker = require_worker_loop(ctx, account_id)?;
    let service = worker.stellar();
    let engine = Arc::clone(&ctx.engine);
    let account = account_id.to_owned();
    service
        .claim_solar_if(term_id, move || {
            engine.worker_loop(&account).is_some_and(|found| Arc::ptr_eq(&found, &worker))
        })
        .await
        .map_err(AppError::from_core)
}

pub async fn light(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    require_account(account_id)?;
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

    fn season_bytes() -> Vec<u8> {
        use qq_farm_core::proto::generated::gamepb::seasonpb::SeasonActivity;
        GetSeasonInfoReply {
            season_info: Some(SeasonInfo {
                season_id: 7,
                server_time: 1_700_000_000,
                activities: vec![
                    SeasonActivity {
                        activity_id: 2_026_072_701,
                        r#type: 13,
                        begin_time: 1_700_000_000 - 86_400,
                        end_time: 1_700_000_000 + 86_400,
                        ..SeasonActivity::default()
                    },
                    SeasonActivity { activity_id: 900, r#type: 3, ..SeasonActivity::default() },
                ],
                pass: Some(SeasonPass {
                    current_level: 1,
                    nodes: vec![SeasonRewardNode { node_id: 1, ..SeasonRewardNode::default() }],
                    ..SeasonPass::default()
                }),
                ..SeasonInfo::default()
            }),
        }
        .encode_to_vec()
    }

    fn shop_reply() -> Vec<u8> {
        use qq_farm_core::proto::generated::gamepb::activitypb::{
            ActivityData, ActivityItem, ActivityOperateReply, StarSandGoods, StarSandGoodsList,
        };
        ActivityOperateReply {
            activity_id: 900,
            operate_type: 7,
            data: Some(ActivityData {
                catalog: Some(StarSandGoodsList {
                    goods: vec![
                        StarSandGoods {
                            goods_id: 4,
                            item: Some(ActivityItem { item_id: 8, count: 2 }),
                            cost: Some(ActivityItem { item_id: 3, count: 3 }),
                            ..StarSandGoods::default()
                        },
                        StarSandGoods {
                            goods_id: 5,
                            cost: Some(ActivityItem { item_id: 3, count: 4 }),
                            owned: true,
                            ..StarSandGoods::default()
                        },
                    ],
                }),
                ..ActivityData::default()
            }),
            ..ActivityOperateReply::default()
        }
        .encode_to_vec()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn exchange_light_and_solar_use_the_worker_facade() {
        use qq_farm_core::proto::generated::corepb::{Item as CoreItem, ItemBag};
        use qq_farm_core::proto::generated::gamepb::activitypb::{
            ActivityData, ActivityOperateReply, ConstellationData, ConstellationNode,
            ExchangeShopRequest, OperateConstellationRequest, QueryActivityRequest,
        };
        use qq_farm_core::proto::generated::gamepb::itempb::BagReply;
        use qq_farm_core::proto::generated::gamepb::solartermspb::{
            ClaimSolarTermsReply, ClaimSolarTermsRequest, GetSolarTermsReply, SolarTermInfo,
            SolarTermReward,
        };

        let _data = TestDataDir::enter();
        let writes = Arc::new(AtomicUsize::new(0));
        let sent = Arc::clone(&writes);
        let bodies = Arc::new(std::sync::Mutex::new(Vec::<(String, Vec<u8>)>::new()));
        let log = Arc::clone(&bodies);
        let (ctx, _runtime, gateway) = context();
        gateway.install_reply_script(move |_service, method, body| {
            log.lock().expect("log").push((method.to_owned(), body.to_vec()));
            match method {
                "GetSeasonInfo" => {
                    assert!(body.is_empty(), "{method}");
                    Ok(season_bytes())
                }
                "ClaimBattlePassRewards" => {
                    assert!(body.is_empty(), "{method}");
                    Ok(ClaimBattlePassRewardsReply::default().encode_to_vec())
                }
                "Bag" => {
                    let balance = if sent.load(Ordering::SeqCst) > 0 { 4 } else { 10 };
                    Ok(BagReply {
                        item_bag: Some(ItemBag {
                            items: vec![CoreItem { id: 3, count: balance, ..CoreItem::default() }],
                            ..ItemBag::default()
                        }),
                    }
                    .encode_to_vec())
                }
                "Operate" => {
                    let kind = QueryActivityRequest::decode(body)
                        .map(|request| request.operate_type)
                        .unwrap_or(0);
                    if kind == 7 {
                        return Ok(shop_reply());
                    }
                    if kind == 1 {
                        sent.fetch_add(1, Ordering::SeqCst);
                        return Ok(ActivityOperateReply {
                            activity_id: 900,
                            operate_type: 1,
                            ..ActivityOperateReply::default()
                        }
                        .encode_to_vec());
                    }
                    if kind == 21 {
                        return Ok(ActivityOperateReply {
                            activity_id: 2_026_072_701,
                            operate_type: 21,
                            data: Some(ActivityData {
                                constellation: Some(ConstellationData {
                                    nodes: vec![ConstellationNode {
                                        node_id: 1,
                                        field_3: true,
                                        ..ConstellationNode::default()
                                    }],
                                    ..ConstellationData::default()
                                }),
                                ..ActivityData::default()
                            }),
                            ..ActivityOperateReply::default()
                        }
                        .encode_to_vec());
                    }
                    Err(qq_farm_core::network::error::NetworkError::Phase(method.into()))
                }
                "GetSolarTerms" => Ok(GetSolarTermsReply {
                    terms: vec![
                        SolarTermInfo {
                            term_id: 8,
                            status: 2,
                            name: "立春".as_bytes().to_vec().into(),
                            ..SolarTermInfo::default()
                        },
                        SolarTermInfo { term_id: 9, status: 1, ..SolarTermInfo::default() },
                    ],
                    ..GetSolarTermsReply::default()
                }
                .encode_to_vec()),
                "ClaimSolarTerms" => Ok(ClaimSolarTermsReply {
                    rewards: vec![SolarTermReward { item_id: 1, count: 2 }],
                    ..ClaimSolarTermsReply::default()
                }
                .encode_to_vec()),
                _ => Err(qq_farm_core::network::error::NetworkError::Phase(method.into())),
            }
        });
        claim_pass(&ctx, "acc").await.unwrap();
        let bought = exchange(&ctx, "acc", 4, 2).await.unwrap();
        assert_eq!(bought["snapshot"]["shop"]["goods"].as_array().unwrap().len(), 2);
        assert_eq!(bought["snapshot"]["shop"]["goods"][0]["balance"], 4);
        let lighted = light(&ctx, "acc").await.unwrap();
        assert_eq!(lighted["snapshot"]["constellation"]["groups"][0]["visualState"], "lit");
        assert!(claim_solar(&ctx, "acc", 9).await.is_err());
        let claimed = claim_solar(&ctx, "acc", 8).await.unwrap();
        assert_eq!(claimed["rewards"][0]["count"], 2);
        let saved = bodies.lock().expect("log");
        let exchange_body = saved.iter().find(|(method, body)| {
            method == "Operate"
                && QueryActivityRequest::decode(body.as_slice()).unwrap().operate_type == 1
        });
        let request = ExchangeShopRequest::decode(exchange_body.unwrap().1.as_slice()).unwrap();
        assert_eq!(request.activity_id, 900);
        assert_eq!(request.operate_type, 1);
        assert_eq!(request.exchange_shop_operate.unwrap().count, 2);
        let light_body = saved.iter().find(|(method, body)| {
            method == "Operate"
                && QueryActivityRequest::decode(body.as_slice()).unwrap().operate_type == 21
        });
        let light_request =
            OperateConstellationRequest::decode(light_body.unwrap().1.as_slice()).unwrap();
        assert!(light_request.field_119.is_some());
        let solar = saved.iter().find(|(method, _)| method == "ClaimSolarTerms").unwrap();
        assert_eq!(ClaimSolarTermsRequest::decode(solar.1.as_slice()).unwrap().term_id, 8);
        assert_eq!(saved.iter().filter(|(method, _)| method == "ClaimSolarTerms").count(), 1);
        assert!(saved.iter().any(|(method, body)| method == "GetSeasonInfo" && body.is_empty()));
        assert!(saved
            .iter()
            .any(|(method, body)| method == "ClaimBattlePassRewards" && body.is_empty()));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn another_account_is_not_blocked_and_a_replaced_worker_drops_the_exchange() {
        use qq_farm_core::proto::generated::corepb::{Item as CoreItem, ItemBag};
        use qq_farm_core::proto::generated::gamepb::activitypb::{
            ActivityOperateReply, QueryActivityRequest,
        };
        use qq_farm_core::proto::generated::gamepb::itempb::BagReply;

        let _data = TestDataDir::enter();
        let (tx, rx) = std::sync::mpsc::channel();
        let rx = Arc::new(std::sync::Mutex::new(Some(rx)));
        let entered = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&entered);
        let waiter = Arc::clone(&rx);
        let acc_writes = Arc::new(AtomicUsize::new(0));
        let acc_count = Arc::clone(&acc_writes);
        let (ctx, runtime, gateway) = context();
        gateway.install_reply_script(move |_service, method, body| {
            if method == "GetSeasonInfo" && !flag.swap(true, Ordering::AcqRel) {
                if let Some(rx) = waiter.lock().expect("wait").take() {
                    let _ = rx.recv();
                }
            }
            if method == "Operate"
                && QueryActivityRequest::decode(body).map(|request| request.operate_type).unwrap_or(0) == 1
            {
                acc_count.fetch_add(1, Ordering::SeqCst);
                return Ok(ActivityOperateReply {
                    activity_id: 900,
                    operate_type: 1,
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            if method == "Operate" {
                return Ok(shop_reply());
            }
            if method == "Bag" {
                return Ok(BagReply {
                    item_bag: Some(ItemBag {
                        items: vec![CoreItem { id: 3, count: 10, ..CoreItem::default() }],
                        ..ItemBag::default()
                    }),
                }
                .encode_to_vec());
            }
            if method == "GetSolarTerms" {
                return Ok(
                    qq_farm_core::proto::generated::gamepb::solartermspb::GetSolarTermsReply::default()
                        .encode_to_vec(),
                );
            }
            Ok(season_bytes())
        });
        let other_gate = Arc::new(Gateway::new(
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
        other_gate.mark_online();
        let other_writes = Arc::new(AtomicUsize::new(0));
        let other_count = Arc::clone(&other_writes);
        other_gate.install_reply_script(move |_service, method, body| {
            if method == "Operate"
                && QueryActivityRequest::decode(body).map(|request| request.operate_type).unwrap_or(0) == 1
            {
                other_count.fetch_add(1, Ordering::SeqCst);
                return Ok(ActivityOperateReply {
                    activity_id: 900,
                    operate_type: 1,
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            if method == "Operate" {
                return Ok(shop_reply());
            }
            if method == "Bag" {
                return Ok(BagReply {
                    item_bag: Some(ItemBag {
                        items: vec![CoreItem { id: 3, count: 10, ..CoreItem::default() }],
                        ..ItemBag::default()
                    }),
                }
                .encode_to_vec());
            }
            if method == "GetSolarTerms" {
                return Ok(
                    qq_farm_core::proto::generated::gamepb::solartermspb::GetSolarTermsReply::default()
                        .encode_to_vec(),
                );
            }
            Ok(season_bytes())
        });
        runtime.register_worker_loop("beta", worker("beta", other_gate));
        let ctx_other = ctx.clone();
        let queued = tokio::spawn(async move { exchange(&ctx, "acc", 4, 1).await });
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        exchange(&ctx_other, "beta", 4, 1).await.unwrap();
        assert_eq!(other_writes.load(Ordering::SeqCst), 1);
        runtime.unregister_worker_loop("acc");
        tx.send(()).unwrap();
        let error = queued.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("账号会话已变化"), "{error}");
        assert_eq!(acc_writes.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_blank_account_is_rejected_before_the_gateway() {
        let _data = TestDataDir::enter();
        let (ctx, _runtime, gateway) = context();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&calls);
        gateway.install_reply_script(move |_service, _method, _body| {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        });
        let error = exchange(&ctx, " ", 4, 1).await.unwrap_err();
        assert!(error.to_string().contains("账号身份无效"), "{error}");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
