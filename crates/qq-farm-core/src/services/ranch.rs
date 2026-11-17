//! 牧场场景读取、生产、喂养和收获。
//!
//! 进入场景 2 期间占用账号自动化互斥锁，结束后回到进入前的场景。

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::sync::{Arc, OnceLock};

use prost::Message;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::animalpb::{
    AnimalDetail, AnimalRef, CleanAllRanchReply, CleanAllRanchRequest, DonateAnimalReply,
    DonateAnimalRequest, FeedAnimalReply, FeedAnimalRequest, GetRanchOpTimesReply,
    GetRanchOpTimesRequest, HarvestRanchReply, HarvestRanchRequest, PlaceAnimalItem,
    PlaceAnimalReply, PlaceAnimalRequest, RanchOpTimesView, TriggerProduceReply,
    TriggerProduceRequest,
};
use crate::proto::generated::gamepb::homesteadpb::{
    AddFeedReply, AddFeedRequest, AllCellsReply, AllCellsRequest, CompleteBuildingUpgradeReply,
    CompleteBuildingUpgradeRequest, FacilityInfo, GetFacilityDetailReply, GetFacilityDetailRequest,
    UpgradeBuildingReply, UpgradeBuildingRequest,
};
use crate::proto::generated::gamepb::mallpb::{
    GetMallListBySlotTypeRequest, GetMallListBySlotTypeResponse, MallGoods, PurchaseLimit,
    PurchaseRequest, PurchaseResponse,
};
use crate::proto::generated::gamepb::ranchpb::{
    ClaimAllRewardsV2Reply, ClaimAllRewardsV2Request, ClaimRanchWeightScoreRewardV2Reply,
    ClaimRanchWeightScoreRewardV2Request, GetBagReply, GetBagRequest, GetIllustratedListV2Reply,
    GetIllustratedListV2Request, GetRanchWeightScoreV2Reply, GetRanchWeightScoreV2Request,
    IllustratedInfoV2, Item as RanchItem, ItemEntry, RanchGiantRecordView, RanchWeightScoreLevelV2,
    ReportShareReply, ReportShareRequest, RotationBuyGoodsReply, RotationBuyGoodsRequest,
    RotationGoodsCard, RotationShopInfoReply, RotationShopInfoRequest, SceneData, SellReply,
    SellRequest, UseReply, UseRequest,
};
use crate::proto::generated::gamepb::shoppb::{
    BuyGoodsReply, BuyGoodsRequest, GoodsInfo, ShopInfoReply, ShopInfoRequest, ShowCond,
};
use crate::proto::generated::gamepb::taskpb::{
    BatchClaimTaskRewardReply, BatchClaimTaskRewardRequest, ClaimTaskRewardReply,
    ClaimTaskRewardRequest, Task, TaskInfo, TaskInfoReply, TaskInfoRequest,
};

#[path = "ranch_patrol.rs"]
mod patrol;

pub use patrol::{patrol_if_due, reset_patrol_schedule};

const RANCH_SCENE: i32 = 2;
const RANCH_MODULES: [i32; 3] = [6, 7, 8];
const GRASS_MACHINE: i32 = 8;
const GRASS_ITEM: i64 = 7001;
const NORMAL_BUCKET: i64 = 7003;
const ORGANIC_BUCKET: i64 = 7004;
const BUCKET_ORGANIC: i32 = 1;
const BUCKET_NORMAL: i32 = 2;
const HOUR_SECS: i64 = 3600;
const HARVEST_ALL: i32 = 1;
const HARVEST_SELECTED: i32 = 4;
const LOCKED_REASON: &str = "牧场尚未解锁，请先在游戏中完成开启条件";
const STATE_CHANGED: &str = "动物状态已变化，请刷新后重新操作";
const HOMESTEAD_SERVICE: &str = "gamepb.homesteadpb.HomesteadService";
const ITEM_SERVICE: &str = "gamepb.itempb.ItemService";
const ANIMAL_SERVICE: &str = "gamepb.animalpb.AnimalService";
const RANCH_SERVICE: &str = "gamepb.animalpb.RanchService";
const FEED_MACHINE_SERVICE: &str = "gamepb.homesteadpb.FeedMachineService";
const SHOP_SERVICE: &str = "gamepb.shoppb.ShopService";
const TASK_SERVICE: &str = "gamepb.taskpb.TaskService";
const SHARE_SERVICE: &str = "gamepb.sharepb.ShareService";
const ILLUSTRATED_SERVICE: &str = "gamepb.illustratedpb.IllustratedService";
const MALL_SERVICE: &str = "gamepb.mallpb.MallService";
const SHOP_CUBS: i64 = 5;
const SHOP_FEED: i64 = 6;
const MARKET_SHOP: i64 = 7;
const RANCH_TASK_GROWTH: i32 = 4;
const RANCH_TASK_DAILY: i32 = 5;
const RANCH_SHARE_ID: i64 = 103;
const RANCH_SHARE_DOC: i64 = 2104;
const BOOK_TYPE: i32 = 101;
const CAN_SLOT_REGULAR: i32 = 6;
const CAN_SLOT_LIMITED: i32 = 7;
const BENEFIT_GOODS_ID: i64 = 2001;
const BENEFIT_LIMIT_TYPE: i32 = 1;
const BENEFIT_LIMIT_COUNT: i64 = 1;
const CAN_MALL_MAX: i64 = 999;
const CAN_SHARE_READY: i32 = 2;
const NEST: i32 = 1;
const SHED: i32 = 2;
const SELLABLE_TYPES: [i64; 2] = [106, 107];
const RELEASE_OP: i64 = 11008;
const RELEASE_KIND: i32 = 1;
const FACILITY_READY: i32 = 1;
const UPGRADE_IDLE: i32 = 0;
const UPGRADE_FINISHED: i32 = 2;
const RANCH_EXP: i64 = 1102;
const SECOND_TIMESTAMP_LIMIT: i64 = 100_000_000_000;

/// 牧场概览。未解锁时返回 `unlocked: false`，不切换场景。
pub async fn snapshot(gateway: &Arc<Gateway>, account_id: &str) -> Result<Value> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Ok(json!({ "unlocked": false, "sceneId": RANCH_SCENE, "reason": LOCKED_REASON }));
    }
    let account = account_id.to_owned();
    in_ranch(gateway, account_id, move |cells, bag| async move {
        Ok(json!({
            "unlocked": true,
            "sceneId": RANCH_SCENE,
            "grass": item_count(&bag, GRASS_ITEM),
            "cubs": cub_views(&bag),
            "products": product_views(&bag),
            "cans": can_views(&bag),
            "pending": pending_views(&account)?,
            "facilities": facility_views(
                &cells,
                ranch_level(item_count(&bag, RANCH_EXP)),
                crate::utils::time::get_server_time_secs().saturating_mul(1000),
            ),
        }))
    })
    .await
}

/// 让可生产的动物生产。没有可生产动物时不发送请求。
pub async fn produce(
    gateway: &Arc<Gateway>,
    account_id: &str,
    ranch_id: Option<i64>,
    animal_id: Option<i32>,
) -> Result<()> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    in_ranch(gateway, account_id, move |cells, _bag| async move {
        let Some(request) = produce_request(&cells, ranch_id, animal_id)? else {
            return Ok(());
        };
        let bytes =
            owned.request(ANIMAL_SERVICE, "TriggerProduce", &request.encode_to_vec()).await?;
        TriggerProduceReply::decode(bytes.as_slice())?;
        Ok(())
    })
    .await
}

/// 用普通或有机罐头喂养可喂养的动物。没有目标时不发送请求。
pub async fn feed(
    gateway: &Arc<Gateway>,
    account_id: &str,
    ranch_id: Option<i64>,
    animal_id: Option<i32>,
    bucket_type: i32,
) -> Result<()> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let now_secs = crate::utils::time::get_server_time_secs();
    in_ranch(gateway, account_id, move |cells, bag| async move {
        let Some(request) = feed_request(&cells, &bag, ranch_id, animal_id, bucket_type, now_secs)?
        else {
            return Ok(());
        };
        let bytes = owned.request(ANIMAL_SERVICE, "FeedAnimal", &request.encode_to_vec()).await?;
        FeedAnimalReply::decode(bytes.as_slice())?;
        Ok(())
    })
    .await
}

/// 清理所有带有障碍的牧场。没有障碍时不发送请求。
pub async fn clean(gateway: &Arc<Gateway>, account_id: &str) -> Result<()> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    in_ranch(gateway, account_id, move |cells, _bag| async move {
        let Some(request) = clean_request(&cells) else {
            return Ok(());
        };
        let bytes = owned.request(RANCH_SERVICE, "CleanAllRanch", &request.encode_to_vec()).await?;
        CleanAllRanchReply::decode(bytes.as_slice())?;
        Ok(())
    })
    .await
}

/// 向牧草机加入指定份数的牧草。
pub async fn add_grass(
    gateway: &Arc<Gateway>,
    account_id: &str,
    facility_id: i64,
    count: i32,
) -> Result<()> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    in_ranch(gateway, account_id, move |cells, bag| async move {
        let request = grass_request(&cells, &bag, facility_id, count)?;
        let bytes =
            owned.request(FEED_MACHINE_SERVICE, "AddFeed", &request.encode_to_vec()).await?;
        AddFeedReply::decode(bytes.as_slice())?;
        Ok(())
    })
    .await
}

/// 幼崽商店或饲料商店的商品。
pub async fn shop(gateway: &Arc<Gateway>, account_id: &str, shop_id: i64) -> Result<Value> {
    require_shop(shop_id)?;
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Ok(json!({ "unlocked": false, "shopId": shop_id, "goods": [] }));
    }
    let owned = Arc::clone(gateway);
    let now = crate::utils::time::get_server_time_secs();
    in_ranch(gateway, account_id, move |_cells, bag| async move {
        let reply = shop_info(&owned, shop_id).await?;
        let goods = shop_offers(&reply.goods_list, shop_id, now)
            .into_iter()
            .map(|offer| offer_json(&offer, unlocked_count(&bag, offer.price_id)))
            .collect::<Vec<_>>();
        Ok(json!({ "unlocked": true, "shopId": shop_id, "goods": goods }))
    })
    .await
}

/// 按当前标价购买牧场商店商品，幼崽留在背包。
pub async fn buy_goods(
    gateway: &Arc<Gateway>,
    account_id: &str,
    shop_id: i64,
    goods_id: i64,
    count: i64,
) -> Result<()> {
    purchase(gateway, account_id, shop_id, goods_id, count, false).await
}

/// 购买幼崽并放入对应的窝或棚。
pub async fn buy_and_place(
    gateway: &Arc<Gateway>,
    account_id: &str,
    shop_id: i64,
    goods_id: i64,
    count: i64,
) -> Result<()> {
    purchase(gateway, account_id, shop_id, goods_id, count, true).await
}

async fn purchase(
    gateway: &Arc<Gateway>,
    account_id: &str,
    shop_id: i64,
    goods_id: i64,
    count: i64,
    auto_place: bool,
) -> Result<()> {
    require_shop(shop_id)?;
    if count <= 0 {
        return Err(Error::Business("数量无效".to_owned()));
    }
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    let now = crate::utils::time::get_server_time_secs();
    in_ranch(gateway, account_id, move |cells, bag| async move {
        let reply = shop_info(&owned, shop_id).await?;
        let offer = shop_offers(&reply.goods_list, shop_id, now)
            .into_iter()
            .find(|offer| offer.id == goods_id)
            .ok_or_else(|| Error::Business("商品尚未解锁、已售罄或配置未确认".to_owned()))?;
        if !offer.available {
            return Err(Error::Business("商品尚未解锁、已售罄或配置未确认".to_owned()));
        }
        if count > offer.max_count || offer.remaining.is_some_and(|remaining| count > remaining) {
            return Err(Error::Business("数量超过本次购买或限购上限".to_owned()));
        }
        let animal_id = placement_target(&offer, &cells, count, auto_place)?;
        let cost = offer.unit_price.saturating_mul(count);
        if unlocked_count(&bag, offer.price_id) < cost {
            return Err(Error::Business("余额不足".to_owned()));
        }
        let plan = money_plan(
            format!("ranch:buy:{goods_id}"),
            purchase_label(offer.item_id, offer.item_count.saturating_mul(count)),
            &bag,
            &[(offer.price_id, cost)],
        );
        let price_id = offer.price_id;
        let item_id = offer.item_id;
        let item_count = offer.item_count;
        let unit_price = offer.unit_price;
        crate::services::ranch_ops::spend(&account, plan, || async move {
            let request =
                BuyGoodsRequest { goods_id, num: count, price: unit_price, auto_add: auto_place };
            let bytes = owned.request(SHOP_SERVICE, "BuyGoods", &request.encode_to_vec()).await?;
            let bought = BuyGoodsReply::decode(bytes.as_slice())?;
            let expected_cost =
                item_signature(&[CoreItem { id: price_id, count: cost, ..CoreItem::default() }]);
            let expected_reward = item_signature(&[CoreItem {
                id: item_id,
                count: item_count.saturating_mul(count),
                ..CoreItem::default()
            }]);
            if bought.goods.as_ref().map(|goods| goods.id) != Some(goods_id)
                || item_signature(&bought.cost_items) != expected_cost
            {
                return Err(Error::Protocol("商品或扣费不匹配".to_owned()));
            }
            match animal_id {
                Some(animal_id) => confirm_placement(&owned, &cells, animal_id, count).await,
                None if item_signature(&bought.get_items) != expected_reward => {
                    Err(Error::Protocol("入包物品不匹配".to_owned()))
                }
                None => Ok(()),
            }
        })
        .await
    })
    .await
}

/// 把背包里的一只或多只幼崽放进指定窝或棚。
pub async fn place(
    gateway: &Arc<Gateway>,
    account_id: &str,
    ranch_id: i64,
    item_id: i64,
    count: i64,
) -> Result<()> {
    if count <= 0 {
        return Err(Error::Business("数量无效".to_owned()));
    }
    if count > i64::from(i32::MAX) {
        return Err(Error::Business("数量超出范围".to_owned()));
    }
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    in_ranch(gateway, account_id, move |cells, bag| async move {
        let request = place_request(&cells, &bag, ranch_id, item_id, count)?;
        let before = item_count(&bag, item_id);
        let plan = money_plan(
            format!("ranch:place:{ranch_id}:{item_id}"),
            "放养幼崽".to_owned(),
            &bag,
            &[(item_id, count)],
        );
        crate::services::ranch_ops::spend(&account, plan, || async move {
            let bytes =
                owned.request(ANIMAL_SERVICE, "PlaceAnimal", &request.encode_to_vec()).await?;
            let reply = PlaceAnimalReply::decode(bytes.as_slice())?;
            if i64::try_from(reply.op_results.len()).ok() != Some(count) {
                return Err(Error::Protocol("放养数量不匹配".to_owned()));
            }
            let after = match ranch_bag(&owned).await {
                Ok(bag) => bag,
                Err(error) => {
                    tracing::error!(%error, "放养后未能重新读取背包");
                    return Err(Error::Protocol("操作后的库存未能读取".to_owned()));
                }
            };
            if before.checked_sub(item_count(&after, item_id)) != Some(count) {
                return Err(Error::Protocol("库存扣减与费用不匹配".to_owned()));
            }
            Ok(())
        })
        .await
    })
    .await
}

/// 牧场集市当前货架。集市未解锁时不切换场景。
pub async fn market(gateway: &Arc<Gateway>, account_id: &str) -> Result<Value> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Ok(json!({ "opened": false, "present": false, "goods": [] }));
    }
    let owned = Arc::clone(gateway);
    let now = crate::utils::time::get_server_time_secs();
    in_ranch(gateway, account_id, move |_cells, bag| async move {
        let reply = market_info(&owned).await?;
        let level = ranch_level(item_count(&bag, RANCH_EXP));
        let goods = reply
            .goods
            .iter()
            .map(|card| {
                market_json(card, &reply, level, now, unlocked_count(&bag, card.cost_item_id))
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "opened": reply.opened,
            "present": reply.present,
            "periodDay": reply.period_day.to_string(),
            "roundIndex": reply.round_index,
            "goods": goods,
        }))
    })
    .await
}

