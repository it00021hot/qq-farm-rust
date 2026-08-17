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
//! POST   /api/fertilizer/buy                    立即买化肥
//! POST   /api/fertilizer/check-and-buy          检查并买化肥
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
use qq_farm_core::services::analytics::SortBy;

use crate::context::{ok, ok_data, ok_empty, AdminContext, ApiError, ApiResult};
use crate::routes::{get_loop, resolve_id};

/// 构造 farm 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/diamond", get(get_diamond))
        .route("/api/automation", post(post_automation))
        .route("/api/fertilizer/buy", post(post_fertilizer_buy))
        .route("/api/fertilizer/check-and-buy", post(post_fertilizer_check_and_buy))
        .route("/api/lands", get(get_lands))
        .route("/api/plant-blacklist", get(get_plant_blacklist).post(post_plant_blacklist).delete(delete_plant_blacklist))
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
struct FertilizerBuyBody {
    #[serde(default, alias = "fertilizerType")]
    fertilizer_type: Option<String>,
    count: i64,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FertilizerCheckBuyBody {
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
    #[serde(default)]
    force: Option<bool>,
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
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
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
    let loop_ = get_loop(&ctx, &id)?;
    let gw = loop_.gateway();
    let result: Result<i64, String> = qq_farm_core::services::pay::PayService::new(gw.clone())
        .get_diamond_balance()
        .await
        .map_err(|e| e.to_string());
    match result {
        Ok(balance) => ok_data(json!({ "diamond": balance.max(0) })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e }))),
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
    let mut auto = serde_json::Map::new();
    if !body.key.is_empty() {
        auto.insert(body.key, body.value);
    }
    for (k, v) in body.rest {
        if k == "accountId" || k == "account_id" {
            continue;
        }
        auto.insert(k, v);
    }
    let snapshot = json!({ "automation": auto });
    qq_farm_core::models::store::account_config::apply_config_snapshot(snapshot, Some(&id), true);
    ctx.engine.reload_worker_config(&id);
    let cur = qq_farm_core::models::store::account_config::get_automation(Some(&id));
    ok_data(cur)
}

async fn post_fertilizer_buy(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<FertilizerBuyBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, body.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let mall = loop_.mall();
    let count = body.count as i32;
    let kind = match body.fertilizer_type.as_deref().unwrap_or("organic") {
        "normal" => qq_farm_core::services::mall::MallFertilizerKind::Normal,
        _ => qq_farm_core::services::mall::MallFertilizerKind::Organic,
    };
    let result = mall.auto_buy_fertilizer(true, kind, count).await;
    ok(json!({ "ok": true, "bought": result }))
}

async fn post_fertilizer_check_and_buy(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<FertilizerCheckBuyBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, body.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let mystery = qq_farm_core::services::mystery_shop::MysteryShopService::new(loop_.gateway().clone());
    let commerce = qq_farm_core::services::commerce::CommerceService::new(
        loop_.mall().clone(),
        Arc::new(mystery),
        loop_.warehouse().clone(),
    );
    let opts = qq_farm_core::services::commerce::FertilizerBothOptions {
        buy_organic: true,
        buy_normal: true,
        organic_count: 0,
        organic_threshold_hours: 0.0,
        normal_count: 0,
        normal_threshold_hours: 0.0,
    };
    let summary = commerce.check_and_buy_fertilizer_both(opts).await;
    let _ = body.force;
    let _ = id;
    Ok(Json(json!({
        "ok": true,
        "organicBought": summary.organic_bought,
        "normalBought": summary.normal_bought,
        "organicCurrentHours": summary.organic_current_hours,
        "normalCurrentHours": summary.normal_current_hours,
        "error": summary.error,
    })))
}

