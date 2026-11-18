//! 雨落成诗气象研究门面。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.weather().snapshot().await.map_err(AppError::from_core)
}

pub async fn advance(ctx: &AppContext, account_id: &str, node_id: i64) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.weather().advance(node_id).await.map_err(AppError::from_core)
}

pub async fn exchange(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.weather().exchange().await.map_err(AppError::from_core)
}

pub async fn summon(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let host_gid = loop_.own_gid();
    loop_.weather().summon(host_gid).await.map_err(AppError::from_core)
}

pub async fn inspect(ctx: &AppContext, account_id: &str, host_gid: i64) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let self_gid = loop_.own_gid();
    loop_
        .weather()
        .inspect(loop_.friend().api(), host_gid, self_gid)
        .await
        .map_err(AppError::from_core)
}

pub async fn collect(ctx: &AppContext, account_id: &str, host_gid: i64) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let self_gid = loop_.own_gid();
    loop_
        .weather()
        .collect(loop_.friend().api(), host_gid, self_gid)
        .await
        .map_err(AppError::from_core)
}

pub async fn frog(ctx: &AppContext, account_id: &str, host_gid: i64) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let self_gid = loop_.own_gid();
    loop_
        .weather()
        .frog(loop_.friend().api(), host_gid, self_gid)
        .await
        .map_err(AppError::from_core)
}

pub async fn cloud(
    ctx: &AppContext,
    account_id: &str,
    host_gid: i64,
    land_id: i64,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let self_gid = loop_.own_gid();
    let land_id = (land_id > 0).then_some(land_id);
    loop_
        .weather()
        .cloud(loop_.friend().api(), host_gid, self_gid, land_id)
        .await
        .map_err(AppError::from_core)
}

pub async fn scan(ctx: &AppContext, account_id: &str, gids: &[i64]) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let self_gid = loop_.own_gid();
    loop_
        .weather()
        .scan(loop_.friend().api(), account_id, gids, self_gid)
        .await
        .map_err(AppError::from_core)
}