/// 按当前集市标价兑换一份商品。
pub async fn exchange(
    gateway: &Arc<Gateway>,
    account_id: &str,
    goods_id: i64,
    count: i64,
) -> Result<()> {
    if count <= 0 {
        return Err(Error::Business("数量无效".to_owned()));
    }
    let Ok(count_i32) = i32::try_from(count) else {
        return Err(Error::Business("数量超出范围".to_owned()));
    };
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    let now = crate::utils::time::get_server_time_secs();
    in_ranch(gateway, account_id, move |_cells, bag| async move {
        let info = market_info(&owned).await?;
        let level = ranch_level(item_count(&bag, RANCH_EXP));
        let Some(card) = info.goods.iter().find(|card| card.goods_id == goods_id) else {
            return Err(Error::Business("集市已离场、商品受限或限购已用完".to_owned()));
        };
        let offer = market_offer(card, info.opened, info.present, level, now);
        if !offer.available || offer.remaining.is_some_and(|remaining| count > remaining) {
            return Err(Error::Business("集市已离场、商品受限或限购已用完".to_owned()));
        }
        let cost = offer.unit_price.saturating_mul(count);
        if unlocked_count(&bag, offer.price_id) < cost {
            return Err(Error::Business(format!("{}不足", item_name(offer.price_id))));
        }
        let price_id = offer.price_id;
        let reward_id = offer.item_id;
        let reward_count = offer.item_count;
        let before = item_count(&bag, price_id);
        let period_day = info.period_day;
        let round_index = info.round_index;
        let plan = money_plan(
            format!("ranch:exchange:{goods_id}"),
            item_name(reward_id),
            &bag,
            &[(price_id, cost)],
        );
        crate::services::ranch_ops::spend(&account, plan, || async move {
            let request =
                RotationBuyGoodsRequest { goods_id, count: count_i32, period_day, round_index };
            let bytes =
                owned.request(SHOP_SERVICE, "RotationBuyGoods", &request.encode_to_vec()).await?;
            let bought = RotationBuyGoodsReply::decode(bytes.as_slice())?;
            let expected = id_count_signature([(reward_id, reward_count.saturating_mul(count))]);
            if bought.card.as_ref().map(|card| card.goods_id) != Some(goods_id)
                || id_count_signature(bought.awards.iter().map(|item| (item.id, item.count)))
                    != expected
            {
                return Err(Error::Protocol("兑换商品或奖励不匹配".to_owned()));
            }
            let after = match ranch_bag(&owned).await {
                Ok(bag) => bag,
                Err(error) => {
                    tracing::error!(%error, "集市兑换后未能重新读取背包");
                    return Err(Error::Protocol("操作后的库存未能读取".to_owned()));
                }
            };
            if before.checked_sub(item_count(&after, price_id)) != Some(cost) {
                return Err(Error::Protocol("库存扣减与费用不匹配".to_owned()));
            }
            Ok(())
        })
        .await
    })
    .await
}

/// 牧场罐头商城。货架只接受 6 和 7。牧场未解锁时不切换场景。
pub async fn can_shop(gateway: &Arc<Gateway>, account_id: &str, slot: i32) -> Result<Value> {
    require_can_slot(slot)?;
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Ok(json!({ "slot": slot, "goods": [] }));
    }
    let owned = Arc::clone(gateway);
    let now = crate::utils::time::get_server_time_secs();
    in_ranch(gateway, account_id, move |_cells, bag| async move {
        let reply = can_mall_list(&owned, slot).await?;
        let goods = can_offers(&reply.goods_list, now)
            .into_iter()
            .map(|offer| {
                let balance = (!offer.free).then(|| unlocked_count(&bag, offer.price_id));
                can_json(&offer, balance)
            })
            .collect::<Vec<_>>();
        Ok(json!({ "slot": slot, "goods": goods }))
    })
    .await
}

/// 按当前罐头商城标价购买或领取一份商品。
pub async fn buy_can(
    gateway: &Arc<Gateway>,
    account_id: &str,
    slot: i32,
    goods_id: i64,
    count: i64,
) -> Result<()> {
    require_can_slot(slot)?;
    if count <= 0 {
        return Err(Error::Business("数量无效".to_owned()));
    }
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    let now = crate::utils::time::get_server_time_secs();
    in_ranch(gateway, account_id, move |_cells, bag| async move {
        let reply = can_mall_list(&owned, slot).await?;
        let Some(offer) =
            can_offers(&reply.goods_list, now).into_iter().find(|offer| offer.id == goods_id)
        else {
            return Err(Error::Business("罐头商品不可购买或超过限购数量".to_owned()));
        };
        reject_can_purchase(&offer, count)?;
        let cost = if offer.free {
            0
        } else {
            offer
                .unit_price
                .checked_mul(count)
                .ok_or_else(|| Error::Business("数量超出范围".to_owned()))?
        };
        if !offer.free && unlocked_count(&bag, offer.price_id) < cost {
            return Err(Error::Business(format!("{}不足", item_name(offer.price_id))));
        }
        let price_id = offer.price_id;
        let free = offer.free;
        let label = offer.name.clone();
        let before = item_count(&bag, price_id);
        let expected = expected_reward_signature(&offer.rewards, count)?;
        let costs = if free { Vec::new() } else { vec![(price_id, cost)] };
        let plan = money_plan(format!("ranch:buyCan:{goods_id}"), label, &bag, &costs);
        crate::services::ranch_ops::spend(&account, plan, || async move {
            let request = PurchaseRequest { goods_id, count };
            let bytes = owned.request(MALL_SERVICE, "Purchase", &request.encode_to_vec()).await?;
            let bought = PurchaseResponse::decode(bytes.as_slice())?;
            if !bought.success
                || bought.goods_id != goods_id
                || id_count_signature(bought.reward_items.iter().map(|item| (item.id, item.count)))
                    != expected
            {
                return Err(Error::Protocol("商品或奖励不匹配".to_owned()));
            }
            if free {
                return Ok(());
            }
            let after = match ranch_bag(&owned).await {
                Ok(bag) => bag,
                Err(error) => {
                    tracing::error!(%error, "购买罐头后未能重新读取背包");
                    return Err(Error::Protocol("操作后的库存未能读取".to_owned()));
                }
            };
            if before.checked_sub(item_count(&after, price_id)) != Some(cost) {
                return Err(Error::Protocol("库存扣减与费用不匹配".to_owned()));
            }
            Ok(())
        })
        .await
    })
    .await
}

/// 领取罐头商城常驻栏的每日福利。已经领过或今天没有这份礼包时返回 `false`。
pub async fn claim_daily_benefit(gateway: &Arc<Gateway>, account_id: &str) -> Result<bool> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    in_ranch(gateway, account_id, move |_cells, _bag| async move {
        let reply = can_mall_list(&owned, CAN_SLOT_REGULAR).await?;
        let benefit = daily_benefit(&reply.goods_list);
        if benefit.claimed || !benefit.available {
            return Ok(false);
        }
        let expected = id_count_signature(benefit.rewards);
        let plan = crate::services::ranch_ops::SpendPlan {
            target: format!("ranch:buyCan:{BENEFIT_GOODS_ID}"),
            label: "领取每日福利".to_owned(),
            costs: Vec::new(),
            balances: Vec::new(),
        };
        crate::services::ranch_ops::spend(&account, plan, || async move {
            let request = PurchaseRequest { goods_id: BENEFIT_GOODS_ID, count: 1 };
            let bytes = owned.request(MALL_SERVICE, "Purchase", &request.encode_to_vec()).await?;
            let bought = PurchaseResponse::decode(bytes.as_slice())?;
            confirm_daily_benefit(&bought, &expected)?;
            Ok(())
        })
        .await?;
        Ok(true)
    })
    .await
}

struct DailyBenefit {
    claimed: bool,
    available: bool,
    rewards: Vec<(i64, i64)>,
}

fn daily_benefit(goods: &[MallGoods]) -> DailyBenefit {
    let Some(good) = goods.iter().find(|good| good.goods_id == BENEFIT_GOODS_ID) else {
        return DailyBenefit { claimed: false, available: false, rewards: Vec::new() };
    };
    let limit = good.purchase_limit.as_ref();
    let claimed = limit.is_some_and(benefit_claimed);
    let share_only = good.share.as_ref().is_some_and(|share| share.share_only);
    let price_free = good.price.as_ref().is_some_and(|price| price.count == 0);
    let daily_limit = limit.is_some_and(|limit| {
        limit.limit_type == BENEFIT_LIMIT_TYPE && limit.limit_count == BENEFIT_LIMIT_COUNT
    });
    DailyBenefit {
        claimed,
        available: good.is_free
            && price_free
            && daily_limit
            && good.is_available
            && !good.is_owned
            && !good.ad_only
            && !share_only
            && !good.reward_items.is_empty()
            && !claimed,
        rewards: good.reward_items.iter().map(|item| (item.id, item.count)).collect(),
    }
}

fn benefit_claimed(limit: &PurchaseLimit) -> bool {
    limit.limit_type == BENEFIT_LIMIT_TYPE
        && limit.limit_count == BENEFIT_LIMIT_COUNT
        && limit.bought_count >= BENEFIT_LIMIT_COUNT
}

fn confirm_daily_benefit(reply: &PurchaseResponse, expected: &str) -> Result<()> {
    let confirmed = reply.success
        && reply.goods_id == BENEFIT_GOODS_ID
        && id_count_signature(reply.reward_items.iter().map(|item| (item.id, item.count)))
            == expected
        && reply.purchase_limit.as_ref().is_some_and(|limit| {
            limit.limit_type == BENEFIT_LIMIT_TYPE && limit.bought_count == BENEFIT_LIMIT_COUNT
        });
    if confirmed {
        Ok(())
    } else {
        Err(Error::Protocol("每日福利领取回包未能确认，请刷新核对，勿重复提交".to_owned()))
    }
}

/// 牧场成长任务和每日任务。牧场未解锁时不切换场景。
pub async fn tasks(gateway: &Arc<Gateway>, account_id: &str) -> Result<Value> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Ok(json!({ "tasks": [] }));
    }
    let owned = Arc::clone(gateway);
    in_ranch(gateway, account_id, move |_cells, _bag| async move {
        let info = ranch_task_info(&owned).await?;
        let tasks = ranch_task_rows(&info).into_iter().map(task_json).collect::<Vec<_>>();
        Ok(json!({ "tasks": tasks }))
    })
    .await
}

/// 领取一条已完成的牧场任务。可分享翻倍的任务先上报分享。
pub async fn claim_task(gateway: &Arc<Gateway>, account_id: &str, task_id: i64) -> Result<()> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    in_ranch(gateway, account_id, move |_cells, _bag| async move {
        let info = ranch_task_info(&owned).await?;
        let Some(task) = ranch_task_rows(&info).into_iter().find(|task| task.id == task_id) else {
            return Err(Error::Business("任务尚未完成或已领取".to_owned()));
        };
        if !task.can_claim {
            return Err(Error::Business("任务尚未完成或已领取".to_owned()));
        }
        let plan = guard_plan(format!("ranch:claimTasks:{task_id}"), "领取牧场任务");
        crate::services::ranch_ops::spend(&account, plan, || async move {
            let multiple = task.share_multiple.max(1);
            if multiple > 1 {
                let request = ReportShareRequest {
                    share_id: RANCH_SHARE_ID,
                    params: vec![task_id],
                    doc_id: RANCH_SHARE_DOC,
                    ..ReportShareRequest::default()
                };
                let bytes =
                    owned.request(SHARE_SERVICE, "ReportShare", &request.encode_to_vec()).await?;
                ReportShareReply::decode(bytes.as_slice())?;
            }
            let request = ClaimTaskRewardRequest { id: task_id, do_shared: multiple > 1 };
            let bytes =
                owned.request(TASK_SERVICE, "ClaimTaskReward", &request.encode_to_vec()).await?;
            let reply = ClaimTaskRewardReply::decode(bytes.as_slice())?;
            let updated = reply.task_info.unwrap_or_default();
            if ranch_task_rows(&updated).iter().any(|row| row.id == task_id && !row.claimed) {
                return Err(Error::Protocol("任务仍显示未领取".to_owned()));
            }
            let expected = expected_reward_signature(&task.rewards, multiple)?;
            if id_count_signature(reply.items.iter().map(|item| (item.id, item.count))) != expected
            {
                return Err(Error::Protocol("任务奖励不匹配".to_owned()));
            }
            Ok(())
        })
        .await
    })
    .await
}

/// 领取当前全部可领的牧场任务。没有可领任务时返回 `false`。
pub async fn claim_ready_tasks(gateway: &Arc<Gateway>, account_id: &str) -> Result<bool> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    in_ranch(gateway, account_id, move |_cells, _bag| async move {
        let info = ranch_task_info(&owned).await?;
        let ready =
            ranch_task_rows(&info).into_iter().filter(|task| task.can_claim).collect::<Vec<_>>();
        if ready.is_empty() {
            return Ok(false);
        }
        let expected = reward_signature_for(&ready)?;
        let plan = guard_plan("ranch:claimTasks:all".to_owned(), "领取牧场任务");
        crate::services::ranch_ops::spend(&account, plan, || async move {
            if claim_together(&ready) {
                let request = BatchClaimTaskRewardRequest {
                    ids: ready.iter().map(|task| task.id).collect(),
                    do_shared: false,
                };
                let bytes = owned
                    .request(TASK_SERVICE, "BatchClaimTaskReward", &request.encode_to_vec())
                    .await?;
                let reply = BatchClaimTaskRewardReply::decode(bytes.as_slice())?;
                confirm_claimed_tasks(
                    &reply.task_info.unwrap_or_default(),
                    &ready,
                    &reply.items,
                    &expected,
                )?;
            } else {
                let mut items = Vec::new();
                for task in &ready {
                    let reply = claim_one_task(&owned, task).await?;
                    items.extend(reply.items);
                }
                if id_count_signature(items.iter().map(|item| (item.id, item.count))) != expected {
                    return Err(Error::Protocol("任务奖励不匹配".to_owned()));
                }
            }
            Ok(true)
        })
        .await
    })
    .await
}

fn claim_together(tasks: &[RanchTaskRow]) -> bool {
    tasks.len() > 1 && tasks.iter().all(|task| task.share_multiple.max(1) == 1)
}

fn reward_signature_for(tasks: &[RanchTaskRow]) -> Result<String> {
    let mut pairs = Vec::new();
    for task in tasks {
        let multiple = task.share_multiple.max(1);
        for (id, count) in &task.rewards {
            let count = count
                .checked_mul(multiple)
                .ok_or_else(|| Error::Business("数量超出范围".to_owned()))?;
            pairs.push((*id, count));
        }
    }
    Ok(id_count_signature(pairs))
}

async fn claim_one_task(gateway: &Gateway, task: &RanchTaskRow) -> Result<ClaimTaskRewardReply> {
    let multiple = task.share_multiple.max(1);
    if multiple > 1 {
        let request = ReportShareRequest {
            share_id: RANCH_SHARE_ID,
            params: vec![task.id],
            doc_id: RANCH_SHARE_DOC,
            ..ReportShareRequest::default()
        };
        let bytes = gateway.request(SHARE_SERVICE, "ReportShare", &request.encode_to_vec()).await?;
        ReportShareReply::decode(bytes.as_slice())?;
    }
    let request = ClaimTaskRewardRequest { id: task.id, do_shared: multiple > 1 };
    let bytes = gateway.request(TASK_SERVICE, "ClaimTaskReward", &request.encode_to_vec()).await?;
    let reply = ClaimTaskRewardReply::decode(bytes.as_slice())?;
    let empty = TaskInfo::default();
    let updated = reply.task_info.as_ref().unwrap_or(&empty);
    if ranch_task_rows(updated).iter().any(|row| row.id == task.id && !row.claimed) {
        return Err(Error::Protocol("任务仍显示未领取".to_owned()));
    }
    Ok(reply)
}

fn confirm_claimed_tasks(
    info: &TaskInfo,
    ready: &[RanchTaskRow],
    items: &[CoreItem],
    expected: &str,
) -> Result<()> {
    let rows = ranch_task_rows(info);
    if ready.iter().any(|task| rows.iter().any(|row| row.id == task.id && !row.claimed)) {
        return Err(Error::Protocol("任务仍显示未领取".to_owned()));
    }
    if id_count_signature(items.iter().map(|item| (item.id, item.count))) != expected {
        return Err(Error::Protocol("任务奖励不匹配".to_owned()));
    }
    Ok(())
}

