//! Activity Center 路由 — 4 端点。
//!
//! 1:1 对应原 `controllers/admin/activity-center-routes.ts`（126 行）。

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok, AdminContext, ApiError, ApiResult};

/// 构造 activity-center 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/activity-center", get(get_snapshot))
        .route("/api/activity-center/season", get(get_season))
        .route("/api/activity-center/pass/claim", post(claim_battle_pass))
        .route("/api/activity-center/constellation/light", post(light_constellation))
        .route("/api/activity-center/shop/exchange", post(exchange_star_sand))
        .route("/api/activity-center/solar-terms/{term_id}/claim", post(claim_solar_term))
}

#[derive(Debug, Deserialize)]
struct AccountQuery {
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExchangeBody {
    goods_id: i64,
    count: i32,
    #[serde(default)]
    account_id: Option<String>,
}

fn get_loop(
    ctx: &AdminContext,
    account_id: &str,
) -> Result<Arc<qq_farm_core::runtime::worker_loop::WorkerLoop>, ApiError> {
    ctx.engine
        .worker_loop(account_id)
        .ok_or_else(|| ApiError::NotFound(format!("worker not running: {account_id}")))
}

fn resolve_account_id(
    ctx: &AdminContext,
    headers: &axum::http::HeaderMap,
    query_id: Option<&str>,
) -> Result<String, ApiError> {
    if let Some(id) = query_id {
        if !id.is_empty() {
            return Ok(id.to_string());
        }
    }
    if let Some(v) = headers.get("x-account-id").and_then(|v| v.to_str().ok()) {
        if !v.is_empty() {
            return Ok(v.to_string());
        }
    }
    if let Ok(v) = std::env::var("FARM_ACCOUNT_ID") {
        if !v.is_empty() {
            return Ok(v);
        }
    }
    let _ = ctx;
    Err(ApiError::BadRequest("missing x-account-id".to_string()))
}

async fn get_snapshot(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let snap = loop_.activity_center().get_activity_center_snapshot().await;
    match snap {
        Ok(s) => ok(json!({ "ok": true, "snapshot": s })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn get_season(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let season = loop_.activity_center().get_current_season_event().await;
    match season {
        Ok(s) => ok(json!({ "ok": true, "season": s })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn claim_battle_pass(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, None)?;
    let loop_ = get_loop(&ctx, &id)?;
    let r = loop_.activity_center().claim_battle_pass_rewards().await;
    match r {
        Ok(_) => ok(json!({ "ok": true })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn light_constellation(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, None)?;
    let loop_ = get_loop(&ctx, &id)?;
    let r = loop_.activity_center().light_constellation().await;
    match r {
        Ok(_) => ok(json!({ "ok": true })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn exchange_star_sand(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<ExchangeBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let goods_id = body.goods_id.to_string();
    let count = body.count.to_string();
    let r = loop_.activity_center().exchange_star_sand_goods(loop_.warehouse().as_ref(), &goods_id, &count).await;
    match r {
        Ok(_) => ok(json!({ "ok": true })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn claim_solar_term(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(term_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, None)?;
    let loop_ = get_loop(&ctx, &id)?;
    let r = loop_.activity_center().claim_solar_term(&term_id).await;
    match r {
        Ok(_) => ok(json!({ "ok": true })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

use axum::extract::Query;
