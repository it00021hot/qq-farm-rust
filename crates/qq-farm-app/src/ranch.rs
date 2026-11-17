//! 牧场面板。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

/// 当前账号的牧场设施和动物。
pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::snapshot(worker.gateway(), account_id)
        .await
        .map_err(AppError::from_core)
}

/// 让指定动物生产；两个编号都缺省时生产全部可生产的动物。
pub async fn produce(
    ctx: &AppContext,
    account_id: &str,
    ranch_id: Option<i64>,
    animal_id: Option<i32>,
) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::produce(worker.gateway(), account_id, ranch_id, animal_id)
        .await
        .map_err(AppError::from_core)
}

/// 用罐头桶喂养。`bucket_type` 为 1 时使用有机罐头，为 2 时使用普通罐头。
pub async fn feed(
    ctx: &AppContext,
    account_id: &str,
    ranch_id: Option<i64>,
    animal_id: Option<i32>,
    bucket_type: i32,
) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::feed(
        worker.gateway(),
        account_id,
        ranch_id,
        animal_id,
        bucket_type,
    )
    .await
    .map_err(AppError::from_core)
}

/// 清理当前牧场里的全部障碍。
pub async fn clean(ctx: &AppContext, account_id: &str) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::clean(worker.gateway(), account_id)
        .await
        .map_err(AppError::from_core)
}

/// 向指定牧草机加入牧草。
pub async fn add_grass(
    ctx: &AppContext,
    account_id: &str,
    facility_id: i64,
    count: i32,
) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::add_grass(worker.gateway(), account_id, facility_id, count)
        .await
        .map_err(AppError::from_core)
}

/// 收获指定动物；两个编号都缺省时收获全部可收获内容。
pub async fn harvest(
    ctx: &AppContext,
    account_id: &str,
    ranch_id: Option<i64>,
    animal_id: Option<i32>,
) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::harvest(worker.gateway(), account_id, ranch_id, animal_id)
        .await
        .map(|_| ())
        .map_err(AppError::from_core)
}

/// 把背包里的幼崽放进指定窝或棚。
pub async fn place(
    ctx: &AppContext,
    account_id: &str,
    ranch_id: i64,
    item_id: i64,
    count: i64,
) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::place(worker.gateway(), account_id, ranch_id, item_id, count)
        .await
        .map_err(AppError::from_core)
}

/// 核对后解除一条结果未知的牧场扣费保护。
pub fn acknowledge(account_id: &str, id: &str) -> AppResult<()> {
    qq_farm_core::services::ranch_ops::acknowledge(account_id, id).map_err(AppError::from_core)?;
    qq_farm_core::services::ranch::reset_patrol_schedule(account_id);
    Ok(())
}

/// 开始升级一座牧场设施。
pub async fn start_upgrade(ctx: &AppContext, account_id: &str, facility_id: i64) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::start_upgrade(worker.gateway(), account_id, facility_id)
        .await
        .map_err(AppError::from_core)
}

/// 完成一座已经到期的牧场设施升级。
pub async fn finish_upgrade(ctx: &AppContext, account_id: &str, facility_id: i64) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::finish_upgrade(worker.gateway(), account_id, facility_id)
        .await
        .map_err(AppError::from_core)
}

/// 放生一只允许放生的动物。
pub async fn release(
    ctx: &AppContext,
    account_id: &str,
    ranch_id: i64,
    animal_inst_id: i32,
) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::release(worker.gateway(), account_id, ranch_id, animal_inst_id)
        .await
        .map_err(AppError::from_core)
}

/// 牧场集市货架。
pub async fn market(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::market(worker.gateway(), account_id)
        .await
        .map_err(AppError::from_core)
}

/// 按当前集市标价兑换商品。
pub async fn exchange(
    ctx: &AppContext,
    account_id: &str,
    goods_id: i64,
    count: i64,
) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::exchange(worker.gateway(), account_id, goods_id, count)
        .await
        .map_err(AppError::from_core)
}

/// 牧场罐头商城。`slot` 为 6 时是常驻商品，为 7 时是限时商品。
pub async fn can_shop(ctx: &AppContext, account_id: &str, slot: i32) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::can_shop(worker.gateway(), account_id, slot)
        .await
        .map_err(AppError::from_core)
}

/// 按当前罐头商城标价购买或领取商品。
pub async fn buy_can(
    ctx: &AppContext,
    account_id: &str,
    slot: i32,
    goods_id: i64,
    count: i64,
) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::buy_can(worker.gateway(), account_id, slot, goods_id, count)
        .await
        .map_err(AppError::from_core)
}

/// 牧场成长任务和每日任务。
pub async fn tasks(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::tasks(worker.gateway(), account_id)
        .await
        .map_err(AppError::from_core)
}