/// 牧场图鉴和巨大图鉴。牧场未解锁时不切换场景。
pub async fn book(gateway: &Arc<Gateway>, account_id: &str) -> Result<Value> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Ok(empty_book());
    }
    let owned = Arc::clone(gateway);
    in_ranch(gateway, account_id, move |_cells, _bag| async move {
        let list = illustrated_list(&owned).await?;
        ensure_book_type(list.r#type)?;
        let weight = weight_book(&owned).await?;
        Ok(book_json(&list, &weight))
    })
    .await
}

/// 领取当前可领的普通牧场图鉴奖励。
pub async fn claim_book(gateway: &Arc<Gateway>, account_id: &str) -> Result<()> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    in_ranch(gateway, account_id, move |_cells, _bag| async move {
        let list = illustrated_list(&owned).await?;
        ensure_book_type(list.r#type)?;
        if !list.has_claimable_reward {
            return Err(Error::Business("图鉴奖励尚未可领".to_owned()));
        }
        let expected =
            id_count_signature(list.total_rewards.iter().map(|item| (item.id, item.count)));
        let plan = guard_plan("ranch:claimIllustrated:normal".to_owned(), "领取牧场图鉴奖励");
        crate::services::ranch_ops::spend(&account, plan, || async move {
            let request = ClaimAllRewardsV2Request { r#type: BOOK_TYPE };
            let bytes = owned
                .request(ILLUSTRATED_SERVICE, "ClaimAllRewardsV2", &request.encode_to_vec())
                .await?;
            let reply = ClaimAllRewardsV2Reply::decode(bytes.as_slice())?;
            if reply.claimed_level_ids.is_empty()
                || id_count_signature(reply.total_rewards.iter().map(|item| (item.id, item.count)))
                    != expected
            {
                return Err(Error::Protocol("图鉴奖励回包未能确认，请刷新核对".to_owned()));
            }
            Ok(())
        })
        .await
    })
    .await
}

/// 领取当前可领的全部巨大图鉴档位。
pub async fn claim_giant_book(gateway: &Arc<Gateway>, account_id: &str) -> Result<()> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    in_ranch(gateway, account_id, move |_cells, _bag| async move {
        let weight = weight_book(&owned).await?;
        let claims = claimable_weight_levels(&weight)
            .into_iter()
            .map(|level| {
                (
                    level.id,
                    id_count_signature(level.rewards.iter().map(|item| (item.id, item.count))),
                )
            })
            .collect::<Vec<_>>();
        if claims.is_empty() {
            return Err(Error::Business("图鉴奖励尚未可领".to_owned()));
        }
        let plan = guard_plan("ranch:claimIllustrated:giant".to_owned(), "领取牧场图鉴奖励");
        crate::services::ranch_ops::spend(&account, plan, || async move {
            for (level_id, expected) in claims {
                let request = ClaimRanchWeightScoreRewardV2Request { id: level_id };
                let bytes = owned
                    .request(
                        ILLUSTRATED_SERVICE,
                        "ClaimRanchWeightScoreRewardV2",
                        &request.encode_to_vec(),
                    )
                    .await?;
                let reply = ClaimRanchWeightScoreRewardV2Reply::decode(bytes.as_slice())?;
                if !reply.claimed_ids.contains(&level_id)
                    || id_count_signature(reply.rewards.iter().map(|item| (item.id, item.count)))
                        != expected
                {
                    return Err(Error::Protocol("巨大图鉴奖励回包未能确认，请刷新核对".to_owned()));
                }
            }
            Ok(())
        })
        .await
    })
    .await
}

/// 把背包里的罐头加入对应的罐头桶。
pub async fn use_can(gateway: &Arc<Gateway>, account_id: &str, uid: i64, count: i64) -> Result<()> {
    if count <= 0 {
        return Err(Error::Business("数量无效".to_owned()));
    }
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    in_ranch(gateway, account_id, move |_cells, bag| async move {
        let can = can_plan(&bag, uid, count)?;
        let before_item = item_count(&bag, can.item_id);
        let before_bucket = item_count(&bag, can.bucket_id);
        let reserved = money_plan(
            format!("ranch:useCan:{uid}"),
            "添加罐头".to_owned(),
            &bag,
            &[(can.item_id, count)],
        );
        crate::services::ranch_ops::spend(&account, reserved, || async move {
            let request = UseRequest {
                item: Some(RanchItem { id: can.item_id, uid, count, ..RanchItem::default() }),
            };
            let bytes = owned.request(ITEM_SERVICE, "Use", &request.encode_to_vec()).await?;
            UseReply::decode(bytes.as_slice())?;
            let after = match ranch_bag(&owned).await {
                Ok(bag) => bag,
                Err(error) => {
                    tracing::error!(%error, "添加罐头后未能重新读取背包");
                    return Err(Error::Protocol("操作后的库存未能读取".to_owned()));
                }
            };
            if before_item.checked_sub(item_count(&after, can.item_id)) != Some(count) {
                return Err(Error::Protocol("库存扣减与费用不匹配".to_owned()));
            }
            if item_count(&after, can.bucket_id).checked_sub(before_bucket) != Some(can.seconds) {
                return Err(Error::Protocol("罐头桶增加时长不匹配".to_owned()));
            }
            Ok(())
        })
        .await
    })
    .await
}

/// 出售背包里的一批可售产物。
pub async fn sell_stack(
    gateway: &Arc<Gateway>,
    account_id: &str,
    uid: i64,
    count: i64,
) -> Result<()> {
    sell(gateway, account_id, SellChoice::Stack { uid, count }).await
}

/// 出售背包里当前可售的全部产物。没有可售产物时不发送请求。
pub async fn sell_all(gateway: &Arc<Gateway>, account_id: &str) -> Result<()> {
    sell(gateway, account_id, SellChoice::All).await
}

#[derive(Clone, Copy)]
enum SellChoice {
    Stack { uid: i64, count: i64 },
    All,
}

async fn sell(gateway: &Arc<Gateway>, account_id: &str, choice: SellChoice) -> Result<()> {
    if let SellChoice::Stack { count, .. } = choice {
        if count <= 0 {
            return Err(Error::Business("数量无效".to_owned()));
        }
    }
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    in_ranch(gateway, account_id, move |_cells, bag| async move {
        let Some(sale) = sale_plan(&bag, choice)? else {
            return Ok(());
        };
        let plan =
            money_plan(sell_target(choice), "出售牧场产物".to_owned(), &bag, &sale_costs(&sale));
        let sold = sale.sold.clone();
        let reward = sale.reward.clone();
        let request = sale.request;
        crate::services::ranch_ops::spend(&account, plan, || async move {
            let bytes = owned.request(ITEM_SERVICE, "Sell", &request.encode_to_vec()).await?;
            let reply = SellReply::decode(bytes.as_slice())?;
            if sold_signature(&reply.sell_items) != sold
                || id_count_signature(reply.get_items.iter().map(|item| (item.id, item.count)))
                    != reward
            {
                return Err(Error::Protocol("出售物品或收益不匹配".to_owned()));
            }
            Ok(())
        })
        .await
    })
    .await
}

/// 收获一只可收获动物，或在不指定动物时收获全部。
pub async fn harvest(
    gateway: &Arc<Gateway>,
    account_id: &str,
    ranch_id: Option<i64>,
    animal_id: Option<i32>,
) -> Result<HarvestRanchReply> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    in_ranch(gateway, account_id, move |cells, _bag| async move {
        let request = harvest_request(&cells, ranch_id, animal_id)?;
        let bytes = owned.request(ANIMAL_SERVICE, "HarvestRanch", &request.encode_to_vec()).await?;
        Ok(HarvestRanchReply::decode(bytes.as_slice())?)
    })
    .await
}

/// 开始升级一座已就绪、等级足够且尚未在升级的牧场设施。
pub async fn start_upgrade(
    gateway: &Arc<Gateway>,
    account_id: &str,
    facility_id: i64,
) -> Result<()> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    in_ranch(gateway, account_id, move |cells, bag| async move {
        let target = start_target(&cells, facility_id, ranch_level(item_count(&bag, RANCH_EXP)))?;
        let request = GetFacilityDetailRequest { instance_id: facility_id, host_gid: 0 };
        let bytes =
            owned.request(HOMESTEAD_SERVICE, "GetFacilityDetail", &request.encode_to_vec()).await?;
        let detail = GetFacilityDetailReply::decode(bytes.as_slice())?;
        let info = detail.info.ok_or_else(|| Error::Business("建筑等级已变化".to_owned()))?;
        if info.facility_id != target.facility_id || info.level != target.level {
            return Err(Error::Business("建筑等级已变化".to_owned()));
        }
        if info.upgrade_cost.is_empty() {
            return Err(Error::Business("升级费用尚未确认".to_owned()));
        }
        if info.upgrade_cost.iter().any(|cost| unlocked_count(&bag, cost.id) < cost.count) {
            return Err(Error::Business("余额不足".to_owned()));
        }
        let costs = info.upgrade_cost.iter().map(|cost| (cost.id, cost.count)).collect::<Vec<_>>();
        let plan = money_plan(
            format!("ranch:upgrade:{facility_id}"),
            "升级牧场设施".to_owned(),
            &bag,
            &costs,
        );
        crate::services::ranch_ops::spend(&account, plan, || async move {
            let request = UpgradeBuildingRequest { inst_id: facility_id };
            let bytes = owned
                .request(HOMESTEAD_SERVICE, "UpgradeBuilding", &request.encode_to_vec())
                .await?;
            let reply = UpgradeBuildingReply::decode(bytes.as_slice())?;
            let upgrade =
                reply.upgrade.ok_or_else(|| Error::Protocol("建筑升级目标不匹配".to_owned()))?;
            if upgrade.inst_id != facility_id
                || upgrade.target_level != target.next_level
                || upgrade.target_facility_id != target.next_facility_id
            {
                return Err(Error::Protocol("建筑升级目标不匹配".to_owned()));
            }
            Ok(())
        })
        .await
    })
    .await
}

/// 完成已经到期或已标记可完成的设施升级。空闲设施不发送请求。
pub async fn finish_upgrade(
    gateway: &Arc<Gateway>,
    account_id: &str,
    facility_id: i64,
) -> Result<()> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let now_ms = crate::utils::time::get_server_time_secs().saturating_mul(1000);
    in_ranch(gateway, account_id, move |cells, _bag| async move {
        let Some(target) = finish_target(&cells, facility_id, now_ms)? else {
            return Ok(());
        };
        let request = CompleteBuildingUpgradeRequest { inst_id: facility_id };
        let bytes = owned
            .request(HOMESTEAD_SERVICE, "CompleteBuildingUpgrade", &request.encode_to_vec())
            .await?;
        let reply = CompleteBuildingUpgradeReply::decode(bytes.as_slice())?;
        let facility =
            reply.facility.ok_or_else(|| Error::Protocol("建筑等级不匹配".to_owned()))?;
        if facility.instance_id != facility_id
            || facility.level != target.next_level
            || facility.facility_id != target.next_facility_id
        {
            return Err(Error::Protocol("建筑等级不匹配".to_owned()));
        }
        Ok(())
    })
    .await
}

/// 放生一只允许放生的动物。
pub async fn release(
    gateway: &Arc<Gateway>,
    account_id: &str,
    ranch_id: i64,
    animal_inst_id: i32,
) -> Result<()> {
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    in_ranch(gateway, account_id, move |cells, _bag| async move {
        let target = release_target(&cells, ranch_id, animal_inst_id)?;
        let label = cells
            .facilities
            .iter()
            .filter_map(|facility| facility.ranch.as_ref())
            .flat_map(|ranch| ranch.animals.iter())
            .find(|animal| {
                animal.ranch_inst_id == ranch_id && animal.animal_inst_id == animal_inst_id
            })
            .map(|animal| {
                release_label(
                    animal.animal_id,
                    animal.mutation.as_ref().map(|mutation| mutation.mutation_type).unwrap_or(0),
                )
            })
            .unwrap_or_else(|| "放生动物".to_owned());
        let times = GetRanchOpTimesRequest { op_ids: Vec::new() };
        let bytes = owned.request(RANCH_SERVICE, "GetRanchOpTimes", &times.encode_to_vec()).await?;
        let times = GetRanchOpTimesReply::decode(bytes.as_slice())?;
        if release_remaining(&times.ops)? == 0 {
            return Err(Error::Business("今日放生次数已用完".to_owned()));
        }
        let plan = guard_plan(format!("ranch:release:{ranch_id}:{animal_inst_id}"), label);
        crate::services::ranch_ops::spend(&account, plan, || async move {
            let request = DonateAnimalRequest { targets: vec![target] };
            let bytes =
                owned.request(ANIMAL_SERVICE, "DonateAnimal", &request.encode_to_vec()).await?;
            let reply = DonateAnimalReply::decode(bytes.as_slice())?;
            if release_matches(&reply, ranch_id, animal_inst_id) {
                Ok(())
            } else {
                Err(Error::Protocol("放生目标或栏位变化不匹配".to_owned()))
            }
        })
        .await
    })
    .await
}

pub(crate) async fn in_ranch<T, F, Fut>(
    gateway: &Arc<Gateway>,
    account_id: &str,
    work: F,
) -> Result<T>
where
    F: FnOnce(AllCellsReply, GetBagReply) -> Fut + Send + 'static,
    Fut: Future<Output = Result<T>> + Send,
    T: Send + 'static,
{
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let gateway = Arc::clone(gateway);
    crate::infra::automation_lock::run_exclusive_automation_task(
        account_id,
        "ranch.scene",
        async move {
            let _ = sender.send(visit(&gateway, work).await);
        },
    )
    .await;
    receiver.await.map_err(|_| Error::Internal("牧场场景任务被取消".to_owned()))?
}

async fn visit<T, F, Fut>(gateway: &Gateway, work: F) -> Result<T>
where
    F: FnOnce(AllCellsReply, GetBagReply) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let previous = gateway.current_scene();
    let cells = if previous == RANCH_SCENE {
        all_cells(gateway).await?
    } else {
        cells_from_scene(gateway).await?
    };
    let bag = ranch_bag(gateway).await?;
    let result = work(cells, bag).await;
    if previous != RANCH_SCENE {
        if let Err(error) = gateway.login_game_scene(previous).await {
            tracing::error!(%error, previous, "离开牧场时未能回到原场景");
            if result.is_ok() {
                return Err(error.into());
            }
        }
    }
    result
}

async fn cells_from_scene(gateway: &Gateway) -> Result<AllCellsReply> {
    let scene = gateway.login_game_scene(RANCH_SCENE).await?;
    let data = SceneData::decode(scene.big_farm_scene_ext_data)?;
    data.homestead_cells.ok_or_else(|| Error::Protocol("牧场场景缺少家园格子".to_owned()))
}

async fn all_cells(gateway: &Gateway) -> Result<AllCellsReply> {
    let bytes =
        gateway.request(HOMESTEAD_SERVICE, "AllCells", &AllCellsRequest {}.encode_to_vec()).await?;
    Ok(AllCellsReply::decode(bytes.as_slice())?)
}

async fn ranch_bag(gateway: &Gateway) -> Result<GetBagReply> {
    let request = GetBagRequest { global_scene_id: RANCH_SCENE, tab_id: 0 };
    let bytes = gateway.request(ITEM_SERVICE, "GetBag", &request.encode_to_vec()).await?;
    let bag = GetBagReply::decode(bytes.as_slice())?;
    if bag.global_scene_id != RANCH_SCENE {
        return Err(Error::Protocol("牧场背包场景不匹配".to_owned()));
    }
    Ok(bag)
}

fn produce_request(
    cells: &AllCellsReply,
    ranch_id: Option<i64>,
    animal_id: Option<i32>,
) -> Result<Option<TriggerProduceRequest>> {
    let animals = selected_animals(cells, ranch_id, animal_id, |animal| animal.can_deliver)?;
    if animals.is_empty() {
        return Ok(None);
    }
    let targets = if ranch_id.is_none() {
        Vec::new()
    } else {
        animals.iter().copied().map(animal_ref).collect()
    };
    Ok(Some(TriggerProduceRequest { targets, ..TriggerProduceRequest::default() }))
}

