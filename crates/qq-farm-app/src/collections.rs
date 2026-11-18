//! 藏品宝典门面。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

/// 变异宝典。只读本地配置，不需要在线账号。
#[must_use]
pub fn mutation_catalog() -> Value {
    qq_farm_core::services::collections::mutation_catalog()
}

/// 蓝晶藏品快照。
pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::collections::CollectionService::new(loop_.gateway().clone())
        .snapshot()
        .await
        .map_err(AppError::from_core)
}

/// 激活或升星。
pub async fn upgrade(ctx: &AppContext, account_id: &str, input: &Value) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::collections::CollectionService::new(loop_.gateway().clone())
        .upgrade(input)
        .await
        .map_err(AppError::from_core)
}

/// 展示或撤下珍藏室藏品。
pub async fn display(ctx: &AppContext, account_id: &str, input: &Value) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::collections::CollectionService::new(loop_.gateway().clone())
        .display(input)
        .await
        .map_err(AppError::from_core)
}

/// 领取已达到的藏品积分奖励。
pub async fn claim(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::collections::CollectionService::new(loop_.gateway().clone())
        .claim()
        .await
        .map_err(AppError::from_core)
}
