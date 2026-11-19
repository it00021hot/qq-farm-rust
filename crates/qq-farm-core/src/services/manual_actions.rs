//! 手动操作的生产调用：强制刷新、好友动作、施肥、背包和商城购买。
#![cfg(test)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use prost::Message as _;

use crate::config::paths::TestDataDir;
use crate::network::encryptor::NoopEncryptor;
use crate::network::error::NetworkError;
use crate::network::gateway::{Gateway, GatewayConfig};
use crate::proto::generated::corepb::{Item as CoreItem, ItemBag};
use crate::proto::generated::gamepb::friendpb::{
    DelFriendRequest, GameFriend, GetGameFriendsReply, GetGameFriendsRequest,
};
use crate::proto::generated::gamepb::itempb::{
    BagReply, LockItemsReply, LockItemsRequest, SellReply, SellRequest, UseReply, UseRequest,
};
use crate::proto::generated::gamepb::mallpb::{
    GetMallListBySlotTypeResponse, MallGoods, PurchaseLimit, PurchaseRequest, PurchaseResponse,
};
use crate::proto::generated::gamepb::plantpb::{
    AllLandsReply, FarmingRequest, FertilizeReply, FertilizeRequest, HarvestRequest, LandInfo,
    PlantInfo, PlantPhaseInfo, PutInsectsReply, PutInsectsRequest, PutWeedsReply, PutWeedsRequest,
};
use crate::services::commerce::service::CommerceService;
use crate::services::farm::FarmService;
use crate::services::friend::api::FriendApi;
use crate::services::friend::scheduler::FriendService;
use crate::services::mall::MallService;
use crate::services::mystery_shop::MysteryShopService;
use crate::services::qqvip::QQVipService;
use crate::services::warehouse::WarehouseService;

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

fn reject(method: &str) -> NetworkError {
    NetworkError::WebSocket(format!("未准备 {method}"))
}

#[tokio::test]
async fn force_refresh_invalidates_the_cached_friend_list_and_returns_the_new_reply() {
    let _data = TestDataDir::enter();
    let gate = gateway();
    let seen = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let seen_script = Arc::clone(&seen);
    let round = Arc::new(AtomicUsize::new(0));
    gate.install_reply_script(move |_service, method, body| {
        if method != "GetGameFriends" {
            return Err(reject(method));
        }
        seen_script.lock().expect("requests").push(body.to_vec());
        let name = if round.fetch_add(1, Ordering::SeqCst) == 0 { "甲" } else { "乙" };
        Ok(GetGameFriendsReply {
            game_friends: vec![GameFriend {
                gid: 7,
                name: name.to_string(),
                ..GameFriend::default()
            }],
            ..GetGameFriendsReply::default()
        }
        .encode_to_vec())
    });
    let friends = FriendService::new(Arc::clone(&gate), 5);
    friends.set_account_id("manual-friends");
    friends.set_host_gid(1);
    crate::models::store::account_config::add_known_friend_gids("manual-friends", &[7]);

    let first = friends.get_friends_list(false).await.expect("首次列表");
    assert_eq!(first.len(), 1);
    assert_eq!(first[0]["name"], "甲");
    let cached = friends.get_friends_list(false).await.expect("缓存列表");
    assert_eq!(cached[0]["name"], "甲");
    assert_eq!(seen.lock().expect("requests").len(), 1, "缓存有效期内不应再拉好友");

    let refreshed = friends.get_friends_list(true).await.expect("强制刷新");
    assert_eq!(refreshed[0]["name"], "乙");
    let bodies = seen.lock().expect("requests");
    assert_eq!(bodies.len(), 2, "强制刷新必须再发获取请求");
    let request = GetGameFriendsRequest::decode(bodies[1].as_slice()).expect("请求可解码");
    assert_eq!(request.gids, vec![7]);
}