fn feed_request(
    cells: &AllCellsReply,
    bag: &GetBagReply,
    ranch_id: Option<i64>,
    animal_id: Option<i32>,
    bucket_type: i32,
    now_secs: i64,
) -> Result<Option<FeedAnimalRequest>> {
    let bucket_id = match bucket_type {
        BUCKET_ORGANIC => ORGANIC_BUCKET,
        BUCKET_NORMAL => NORMAL_BUCKET,
        _ => return Err(Error::Business("请选择有机或普通罐头".to_owned())),
    };
    let animals =
        selected_animals(cells, ranch_id, animal_id, |animal| can_use_bucket(animal, bucket_type))?;
    if animals.is_empty() {
        return Ok(None);
    }
    for animal in &animals {
        let timing = animal_timing(animal.animal_id)?;
        let _ = feed_upper_bound(animal, timing, now_secs)?;
    }
    if item_count(bag, bucket_id) <= 0 {
        return Err(Error::Business("罐头桶为空，请先添加罐头".to_owned()));
    }
    Ok(Some(FeedAnimalRequest {
        targets: animals.iter().copied().map(animal_ref).collect(),
        bucket_type,
    }))
}

fn selected_animals<'a>(
    cells: &'a AllCellsReply,
    ranch_id: Option<i64>,
    animal_id: Option<i32>,
    eligible: impl Fn(&AnimalDetail) -> bool,
) -> Result<Vec<&'a AnimalDetail>> {
    let mut animals = cells
        .facilities
        .iter()
        .filter_map(|facility| facility.ranch.as_ref())
        .flat_map(|ranch| ranch.animals.iter().filter(|animal| eligible(animal)));
    let Some(ranch_id) = ranch_id else {
        return Ok(animals.collect());
    };
    let Some(animal_id) = animal_id else {
        return Err(Error::Business(STATE_CHANGED.to_owned()));
    };
    animals
        .find(|animal| animal.ranch_inst_id == ranch_id && animal.animal_inst_id == animal_id)
        .map(|animal| vec![animal])
        .ok_or_else(|| Error::Business(STATE_CHANGED.to_owned()))
}

fn can_use_bucket(animal: &AnimalDetail, bucket_type: i32) -> bool {
    animal.can_feed && (bucket_type == BUCKET_ORGANIC || !animal.limited_can_used)
}

fn animal_ref(animal: &AnimalDetail) -> AnimalRef {
    AnimalRef { ranch_inst_id: animal.ranch_inst_id, animal_inst_id: animal.animal_inst_id }
}

fn feed_upper_bound(animal: &AnimalDetail, timing: &AnimalTiming, now_secs: i64) -> Result<i64> {
    let (duration, deadline_ms) = match animal.stage {
        1 => (timing.infant_sec, animal.infant_ready_at_ms),
        2 => (timing.grow_sec, animal.grow_ready_at_ms),
        3 => (timing.product_round_sec, animal.produce_ready_at_ms),
        4 => (timing.mature_sec, animal.mature_ready_at_ms),
        _ => (0, 0),
    };
    let remaining = (div_ceil(deadline_ms, 1000) - now_secs).max(0);
    let upper = duration.max(div_ceil(remaining, HOUR_SECS) * HOUR_SECS);
    if upper == 0 {
        Err(Error::Business("当前阶段的罐头消耗无法确认".to_owned()))
    } else {
        Ok(upper)
    }
}

fn div_ceil(value: i64, divisor: i64) -> i64 {
    if value <= 0 {
        0
    } else {
        (value + divisor - 1) / divisor
    }
}

#[derive(Deserialize)]
struct AnimalTiming {
    id: i64,
    name: String,
    infant_sec: i64,
    grow_sec: i64,
    mature_sec: i64,
    product_round_sec: i64,
    cub_item_id: i64,
    husbandry_type: String,
    #[serde(default)]
    enable_flag: bool,
}

fn animal_timing(id: i64) -> Result<&'static AnimalTiming> {
    animal_configs().get(&id).ok_or_else(|| Error::Business("动物阶段配置尚未同步".to_owned()))
}

fn animal_configs() -> &'static HashMap<i64, AnimalTiming> {
    static CONFIGS: OnceLock<HashMap<i64, AnimalTiming>> = OnceLock::new();
    CONFIGS.get_or_init(|| {
        let rows: Vec<AnimalTiming> =
            serde_json::from_str(include_str!("../../assets/ranch/MCAnimalConfig.json"))
                .expect("牧场动物配置无法解析");
        rows.into_iter().map(|row| (row.id, row)).collect()
    })
}

pub(crate) fn animal_name(id: i64) -> String {
    animal_configs().get(&id).map(|row| row.name.clone()).unwrap_or_else(|| format!("动物{id}"))
}

#[derive(Clone, Serialize)]
pub struct RanchAnimalChoice {
    pub id: i64,
    pub name: String,
    pub nest: bool,
}

/// 已启用、并且能放进窝或棚的动物。供设置页选择指定幼崽。
pub fn animal_choices() -> Vec<RanchAnimalChoice> {
    let mut choices = animal_configs()
        .values()
        .filter_map(|animal| {
            let nest = match husbandry_kind(&animal.husbandry_type) {
                Some(NEST) => true,
                Some(SHED) => false,
                _ => return None,
            };
            animal.enable_flag.then_some(RanchAnimalChoice {
                id: animal.id,
                name: animal.name.clone(),
                nest,
            })
        })
        .collect::<Vec<_>>();
    choices.sort_by_key(|choice| choice.id);
    choices
}

pub(crate) fn animal_fits(id: i64, nest: bool) -> bool {
    let kind = if nest { NEST } else { SHED };
    animal_configs().get(&id).is_some_and(|animal| {
        animal.enable_flag && husbandry_kind(&animal.husbandry_type) == Some(kind)
    })
}

fn animal_enabled(id: i64) -> bool {
    animal_configs().get(&id).is_some_and(|animal| animal.enable_flag)
}

fn reward_name(item_id: i64) -> String {
    animal_configs()
        .values()
        .find(|row| row.cub_item_id == item_id)
        .map(|row| row.name.clone())
        .unwrap_or_else(|| format!("物品{item_id}"))
}

#[derive(Clone, PartialEq, Eq)]
struct ShopOffer {
    id: i64,
    name: String,
    item_id: i64,
    item_count: i64,
    price_id: i64,
    unit_price: i64,
    max_count: i64,
    remaining: Option<i64>,
    available: bool,
    animal_id: Option<i64>,
    husbandry_kind: Option<i32>,
    required_level: i64,
}

fn require_shop(shop_id: i64) -> Result<()> {
    if shop_id == SHOP_CUBS || shop_id == SHOP_FEED {
        Ok(())
    } else {
        Err(Error::Business("不支持的牧场商店".to_owned()))
    }
}

async fn shop_info(gateway: &Gateway, shop_id: i64) -> Result<ShopInfoReply> {
    let request = ShopInfoRequest { shop_id };
    let bytes = gateway.request(SHOP_SERVICE, "ShopInfo", &request.encode_to_vec()).await?;
    Ok(ShopInfoReply::decode(bytes.as_slice())?)
}

fn shop_offers(goods: &[GoodsInfo], shop_id: i64, now: i64) -> Vec<ShopOffer> {
    goods.iter().map(|goods| shop_offer(goods, shop_id, now)).collect()
}

fn shop_offer(goods: &GoodsInfo, shop_id: i64, now: i64) -> ShopOffer {
    let config = shop_goods()
        .get(&goods.id)
        .filter(|config| config.shop_id == shop_id && config.item_id == goods.item_id);
    let discounted = goods.discount_start_time > 0
        && goods.discount_start_time <= now
        && now < goods.discount_end_time;
    let unit_price = if discounted { goods.discount_price } else { goods.price };
    let max_count =
        config.map(|config| config.one_buy_count).filter(|count| *count > 0).unwrap_or(1);
    let remaining = (goods.limit_count > 0).then_some(goods.limit_count - goods.bought_num);
    let available = config.is_some()
        && goods.unlocked
        && !goods.owned
        && remaining.is_none_or(|remaining| remaining > 0);
    let cub = cub_for_item(goods.item_id);
    ShopOffer {
        id: goods.id,
        name: reward_name(goods.item_id),
        item_id: goods.item_id,
        item_count: goods.item_count,
        price_id: config.map(|config| config.price_id).unwrap_or(0),
        unit_price,
        max_count,
        remaining,
        available,
        animal_id: cub.map(|animal| animal.id),
        husbandry_kind: cub.and_then(|animal| husbandry_kind(&animal.husbandry_type)),
        required_level: config.map(|config| required_level(&config.conds)).unwrap_or(0),
    }
}

fn required_level(conds: &str) -> i64 {
    conds.rsplit_once(':').and_then(|(_, level)| level.parse().ok()).unwrap_or(0)
}

fn cub_for_item(item_id: i64) -> Option<&'static AnimalTiming> {
    animal_configs().values().find(|animal| animal.cub_item_id == item_id)
}

fn husbandry_kind(label: &str) -> Option<i32> {
    match label {
        "窝" => Some(NEST),
        "棚" => Some(SHED),
        _ => None,
    }
}

fn placement_target(
    offer: &ShopOffer,
    cells: &AllCellsReply,
    count: i64,
    auto_place: bool,
) -> Result<Option<i64>> {
    if !auto_place {
        return Ok(None);
    }
    let (Some(animal_id), Some(kind)) = (offer.animal_id, offer.husbandry_kind) else {
        return Err(Error::Business("只有幼崽可以自动放养".to_owned()));
    };
    if open_stalls(cells, kind) < count {
        return Err(Error::Business("窝或棚的空位不足，请减少数量或先放入背包".to_owned()));
    }
    Ok(Some(animal_id))
}

fn open_stalls(cells: &AllCellsReply, kind: i32) -> i64 {
    cells
        .facilities
        .iter()
        .filter_map(|facility| facility.ranch.as_ref())
        .filter(|ranch| ranch.husbandry_type == kind)
        .map(|ranch| i64::from(ranch.capacity) - i64::from(ranch.animal_count))
        .sum()
}

async fn confirm_placement(
    gateway: &Gateway,
    before: &AllCellsReply,
    animal_id: i64,
    count: i64,
) -> Result<()> {
    let after = match all_cells(gateway).await {
        Ok(cells) => cells,
        Err(error) => {
            tracing::error!(%error, "自动放养后未能重新读取栏位");
            return Err(Error::Protocol("自动放养后的栏位状态未能读取".to_owned()));
        }
    };
    if placed_animals_match(before, &after, animal_id, count) {
        Ok(())
    } else {
        Err(Error::Protocol("自动放养的动物或数量不匹配".to_owned()))
    }
}

fn placed_animals_match(
    before: &AllCellsReply,
    after: &AllCellsReply,
    animal_id: i64,
    count: i64,
) -> bool {
    let previous = stall_animals(before);
    let placed = stall_animals(after)
        .into_iter()
        .filter(|(key, _)| !previous.contains_key(key))
        .map(|(_, id)| id)
        .collect::<Vec<_>>();
    i64::try_from(placed.len()).ok() == Some(count) && placed.iter().all(|id| *id == animal_id)
}

fn stall_animals(cells: &AllCellsReply) -> BTreeMap<(i64, i32), i64> {
    let mut animals = BTreeMap::new();
    for facility in &cells.facilities {
        let Some(ranch) = facility.ranch.as_ref() else {
            continue;
        };
        for animal in &ranch.animals {
            animals.insert((facility.instance_id, animal.animal_inst_id), animal.animal_id);
        }
    }
    animals
}

fn offer_json(offer: &ShopOffer, balance: i64) -> Value {
    json!({
        "id": offer.id.to_string(),
        "name": offer.name,
        "priceId": offer.price_id,
        "price": offer.unit_price,
        "balance": balance,
        "maxCount": offer.max_count,
        "remaining": offer.remaining,
        "available": offer.available,
    })
}

fn unlocked_count(bag: &GetBagReply, id: i64) -> i64 {
    bag.physical_items
        .iter()
        .chain(bag.virtual_items.iter())
        .filter(|item| item.id == id && !item.is_locked)
        .map(|item| item.count)
        .sum()
}

fn item_signature(items: &[CoreItem]) -> String {
    let mut totals = BTreeMap::<i64, i64>::new();
    for item in items {
        if item.count != 0 {
            *totals.entry(item.id).or_default() += item.count;
        }
    }
    let mut parts = totals.iter().map(|(id, count)| format!("{id}:{count}")).collect::<Vec<_>>();
    parts.sort();
    parts.join("|")
}

#[derive(Deserialize)]
struct GoodsConfig {
    id: i64,
    shop_id: i64,
    item_id: i64,
    price_id: i64,
    #[serde(default)]
    one_buy_count: i64,
    #[serde(default)]
    conds: String,
}

fn shop_goods() -> &'static HashMap<i64, GoodsConfig> {
    static GOODS: OnceLock<HashMap<i64, GoodsConfig>> = OnceLock::new();
    GOODS.get_or_init(|| {
        let rows: Vec<GoodsConfig> =
            serde_json::from_str(include_str!("../../assets/ranch/Goods.json"))
                .expect("牧场商店配置无法解析");
        rows.into_iter().map(|row| (row.id, row)).collect()
    })
}

fn harvest_ready(cells: &AllCellsReply) -> bool {
    cells.facilities.iter().filter_map(|facility| facility.ranch.as_ref()).any(|ranch| {
        ranch.animals.iter().any(|animal| animal.can_harvest)
            || ranch.shelves.iter().any(|shelf| shelf.can_collect)
    })
}

fn harvest_request(
    cells: &AllCellsReply,
    ranch_id: Option<i64>,
    animal_id: Option<i32>,
) -> Result<HarvestRanchRequest> {
    let (Some(ranch_id), Some(animal_id)) = (ranch_id, animal_id) else {
        return Ok(HarvestRanchRequest { op: HARVEST_ALL, ..HarvestRanchRequest::default() });
    };
    let animal = cells
        .facilities
        .iter()
        .filter_map(|facility| facility.ranch.as_ref())
        .flat_map(|ranch| ranch.animals.iter())
        .find(|animal| animal.ranch_inst_id == ranch_id && animal.animal_inst_id == animal_id)
        .filter(|animal| animal.can_harvest)
        .ok_or_else(|| Error::Business(STATE_CHANGED.to_owned()))?;
    Ok(HarvestRanchRequest {
        op: HARVEST_SELECTED,
        animals: vec![AnimalRef {
            ranch_inst_id: animal.ranch_inst_id,
            animal_inst_id: animal.animal_inst_id,
        }],
        ..HarvestRanchRequest::default()
    })
}

fn facility_views(cells: &AllCellsReply, level: i64, now_ms: i64) -> Vec<Value> {
    cells
        .facilities
        .iter()
        .filter(|facility| RANCH_MODULES.contains(&facility.module_type))
        .map(|facility| facility_view(facility, level, now_ms))
        .collect()
}

fn clean_request(cells: &AllCellsReply) -> Option<CleanAllRanchRequest> {
    let ranch_inst_ids = cells
        .facilities
        .iter()
        .filter(|facility| facility.ranch.as_ref().is_some_and(|ranch| !ranch.hazards.is_empty()))
        .map(|facility| facility.instance_id)
        .collect::<Vec<_>>();
    if ranch_inst_ids.is_empty() {
        None
    } else {
        Some(CleanAllRanchRequest { ranch_inst_ids, ..CleanAllRanchRequest::default() })
    }
}

fn grass_request(
    cells: &AllCellsReply,
    bag: &GetBagReply,
    facility_id: i64,
    count: i32,
) -> Result<AddFeedRequest> {
    if count <= 0 {
        return Err(Error::Business("数量无效".to_owned()));
    }
    let Some(facility) = cells.facilities.iter().find(|facility| {
        facility.instance_id == facility_id && facility.module_type == GRASS_MACHINE
    }) else {
        return Err(Error::Business("牧草机剩余容量不足".to_owned()));
    };
    let Some(machine) = facility.feed_machine.as_ref() else {
        return Err(Error::Business("牧草机剩余容量不足".to_owned()));
    };
    let room = facility.capacity - machine.feed_count;
    if count > room {
        return Err(Error::Business("牧草机剩余容量不足".to_owned()));
    }
    if item_count(bag, GRASS_ITEM) < i64::from(count) {
        return Err(Error::Business("牧草不足".to_owned()));
    }
    Ok(AddFeedRequest { instance_id: facility_id, add_count: count, use_gold: false, host_gid: 0 })
}

