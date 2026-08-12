//! Friend 路由 — 12 端点。
//!
//! 1:1 对应原 `controllers/admin/friend-routes.ts`（378 行）。

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok, AdminContext, ApiError, ApiResult};

/// 构造 friend 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/friends", get(get_friends))
        .route("/api/friends/clear-cache", post(clear_friends_cache))
        .route("/api/interact-records", get(get_interact_records))
        .route("/api/friend/{gid}/lands", get(get_friend_lands))
        .route("/api/friend/{gid}/op", post(do_friend_op))
        .route("/api/friend-blacklist", get(get_friend_blacklist).post(toggle_friend_blacklist))
        .route("/api/friend-known-gids", get(get_friend_known_gids).post(post_friend_known_gids))
        .route("/api/friend-known-gids/remove", post(remove_friend_known_gid))
        .route("/api/friend-known-gids/batch-add", post(batch_add_friend_known_gids))
        .route("/api/friend-known-gids/batch-remove", post(batch_remove_friend_known_gids))
}

#[derive(Debug, Deserialize)]
struct AccountQuery {
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpBody {
    op: String,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ToggleBody {
    gid: i64,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KnownGidBody {
    gid: i64,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BatchKnownGidsBody {
    gids: Vec<i64>,
    #[serde(default)]
    account_id: Option<String>,
}

fn resolve_account_id(
    ctx: &AdminContext,
    headers: &axum::http::HeaderMap,
    query_id: Option<&str>,
) -> Result<String, ApiError> {
    crate::routes::resolve_account_id_required(ctx, headers, query_id)
}

fn get_loop(
    ctx: &AdminContext,
    account_id: &str,
) -> Result<Arc<qq_farm_core::runtime::worker_loop::WorkerLoop>, ApiError> {
    ctx.engine
        .worker_loop(account_id)
        .ok_or_else(|| ApiError::NotFound(format!("worker not running: {account_id}")))
}

async fn get_friends(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let force = q.account_id.as_deref().map(|s| s == "__force__").unwrap_or(false);
    let _ = force;
    let friends = loop_.friend().get_friends_list(true).await;
    match friends {
        Ok(f) => ok(json!({ "ok": true, "friends": f })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn clear_friends_cache(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    qq_farm_core::services::friend::scheduler::FriendService::clear_friends_list_cache(loop_.friend().as_ref());
    ok(json!({ "ok": true }))
}

async fn get_interact_records(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let records = qq_farm_core::services::interact::InteractService::new(loop_.gateway().clone())
        .get_interact_records()
        .await;
    match records {
        Ok(r) => ok(json!({ "ok": true, "records": r })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn get_friend_lands(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(gid): Path<i64>,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let lands = loop_.friend().get_friend_lands_detail(gid).await;
    match lands {
        Ok(l) => ok(json!({ "ok": true, "lands": l })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn do_friend_op(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(gid): Path<i64>,
    Json(body): Json<OpBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref().or(qid(&headers)))?;
    let loop_ = get_loop(&ctx, &id)?;
    let op = qq_farm_core::models::types::FriendOperation::from_str_opt(&body.op)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown op: {}", body.op)))?;
    let r = loop_.friend().do_friend_operation(op, gid).await;
    match r {
        Ok(ret) => ok(json!({ "ok": true, "result": ret })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

fn qid(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers.get("x-account-id").and_then(|v| v.to_str().ok())
}

async fn get_friend_blacklist(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    let list = qq_farm_core::models::store::account_config::get_friend_blacklist(Some(&id));
    ok(json!({ "ok": true, "blacklist": list }))
}

async fn toggle_friend_blacklist(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<ToggleBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    let list = qq_farm_core::models::store::account_config::toggle_friend_blacklist(
        &id,
        body.gid,
    );
    ok(json!({ "ok": true, "blacklist": list }))
}

async fn get_friend_known_gids(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    let list = qq_farm_core::models::store::account_config::get_known_friend_gids(Some(&id));
    ok(json!({ "ok": true, "knownGids": list }))
}

async fn post_friend_known_gids(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<KnownGidBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    let list = qq_farm_core::models::store::account_config::add_known_friend_gid(&id, body.gid);
    ok(json!({ "ok": true, "knownGids": list }))
}

async fn remove_friend_known_gid(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<KnownGidBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    let list = qq_farm_core::models::store::account_config::remove_known_friend_gid(&id, body.gid);
    ok(json!({ "ok": true, "knownGids": list }))
}

async fn batch_add_friend_known_gids(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BatchKnownGidsBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    let list = qq_farm_core::models::store::account_config::add_known_friend_gids(&id, &body.gids);
    ok(json!({ "ok": true, "knownGids": list }))
}

async fn batch_remove_friend_known_gids(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BatchKnownGidsBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    let list = qq_farm_core::models::store::account_config::remove_known_friend_gids(&id, &body.gids);
    ok(json!({ "ok": true, "knownGids": list }))
}
