//! 宠物模块门面（pets + dog-skill-gifts）。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;
use qq_farm_core::services::dog_skill_gifts::DogSkillGiftService;
use qq_farm_core::services::pets::PetService;

/// 宠物快照（狗列表 / 狗粮 / 护主时间 / 待领礼包）。
pub async fn pet_info(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    PetService::new(loop_.gateway().clone())
        .get_pet_info()
        .await
        .map_err(AppError::from_core)
}

/// 上场宠物。
pub async fn pet_deploy(ctx: &AppContext, account_id: &str, dog_id: i64) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    PetService::new(loop_.gateway().clone())
        .deploy_dog(dog_id)
        .await
        .map_err(AppError::from_core)
}

/// 收回宠物。
pub async fn pet_withdraw(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    PetService::new(loop_.gateway().clone())
        .withdraw_dog()
        .await
        .map_err(AppError::from_core)
}

/// 使用狗粮。
pub async fn pet_food_use(
    ctx: &AppContext,
    account_id: &str,
    item_id: i64,
    count: i64,
    uid: i64,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    PetService::new(loop_.gateway().clone())
        .use_dog_food(item_id, count, uid)
        .await
        .map_err(AppError::from_core)
}

/// 守护记录。
pub async fn pet_protect_logs(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    PetService::new(loop_.gateway().clone())
        .get_protect_logs()
        .await
        .map_err(AppError::from_core)
}

/// 同气连枝礼包状态（待领数量）。
pub async fn dog_skill_gifts_status(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let svc = DogSkillGiftService::new(loop_.gateway().clone());
    let reply = svc.get_dog_info().await.map_err(AppError::from_core)?;
    Ok(serde_json::json!({ "pending": DogSkillGiftService::pending_gift_count(&reply) }))
}

/// 手动领取同气连枝礼包。
pub async fn dog_skill_gifts_claim(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let svc = DogSkillGiftService::new(loop_.gateway().clone());
    Ok(svc.check_and_claim(0).await)
}