fn facility_view(facility: &FacilityInfo, level: i64, now_ms: i64) -> Value {
    json!({
        "id": facility.instance_id.to_string(),
        "facilityId": facility.facility_id,
        "moduleType": facility.module_type,
        "level": facility.level,
        "upgrade": upgrade_action(facility, level, now_ms),
        "capacity": facility.capacity,
        "grassStored": facility.feed_machine.as_ref().map(|machine| machine.feed_count).unwrap_or(0),
        "hazardCount": facility.ranch.as_ref().map(|ranch| ranch.hazards.len()).unwrap_or(0),
        "husbandryKind": facility.ranch.as_ref().map(|ranch| ranch.husbandry_type).unwrap_or(0),
        "openRoom": facility
            .ranch
            .as_ref()
            .map(|ranch| i64::from(ranch.capacity) - i64::from(ranch.animal_count))
            .unwrap_or(0),
        "animals": facility.ranch.as_ref().map(animal_views).unwrap_or_default(),
    })
}

fn animal_views(ranch: &crate::proto::generated::gamepb::animalpb::RanchDetail) -> Vec<Value> {
    ranch.animals.iter().map(animal_view).collect()
}

fn animal_view(animal: &AnimalDetail) -> Value {
    json!({
        "id": animal.animal_inst_id,
        "ranchId": animal.ranch_inst_id.to_string(),
        "animalId": animal.animal_id,
        "name": animal_name(animal.animal_id),
        "stageName": stage_name(animal),
        "canHarvest": animal.can_harvest,
        "canProduce": animal.can_deliver,
        "canFeed": animal.can_feed,
        "canRelease": animal.can_donate,
        "limitedCanUsed": animal.limited_can_used,
    })
}

struct UpgradeTarget {
    facility_id: i32,
    level: i32,
    next_facility_id: i32,
    next_level: i32,
}

fn start_target(cells: &AllCellsReply, facility_id: i64, level: i64) -> Result<UpgradeTarget> {
    let facility = ready_facility(cells, facility_id)?;
    let Some(next) = next_facility(facility.facility_id) else {
        return Err(Error::Business("尚未满足等级要求、已满级或正在升级".to_owned()));
    };
    let required = next.upgrade_conditions.as_deref().and_then(required_ranch_level);
    if required.is_none_or(|required| level < required) || facility.upgrade_state != UPGRADE_IDLE {
        return Err(Error::Business("尚未满足等级要求、已满级或正在升级".to_owned()));
    }
    Ok(upgrade_target(facility, next)?)
}

fn finish_target(
    cells: &AllCellsReply,
    facility_id: i64,
    now_ms: i64,
) -> Result<Option<UpgradeTarget>> {
    let facility = ready_facility(cells, facility_id)?;
    if facility.upgrade_state == UPGRADE_IDLE {
        return Ok(None);
    }
    if facility.upgrade_state != UPGRADE_FINISHED
        && upgrade_end_ms(facility.upgrade_end_time) > now_ms
    {
        return Err(Error::Business("设施仍在升级中".to_owned()));
    }
    let Some(next) = next_facility(facility.facility_id) else {
        return Err(Error::Business("建筑目标等级配置尚未同步".to_owned()));
    };
    Ok(Some(upgrade_target(facility, next)?))
}

fn upgrade_target(facility: &FacilityInfo, next: &FacilityConfig) -> Result<UpgradeTarget> {
    let next_facility_id = i32::try_from(next.facility_id)
        .map_err(|_| Error::Business("建筑目标等级配置尚未同步".to_owned()))?;
    Ok(UpgradeTarget {
        facility_id: facility.facility_id,
        level: facility.level,
        next_facility_id,
        next_level: next.level,
    })
}

fn upgrade_action(facility: &FacilityInfo, level: i64, now_ms: i64) -> &'static str {
    if facility.state != FACILITY_READY {
        return "";
    }
    if facility.upgrade_state == UPGRADE_IDLE {
        return if start_target(
            &AllCellsReply { facilities: vec![facility.clone()], ..AllCellsReply::default() },
            facility.instance_id,
            level,
        )
        .is_ok()
        {
            "start"
        } else {
            ""
        };
    }
    if facility.upgrade_state == UPGRADE_FINISHED
        || upgrade_end_ms(facility.upgrade_end_time) <= now_ms
    {
        "finish"
    } else {
        ""
    }
}

fn ready_facility(cells: &AllCellsReply, facility_id: i64) -> Result<&FacilityInfo> {
    cells
        .facilities
        .iter()
        .find(|facility| {
            facility.instance_id == facility_id && RANCH_MODULES.contains(&facility.module_type)
        })
        .filter(|facility| facility.state == FACILITY_READY)
        .ok_or_else(|| Error::Business("牧场设施尚未建造或未就绪".to_owned()))
}

fn next_facility(facility_id: i32) -> Option<&'static FacilityConfig> {
    let current = facility_catalog().get(&i64::from(facility_id))?;
    let next_id = current.next_facility_id?;
    facility_catalog().get(&next_id)
}

fn required_ranch_level(conditions: &str) -> Option<i64> {
    let level = conditions.strip_prefix("RANCH_LEVEL:")?;
    if level.chars().all(|ch| ch.is_ascii_digit()) && !level.is_empty() {
        level.parse().ok()
    } else {
        None
    }
}

fn upgrade_end_ms(end: i64) -> i64 {
    if end < SECOND_TIMESTAMP_LIMIT {
        end.saturating_mul(1000)
    } else {
        end
    }
}

fn ranch_level(exp: i64) -> i64 {
    level_rows().iter().rev().find(|row| row.exp <= exp).map(|row| row.level).unwrap_or(0)
}

#[derive(Deserialize)]
struct FacilityConfig {
    facility_id: i64,
    level: i32,
    #[serde(default)]
    next_facility_id: Option<i64>,
    #[serde(default)]
    upgrade_conditions: Option<String>,
}

fn facility_catalog() -> &'static HashMap<i64, FacilityConfig> {
    static FACILITIES: OnceLock<HashMap<i64, FacilityConfig>> = OnceLock::new();
    FACILITIES.get_or_init(|| {
        let rows: Vec<FacilityConfig> =
            serde_json::from_str(include_str!("../../assets/ranch/MCFacilityConfig.json"))
                .expect("牧场设施配置无法解析");
        rows.into_iter().map(|row| (row.facility_id, row)).collect()
    })
}

#[derive(Deserialize)]
struct LevelRow {
    level: i64,
    exp: i64,
}

fn level_rows() -> &'static [LevelRow] {
    static LEVELS: OnceLock<Vec<LevelRow>> = OnceLock::new();
    LEVELS
        .get_or_init(|| {
            serde_json::from_str(include_str!("../../assets/ranch/BigFarmLevel.json"))
                .expect("牧场等级配置无法解析")
        })
        .as_slice()
}

fn release_target(cells: &AllCellsReply, ranch_id: i64, animal_inst_id: i32) -> Result<AnimalRef> {
    let animal = cells
        .facilities
        .iter()
        .filter_map(|facility| facility.ranch.as_ref())
        .flat_map(|ranch| ranch.animals.iter())
        .find(|animal| {
            animal.ranch_inst_id == ranch_id
                && animal.animal_inst_id == animal_inst_id
                && animal.can_donate
        })
        .ok_or_else(|| Error::Business(STATE_CHANGED.to_owned()))?;
    Ok(AnimalRef { ranch_inst_id: animal.ranch_inst_id, animal_inst_id: animal.animal_inst_id })
}

fn release_remaining(ops: &[RanchOpTimesView]) -> Result<i64> {
    let Some(op) = ops.iter().find(|op| op.op_id == RELEASE_OP) else {
        return Err(Error::Protocol("牧场互动剩余次数未返回".to_owned()));
    };
    if op.remaining_times < 0 {
        Err(Error::Protocol("牧场互动剩余次数无效".to_owned()))
    } else {
        Ok(op.remaining_times)
    }
}

fn release_matches(reply: &DonateAnimalReply, ranch_id: i64, animal_inst_id: i32) -> bool {
    let [result] = reply.op_results.as_slice() else {
        return false;
    };
    if result.kind != RELEASE_KIND
        || result.facility_inst_id != ranch_id
        || result.target_id != i64::from(animal_inst_id)
        || !result.costs.is_empty()
        || result.rewards.iter().any(|item| item.count <= 0)
    {
        return false;
    }
    reply.ranches.iter().any(|ranch| {
        ranch.ranch_inst_id == ranch_id
            && ranch.animals.iter().all(|animal| animal.animal_inst_id != animal_inst_id)
    })
}

fn stage_name(animal: &AnimalDetail) -> &'static str {
    if animal.can_harvest {
        "可收获"
    } else if animal.can_deliver {
        "可生产"
    } else if animal.grass_blocked {
        "缺少牧草"
    } else {
        match animal.stage {
            1 => "幼崽期",
            2 => "成长期",
            3 => "生产期",
            4 => "成熟期",
            5 => "可收获",
            _ => "未知",
        }
    }
}

fn place_request(
    cells: &AllCellsReply,
    bag: &GetBagReply,
    ranch_id: i64,
    item_id: i64,
    count: i64,
) -> Result<PlaceAnimalRequest> {
    let Some(cub) = cub_catalog().get(&item_id) else {
        return Err(Error::Business("幼崽与窝或棚不匹配".to_owned()));
    };
    let Some(ranch) = cells
        .facilities
        .iter()
        .find(|facility| facility.instance_id == ranch_id)
        .and_then(|facility| facility.ranch.as_ref())
    else {
        return Err(Error::Business("幼崽与窝或棚不匹配".to_owned()));
    };
    if ranch.husbandry_type != cub.kind {
        return Err(Error::Business("幼崽与窝或棚不匹配".to_owned()));
    }
    if count > i64::from(ranch.capacity) - i64::from(ranch.animal_count) {
        return Err(Error::Business("窝或棚的空位不足".to_owned()));
    }
    if item_count(bag, item_id) < count {
        return Err(Error::Business("幼崽不足".to_owned()));
    }
    let items = (0..count)
        .map(|_| PlaceAnimalItem {
            ranch_inst_id: ranch_id,
            animal_id: cub.animal_id,
            giant_cub_item_id: cub.giant_item_id,
        })
        .collect();
    Ok(PlaceAnimalRequest { items })
}

struct Sale {
    request: SellRequest,
    sold: String,
    reward: String,
}

fn sale_plan(bag: &GetBagReply, choice: SellChoice) -> Result<Option<Sale>> {
    let selected = match choice {
        SellChoice::All => bag
            .physical_items
            .iter()
            .filter(|item| sellable(item))
            .map(|item| (item.uid, item.count))
            .collect::<Vec<_>>(),
        SellChoice::Stack { uid, count } => vec![(uid, count)],
    };
    if selected.is_empty() {
        return Ok(None);
    }
    let mut payload = Vec::new();
    let mut rewards = BTreeMap::<i64, i64>::new();
    for (uid, count) in selected {
        let Some(item) = bag.physical_items.iter().find(|item| item.uid == uid) else {
            return Err(Error::Business("产物库存、锁定或出售条件已变化".to_owned()));
        };
        if !sellable(item) || item.count < count {
            return Err(Error::Business("产物库存、锁定或出售条件已变化".to_owned()));
        }
        if let Some(show) = item.show.as_ref() {
            for price in &show.sells {
                *rewards.entry(price.id).or_default() += price.count.saturating_mul(count);
            }
        }
        payload.push(RanchItem { id: item.id, uid, count, ..RanchItem::default() });
    }
    Ok(Some(Sale {
        sold: sold_signature(&payload),
        reward: id_count_signature(rewards),
        request: SellRequest { items: payload },
    }))
}

fn sell_target(choice: SellChoice) -> String {
    match choice {
        SellChoice::All => "ranch:sell:all".to_owned(),
        SellChoice::Stack { uid, .. } => format!("ranch:sell:{uid}"),
    }
}

fn sale_costs(sale: &Sale) -> Vec<(i64, i64)> {
    let mut totals = BTreeMap::<i64, i64>::new();
    for item in &sale.request.items {
        *totals.entry(item.id).or_default() += item.count;
    }
    totals.into_iter().collect()
}

fn sellable(item: &RanchItem) -> bool {
    !item.is_locked
        && item.count > 0
        && item_type(item.id).is_some_and(|kind| SELLABLE_TYPES.contains(&kind))
        && item
            .show
            .as_ref()
            .is_some_and(|show| !show.sells.is_empty() && show.sell_cond.is_empty())
}

struct MarketOffer {
    item_id: i64,
    item_count: i64,
    price_id: i64,
    unit_price: i64,
    remaining: Option<i64>,
    available: bool,
}

fn empty_book() -> Value {
    json!({
        "level": 0,
        "progress": 0,
        "nextProgress": 0,
        "canClaim": false,
        "score": 0,
        "levels": [],
        "animals": [],
    })
}

fn ensure_book_type(kind: i32) -> Result<()> {
    if kind == BOOK_TYPE {
        Ok(())
    } else {
        Err(Error::Protocol("牧场图鉴类型不匹配".to_owned()))
    }
}

async fn illustrated_list(gateway: &Gateway) -> Result<GetIllustratedListV2Reply> {
    let request =
        GetIllustratedListV2Request { r#type: BOOK_TYPE, ..GetIllustratedListV2Request::default() };
    let bytes = gateway
        .request(ILLUSTRATED_SERVICE, "GetIllustratedListV2", &request.encode_to_vec())
        .await?;
    Ok(GetIllustratedListV2Reply::decode(bytes.as_slice())?)
}

async fn weight_book(gateway: &Gateway) -> Result<GetRanchWeightScoreV2Reply> {
    let request = GetRanchWeightScoreV2Request::default();
    let bytes = gateway
        .request(ILLUSTRATED_SERVICE, "GetRanchWeightScoreV2", &request.encode_to_vec())
        .await?;
    Ok(GetRanchWeightScoreV2Reply::decode(bytes.as_slice())?)
}

fn claimable_weight_levels(weight: &GetRanchWeightScoreV2Reply) -> Vec<&RanchWeightScoreLevelV2> {
    weight.levels.iter().filter(|level| level.can_claim && !level.claimed).collect()
}

fn book_json(list: &GetIllustratedListV2Reply, weight: &GetRanchWeightScoreV2Reply) -> Value {
    let mut animals = list.list.iter().collect::<Vec<_>>();
    animals.sort_by_key(|entry| animal_sort_key(entry.fruit_id));
    json!({
        "level": list.current_level,
        "progress": list.total_progress,
        "nextProgress": list.next_level_progress,
        "canClaim": list.has_claimable_reward,
        "score": weight.total_score,
        "levels": weight.levels.iter().map(weight_level_json).collect::<Vec<_>>(),
        "animals": animals.iter().copied().map(animal_json).collect::<Vec<_>>(),
    })
}

fn weight_level_json(level: &RanchWeightScoreLevelV2) -> Value {
    json!({
        "id": level.id,
        "score": level.score_condition,
        "canClaim": level.can_claim && !level.claimed,
    })
}

fn animal_json(entry: &IllustratedInfoV2) -> Value {
    json!({
        "id": entry.fruit_id,
        "name": item_name(i64::from(entry.fruit_id)),
        "unlocked": entry.unlocked,
        "progress": entry.progress,
        "giant": entry.giant.as_ref().map(giant_json),
    })
}

fn giant_json(giant: &RanchGiantRecordView) -> Value {
    json!({
        "unlocked": giant.unlocked,
        "tierName": tier_name(giant.tier),
        "weightKg": weight_kg(giant.best_weight_x10),
    })
}

fn animal_sort_key(fruit_id: i32) -> (i32, i32) {
    (illustrated_sort().get(&fruit_id).copied().unwrap_or(fruit_id), fruit_id)
}

fn weight_kg(weight_x10: i32) -> String {
    format!("{}.{:02}", weight_x10 / 100, (weight_x10 % 100).unsigned_abs())
}

fn tier_name(tier: i32) -> String {
    weight_tiers().get(&tier).cloned().unwrap_or_default()
}

fn illustrated_sort() -> &'static HashMap<i32, i32> {
    static SORTS: OnceLock<HashMap<i32, i32>> = OnceLock::new();
    SORTS.get_or_init(|| {
        #[derive(Deserialize)]
        struct Row {
            param: i32,
            sort: i32,
        }
        serde_json::from_str::<Vec<Row>>(include_str!("../../assets/ranch/Illustrated.json"))
            .expect("牧场图鉴排序无法解析")
            .into_iter()
            .map(|row| (row.param, row.sort))
            .collect()
    })
}