async fn get_lands(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let result = loop_.farm().get_lands_detail().await;
    match result {
        Ok((lands, summary)) => ok_data(json!({
            "lands": lands,
            "summary": summary,
        })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn get_plant_blacklist(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    let list = qq_farm_core::models::store::account_config::get_plant_blacklist(Some(id.as_str()).filter(|s| !s.is_empty()));
    ok_data(list)
}

async fn post_plant_blacklist(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BlacklistBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, body.account_id.as_deref())?;
    let mut current = qq_farm_core::models::store::account_config::get_plant_blacklist(Some(&id));
    if !current.contains(&body.seed_id) {
        current.push(body.seed_id);
    }
    let list = qq_farm_core::models::store::account_config::set_plant_blacklist(&id, current);
    ok_data(list)
}

async fn post_plant_blacklist_batch(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BlacklistBatchBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, body.account_id.as_deref())?;
    let list = qq_farm_core::models::store::account_config::set_plant_blacklist(&id, body.seed_ids);
    ok_data(list)
}

async fn delete_plant_blacklist_seed(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(seed_id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    let _id = resolve_id(&ctx, &headers, None)?;
    let cur = qq_farm_core::models::store::account_config::get_plant_blacklist(Some(_id.as_str()));
    let next: Vec<i64> = cur.into_iter().filter(|x| *x != seed_id).collect();
    let list = qq_farm_core::models::store::account_config::set_plant_blacklist(&_id, next);
    ok_data(list)
}

async fn delete_plant_blacklist(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<serde_json::Value> {
    let _id = resolve_id(&ctx, &headers, None)?;
    let _ = qq_farm_core::models::store::account_config::set_plant_blacklist(&_id, vec![]);
    ok_empty()
}

async fn get_seeds(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let _id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    let cfg = qq_farm_core::config::game_config::global();
    let seeds: Vec<serde_json::Value> = cfg
        .get_all_plants()
        .into_iter()
        .filter_map(|p| {
            p.seed_id.map(|sid| {
                serde_json::json!({
                    "seed_id": sid,
                    "name": p.name,
                    "plant_id": p.id,
                    "land_level_need": p.land_level_need,
                    "seasons": p.seasons,
                    "grow_phases": p.grow_phases,
                })
            })
        })
        .collect();
    let _ = ctx;
    ok_data(seeds)
}

async fn get_bag(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let detail = loop_.warehouse().get_bag_detail().await;
    match detail {
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
    let loop_ = get_loop(&ctx, &id)?;
    let r = loop_
        .warehouse()
        .use_item(body.item_id, body.count.max(1), body.uid)
        .await;
    match r {
        Ok(_reply) => ok(json!({ "ok": true })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn post_bag_sell(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BagSellBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, body.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let sell: Vec<(i64, i64, i64)> = body.items.iter().map(|i| (i.id, i.count, i.uid)).collect();
    let r = loop_.warehouse().sell_items(&sell).await;
    match r {
        Ok(_reply) => ok(json!({ "ok": true })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn get_bag_seeds(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let seeds = loop_.warehouse().get_bag_seeds().await;
    match seeds {
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
    let loop_ = get_loop(&ctx, &id)?;
    let farm = loop_.farm();
    let op = body.op.to_lowercase();
    let result: Result<serde_json::Value, qq_farm_core::error::Error> = match op.as_str() {
        "harvest" => farm
            .op_harvest()
            .await
            .map(|n| json!({ "ok": true, "op": "harvest", "count": n })),
        "water" => farm
            .op_water()
            .await
            .map(|n| json!({ "ok": true, "op": "water", "count": n })),
        "weed" => farm
            .op_weed()
            .await
            .map(|n| json!({ "ok": true, "op": "weed", "count": n })),
        "insecticide" | "bug" => farm
            .op_insecticide()
            .await
            .map(|n| json!({ "ok": true, "op": "insecticide", "count": n })),
        "fertilize" => farm
            .op_fertilize()
            .await
            .map(|r| json!({ "ok": true, "op": "fertilize", "normal": r.normal, "organic": r.organic })),
        "plant" => farm
            .op_plant()
            .await
            .map(|n| json!({ "ok": true, "op": "plant", "count": n })),
        "remove" => farm
            .op_remove()
            .await
            .map(|n| json!({ "ok": true, "op": "remove", "count": n })),
        "upgrade" => farm
            .op_upgrade()
            .await
            .map(|id| json!({ "ok": true, "op": "upgrade", "land_id": id })),
        "unlock" => farm
            .op_unlock(false)
            .await
            .map(|id| json!({ "ok": true, "op": "unlock", "land_id": id })),
        "cycle" | "all" => farm
            .run_farm_operation()
            .await
            .map(|()| json!({ "ok": true, "op": "cycle" })),
        other => Err(qq_farm_core::error::Error::Protocol(format!(
            "unknown op: {other}"
        ))),
    };
    match result {
        Ok(v) => Ok(Json(v)),
        Err(e) => Ok(Json(json!({ "ok": false, "op": op, "error": e.to_string() }))),
    }
}

async fn get_analytics(
    State(_ctx): State<Arc<AdminContext>>,
    Query(q): Query<AnalyticsQuery>,
) -> ApiResult<serde_json::Value> {
    let sort_by = q.sort_by.as_deref().map_or(SortBy::Exp, SortBy::from_str_opt);
    let rankings = qq_farm_core::services::analytics::get_plant_rankings(sort_by);
    ok_data(rankings)
}
