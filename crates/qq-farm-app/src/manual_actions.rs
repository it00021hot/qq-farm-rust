//! 手动操作从 app 门面进入当前 worker 的生产调用。
#![cfg(test)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use prost::Message as _;
use qq_farm_core::config::paths::TestDataDir;
use qq_farm_core::models::account::AccountSession;
use qq_farm_core::models::types::AccountConfigSnapshot;
use qq_farm_core::network::encryptor::NoopEncryptor;
use qq_farm_core::network::error::NetworkError;
use qq_farm_core::network::gateway::{Gateway, GatewayConfig};
use qq_farm_core::proto::generated::corepb::{Item as CoreItem, ItemBag};
use qq_farm_core::proto::generated::gamepb::friendpb::{
    GameFriend, GetGameFriendsReply, GetGameFriendsRequest,
};
use qq_farm_core::proto::generated::gamepb::itempb::{BagReply, UseReply};
use qq_farm_core::proto::generated::gamepb::mallpb::{
    GetMallListBySlotTypeResponse, MallGoods, PurchaseRequest, PurchaseResponse,
};
use qq_farm_core::proto::generated::gamepb::plantpb::{
    AllLandsReply, FertilizeReply, FertilizeRequest, LandInfo, PlantInfo, PlantPhaseInfo,
};
use qq_farm_core::runtime::engine::{EngineConfig, RuntimeEngine};
use qq_farm_core::runtime::runtime_state::{AccountStoreLike, RuntimeState};
use qq_farm_core::runtime::worker_loop::{WorkerLoop, WorkerLoopConfig};
use qq_farm_core::services::{
    email::EmailService, farm::FarmService, friend::scheduler::FriendService, mall::MallService,
    monthcard::MonthCardService, mystery_shop::MysteryShopService, qqvip::QQVipService,
    share::ShareService, task::TaskService, warehouse::WarehouseService,
};

use crate::session::AppContext;