fn weight_tiers() -> &'static HashMap<i32, String> {
    static TIERS: OnceLock<HashMap<i32, String>> = OnceLock::new();
    TIERS.get_or_init(|| {
        #[derive(Deserialize)]
        struct Row {
            tier: i32,
            tier_name: String,
        }
        serde_json::from_str::<Vec<Row>>(include_str!("../../assets/ranch/MCRanchWeightTier.json"))
            .expect("牧场重量档位无法解析")
            .into_iter()
            .map(|row| (row.tier, row.tier_name))
            .collect()
    })
}

struct RanchTaskRow {
    id: i64,
    name: String,
    category: &'static str,
    progress: i64,
    total: i64,
    claimed: bool,
    can_claim: bool,
    share_multiple: i64,
    rewards: Vec<(i64, i64)>,
}

async fn ranch_task_info(gateway: &Gateway) -> Result<TaskInfo> {
    let request = TaskInfoRequest { global_scene_id: RANCH_SCENE };
    let bytes = gateway.request(TASK_SERVICE, "TaskInfo", &request.encode_to_vec()).await?;
    Ok(TaskInfoReply::decode(bytes.as_slice())?.task_info.unwrap_or_default())
}

fn ranch_task_rows(info: &TaskInfo) -> Vec<RanchTaskRow> {
    let mut rows: Vec<RanchTaskRow> = Vec::new();
    for task in info.growth_tasks.iter().chain(info.daily_tasks.iter()).chain(info.tasks.iter()) {
        if task.task_type != RANCH_TASK_GROWTH && task.task_type != RANCH_TASK_DAILY {
            continue;
        }
        let row = ranch_task_row(task);
        if let Some(existing) = rows.iter_mut().find(|existing| existing.id == task.id) {
            *existing = row;
        } else {
            rows.push(row);
        }
    }
    rows
}

fn ranch_task_row(task: &Task) -> RanchTaskRow {
    RanchTaskRow {
        id: task.id,
        name: if task.desc.is_empty() { format!("任务{}", task.id) } else { task.desc.clone() },
        category: if task.task_type == RANCH_TASK_DAILY { "daily" } else { "growth" },
        progress: task.progress,
        total: task.total_progress,
        claimed: task.is_claimed,
        can_claim: task.is_unlocked
            && !task.is_claimed
            && task.total_progress > 0
            && task.progress >= task.total_progress,
        share_multiple: task.share_multiple,
        rewards: task.rewards.iter().map(|item| (item.id, item.count)).collect(),
    }
}

fn task_json(task: RanchTaskRow) -> Value {
    json!({
        "id": task.id.to_string(),
        "name": task.name,
        "category": task.category,
        "progress": task.progress,
        "total": task.total,
        "canClaim": task.can_claim,
        "shareMultiple": task.share_multiple.max(1),
    })
}

fn expected_reward_signature(rewards: &[(i64, i64)], multiple: i64) -> Result<String> {
    let mut pairs = Vec::with_capacity(rewards.len());
    for (id, count) in rewards {
        let count = count
            .checked_mul(multiple)
            .ok_or_else(|| Error::Business("数量超出范围".to_owned()))?;
        pairs.push((*id, count));
    }
    Ok(id_count_signature(pairs))
}

async fn market_info(gateway: &Gateway) -> Result<RotationShopInfoReply> {
    let request = RotationShopInfoRequest { shop_id: MARKET_SHOP };
    let bytes = gateway.request(SHOP_SERVICE, "RotationShopInfo", &request.encode_to_vec()).await?;
    Ok(RotationShopInfoReply::decode(bytes.as_slice())?)
}

fn market_offer(
    card: &RotationGoodsCard,
    opened: bool,
    present: bool,
    level: i64,
    now: i64,
) -> MarketOffer {
    let discounted = card.discount_start_time > 0
        && card.discount_start_time <= now
        && now < card.discount_end_time;
    let remaining = (card.limit_count > 0).then_some((card.limit_count - card.limit_used).max(0));
    MarketOffer {
        item_id: card.item_id,
        item_count: card.item_count,
        price_id: card.cost_item_id,
        unit_price: if discounted { card.discount_price } else { card.cost_count },
        remaining,
        available: opened
            && present
            && level_met(&card.buy_conds, level)
            && (card.limit_count == 0 || card.limit_used < card.limit_count),
    }
}

fn level_met(conds: &[ShowCond], level: i64) -> bool {
    conds.iter().all(|cond| {
        cond.r#type == "最低等级"
            && !cond.params.is_empty()
            && cond.params.chars().all(|ch| ch.is_ascii_digit())
            && cond.params.parse::<i64>().ok().is_some_and(|required| level >= required)
    })
}

fn market_json(
    card: &RotationGoodsCard,
    info: &RotationShopInfoReply,
    level: i64,
    now: i64,
    balance: i64,
) -> Value {
    let offer = market_offer(card, info.opened, info.present, level, now);
    json!({
        "id": card.goods_id.to_string(),
        "name": item_name(card.item_id),
        "price": offer.unit_price,
        "priceName": item_name(offer.price_id),
        "balance": balance,
        "remaining": offer.remaining,
        "available": offer.available,
    })
}

struct CanOffer {
    id: i64,
    name: String,
    price_id: i64,
    unit_price: i64,
    free: bool,
    remaining: Option<i64>,
    available: bool,
    rewards: Vec<(i64, i64)>,
}

fn require_can_slot(slot: i32) -> Result<()> {
    if slot == CAN_SLOT_REGULAR || slot == CAN_SLOT_LIMITED {
        Ok(())
    } else {
        Err(Error::Business("未知牧场罐头商城".to_owned()))
    }
}

async fn can_mall_list(gateway: &Gateway, slot: i32) -> Result<GetMallListBySlotTypeResponse> {
    let request = GetMallListBySlotTypeRequest {
        slot_type: slot,
        is_manual_open: true,
        global_scene_id: RANCH_SCENE,
    };
    let bytes =
        gateway.request(MALL_SERVICE, "GetMallListBySlotType", &request.encode_to_vec()).await?;
    Ok(GetMallListBySlotTypeResponse::decode(bytes.as_slice())?)
}

fn can_offers(goods: &[MallGoods], now: i64) -> Vec<CanOffer> {
    goods.iter().filter_map(|good| can_offer(good, now)).collect()
}

fn can_offer(good: &MallGoods, now: i64) -> Option<CanOffer> {
    if good.reward_items.is_empty()
        || good.reward_items.iter().any(|item| !can_catalog().contains_key(&item.id))
    {
        return None;
    }
    let unit_price = can_unit_price(good, now);
    let remaining = can_remaining(good.purchase_limit.as_ref());
    let reward_id = good.reward_items[0].id;
    Some(CanOffer {
        id: good.goods_id,
        name: if good.name.is_empty() { item_name(reward_id) } else { good.name.clone() },
        price_id: good.price.as_ref().map(|price| price.id).unwrap_or(0),
        unit_price,
        free: good.is_free && unit_price == 0,
        remaining,
        available: can_available(good, unit_price, remaining),
        rewards: good.reward_items.iter().map(|item| (item.id, item.count)).collect(),
    })
}

fn can_unit_price(good: &MallGoods, now: i64) -> i64 {
    let list_price = good.price.as_ref().map(|price| price.count).unwrap_or(0);
    let promotion = good.discount_price > 0
        && good.promotion_start_time <= now
        && now < good.promotion_end_time;
    if promotion {
        good.discount_price
    } else {
        list_price
    }
}

fn can_remaining(limit: Option<&PurchaseLimit>) -> Option<i64> {
    let limit = limit?;
    if limit.limit_type == 0 {
        return None;
    }
    Some((limit.limit_count - limit.bought_count).max(0))
}

fn can_available(good: &MallGoods, unit_price: i64, remaining: Option<i64>) -> bool {
    let price_id = good.price.as_ref().map(|price| price.id).unwrap_or(0);
    let free = good.is_free && unit_price == 0;
    let share_blocked = good
        .share
        .as_ref()
        .is_some_and(|share| share.share_only && share.share_status != CAN_SHARE_READY);
    let priced = free || price_id > 0 && unit_price > 0;
    let in_stock = remaining.is_none_or(|left| left > 0);
    let listed = good.is_available || good.purchase_limit.is_none();
    !good.is_owned && !good.ad_only && !share_blocked && in_stock && listed && priced
}

fn reject_can_purchase(offer: &CanOffer, count: i64) -> Result<()> {
    if !offer.available
        || count > CAN_MALL_MAX
        || offer.remaining.is_some_and(|remaining| count > remaining)
    {
        return Err(Error::Business("罐头商品不可购买或超过限购数量".to_owned()));
    }
    Ok(())
}

fn can_json(offer: &CanOffer, balance: Option<i64>) -> Value {
    json!({
        "id": offer.id.to_string(),
        "name": offer.name,
        "price": offer.unit_price,
        "priceName": if offer.free { String::new() } else { item_name(offer.price_id) },
        "free": offer.free,
        "balance": balance,
        "remaining": offer.remaining,
        "available": offer.available,
    })
}

struct CanPlan {
    item_id: i64,
    bucket_id: i64,
    seconds: i64,
}

fn can_plan(bag: &GetBagReply, uid: i64, count: i64) -> Result<CanPlan> {
    let Some(item) = bag.physical_items.iter().find(|item| item.uid == uid) else {
        return Err(Error::Business("罐头库存不足或已锁定".to_owned()));
    };
    let Some(config) = can_catalog().get(&item.id) else {
        return Err(Error::Business("罐头库存不足或已锁定".to_owned()));
    };
    if item.is_locked || item.count < count {
        return Err(Error::Business("罐头库存不足或已锁定".to_owned()));
    }
    let Some(seconds) = config.speed_up_secs.checked_mul(count) else {
        return Err(Error::Business("数量超出范围".to_owned()));
    };
    Ok(CanPlan {
        item_id: item.id,
        bucket_id: if config.feed_type == "有机罐头" { ORGANIC_BUCKET } else { NORMAL_BUCKET },
        seconds,
    })
}

fn can_views(bag: &GetBagReply) -> Vec<Value> {
    bag.physical_items
        .iter()
        .filter(|item| !item.is_locked && item.count > 0 && can_catalog().contains_key(&item.id))
        .filter_map(|item| {
            let config = can_catalog().get(&item.id)?;
            Some(json!({
                "uid": item.uid.to_string(),
                "name": item_name(item.id),
                "count": item.count,
                "seconds": config.speed_up_secs,
                "organic": config.feed_type == "有机罐头",
            }))
        })
        .collect()
}

#[derive(Deserialize)]
struct CanConfig {
    id: i64,
    feed_type: String,
    speed_up_secs: i64,
}

fn can_catalog() -> &'static HashMap<i64, CanConfig> {
    static CANS: OnceLock<HashMap<i64, CanConfig>> = OnceLock::new();
    CANS.get_or_init(|| {
        let rows: Vec<CanConfig> =
            serde_json::from_str(include_str!("../../assets/ranch/MCCanConfig.json"))
                .expect("牧场罐头配置无法解析");
        rows.into_iter().map(|row| (row.id, row)).collect()
    })
}

fn product_views(bag: &GetBagReply) -> Vec<Value> {
    bag.physical_items
        .iter()
        .filter(|item| sellable(item))
        .map(|item| {
            json!({
                "uid": item.uid.to_string(),
                "name": item_name(item.id),
                "count": item.count,
                "price": price_label(item.show.as_ref().map(|show| show.sells.as_slice()).unwrap_or(&[])),
            })
        })
        .collect()
}

fn price_label(sells: &[ItemEntry]) -> String {
    sells
        .iter()
        .map(|price| format!("{} {}", item_name(price.id), price.count))
        .collect::<Vec<_>>()
        .join("、")
}

fn sold_signature(items: &[RanchItem]) -> String {
    let mut parts = items
        .iter()
        .map(|item| format!("{}:{}:{}", item.id, item.uid, item.count))
        .collect::<Vec<_>>();
    parts.sort();
    parts.join("|")
}

pub(crate) fn id_count_signature(items: impl IntoIterator<Item = (i64, i64)>) -> String {
    let mut totals = BTreeMap::<i64, i64>::new();
    for (id, count) in items {
        if count != 0 {
            *totals.entry(id).or_default() += count;
        }
    }
    let mut parts = totals.iter().map(|(id, count)| format!("{id}:{count}")).collect::<Vec<_>>();
    parts.sort();
    parts.join("|")
}

fn item_type(id: i64) -> Option<i64> {
    item_catalog().get(&id).and_then(|item| item.kind)
}

fn purchase_label(item_id: i64, count: i64) -> String {
    format!("购买{} ×{count}", item_name(item_id))
}

fn shown_purchase_label(label: &str, target: &str) -> String {
    if label != "购买牧场商品" {
        return label.to_owned();
    }
    let Some(goods_id) = target.strip_prefix("ranch:buy:").and_then(|id| id.parse::<i64>().ok())
    else {
        return label.to_owned();
    };
    shop_goods()
        .get(&goods_id)
        .map(|goods| format!("购买{}", item_name(goods.item_id)))
        .unwrap_or_else(|| label.to_owned())
}

fn pending_views(account_id: &str) -> Result<Vec<Value>> {
    crate::services::ranch_ops::pending_json(account_id)?
        .into_iter()
        .map(|mut row| {
            let target = row.get("target").and_then(Value::as_str).unwrap_or("").to_owned();
            let label = row.get("label").and_then(Value::as_str).unwrap_or("");
            row["label"] = json!(shown_purchase_label(label, &target));
            if let Some(costs) = row.get_mut("costs").and_then(Value::as_array_mut) {
                for cost in costs {
                    let name = cost
                        .get("resourceKey")
                        .and_then(Value::as_str)
                        .and_then(|key| key.strip_prefix("item:"))
                        .and_then(|id| id.parse::<i64>().ok())
                        .map(item_name)
                        .unwrap_or_else(|| "物品".to_owned());
                    cost["name"] = json!(name);
                }
            }
            Ok(row)
        })
        .collect()
}

fn item_name(id: i64) -> String {
    item_catalog()
        .get(&id)
        .and_then(|item| item.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("物品{id}"))
}

#[derive(Deserialize)]
struct ItemMeta {
    id: i64,
    #[serde(default, rename = "type")]
    kind: Option<i64>,
    #[serde(default)]
    name: Option<String>,
}

fn item_catalog() -> &'static HashMap<i64, ItemMeta> {
    static ITEMS: OnceLock<HashMap<i64, ItemMeta>> = OnceLock::new();
    ITEMS.get_or_init(|| {
        let rows: Vec<ItemMeta> =
            serde_json::from_str(include_str!("../../assets/ranch/MCItemInfo.json"))
                .expect("牧场物品配置无法解析");
        rows.into_iter().map(|row| (row.id, row)).collect()
    })
}

fn cub_views(bag: &GetBagReply) -> Vec<Value> {
    let mut counts = BTreeMap::<i64, i64>::new();
    for item in bag.physical_items.iter().chain(bag.virtual_items.iter()) {
        if cub_catalog().contains_key(&item.id) {
            *counts.entry(item.id).or_default() += item.count;
        }
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .filter_map(|(item_id, count)| {
            let cub = cub_catalog().get(&item_id)?;
            Some(json!({
                "id": item_id.to_string(),
                "name": cub.name,
                "count": count,
                "kind": cub.kind,
            }))
        })
        .collect()
}

struct CubSpec {
    animal_id: i64,
    name: String,
    giant_item_id: i64,
    kind: i32,
}

fn cub_catalog() -> &'static HashMap<i64, CubSpec> {
    static CUBS: OnceLock<HashMap<i64, CubSpec>> = OnceLock::new();
    CUBS.get_or_init(|| {
        let mut cubs = HashMap::new();
        for animal in animal_configs().values() {
            let Some(kind) = husbandry_kind(&animal.husbandry_type) else {
                continue;
            };
            cubs.insert(
                animal.cub_item_id,
                CubSpec { animal_id: animal.id, name: animal.name.clone(), giant_item_id: 0, kind },
            );
        }
        for mutation in mutation_configs() {
            let Some(animal) = animal_configs().get(&mutation.animal_id) else {
                continue;
            };
            let Some(kind) = husbandry_kind(&animal.husbandry_type) else {
                continue;
            };
            let name = format!("巨大{}", animal.name);
            cubs.insert(
                mutation.giant_cub_item_id,
                CubSpec {
                    animal_id: animal.id,
                    name,
                    giant_item_id: mutation.giant_cub_item_id,
                    kind,
                },
            );
        }
        cubs
    })
}

