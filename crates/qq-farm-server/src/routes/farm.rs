//! Farm 路由 — 农场数据与操作。
//!
//! 1:1 对应原 `controllers/admin/farm-routes.ts`（不含 config / daily-gifts / account lifecycle）。
//!
//! ## 端点清单
//!
//! ```text
//! GET    /api/status                            完整状态
//! GET    /api/diamond                           钻石余额
//! POST   /api/automation                        切换单个自动化开关
//! GET    /api/lands                             土地详情
//! GET    /api/plant-blacklist                   获取植物黑名单
//! POST   /api/plant-blacklist                   添加植物黑名单
//! DELETE /api/plant-blacklist/{seed_id}         删除单个
//! POST   /api/plant-blacklist/batch             批量添加
//! DELETE /api/plant-blacklist                   清空
//! GET    /api/seeds                             可用种子
//! GET    /api/bag                               背包
//! POST   /api/bag/use                           使用物品
//! POST   /api/bag/sell                          卖物品
//! GET    /api/bag/seeds                         背包种子
//! POST   /api/farm/operate                      单次农场操作
//! GET    /api/analytics                         种植排行
//! ```

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok, ok_data, ok_empty, AdminContext, ApiError, ApiResult};
use crate::routes::resolve_id;

/// 构造 farm 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/diamond", get(get_diamond))
        .route("/api/automation", post(post_automation))
        .route("/api/lands", get(get_lands))
        .route(
            "/api/plant-blacklist",
            get(get_plant_blacklist).post(post_plant_blacklist).delete(delete_plant_blacklist),
        )
        .route("/api/plant-blacklist/batch", post(post_plant_blacklist_batch))
        .route("/api/plant-blacklist/{seed_id}", delete(delete_plant_blacklist_seed))
        .route("/api/seeds", get(get_seeds))
        .route("/api/bag", get(get_bag))
        .route("/api/bag/use", post(post_bag_use))
        .route("/api/bag/sell", post(post_bag_sell))
        .route("/api/bag/seeds", get(get_bag_seeds))
        .route("/api/farm/operate", post(post_farm_operate))
        .route("/api/analytics", get(get_analytics))
}

#[derive(Debug, Deserialize)]
struct AccountQuery {
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AutomationBody {
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: serde_json::Value,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
    #[serde(flatten)]
    rest: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct BlacklistBody {
    #[serde(alias = "seedId")]
    seed_id: i64,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BlacklistBatchBody {
    #[serde(alias = "seedIds")]
    seed_ids: Vec<i64>,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BagUseBody {
    #[serde(alias = "itemId")]
    item_id: i64,
    count: i64,
    #[serde(default)]
    uid: i64,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BagSellBody {
    items: Vec<SellItemBody>,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SellItemBody {
    id: i64,
    count: i64,
    #[serde(default)]
    uid: i64,
}

#[derive(Debug, Deserialize)]
struct FarmOperateBody {
    #[serde(default, alias = "opType")]
    op: String,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnalyticsQuery {
    #[serde(default, alias = "sortBy", alias = "sort")]
    sort_by: Option<String>,
}

async fn get_status(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    if id.is_empty() {
        return Ok(Json(json!({ "ok": false, "error": "Missing x-account-id" })));
    }
    let data = qq_farm_app::farm::panel_status_with_progress(&ctx.app_context(), &id);
    ok_data(data)
}

async fn get_diamond(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    match qq_farm_app::farm::diamond_balance(&ctx.app_context(), &id).await {
        Ok(balance) => ok_data(json!({ "diamond": balance.max(0) })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn post_automation(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<AutomationBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, body.account_id.as_deref())?;
    if id.is_empty() {
        return Err(ApiError::BadRequest("Missing x-account-id".to_string()));
    }
    let extra = serde_json::Value::Object(body.rest);
    let data =
        qq_farm_app::farm::set_automation(&ctx.app_context(), &id, &body.key, body.value, extra)?;
    ok_data(data)
}

async fn get_lands(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    match qq_farm_app::farm::lands(&ctx.app_context(), &id).await {
        Ok(data) => ok_data(data),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn get_plant_blacklist(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    ok_data(qq_farm_app::farm::plant_blacklist(&id))
}

async fn post_plant_blacklist(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BlacklistBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, body.account_id.as_deref())?;
    ok_data(qq_farm_app::farm::add_plant_blacklist(&id, body.seed_id))
}

async fn post_plant_blacklist_batch(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BlacklistBatchBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, body.account_id.as_deref())?;
    ok_data(qq_farm_app::farm::set_plant_blacklist(&id, body.seed_ids))
}

async fn delete_plant_blacklist_seed(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(seed_id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, None)?;
    ok_data(qq_farm_app::farm::remove_plant_blacklist(&id, seed_id))
}

async fn delete_plant_blacklist(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, None)?;
    let _ = qq_farm_app::farm::set_plant_blacklist(&id, vec![]);
    ok_empty()
}

async fn get_seeds(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let _id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    let _ = ctx;
    ok_data(qq_farm_app::farm::seeds_catalog())
}

async fn get_bag(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    match qq_farm_app::farm::bag(&ctx.app_context(), &id).await {
        Ok(d) => ok_data(d),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn post_bag_use(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BagUseBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, body.account_id.as_deref())?;
    match qq_farm_app::farm::bag_use(
        &ctx.app_context(),
        &id,
        body.item_id,
        body.count.max(1),
        body.uid,
    )
    .await
    {
        Ok(()) => ok(json!({ "ok": true })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn post_bag_sell(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BagSellBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, body.account_id.as_deref())?;
    let sell: Vec<(i64, i64, i64)> = body.items.iter().map(|i| (i.id, i.count, i.uid)).collect();
    match qq_farm_app::farm::bag_sell(&ctx.app_context(), &id, &sell).await {
        Ok(()) => ok(json!({ "ok": true })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn get_bag_seeds(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    match qq_farm_app::farm::bag_seeds(&ctx.app_context(), &id).await {
        Ok(seeds) => ok_data(seeds),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn post_farm_operate(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<FarmOperateBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, body.account_id.as_deref())?;
    let op = body.op.to_lowercase();
    match qq_farm_app::farm::operate(&ctx.app_context(), &id, &op).await {
        Ok(v) => Ok(Json(v)),
        Err(e) => Ok(Json(json!({ "ok": false, "op": op, "error": e.to_string() }))),
    }
}

async fn get_analytics(
    State(_ctx): State<Arc<AdminContext>>,
    Query(q): Query<AnalyticsQuery>,
) -> ApiResult<serde_json::Value> {
    ok_data(qq_farm_app::farm::analytics(q.sort_by.as_deref()))
}
