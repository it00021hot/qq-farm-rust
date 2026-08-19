//! 商城 / 神秘商人门面。

use std::sync::Arc;

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

fn commerce_from_loop(
    loop_: &qq_farm_core::runtime::worker_loop::WorkerLoop,
) -> qq_farm_core::services::commerce::CommerceService {
    let mystery =
        qq_farm_core::services::mystery_shop::MysteryShopService::new(loop_.gateway().clone());
    qq_farm_core::services::commerce::CommerceService::new(
        loop_.mall().clone(),
        Arc::new(mystery),
        loop_.warehouse().clone(),
    )
}

/// 游戏商城目录。
pub async fn mall_catalog(
    ctx: &AppContext,
    account_id: &str,
    slot_type: Option<i32>,
    sub_slot_type: Option<i32>,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let commerce = commerce_from_loop(loop_.as_ref());
    let dto =
        commerce.get_mall_catalog(slot_type, sub_slot_type).await.map_err(AppError::from_core)?;
    serde_json::to_value(dto).map_err(|e| AppError::Internal(e.to_string()))
}

/// 购买商城商品。
pub async fn purchase_mall(
    ctx: &AppContext,
    account_id: &str,
    goods_id: i32,
    count: i32,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let commerce = commerce_from_loop(loop_.as_ref());
    let dto = commerce
        .purchase_mall_product(&goods_id.to_string(), &count.to_string())
        .await
        .map_err(AppError::from_core)?;
    serde_json::to_value(dto).map_err(|e| AppError::Internal(e.to_string()))
}

/// 神秘商人。
pub async fn mystery_shop(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let commerce = commerce_from_loop(loop_.as_ref());
    let dto = commerce.get_mystery_shop().await.map_err(AppError::from_core)?;
    serde_json::to_value(dto).map_err(|e| AppError::Internal(e.to_string()))
}

/// 购买神秘商人商品。
pub async fn purchase_mystery(
    ctx: &AppContext,
    account_id: &str,
    offer_id: &str,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let commerce = commerce_from_loop(loop_.as_ref());
    let dto = commerce.purchase_mystery_offer(offer_id).await.map_err(AppError::from_core)?;
    serde_json::to_value(dto).map_err(|e| AppError::Internal(e.to_string()))
}