#[derive(Deserialize)]
struct MutationConfig {
    animal_id: i64,
    giant_cub_item_id: i64,
}

fn mutation_configs() -> &'static [MutationConfig] {
    static MUTATIONS: OnceLock<Vec<MutationConfig>> = OnceLock::new();
    MUTATIONS
        .get_or_init(|| {
            serde_json::from_str(include_str!("../../assets/ranch/MCAnimalMutation.json"))
                .expect("牧场变异配置无法解析")
        })
        .as_slice()
}

pub(crate) fn guard_plan(
    target: String,
    label: impl Into<String>,
) -> crate::services::ranch_ops::SpendPlan {
    crate::services::ranch_ops::SpendPlan {
        target,
        label: label.into(),
        costs: Vec::new(),
        balances: Vec::new(),
    }
}

fn release_label(animal_id: i64, mutation_type: i32) -> String {
    let prefix = if mutation_type == 0 { "" } else { "巨大" };
    format!("放生{prefix}{}", animal_name(animal_id))
}

fn money_plan(
    target: String,
    label: String,
    bag: &GetBagReply,
    costs: &[(i64, i64)],
) -> crate::services::ranch_ops::SpendPlan {
    crate::services::ranch_ops::SpendPlan {
        target,
        label,
        costs: costs
            .iter()
            .map(|(item_id, amount)| crate::services::ranch_ops::SpendCost {
                item_id: *item_id,
                amount: *amount,
            })
            .collect(),
        balances: costs
            .iter()
            .map(|(item_id, _)| {
                let total = item_count(bag, *item_id);
                crate::services::ranch_ops::SpendBalance {
                    item_id: *item_id,
                    total,
                    locked: total.saturating_sub(unlocked_count(bag, *item_id)),
                }
            })
            .collect(),
    }
}

