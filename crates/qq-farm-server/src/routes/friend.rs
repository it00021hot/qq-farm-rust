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

use crate::context::{ok, ok_data, AdminContext, ApiError, ApiResult};
use crate::routes::{get_loop, resolve_account_id_required as resolve_account_id};

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

async fn get_friends(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let force = q
        .force_sync
        .as_deref()
        .map(|s| s == "true" || s == "1")
        .unwrap_or(false);
    let friends = loop_.friend().get_friends_list(force).await;
    match friends {
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
    let loop_ = get_loop(&ctx, &id)?;
    let lands = loop_.friend().get_friend_lands_detail(gid).await;
    match lands {
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
    let loop_ = get_loop(&ctx, &id)?;
    let op = qq_farm_core::models::types::FriendOperation::from_str_opt(&body.op)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown op: {}", body.op)))?;
    let r = loop_.friend().do_friend_operation(op, gid).await;
    match r {
        Ok(ret) => {
            let stolen = ret.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
            if matches!(op, qq_farm_core::models::types::FriendOperation::Steal) && stolen > 0 {
                let _ = loop_.warehouse().sell_all_fruits().await;
            }
            ok_data(ret)
        }
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
    ok_data(list)
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
    ok_data(list)
}

fn known_gid_settings(account_id: &str) -> serde_json::Value {
    json!({
        "knownFriendGids": qq_farm_core::models::store::account_config::get_known_friend_gids(Some(account_id)),
        "knownFriendGidSyncCooldownSec": qq_farm_core::models::store::account_config::get_known_friend_gid_sync_cooldown_sec(Some(account_id)),
        "friendsListCacheTtlSec": qq_farm_core::models::store::account_config::get_friends_list_cache_ttl_sec(Some(account_id)),
    })
}

async fn get_friend_known_gids(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    ok_data(known_gid_settings(&id))
}

async fn post_friend_known_gids(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<KnownGidBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    if let Some(gids) = body.known_friend_gids {
        qq_farm_core::models::store::account_config::set_known_friend_gids(&id, gids);
    } else if let Some(gid) = body.gid {
        qq_farm_core::models::store::account_config::add_known_friend_gid(&id, gid);
    }
    if let Some(sec) = body.known_friend_gid_sync_cooldown_sec {
        qq_farm_core::models::store::account_config::set_known_friend_gid_sync_cooldown_sec(&id, sec);
    }
    if let Some(sec) = body.friends_list_cache_ttl_sec {
        qq_farm_core::models::store::account_config::set_friends_list_cache_ttl_sec(&id, sec);
    }
    ok_data(known_gid_settings(&id))
}

async fn remove_friend_known_gid(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<KnownGidBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    let gid = body.gid.unwrap_or(0);
    let _ = qq_farm_core::models::store::account_config::remove_known_friend_gid(&id, gid);
    ok_data(known_gid_settings(&id))
}

async fn batch_add_friend_known_gids(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BatchKnownGidsBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    let _list = qq_farm_core::models::store::account_config::add_known_friend_gids(&id, &body.gids);
    ok_data(known_gid_settings(&id))
}

async fn batch_remove_friend_known_gids(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BatchKnownGidsBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    let _list = qq_farm_core::models::store::account_config::remove_known_friend_gids(&id, &body.gids);
    ok_data(known_gid_settings(&id))
}
