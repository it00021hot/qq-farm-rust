//! 雨落成诗气象研究门面。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::weather::WeatherService::new(loop_.gateway().clone())
        .snapshot()
        .await
        .map_err(AppError::from_core)
}

pub async fn advance(ctx: &AppContext, account_id: &str, node_id: i64) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::weather::WeatherService::new(loop_.gateway().clone())
        .advance(node_id)
        .await
        .map_err(AppError::from_core)
}

pub async fn exchange(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::weather::WeatherService::new(loop_.gateway().clone())
        .exchange()
        .await
        .map_err(AppError::from_core)
}

pub async fn summon(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let host_gid = loop_.own_gid();
    qq_farm_core::services::weather::WeatherService::new(loop_.gateway().clone())
        .summon(host_gid)
        .await
        .map_err(AppError::from_core)
}

pub async fn inspect(ctx: &AppContext, account_id: &str, host_gid: i64) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let self_gid = loop_.own_gid();
    qq_farm_core::services::weather::WeatherService::new(loop_.gateway().clone())
        .inspect(loop_.friend().api(), host_gid, self_gid)
        .await
        .map_err(AppError::from_core)
}

pub async fn collect(ctx: &AppContext, account_id: &str, host_gid: i64) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let self_gid = loop_.own_gid();
    qq_farm_core::services::weather::WeatherService::new(loop_.gateway().clone())
        .collect(loop_.friend().api(), host_gid, self_gid)
        .await
        .map_err(AppError::from_core)
}
