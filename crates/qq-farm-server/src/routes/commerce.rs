//! Commerce 路由 — 鉴权后转发 [`qq_farm_app::commerce`]。

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok_data, AdminContext, ApiResult};
use crate::routes::resolve_account_id_required as resolve_account_id;

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
    #[serde(default, alias = "slotType")]
    slot_type: Option<i32>,
    #[serde(default, alias = "subSlotType")]
    sub_slot_type: Option<i32>,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PurchaseMallBody {
    #[serde(alias = "goodsId")]
    goods_id: i32,
    count: i32,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PurchaseMysteryBody {
    #[serde(default, alias = "npcId", alias = "offerId")]
    offer_id: serde_json::Value,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AccountQuery {
    #[serde(default)]
    account_id: Option<String>,
}

async fn get_mall(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<MallQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    match qq_farm_app::commerce::mall_catalog(&ctx.app_context(), &id, q.slot_type, q.sub_slot_type)
        .await
    {
        Ok(dto) => ok_data(dto),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn purchase_mall(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<PurchaseMallBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    match qq_farm_app::commerce::purchase_mall(&ctx.app_context(), &id, body.goods_id, body.count)
        .await
    {
        Ok(dto) => ok_data(dto),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn get_mystery_shop(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    match qq_farm_app::commerce::mystery_shop(&ctx.app_context(), &id).await {
        Ok(dto) => ok_data(dto),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn purchase_mystery(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<PurchaseMysteryBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    let offer = match &body.offer_id {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    match qq_farm_app::commerce::purchase_mystery(&ctx.app_context(), &id, &offer).await {
        Ok(dto) => ok_data(dto),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}
