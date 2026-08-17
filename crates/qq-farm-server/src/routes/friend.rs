//! Friend 路由 — 鉴权后转发 [`qq_farm_app::friend`]。

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok, ok_data, AdminContext, ApiResult};
use crate::routes::resolve_account_id_required as resolve_account_id;

/// 构造 friend 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/friends", get(get_friends))
        .route("/api/friends/clear-cache", post(clear_friends_cache))
        .route("/api/interact-records", get(get_interact_records))
        .route("/api/friend/{gid}/lands", get(get_friend_lands))
        .route("/api/friend/{gid}/op", post(do_friend_op))
        .route("/api/friend-blacklist", get(get_friend_blacklist).post(toggle_friend_blacklist))
        .route("/api/friend-blacklist/toggle", post(toggle_friend_blacklist))
        .route("/api/friend-known-gids", get(get_friend_known_gids).post(post_friend_known_gids))
        .route("/api/friend-known-gids/remove", post(remove_friend_known_gid))
        .route("/api/friend-known-gids/batch-add", post(batch_add_friend_known_gids))
        .route("/api/friend-known-gids/batch-remove", post(batch_remove_friend_known_gids))
}

#[derive(Debug, Deserialize)]
struct AccountQuery {
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
    #[serde(default, alias = "forceSync")]
    force_sync: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpBody {
    #[serde(default, alias = "opType")]
    op: String,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ToggleBody {
    gid: i64,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KnownGidBody {
    #[serde(default)]
    gid: Option<i64>,
    #[serde(default, alias = "knownFriendGids")]
    known_friend_gids: Option<Vec<i64>>,
    #[serde(default, alias = "knownFriendGidSyncCooldownSec")]
    known_friend_gid_sync_cooldown_sec: Option<i64>,
    #[serde(default, alias = "friendsListCacheTtlSec")]
    friends_list_cache_ttl_sec: Option<i64>,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BatchKnownGidsBody {
    gids: Vec<i64>,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

fn qid(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers.get("x-account-id").and_then(|v| v.to_str().ok())
}

async fn get_friends(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    let force = q
        .force_sync
        .as_deref()
        .map(|s| s == "true" || s == "1")
        .unwrap_or(false);
    match qq_farm_app::friend::list_friends(&ctx.app_context(), &id, force).await {
        Ok(f) => ok_data(f),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn clear_friends_cache(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    qq_farm_app::friend::clear_friends_cache(&ctx.app_context(), &id)?;
    ok(json!({ "ok": true }))
}

async fn get_interact_records(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    match qq_farm_app::friend::interact_records(&ctx.app_context(), &id).await {
        Ok(r) => ok_data(r),
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
    match qq_farm_app::friend::friend_lands(&ctx.app_context(), &id, gid).await {
        Ok(l) => ok_data(l),
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
    match qq_farm_app::friend::friend_op(&ctx.app_context(), &id, gid, &body.op).await {
        Ok(ret) => ok_data(ret),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn get_friend_blacklist(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    ok_data(qq_farm_app::friend::friend_blacklist(&id))
}

async fn toggle_friend_blacklist(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<ToggleBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    ok_data(qq_farm_app::friend::toggle_friend_blacklist(&id, body.gid))
}

async fn get_friend_known_gids(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    ok_data(qq_farm_app::friend::known_gid_settings(&id))
}

async fn post_friend_known_gids(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<KnownGidBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    if let Some(gids) = body.known_friend_gids {
        qq_farm_app::friend::set_known_gids(&id, gids);
    } else if let Some(gid) = body.gid {
        qq_farm_app::friend::add_known_gid(&id, gid);
    }
    ok_data(qq_farm_app::friend::set_known_gid_cooldowns(
        &id,
        body.known_friend_gid_sync_cooldown_sec,
        body.friends_list_cache_ttl_sec,
    ))
}

async fn remove_friend_known_gid(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<KnownGidBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    ok_data(qq_farm_app::friend::remove_known_gid(&id, body.gid.unwrap_or(0)))
}

async fn batch_add_friend_known_gids(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BatchKnownGidsBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    ok_data(qq_farm_app::friend::batch_add_known_gids(&id, &body.gids))
}

async fn batch_remove_friend_known_gids(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BatchKnownGidsBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    ok_data(qq_farm_app::friend::batch_remove_known_gids(&id, &body.gids))
}