fn item_count(bag: &GetBagReply, id: i64) -> i64 {
    bag.physical_items
        .iter()
        .chain(bag.virtual_items.iter())
        .filter(|item| item.id == id)
        .map(|item| item.count)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::generated::gamepb::animalpb::{RanchDetail, ShelfDetail};
    use crate::proto::generated::gamepb::mallpb::MallShareInfo;

    #[test]
    fn purchase_record_names_the_goods() {
        assert_eq!(release_label(424242, 0), "放生动物424242");
        assert_eq!(release_label(424242, 2), "放生巨大动物424242");
        let guarded = guard_plan("ranch:release:1:2".to_owned(), "放生动物");
        assert!(guarded.costs.is_empty());
        assert_eq!(purchase_label(7001, 3), "购买牧草 ×3");
        assert_eq!(shown_purchase_label("购买牧草 ×3", "ranch:buy:15"), "购买牧草 ×3");
        let renamed = shown_purchase_label("购买牧场商品", "ranch:buy:15");
        assert!(renamed.starts_with("购买"));
        assert_ne!(renamed, "购买牧场商品");
        assert_eq!(shown_purchase_label("购买牧场商品", "ranch:missing"), "购买牧场商品");
    }

    fn task_row(id: i64, multiple: i64, reward_id: i64, reward_count: i64) -> RanchTaskRow {
        RanchTaskRow {
            id,
            name: String::new(),
            category: "daily",
            progress: 1,
            total: 1,
            claimed: false,
            can_claim: true,
            share_multiple: multiple,
            rewards: vec![(reward_id, reward_count)],
        }
    }

    fn animal(id: i32, harvest: bool) -> AnimalDetail {
        AnimalDetail {
            ranch_inst_id: 9,
            animal_inst_id: id,
            animal_id: 30,
            can_harvest: harvest,
            stage: 2,
            ..AnimalDetail::default()
        }
    }

    #[test]
    fn patrol_harvests_only_when_an_animal_or_shelf_is_ready() {
        let idle = AllCellsReply {
            facilities: vec![FacilityInfo {
                ranch: Some(RanchDetail {
                    animals: vec![animal(1, false)],
                    ..RanchDetail::default()
                }),
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        };
        assert!(!harvest_ready(&idle));
        let ready = AllCellsReply {
            facilities: vec![FacilityInfo {
                ranch: Some(RanchDetail {
                    animals: vec![animal(1, true)],
                    ..RanchDetail::default()
                }),
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        };
        assert!(harvest_ready(&ready));
        let shelf = AllCellsReply {
            facilities: vec![FacilityInfo {
                ranch: Some(RanchDetail {
                    shelves: vec![ShelfDetail { can_collect: true, ..ShelfDetail::default() }],
                    ..RanchDetail::default()
                }),
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        };
        assert!(harvest_ready(&shelf));
    }

    #[test]
    fn overview_keeps_ranch_modules_and_harvest_state() {
        let cells = AllCellsReply {
            facilities: vec![
                FacilityInfo {
                    instance_id: 4,
                    facility_id: 8,
                    module_type: 1,
                    ..FacilityInfo::default()
                },
                FacilityInfo {
                    instance_id: 9,
                    facility_id: 12,
                    module_type: 6,
                    level: 2,
                    ranch: Some(RanchDetail {
                        animals: vec![animal(3, true)],
                        ..RanchDetail::default()
                    }),
                    ..FacilityInfo::default()
                },
            ],
            ..AllCellsReply::default()
        };
        let views = facility_views(&cells, 0, 0);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0]["id"], "9");
        assert_eq!(views[0]["animals"][0]["stageName"], "可收获");
    }

    #[test]
    fn selected_harvest_requires_a_ready_animal() {
        let cells = AllCellsReply {
            facilities: vec![FacilityInfo {
                ranch: Some(RanchDetail {
                    animals: vec![animal(3, false)],
                    ..RanchDetail::default()
                }),
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        };
        assert!(harvest_request(&cells, Some(9), Some(3)).is_err());
        let ready = AllCellsReply {
            facilities: vec![FacilityInfo {
                ranch: Some(RanchDetail {
                    animals: vec![animal(3, true)],
                    ..RanchDetail::default()
                }),
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        };
        let request = harvest_request(&ready, Some(9), Some(3)).expect("ready");
        assert_eq!(request.op, HARVEST_SELECTED);
        assert_eq!(request.animals[0].animal_inst_id, 3);
        assert_eq!(harvest_request(&ready, None, None).expect("all").op, HARVEST_ALL);
    }

    #[test]
    fn produce_skips_when_nothing_can_deliver() {
        let cells = AllCellsReply {
            facilities: vec![FacilityInfo {
                ranch: Some(RanchDetail {
                    animals: vec![animal(3, true)],
                    ..RanchDetail::default()
                }),
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        };
        assert!(produce_request(&cells, None, None).expect("skip").is_none());
    }

    #[test]
    fn feed_rejects_an_empty_normal_bucket() {
        let mut fed = animal(3, false);
        fed.animal_id = 60001;
        fed.can_feed = true;
        fed.stage = 2;
        fed.grow_ready_at_ms = 3_600_000;
        let cells = AllCellsReply {
            facilities: vec![FacilityInfo {
                ranch: Some(RanchDetail { animals: vec![fed], ..RanchDetail::default() }),
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        };
        let error =
            feed_request(&cells, &GetBagReply::default(), Some(9), Some(3), BUCKET_NORMAL, 0)
                .expect_err("empty");
        assert!(error.to_string().contains("罐头桶为空"));
    }

    #[test]
    fn chicken_name_comes_from_the_ranch_config() {
        assert_eq!(animal_name(60001), "鸡");
    }

    #[test]
    fn clean_selects_facilities_with_hazards() {
        let cells = AllCellsReply {
            facilities: vec![
                FacilityInfo { instance_id: 1, ..FacilityInfo::default() },
                FacilityInfo {
                    instance_id: 9,
                    ranch: Some(RanchDetail {
                        hazards: vec![crate::proto::generated::gamepb::animalpb::RanchHazardView {
                            hazard_id: 4,
                            ..Default::default()
                        }],
                        ..RanchDetail::default()
                    }),
                    ..FacilityInfo::default()
                },
            ],
            ..AllCellsReply::default()
        };
        let request = clean_request(&cells).expect("hazards");
        assert_eq!(request.ranch_inst_ids, vec![9]);
    }

    #[test]
    fn grass_rejects_a_full_machine_and_an_empty_bag() {
        let full = AllCellsReply {
            facilities: vec![FacilityInfo {
                instance_id: 8,
                module_type: GRASS_MACHINE,
                capacity: 2,
                feed_machine: Some(
                    crate::proto::generated::gamepb::homesteadpb::FeedMachineDetail {
                        feed_count: 2,
                        ..Default::default()
                    },
                ),
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        };
        assert!(grass_request(&full, &GetBagReply::default(), 8, 1).is_err());
        let open = AllCellsReply {
            facilities: vec![FacilityInfo {
                instance_id: 8,
                module_type: GRASS_MACHINE,
                capacity: 2,
                feed_machine: Some(
                    crate::proto::generated::gamepb::homesteadpb::FeedMachineDetail {
                        feed_count: 0,
                        ..Default::default()
                    },
                ),
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        };
        let error = grass_request(&open, &GetBagReply::default(), 8, 1).expect_err("grass");
        assert!(error.to_string().contains("牧草不足"));
    }

    #[test]
    fn chicken_cub_uses_the_configured_shop_price() {
        let goods = GoodsInfo {
            id: 10002,
            item_id: 60001,
            item_count: 1,
            price: 2,
            unlocked: true,
            ..GoodsInfo::default()
        };
        let offer = shop_offer(&goods, SHOP_CUBS, 0);
        assert!(offer.available);
        assert_eq!(offer.name, "鸡");
        assert_eq!(offer.price_id, 1007);
        assert_eq!(offer.unit_price, 2);
        assert_eq!(offer.max_count, 99);
        assert_eq!(offer.animal_id, Some(60001));
        assert_eq!(offer.husbandry_kind, Some(NEST));
        assert_eq!(offer.required_level, 1);
    }

    #[test]
    fn shop_config_must_match_the_requested_shelf() {
        let goods = GoodsInfo {
            id: 10002,
            item_id: 60001,
            unlocked: true,
            price: 2,
            ..GoodsInfo::default()
        };
        assert!(!shop_offer(&goods, SHOP_FEED, 0).available);
        assert!(require_shop(7).is_err());
    }

    #[test]
    fn discount_replaces_the_unit_price_inside_its_window() {
        let goods = GoodsInfo {
            id: 10002,
            item_id: 60001,
            unlocked: true,
            price: 2,
            discount_price: 1,
            discount_start_time: 100,
            discount_end_time: 200,
            ..GoodsInfo::default()
        };
        assert_eq!(shop_offer(&goods, SHOP_CUBS, 150).unit_price, 1);
        assert_eq!(shop_offer(&goods, SHOP_CUBS, 200).unit_price, 2);
    }

    #[test]
    fn item_signature_sorts_textually() {
        let items = [
            CoreItem { id: 10, count: 1, ..CoreItem::default() },
            CoreItem { id: 2, count: 1, ..CoreItem::default() },
        ];
        assert_eq!(item_signature(&items), "10:1|2:1");
    }

    #[test]
    fn open_stalls_follow_the_husbandry_kind() {
        let cells = AllCellsReply {
            facilities: vec![
                FacilityInfo {
                    instance_id: 1,
                    ranch: Some(RanchDetail {
                        husbandry_type: NEST,
                        capacity: 4,
                        animal_count: 1,
                        ..RanchDetail::default()
                    }),
                    ..FacilityInfo::default()
                },
                FacilityInfo {
                    instance_id: 2,
                    ranch: Some(RanchDetail {
                        husbandry_type: SHED,
                        capacity: 3,
                        ..RanchDetail::default()
                    }),
                    ..FacilityInfo::default()
                },
            ],
            ..AllCellsReply::default()
        };
        assert_eq!(open_stalls(&cells, NEST), 3);
        assert_eq!(open_stalls(&cells, SHED), 3);
        let offer = shop_offer(
            &GoodsInfo {
                id: 10002,
                item_id: 60001,
                unlocked: true,
                price: 2,
                ..GoodsInfo::default()
            },
            SHOP_CUBS,
            0,
        );
        assert!(placement_target(&offer, &cells, 3, true).is_ok());
        let error = placement_target(&offer, &cells, 4, true).expect_err("room");
        assert!(error.to_string().contains("空位不足"));
    }

    #[test]
    fn placed_animals_are_the_new_instances() {
        let before = stall_with(vec![(1, 60001)]);
        let after = stall_with(vec![(1, 60001), (2, 60001)]);
        assert!(placed_animals_match(&before, &after, 60001, 1));
        assert!(!placed_animals_match(&before, &after, 60001, 2));
        assert!(!placed_animals_match(&before, &after, 60002, 1));
        let feed = shop_offer(
            &GoodsInfo { id: 1, item_id: 1, unlocked: true, price: 1, ..GoodsInfo::default() },
            SHOP_FEED,
            0,
        );
        let error = placement_target(&feed, &before, 1, true).expect_err("cub");
        assert!(error.to_string().contains("只有幼崽"));
    }

    #[test]
    fn place_accepts_a_chicken_cub_in_an_open_nest() {
        let nest = nest_with(NEST, 2, 1);
        let bag = bag_with(60001, 2);
        let request = place_request(&nest, &bag, 9, 60001, 1).expect("place");
        assert_eq!(request.items.len(), 1);
        assert_eq!(request.items[0].animal_id, 60001);
        assert_eq!(request.items[0].giant_cub_item_id, 0);
        assert!(place_request(&nest, &bag, 9, 60001, 2)
            .expect_err("full")
            .to_string()
            .contains("空位不足"));
        assert!(place_request(&nest_with(SHED, 2, 0), &bag, 9, 60001, 1)
            .expect_err("kind")
            .to_string()
            .contains("不匹配"));
        assert!(place_request(&nest, &bag_with(60001, 0), 9, 60001, 1)
            .expect_err("empty")
            .to_string()
            .contains("幼崽不足"));
    }

    #[test]
    fn giant_cub_places_the_mutation_item() {
        let request =
            place_request(&nest_with(NEST, 1, 0), &bag_with(65002, 1), 9, 65002, 1).expect("giant");
        assert_eq!(request.items[0].animal_id, 60002);
        assert_eq!(request.items[0].giant_cub_item_id, 65002);
        assert_eq!(cub_views(&bag_with(65002, 1))[0]["name"], "巨大鸭");
    }

    #[test]
    fn enabled_animals_list_the_pen_they_fit() {
        let choices = animal_choices();
        let chicken = choices.iter().find(|choice| choice.id == 60001).expect("chicken");
        assert_eq!(chicken.name, "鸡");
        assert!(chicken.nest);
        assert!(choices.iter().any(|choice| !choice.nest));
        assert!(choices.iter().all(|choice| animal_fits(choice.id, choice.nest)));
    }

    #[test]
    fn chicken_product_sells_for_the_shown_price() {
        let bag = GetBagReply {
            physical_items: vec![
                product(2060001, 7, 2, false),
                product(2060001, 8, 1, true),
                product(1001, 9, 1, false),
            ],
            ..GetBagReply::default()
        };
        let sale =
            sale_plan(&bag, SellChoice::Stack { uid: 7, count: 2 }).expect("plan").expect("sale");
        assert_eq!(sale.sold, "2060001:7:2");
        assert_eq!(sale.reward, "1007:6");
        assert_eq!(sell_target(SellChoice::Stack { uid: 7, count: 2 }), "ranch:sell:7");
        assert_eq!(sell_target(SellChoice::All), "ranch:sell:all");
        assert_eq!(sale_costs(&sale), vec![(2060001, 2)]);
        assert!(sale_plan(&bag, SellChoice::Stack { uid: 8, count: 1 }).is_err());
        assert!(sale_plan(&bag, SellChoice::Stack { uid: 9, count: 1 }).is_err());
        let views = product_views(&bag);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0]["name"], "鸡");
        assert_eq!(views[0]["price"], "金券 3");
        let all = sale_plan(&bag, SellChoice::All).expect("all").expect("rows");
        assert_eq!(all.sold, "2060001:7:2");
        assert_eq!(all.reward, "1007:6");
    }

    fn product(id: i64, uid: i64, count: i64, locked: bool) -> RanchItem {
        RanchItem {
            id,
            uid,
            count,
            is_locked: locked,
            show: Some(crate::proto::generated::gamepb::ranchpb::ItemShow {
                sells: vec![ItemEntry { id: 1007, count: 3 }],
                ..Default::default()
            }),
            ..RanchItem::default()
        }
    }

    #[test]
    fn release_clears_only_a_donatable_animal() {
        let mut donor = animal(3, false);
        donor.can_donate = true;
        donor.ranch_inst_id = 9;
        let cells = AllCellsReply {
            facilities: vec![FacilityInfo {
                instance_id: 9,
                ranch: Some(RanchDetail { animals: vec![donor], ..RanchDetail::default() }),
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        };
        assert_eq!(release_target(&cells, 9, 3).expect("target").animal_inst_id, 3);
        assert!(release_target(&cells, 9, 4).is_err());
        assert_eq!(
            release_remaining(&[RanchOpTimesView {
                op_id: RELEASE_OP,
                remaining_times: 2,
                ..RanchOpTimesView::default()
            }])
            .expect("times"),
            2
        );
        assert!(release_remaining(&[]).is_err());
        let mut reply = DonateAnimalReply {
            ranches: vec![RanchDetail { ranch_inst_id: 9, ..RanchDetail::default() }],
            op_results: vec![crate::proto::generated::corepb::FacilityOpResult {
                facility_inst_id: 9,
                kind: RELEASE_KIND,
                target_id: 3,
                rewards: vec![CoreItem { id: 1007, count: 1, ..CoreItem::default() }],
                ..Default::default()
            }],
        };
        assert!(release_matches(&reply, 9, 3));
        reply.ranches[0].animals.push(AnimalDetail {
            animal_inst_id: 3,
            ranch_inst_id: 9,
            ..AnimalDetail::default()
        });
        assert!(!release_matches(&reply, 9, 3));
        reply.op_results[0].rewards[0].count = 0;
        reply.ranches[0].animals.clear();
        assert!(!release_matches(&reply, 9, 3));
    }

    #[test]
    fn coop_upgrade_follows_ranch_level_and_timer() {
        let ready = coop(FACILITY_READY, UPGRADE_IDLE, 0);
        let target = start_target(&ready, 9, 8).expect("start");
        assert_eq!(target.next_facility_id, 6001002);
        assert_eq!(target.next_level, 2);
        assert!(start_target(&ready, 9, 7).is_err());
        assert!(start_target(&coop(FACILITY_READY, 1, 0), 9, 8).is_err());
        assert!(start_target(&coop(0, UPGRADE_IDLE, 0), 9, 8).is_err());
        assert!(finish_target(&ready, 9, 0).expect("idle").is_none());
        assert_eq!(
            finish_target(&coop(FACILITY_READY, UPGRADE_FINISHED, 9_000_000_000_000), 9, 0)
                .expect("marked")
                .expect("target")
                .next_level,
            2
        );
        assert!(finish_target(&coop(FACILITY_READY, 1, 5_000), 9, 1_000).is_err());
        assert_eq!(
            finish_target(&coop(FACILITY_READY, 1, 1), 9, 2_000)
                .expect("due")
                .expect("target")
                .next_facility_id,
            6001002
        );
        assert_eq!(upgrade_action(&ready.facilities[0], 8, 0), "start");
        assert_eq!(ranch_level(0), 1);
        assert_eq!(ranch_level(299), 1);
        assert_eq!(ranch_level(300), 2);
    }

    #[test]
    fn can_shop_keeps_priced_cans_and_the_free_gift() {
        assert!(require_can_slot(8).is_err());
        assert!(require_can_slot(CAN_SLOT_REGULAR).is_ok());
        let mut promotion = listed_can(1, 80021, 10);
        promotion.discount_price = 4;
        promotion.promotion_start_time = 100;
        promotion.promotion_end_time = 200;
        let mut free = listed_can(2001, 80021, 0);
        free.is_free = true;
        free.purchase_limit = Some(PurchaseLimit {
            limit_type: 1,
            bought_count: 0,
            limit_count: 1,
            ..PurchaseLimit::default()
        });
        let mut blocked = free.clone();
        blocked.goods_id = 2002;
        blocked.share =
            Some(MallShareInfo { share_only: true, share_status: 1, ..MallShareInfo::default() });
        let mut ready = blocked.clone();
        ready.goods_id = 2003;
        ready.share = Some(MallShareInfo {
            share_only: true,
            share_status: CAN_SHARE_READY,
            ..MallShareInfo::default()
        });
        let mut sold_out = free.clone();
        sold_out.goods_id = 2004;
        sold_out.purchase_limit = Some(PurchaseLimit {
            limit_type: 1,
            bought_count: 1,
            limit_count: 1,
            ..PurchaseLimit::default()
        });
        let other = listed_can(9, 1, 3);
        let open =
            can_offers(&[promotion.clone(), free.clone(), blocked, ready, sold_out, other], 150);
        assert_eq!(
            open.iter().map(|offer| offer.id).collect::<Vec<_>>(),
            vec![1, 2001, 2002, 2003, 2004]
        );
        assert_eq!(open[0].unit_price, 4);
        assert!(open[1].free && open[1].available && open[1].remaining == Some(1));
        assert!(!open[2].available);
        assert!(open[3].available);
        assert!(!open[4].available && open[4].remaining == Some(0));
        assert_eq!(can_offers(&[promotion], 200)[0].unit_price, 10);
        assert!(reject_can_purchase(&open[4], 1).is_err());
        let gift = daily_benefit(&[free.clone()]);
        assert!(gift.available && !gift.claimed);
        assert_eq!(id_count_signature(gift.rewards), "80021:1");
        let mut claimed_gift = free.clone();
        claimed_gift.purchase_limit.as_mut().expect("limit").bought_count = 1;
        assert!(daily_benefit(&[claimed_gift]).claimed);
        let mut share_only = free;
        share_only.share =
            Some(MallShareInfo { share_only: true, share_status: 1, ..MallShareInfo::default() });
        assert!(!daily_benefit(&[share_only]).available);
        assert!(!daily_benefit(&[listed_can(2001, 80021, 1)]).available);
        assert!(!daily_benefit(&[]).available);
        let mut bought = PurchaseResponse {
            goods_id: BENEFIT_GOODS_ID,
            success: true,
            reward_items: vec![CoreItem { id: 80021, count: 1, ..CoreItem::default() }],
            purchase_limit: Some(PurchaseLimit {
                limit_type: BENEFIT_LIMIT_TYPE,
                bought_count: BENEFIT_LIMIT_COUNT,
                limit_count: BENEFIT_LIMIT_COUNT,
                ..PurchaseLimit::default()
            }),
            ..PurchaseResponse::default()
        };
        assert!(confirm_daily_benefit(&bought, "80021:1").is_ok());
        bought.purchase_limit.as_mut().expect("limit").bought_count = 0;
        assert!(confirm_daily_benefit(&bought, "80021:1").is_err());
    }

    fn listed_can(id: i64, reward: i64, price: i64) -> MallGoods {
        MallGoods {
            goods_id: id,
            reward_items: vec![CoreItem { id: reward, count: 1, ..CoreItem::default() }],
            price: Some(CoreItem { id: 1007, count: price, ..CoreItem::default() }),
            is_available: true,
            ..MallGoods::default()
        }
    }

    #[test]
    fn book_sorts_ranch_animals_and_keeps_claimable_rewards() {
        let list = GetIllustratedListV2Reply {
            r#type: BOOK_TYPE,
            current_level: 2,
            total_progress: 3,
            next_level_progress: 5,
            has_claimable_reward: true,
            total_rewards: vec![RanchItem { id: 1007, count: 4, ..RanchItem::default() }],
            list: vec![
                IllustratedInfoV2 {
                    fruit_id: 2060007,
                    unlocked: true,
                    ..IllustratedInfoV2::default()
                },
                IllustratedInfoV2 {
                    fruit_id: 2070002,
                    unlocked: true,
                    ..IllustratedInfoV2::default()
                },
                IllustratedInfoV2 {
                    fruit_id: 9,
                    giant: Some(RanchGiantRecordView {
                        unlocked: true,
                        best_weight_x10: 1124,
                        tier: 4,
                        ..RanchGiantRecordView::default()
                    }),
                    ..IllustratedInfoV2::default()
                },
            ],
            ..GetIllustratedListV2Reply::default()
        };
        let weight = GetRanchWeightScoreV2Reply {
            total_score: 30,
            levels: vec![
                RanchWeightScoreLevelV2 {
                    id: 1,
                    score_condition: 10,
                    can_claim: true,
                    claimed: false,
                    rewards: vec![RanchItem { id: 1007, count: 2, ..RanchItem::default() }],
                    ..RanchWeightScoreLevelV2::default()
                },
                RanchWeightScoreLevelV2 {
                    id: 2,
                    can_claim: true,
                    claimed: true,
                    ..RanchWeightScoreLevelV2::default()
                },
            ],
            ..GetRanchWeightScoreV2Reply::default()
        };
        assert!(ensure_book_type(1).is_err());
        let value = book_json(&list, &weight);
        let ids = value["animals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|animal| animal["id"].as_i64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![9, 2070002, 2060007]);
        assert_eq!(value["animals"][0]["giant"]["weightKg"], "11.24");
        assert_eq!(value["animals"][0]["giant"]["tierName"], "巨无霸");
        assert_eq!(value["canClaim"], true);
        assert_eq!(value["levels"][0]["canClaim"], true);
        assert_eq!(value["levels"][1]["canClaim"], false);
        let claimable = claimable_weight_levels(&weight);
        assert_eq!(claimable.len(), 1);
        assert_eq!(
            id_count_signature(claimable[0].rewards.iter().map(|item| (item.id, item.count))),
            "1007:2"
        );
    }

    #[test]
    fn ranch_tasks_keep_scene_tasks_and_share_rewards() {
        let info = TaskInfo {
            growth_tasks: vec![Task {
                id: 11,
                task_type: 1,
                is_unlocked: true,
                progress: 1,
                total_progress: 1,
                desc: "农场".to_owned(),
                ..Task::default()
            }],
            daily_tasks: vec![Task {
                id: 21,
                task_type: 5,
                is_unlocked: true,
                progress: 2,
                total_progress: 2,
                desc: "每日".to_owned(),
                share_multiple: 2,
                rewards: vec![CoreItem { id: 1007, count: 3, ..CoreItem::default() }],
                ..Task::default()
            }],
            tasks: vec![
                Task {
                    id: 21,
                    task_type: 5,
                    is_unlocked: true,
                    is_claimed: true,
                    progress: 2,
                    total_progress: 2,
                    desc: "已领".to_owned(),
                    share_multiple: 2,
                    rewards: vec![CoreItem { id: 1007, count: 3, ..CoreItem::default() }],
                    ..Task::default()
                },
                Task {
                    id: 31,
                    task_type: 4,
                    is_unlocked: true,
                    progress: 1,
                    total_progress: 3,
                    ..Task::default()
                },
                Task {
                    id: 41,
                    task_type: 4,
                    is_unlocked: true,
                    progress: 1,
                    total_progress: 1,
                    desc: "可领".to_owned(),
                    rewards: vec![CoreItem { id: 1007, count: 1, ..CoreItem::default() }],
                    ..Task::default()
                },
            ],
            ..TaskInfo::default()
        };
        let rows = ranch_task_rows(&info);
        assert_eq!(rows.iter().map(|row| row.id).collect::<Vec<_>>(), vec![21, 31, 41]);
        assert!(rows[0].claimed);
        assert!(!rows[0].can_claim);
        assert_eq!(rows[0].name, "已领");
        assert_eq!(
            expected_reward_signature(&rows[0].rewards, rows[0].share_multiple.max(1)).unwrap(),
            "1007:6"
        );
        assert_eq!(rows[1].name, "任务31");
        assert!(!rows[1].can_claim);
        assert!(rows[2].can_claim);
        assert_eq!(rows[2].category, "growth");
        let ordinary = [task_row(1, 1, 1007, 2), task_row(2, 1, 1007, 3)];
        assert!(claim_together(&ordinary));
        assert_eq!(reward_signature_for(&ordinary).unwrap(), "1007:5");
        let doubled = [task_row(1, 2, 1007, 3)];
        assert!(!claim_together(&doubled));
        assert_eq!(reward_signature_for(&doubled).unwrap(), "1007:6");
        let mixed = [task_row(1, 1, 80021, 1), task_row(2, 2, 1007, 1)];
        assert!(!claim_together(&mixed));
        assert_eq!(reward_signature_for(&mixed).unwrap(), "1007:2|80021:1");
    }

    #[test]
    fn market_exchange_follows_presence_level_and_discount() {
        let card = RotationGoodsCard {
            goods_id: 3,
            item_id: 2060001,
            item_count: 1,
            cost_item_id: 1007,
            cost_count: 5,
            discount_price: 1,
            discount_start_time: 100,
            discount_end_time: 200,
            limit_count: 2,
            buy_conds: vec![ShowCond { r#type: "最低等级".to_owned(), params: "8".to_owned() }],
            ..RotationGoodsCard::default()
        };
        let open = market_offer(&card, true, true, 8, 150);
        assert!(open.available);
        assert_eq!(open.unit_price, 1);
        assert!(!market_offer(&card, false, true, 8, 150).available);
        assert!(!market_offer(&card, true, true, 7, 150).available);
        assert_eq!(market_offer(&card, true, true, 8, 200).unit_price, 5);
        let mut sold_out = card;
        sold_out.limit_used = 2;
        let closed = market_offer(&sold_out, true, true, 8, 150);
        assert!(!closed.available);
        assert_eq!(closed.remaining, Some(0));
    }

    #[test]
    fn cans_fill_the_matching_bucket() {
        let normal = GetBagReply {
            physical_items: vec![
                RanchItem { id: 80021, uid: 4, count: 2, ..RanchItem::default() },
                RanchItem { id: 80031, uid: 5, count: 1, is_locked: true, ..RanchItem::default() },
            ],
            ..GetBagReply::default()
        };
        let plan = can_plan(&normal, 4, 1).expect("normal");
        assert_eq!(plan.bucket_id, NORMAL_BUCKET);
        assert_eq!(plan.seconds, 3600);
        assert!(can_plan(&normal, 5, 1).is_err());
        assert_eq!(can_views(&normal).len(), 1);
        let organic = GetBagReply {
            physical_items: vec![RanchItem { id: 80031, uid: 6, count: 2, ..RanchItem::default() }],
            ..GetBagReply::default()
        };
        let plan = can_plan(&organic, 6, 2).expect("organic");
        assert_eq!(plan.bucket_id, ORGANIC_BUCKET);
        assert_eq!(plan.seconds, 7200);
    }

    fn coop(state: i32, upgrade_state: i32, end: i64) -> AllCellsReply {
        AllCellsReply {
            facilities: vec![FacilityInfo {
                instance_id: 9,
                facility_id: 6001001,
                module_type: 6,
                state,
                level: 1,
                upgrade_state,
                upgrade_end_time: end,
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        }
    }

    fn nest_with(kind: i32, capacity: i32, occupied: i32) -> AllCellsReply {
        AllCellsReply {
            facilities: vec![FacilityInfo {
                instance_id: 9,
                ranch: Some(RanchDetail {
                    husbandry_type: kind,
                    capacity,
                    animal_count: occupied,
                    ..RanchDetail::default()
                }),
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        }
    }

    fn bag_with(id: i64, count: i64) -> GetBagReply {
        GetBagReply {
            physical_items: vec![crate::proto::generated::gamepb::ranchpb::Item {
                id,
                count,
                ..Default::default()
            }],
            ..GetBagReply::default()
        }
    }

    fn stall_with(animals: Vec<(i32, i64)>) -> AllCellsReply {
        AllCellsReply {
            facilities: vec![FacilityInfo {
                instance_id: 1,
                ranch: Some(RanchDetail {
                    animals: animals
                        .into_iter()
                        .map(|(instance, animal_id)| AnimalDetail {
                            animal_inst_id: instance,
                            animal_id,
                            ..AnimalDetail::default()
                        })
                        .collect(),
                    ..RanchDetail::default()
                }),
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        }
    }
}
