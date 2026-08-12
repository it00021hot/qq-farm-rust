//! Commerce 路由 — 4 端点。
//!
//! 1:1 对应原 `controllers/admin/commerce-routes.ts`（68 行）。

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok, AdminContext, ApiError, ApiResult};

/// 构造 commerce 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/game-mall", get(get_mall))
        .route("/api/game-mall/purchase", post(purchase_mall))
        .route("/api/mystery-shop", get(get_mystery_shop))
        .route("/api/mystery-shop/purchase", post(purchase_mystery))
}

#[derive(Debug, Deserialize)]
struct MallQuery {
    #[serde(default)]
    slot_type: Option<i32>,
    #[serde(default)]
    sub_slot_type: Option<i32>,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PurchaseMallBody {
    goods_id: i32,
    count: i32,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PurchaseMysteryBody {
    offer_id: String,
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
    crate::routes::resolve_account_id_required(ctx, headers, query_id)
}

async fn get_mall(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<MallQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let mystery = qq_farm_core::services::mystery_shop::MysteryShopService::new(loop_.gateway().clone());
    let commerce = qq_farm_core::services::commerce::CommerceService::new(
        loop_.mall().clone(),
        Arc::new(mystery),
        loop_.warehouse().clone(),
    );
    let r = commerce.get_mall_catalog(q.slot_type, q.sub_slot_type).await;
    match r {
        Ok(dto) => ok(json!({ "ok": true, "mall": dto })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn purchase_mall(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<PurchaseMallBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let mystery = qq_farm_core::services::mystery_shop::MysteryShopService::new(loop_.gateway().clone());
    let commerce = qq_farm_core::services::commerce::CommerceService::new(
        loop_.mall().clone(),
        Arc::new(mystery),
        loop_.warehouse().clone(),
    );
    let r = commerce.purchase_mall_product(&body.goods_id.to_string(), &body.count.to_string()).await;
    match r {
        Ok(dto) => ok(json!({ "ok": true, "purchase": dto })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn get_mystery_shop(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let mystery = qq_farm_core::services::mystery_shop::MysteryShopService::new(loop_.gateway().clone());
    let commerce = qq_farm_core::services::commerce::CommerceService::new(
        loop_.mall().clone(),
        Arc::new(mystery),
        loop_.warehouse().clone(),
    );
    let r = commerce.get_mystery_shop().await;
    match r {
        Ok(dto) => ok(json!({ "ok": true, "mystery": dto })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn purchase_mystery(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<PurchaseMysteryBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let mystery = qq_farm_core::services::mystery_shop::MysteryShopService::new(loop_.gateway().clone());
    let commerce = qq_farm_core::services::commerce::CommerceService::new(
        loop_.mall().clone(),
        Arc::new(mystery),
        loop_.warehouse().clone(),
    );
    let r = commerce.purchase_mystery_offer(&body.offer_id).await;
    match r {
        Ok(dto) => ok(json!({ "ok": true, "purchase": dto })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

#[derive(Debug, Deserialize)]
struct AccountQuery {
    #[serde(default)]
    account_id: Option<String>,
}