#[tokio::test]
async fn friend_help_steal_trouble_and_delete_send_source_parameters() {
    let _data = TestDataDir::enter();
    let gate = gateway();
    let seen = Arc::new(Mutex::new(Vec::<(String, Vec<u8>)>::new()));
    let seen_script = Arc::clone(&seen);
    gate.install_reply_script(move |_service, method, body| {
        seen_script.lock().expect("requests").push((method.to_string(), body.to_vec()));
        let land = |id| LandInfo { id, ..Default::default() };
        match method {
            "PutWeeds" => {
                Ok(PutWeedsReply { land: vec![land(5)], ..Default::default() }.encode_to_vec())
            }
            "PutInsects" => {
                Ok(PutInsectsReply { land: vec![land(6)], ..Default::default() }.encode_to_vec())
            }
            _ => Ok(Vec::new()),
        }
    });
    let api = FriendApi::new(Arc::clone(&gate));
    let friends = FriendService::new(Arc::clone(&gate), 5);
    friends.set_account_id("manual-ops");

    let invalid = friends
        .do_friend_operation(crate::models::types::FriendOperation::Steal, 0)
        .await
        .expect("无效好友返回结果");
    assert_eq!(invalid["ok"], false);
    assert!(seen.lock().expect("requests").is_empty(), "无效好友不进农场");

    let help = api.help_farm(9, vec![]).await.expect("空地帮忙");
    assert_eq!(help.operation_count, 0);
    assert!(seen.lock().expect("requests").is_empty(), "没有目标地不发帮忙");

    api.help_farm(9, vec![3, 3]).await.expect("帮忙");
    api.steal_farm(9, vec![4], false).await.expect("偷菜");
    let weeds = api.put_weeds(9, vec![5]).await.expect("放草");
    assert_eq!(weeds, 1);
    let insects = api.put_insects(9, vec![6]).await.expect("放虫");
    assert_eq!(insects, 1);
    friends.delete_friend(9).await.expect("删除");
    let missing = friends.delete_friend(0).await;
    assert!(missing.is_err(), "无效 GID 不删除");

    let sent = seen.lock().expect("requests");
    assert_eq!(
        sent.iter().map(|(method, _)| method.as_str()).collect::<Vec<_>>(),
        vec!["Farming", "Harvest", "PutWeeds", "PutInsects", "DelFriend"]
    );
    let farming = FarmingRequest::decode(sent[0].1.as_slice()).expect("帮忙请求");
    assert_eq!((farming.host_gid, farming.land_ids, farming.field_4), (9, vec![3], 2));
    let harvest = HarvestRequest::decode(sent[1].1.as_slice()).expect("偷菜请求");
    assert_eq!((harvest.host_gid, harvest.land_ids, harvest.is_all), (9, vec![4], false));
    let weed = PutWeedsRequest::decode(sent[2].1.as_slice()).expect("放草请求");
    assert_eq!((weed.host_gid, weed.land_ids), (9, vec![5]));
    let insect = PutInsectsRequest::decode(sent[3].1.as_slice()).expect("放虫请求");
    assert_eq!((insect.host_gid, insect.land_ids), (9, vec![6]));
    let deleted = DelFriendRequest::decode(sent[4].1.as_slice()).expect("删除请求");
    assert_eq!(deleted.friend_gid, 9);
}

#[tokio::test]
async fn fertilize_all_sends_only_growing_lands_and_keeps_the_reply_count() {
    let _data = TestDataDir::enter();
    crate::config::game_config::global().load();
    let gate = gateway();
    let seen = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let seen_script = Arc::clone(&seen);
    gate.install_reply_script(move |_service, method, body| match method {
        "AllLands" => {
            let now = crate::utils::time::get_server_time_secs();
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
            let locked = LandInfo { id: 1, unlocked: false, ..growing.clone() };
            Ok(AllLandsReply { lands: vec![locked, growing], ..Default::default() }.encode_to_vec())
        }
        "Fertilize" => {
            seen_script.lock().expect("requests").push(body.to_vec());
            Ok(FertilizeReply {
                land: vec![LandInfo { id: 2, ..Default::default() }],
                fertilizer: Some(CoreItem { id: 1011, count: 80, ..Default::default() }),
                ..Default::default()
            }
            .encode_to_vec())
        }
        other => Err(reject(other)),
    });
    let farm = FarmService::new(gate);
    farm.set_account_id("manual-farm");
    let result = farm.fertilize_all_own_lands("normal", None).await.expect("施肥");
    assert_eq!(result["successCount"], 1);
    assert_eq!(result["requestedCount"], 1);
    {
        let bodies = seen.lock().expect("requests");
        assert_eq!(bodies.len(), 1);
        let request = FertilizeRequest::decode(bodies[0].as_slice()).expect("施肥请求");
        assert_eq!(request.land_ids, vec![2]);
        assert_eq!(request.fertilizer_id, 1011);
    }
    let skipped = farm.fertilize_all_own_lands("normal", Some(&[1])).await.expect("排除地");
    assert_eq!(skipped["requestedCount"], 0);
    assert_eq!(seen.lock().expect("requests").len(), 1, "选中未解锁地不施肥");
}