/// 领取一条已完成的牧场任务。
pub async fn claim_task(ctx: &AppContext, account_id: &str, task_id: i64) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::claim_task(worker.gateway(), account_id, task_id)
        .await
        .map_err(AppError::from_core)
}

/// 牧场图鉴和巨大图鉴。
pub async fn book(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::book(worker.gateway(), account_id)
        .await
        .map_err(AppError::from_core)
}

/// 领取当前可领的普通牧场图鉴奖励。
pub async fn claim_book(ctx: &AppContext, account_id: &str) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::claim_book(worker.gateway(), account_id)
        .await
        .map_err(AppError::from_core)
}

/// 领取当前可领的全部巨大图鉴档位。
pub async fn claim_giant_book(ctx: &AppContext, account_id: &str) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::claim_giant_book(worker.gateway(), account_id)
        .await
        .map_err(AppError::from_core)
}

/// 把背包里的一罐罐头加入对应罐头桶。
pub async fn use_can(ctx: &AppContext, account_id: &str, uid: i64, count: i64) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::use_can(worker.gateway(), account_id, uid, count)
        .await
        .map_err(AppError::from_core)
}

/// 出售背包里的一批可售产物。
pub async fn sell_stack(ctx: &AppContext, account_id: &str, uid: i64, count: i64) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::sell_stack(worker.gateway(), account_id, uid, count)
        .await
        .map_err(AppError::from_core)
}

/// 出售当前可售的全部产物。
pub async fn sell_all(ctx: &AppContext, account_id: &str) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::sell_all(worker.gateway(), account_id)
        .await
        .map_err(AppError::from_core)
}

/// 幼崽商店（5）或饲料商店（6）。
pub async fn shop(ctx: &AppContext, account_id: &str, shop_id: i64) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::shop(worker.gateway(), account_id, shop_id)
        .await
        .map_err(AppError::from_core)
}

/// 按当前标价购买一份牧场商品。
pub async fn buy_goods(
    ctx: &AppContext,
    account_id: &str,
    shop_id: i64,
    goods_id: i64,
    count: i64,
) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::buy_goods(worker.gateway(), account_id, shop_id, goods_id, count)
        .await
        .map_err(AppError::from_core)
}

/// 购买幼崽并放入对应的窝或棚。
pub async fn buy_and_place(
    ctx: &AppContext,
    account_id: &str,
    shop_id: i64,
    goods_id: i64,
    count: i64,
) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch::buy_and_place(
        worker.gateway(),
        account_id,
        shop_id,
        goods_id,
        count,
    )
    .await
    .map_err(AppError::from_core)
}

/// 今天是否还能领牧场每日分享。
pub async fn daily_share(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch_share::daily_share(worker.gateway(), account_id)
        .await
        .map_err(AppError::from_core)
}

/// 领取牧场每日分享的 5 个普通罐头。
pub async fn claim_daily_share(ctx: &AppContext, account_id: &str) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch_share::claim_daily_share(worker.gateway(), account_id)
        .await
        .map_err(AppError::from_core)
}

/// 好友牧场当前可操作的栏舍。
pub async fn friend_ranch(ctx: &AppContext, account_id: &str, host: i64) -> AppResult<Value> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch_friend::friend_ranch(
        worker.gateway(),
        account_id,
        host,
        worker.own_gid(),
    )
    .await
    .map_err(AppError::from_core)
}

/// 偷取好友当前可偷的产物。
pub async fn steal_friend(ctx: &AppContext, account_id: &str, host: i64) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch_friend::steal_friend(
        worker.gateway(),
        account_id,
        host,
        worker.own_gid(),
    )
    .await
    .map_err(AppError::from_core)
}

/// 抚摸好友栏舍里的一只动物。
pub async fn stroke_friend(
    ctx: &AppContext,
    account_id: &str,
    host: i64,
    ranch_id: i64,
    animal_inst_id: i32,
) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch_friend::stroke_friend(
        worker.gateway(),
        account_id,
        host,
        worker.own_gid(),
        ranch_id,
        animal_inst_id,
    )
    .await
    .map_err(AppError::from_core)
}

/// 向好友栏舍放一只蚊子。
pub async fn mosquito_friend(
    ctx: &AppContext,
    account_id: &str,
    host: i64,
    ranch_id: i64,
) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch_friend::mosquito_friend(
        worker.gateway(),
        account_id,
        host,
        worker.own_gid(),
        ranch_id,
    )
    .await
    .map_err(AppError::from_core)
}

/// 清理好友栏舍里不是自己放的障碍。
pub async fn clean_friend(ctx: &AppContext, account_id: &str, host: i64) -> AppResult<()> {
    let worker = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch_friend::clean_friend(
        worker.gateway(),
        account_id,
        host,
        worker.own_gid(),
    )
    .await
    .map_err(AppError::from_core)
}