struct EmptyStore;
impl AccountStoreLike for EmptyStore {
    fn get_config_snapshot(&self, _id: &str) -> AccountConfigSnapshot {
        AccountConfigSnapshot::default()
    }
    fn get_automation(&self, _id: &str) -> serde_json::Value {
        serde_json::Value::Null
    }
    fn get_preferred_seed(&self, _id: &str) -> i64 {
        0
    }
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

fn worker(id: &str, gateway: Arc<Gateway>) -> Arc<WorkerLoop> {
    let (tx, _) = tokio::sync::broadcast::channel(8);
    let friend = Arc::new(FriendService::new(Arc::clone(&gateway), 5));
    friend.set_account_id(id);
    friend.set_host_gid(1);
    Arc::new(WorkerLoop::new(
        AccountSession::new(id, "code", "name"),
        WorkerLoopConfig::default(),
        Arc::clone(&gateway),
        tx,
        Arc::new(FarmService::new(Arc::clone(&gateway))),
        friend,
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

fn context(id: &str, gateway: Arc<Gateway>) -> AppContext {
    let runtime = Arc::new(RuntimeEngine::assemble_with(
        EngineConfig::default(),
        Arc::new(RuntimeState::new(Arc::new(EmptyStore), Vec::new())),
        None,
    ));
    runtime.register_worker_loop(id, worker(id, gateway));
    AppContext::new(runtime)
}

#[tokio::test]
async fn friend_page_force_refresh_returns_the_new_list() {
    let _data = TestDataDir::enter();
    qq_farm_core::models::store::account_config::add_known_friend_gids("friends-page", &[7]);
    let gate = gateway();
    let seen = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let seen_script = Arc::clone(&seen);
    let round = Arc::new(AtomicUsize::new(0));
    gate.install_reply_script(move |_service, method, body| {
        if method != "GetGameFriends" {
            return Err(NetworkError::WebSocket(format!("未准备 {method}")));
        }
        seen_script.lock().expect("requests").push(body.to_vec());
        let name = if round.fetch_add(1, Ordering::SeqCst) == 0 { "甲" } else { "乙" };
        Ok(GetGameFriendsReply {
            game_friends: vec![GameFriend { gid: 7, name: name.into(), ..Default::default() }],
            ..Default::default()
        }
        .encode_to_vec())
    });
    let ctx = context("friends-page", gate);
    let first = crate::friend::list_friends(&ctx, "friends-page", false).await.expect("首次");
    let cached = crate::friend::list_friends(&ctx, "friends-page", false).await.expect("缓存");
    assert_eq!(first[0].name, "甲");
    assert_eq!(cached[0].name, "甲");
    assert_eq!(seen.lock().expect("requests").len(), 1);
    let refreshed = crate::friend::list_friends(&ctx, "friends-page", true).await.expect("刷新");
    assert_eq!(refreshed[0].name, "乙");
    let bodies = seen.lock().expect("requests");
    assert_eq!(bodies.len(), 2);
    assert_eq!(GetGameFriendsRequest::decode(bodies[1].as_slice()).expect("请求").gids, vec![7]);
}

#[tokio::test]
async fn friend_page_delete_sends_del_friend_and_invalid_gid_does_not() {
    let _data = TestDataDir::enter();
    let gate = gateway();
    let seen = Arc::new(AtomicUsize::new(0));
    let seen_script = Arc::clone(&seen);
    gate.install_reply_script(move |_service, method, _body| {
        if method == "DelFriend" {
            seen_script.fetch_add(1, Ordering::SeqCst);
            return Ok(Vec::new());
        }
        Err(NetworkError::WebSocket(format!("未准备 {method}")))
    });
    let ctx = context("friends-op", gate);
    let invalid = crate::friend::friend_op(&ctx, "friends-op", 0, "steal").await.expect("无效");
    assert_eq!(invalid["ok"], false);
    assert_eq!(seen.load(Ordering::SeqCst), 0);
    crate::friend::delete_friend(&ctx, "friends-op", 9).await.expect("删除");
    assert_eq!(seen.load(Ordering::SeqCst), 1);
    assert!(crate::friend::delete_friend(&ctx, "friends-op", 0).await.is_err());
    assert_eq!(seen.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn fertilize_page_sends_the_growing_land_once() {
    let _data = TestDataDir::enter();
    qq_farm_core::config::game_config::global().load();
    let gate = gateway();
    let seen = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let seen_script = Arc::clone(&seen);
    gate.install_reply_script(move |_service, method, body| match method {
        "AllLands" => {
            let now = qq_farm_core::utils::time::get_server_time_secs();
            let growing = LandInfo {
                id: 2,
                unlocked: true,
                plant: Some(PlantInfo {
                    id: 102_0002,
                    phases: vec![
                        PlantPhaseInfo { phase: 3, begin_time: now - 600, ..Default::default() },
                        PlantPhaseInfo { phase: 6, begin_time: now + 3600, ..Default::default() },
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            };
            Ok(AllLandsReply {
                lands: vec![LandInfo { id: 1, unlocked: false, ..growing.clone() }, growing],
                ..Default::default()
            }
            .encode_to_vec())
        }
        "Fertilize" => {
            seen_script.lock().expect("requests").push(body.to_vec());
            Ok(FertilizeReply {
                land: vec![LandInfo { id: 2, ..Default::default() }],
                ..Default::default()
            }
            .encode_to_vec())
        }
        other => Err(NetworkError::WebSocket(format!("未准备 {other}"))),
    });
    let ctx = context("farm-page", gate);
    let result = crate::farm::fertilize_all(&ctx, "farm-page", "normal").await.expect("施肥");
    assert_eq!(result["successCount"], 1);
    let bodies = seen.lock().expect("requests");
    assert_eq!(bodies.len(), 1);
    let request = FertilizeRequest::decode(bodies[0].as_slice()).expect("请求");
    assert_eq!((request.land_ids.clone(), request.fertilizer_id), (vec![2], 1011));
}

#[tokio::test]
async fn bag_page_use_returns_reply_rewards_and_a_locked_stack_does_not_send() {
    let _data = TestDataDir::enter();
    let gate = gateway();
    let uses = Arc::new(AtomicUsize::new(0));
    let uses_script = Arc::clone(&uses);
    gate.install_reply_script(move |_service, method, _body| match method {
        "Bag" => Ok(BagReply {
            item_bag: Some(ItemBag {
                items: vec![
                    CoreItem { id: 41, count: 4, uid: 11, ..Default::default() },
                    CoreItem { id: 41, count: 2, uid: 12, locked: true, ..Default::default() },
                ],
                ..Default::default()
            }),
        }
        .encode_to_vec()),
        "Use" => {
            uses_script.fetch_add(1, Ordering::SeqCst);
            Ok(UseReply {
                items: vec![CoreItem { id: 8, count: 2, ..Default::default() }],
                ..Default::default()
            }
            .encode_to_vec())
        }
        other => Err(NetworkError::WebSocket(format!("未准备 {other}"))),
    });
    let ctx = context("bag-page", gate);
    assert!(crate::farm::bag_use(&ctx, "bag-page", 41, 5, 12).await.is_err());
    assert_eq!(uses.load(Ordering::SeqCst), 0);
    let used = crate::farm::bag_use(&ctx, "bag-page", 41, 2, 11).await.expect("使用");
    assert_eq!(uses.load(Ordering::SeqCst), 1);
    let text = used.to_string();
    assert!(text.contains("8") && text.contains("2"), "使用结果应带回包奖励，实际 {text}");
}

#[tokio::test]
async fn mall_page_purchase_keeps_reply_rewards() {
    let _data = TestDataDir::enter();
    let gate = gateway();
    let purchases = Arc::new(AtomicUsize::new(0));
    let purchases_script = Arc::clone(&purchases);
    gate.install_reply_script(move |_service, method, body| match method {
        "GetMallListBySlotType" => Ok(GetMallListBySlotTypeResponse {
            goods_list: vec![MallGoods {
                goods_id: 50,
                is_free: true,
                is_available: true,
                ..Default::default()
            }],
            ..Default::default()
        }
        .encode_to_vec()),
        "Purchase" => {
            let request = PurchaseRequest::decode(body).expect("请求");
            assert_eq!((request.goods_id, request.count), (50, 1));
            purchases_script.fetch_add(1, Ordering::SeqCst);
            Ok(PurchaseResponse {
                success: true,
                goods_id: 50,
                reward_items: vec![CoreItem { id: 9, count: 2, ..Default::default() }],
                ..Default::default()
            }
            .encode_to_vec())
        }
        "Bag" => Ok(BagReply::default().encode_to_vec()),
        other => Err(NetworkError::WebSocket(format!("未准备 {other}"))),
    });
    let ctx = context("mall-page", gate);
    assert!(crate::commerce::purchase_mall(&ctx, "mall-page", 50, 0, Some(1), None).await.is_err());
    assert_eq!(purchases.load(Ordering::SeqCst), 0);
    let bought = crate::commerce::purchase_mall(&ctx, "mall-page", 50, 1, Some(1), None)
        .await
        .expect("购买");
    assert_eq!(purchases.load(Ordering::SeqCst), 1);
    assert_eq!(bought["purchase"]["rewards"][0]["count"], 2);
}