#[tokio::test]
async fn bag_use_lock_and_sell_apply_reply_counts_and_reject_before_sending() {
    let _data = TestDataDir::enter();
    let config = crate::config::game_config::global();
    config.load();
    let seed = config.get_items_by_type(5).into_iter().next().expect("配置里应有种子");
    let fruit = config
        .get_items_by_type(6)
        .into_iter()
        .find(|item| config.get_effective_sell_info_by_id_at(item.id, 0).sellable)
        .expect("配置里应有可售果实");
    let gate = gateway();
    let seen = Arc::new(Mutex::new(Vec::<(String, Vec<u8>)>::new()));
    let seen_script = Arc::clone(&seen);
    let seed_id = seed.id;
    let fruit_id = fruit.id;
    gate.install_reply_script(move |_service, method, body| {
        seen_script.lock().expect("requests").push((method.to_string(), body.to_vec()));
        match method {
            "Bag" => Ok(BagReply {
                item_bag: Some(ItemBag {
                    items: vec![
                        CoreItem { id: seed_id, count: 4, uid: 11, ..Default::default() },
                        CoreItem {
                            id: seed_id,
                            count: 2,
                            uid: 12,
                            locked: true,
                            ..Default::default()
                        },
                        CoreItem { id: fruit_id, count: 3, uid: 21, ..Default::default() },
                    ],
                    ..Default::default()
                }),
            }
            .encode_to_vec()),
            "Use" => Ok(UseReply {
                items: vec![CoreItem { id: 8, count: 2, ..Default::default() }],
                ..Default::default()
            }
            .encode_to_vec()),
            "LockItems" => Ok(LockItemsReply { item_uids: vec![11] }.encode_to_vec()),
            "Sell" => Ok(SellReply {
                get_items: vec![CoreItem { id: 1001, count: 30, ..Default::default() }],
                ..Default::default()
            }
            .encode_to_vec()),
            other => Err(reject(other)),
        }
    });
    let bag = WarehouseService::new(Arc::clone(&gate));
    let denied = bag.use_item(seed_id, 5, 12).await;
    assert!(denied.is_err(), "锁定数量不够时不使用");
    assert!(
        seen.lock().expect("requests").iter().all(|(method, _)| method != "Use"),
        "拒绝使用时不发 Use"
    );
    let used = bag.use_item(seed_id, 2, 11).await.expect("使用");
    assert_eq!(
        used.items.iter().map(|item| (item.id, item.count)).collect::<Vec<_>>(),
        vec![(8, 2)]
    );
    let locked = bag.set_items_locked(&[11], true).await.expect("锁定");
    assert_eq!(locked, vec![11]);
    let sold = bag.sell_items(&[(fruit_id, 3, 21)]).await.expect("出售");
    assert_eq!(sold.get_items.iter().map(|item| item.count).collect::<Vec<_>>(), vec![30]);

    let sent = seen.lock().expect("requests");
    let use_body = sent.iter().find(|(method, _)| method == "Use").expect("使用请求").1.as_slice();
    let use_request = UseRequest::decode(use_body).expect("使用请求可解码");
    let item = use_request.item.expect("使用物品");
    assert_eq!((item.id, item.count, item.uid), (seed_id, 2, 11));
    let lock_body =
        sent.iter().find(|(method, _)| method == "LockItems").expect("锁定请求").1.as_slice();
    assert_eq!(LockItemsRequest::decode(lock_body).expect("锁定").item_uids, vec![11]);
    let sell_body =
        sent.iter().find(|(method, _)| method == "Sell").expect("出售请求").1.as_slice();
    let sell = SellRequest::decode(sell_body).expect("出售");
    assert_eq!(sell.items.len(), 1);
    assert_eq!((sell.items[0].id, sell.items[0].count, sell.items[0].uid), (fruit_id, 3, 21));
}

#[tokio::test]
async fn mall_purchase_sends_goods_and_count_and_keeps_only_reply_rewards() {
    let _data = TestDataDir::enter();
    let gate = gateway();
    let lists = Arc::new(AtomicUsize::new(0));
    let purchases = Arc::new(AtomicUsize::new(0));
    let lists_script = Arc::clone(&lists);
    let purchases_script = Arc::clone(&purchases);
    gate.install_reply_script(move |_service, method, body| match method {
        "GetMallListBySlotType" => {
            let nth = lists_script.fetch_add(1, Ordering::SeqCst);
            if nth == 1 {
                return Err(reject("刷新目录"));
            }
            let goods = match nth {
                0 => MallGoods {
                    goods_id: 50,
                    is_free: true,
                    is_available: true,
                    ..Default::default()
                },
                2 => MallGoods {
                    goods_id: 60,
                    is_free: false,
                    is_available: true,
                    ..Default::default()
                },
                _ => MallGoods {
                    goods_id: 61,
                    is_free: true,
                    is_available: true,
                    purchase_limit: Some(PurchaseLimit {
                        limit_type: 1,
                        bought_count: 1,
                        limit_count: 1,
                    }),
                    ..Default::default()
                },
            };
            Ok(GetMallListBySlotTypeResponse { goods_list: vec![goods], ..Default::default() }
                .encode_to_vec())
        }
        "Purchase" => {
            purchases_script.fetch_add(1, Ordering::SeqCst);
            let request = PurchaseRequest::decode(body).expect("购买请求");
            assert_eq!((request.goods_id, request.count), (50, 1));
            Ok(PurchaseResponse {
                success: true,
                goods_id: 50,
                reward_items: vec![CoreItem { id: 9, count: 2, ..Default::default() }],
                ..Default::default()
            }
            .encode_to_vec())
        }
        "Bag" => Ok(BagReply::default().encode_to_vec()),
        other => Err(reject(other)),
    });
    let mall = Arc::new(MallService::new(Arc::clone(&gate)));
    let commerce = CommerceService::new(
        Arc::clone(&mall),
        Arc::new(MysteryShopService::new(Arc::clone(&gate))),
        Arc::new(WarehouseService::new(Arc::clone(&gate))),
        Arc::new(QQVipService::new(gate, Arc::clone(&mall))),
    );
    let denied = commerce.purchase_mall_product("50", "0", Some(1), None).await;
    assert!(denied.is_err(), "数量 0 不购买");
    assert_eq!(purchases.load(Ordering::SeqCst), 0);

    let bought = commerce.purchase_mall_product("50", "1", Some(1), None).await.expect("购买");
    assert_eq!(purchases.load(Ordering::SeqCst), 1);
    assert_eq!(bought.purchase.rewards.len(), 1);
    assert_eq!(bought.purchase.rewards[0].count, 2);
    assert!(bought.refresh_required, "购买后的目录刷新失败不重买");
    assert_eq!(purchases.load(Ordering::SeqCst), 1);

    let unknown_cost = commerce.purchase_mall_product("60", "1", Some(1), None).await;
    assert!(unknown_cost.is_err(), "价格未确认不购买");
    let over_limit = commerce.purchase_mall_product("61", "1", Some(1), None).await;
    assert!(over_limit.is_err(), "已达限购不购买");
    assert_eq!(purchases.load(Ordering::SeqCst), 1);
}
