//! Farm 路由 — 农场数据、操作与游戏配置 CRUD。
//!
//! 1:1 对应原 `controllers/admin/farm-routes.ts`。
//!
//! 含种子/果实/物品 overlay 写入与配置图片上传。
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
//! DELETE /api/plant-blacklist/{           删除单个
//! POST   /api/plant-blacklist/batch             批量添加
//! DELETE /api/plant-blacklist                   清空
//! GET    /api/seeds                             可用种子
//! GET    /api/bag                               背包
//! POST   /api/bag/use                           使用物品
//! POST   /api/bag/sell                          卖物品
//! GET    /api/bag/seeds                         背包种子
//! GET    /api/daily-gifts                       每日礼包概览
//! POST   /api/accounts/{/start                启动账号
//! POST   /api/accounts/{/stop                 停止账号
//! POST   /api/farm/operate                      单次农场操作
//! GET    /api/analytics                         种植排行
//! GET    /api/config/seeds                      配置：种子
//! GET    /api/config/fruits                     配置：果实
//! GET    /api/config/items                      配置：物品
//! GET    /api/config/item-types                 配置：物品类型
//! GET    /api/config/plants                     配置：植物
//! GET    /api/daily-gift-overview               每日礼包概览（应用格式）
//! ```

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use qq_farm_core::services::analytics::SortBy;

use crate::context::{ok, ok_data, ok_empty, AdminContext, ApiError, ApiResult};
use crate::routes::resolve_account_id;

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
        .route("/api/daily-gifts", get(get_daily_gifts))
        .route("/api/daily-gift-overview", get(get_daily_gift_overview))
        .route("/api/accounts/{id}/start", post(post_account_start))
        .route("/api/accounts/{id}/stop", post(post_account_stop))
        .route("/api/farm/operate", post(post_farm_operate))
        .route("/api/analytics", get(get_analytics))
        .route("/api/config/seeds", get(get_config_seeds))
        .route("/api/config/fruits", get(get_config_fruits))
        .route("/api/config/items", get(get_config_items))
        .route("/api/config/item-types", get(get_config_item_types))
        .route("/api/config/plants", get(get_config_plants))
        .route("/api/seed", post(post_seed))
        .route("/api/config/fruit", post(post_fruit))
        .route("/api/config/seed/{id}", put(put_seed).delete(delete_config_seed))
        .route("/api/config/fruit/{id}", put(put_fruit).delete(delete_config_fruit))
        .route("/api/config/item/{id}", put(put_item).delete(delete_config_item))
        .route("/api/config/images/{name}", get(get_config_image))
}

// ===== Query / Body =====

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

// ===== Handlers =====

/// 获取某账号的 WorkerLoop（cloned Arc，handler 异步安全）
fn get_loop(
    ctx: &AdminContext,
    account_id: &str,
) -> Result<Arc<qq_farm_core::runtime::worker_loop::WorkerLoop>, ApiError> {
    if account_id.is_empty() {
        return Err(ApiError::BadRequest("missing x-account-id".to_string()));
    }
    ctx.engine
        .worker_loop(account_id)
        .ok_or(ApiError::AccountNotRunning)
}

fn resolve_id(
    ctx: &AdminContext,
    headers: &axum::http::HeaderMap,
    query_id: Option<&str>,
) -> Result<String, ApiError> {
    let id = resolve_account_id(ctx, headers, query_id);
    crate::routes::ensure_account_access(ctx, headers, &id)?;
    Ok(id)
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
    let mut data = ctx.engine.panel_status(&id);
    if let Some(status) = data.get("status") {
        let level = status.get("level").and_then(|v| v.as_i64()).unwrap_or(0);
        let exp = status.get("exp").and_then(|v| v.as_i64()).unwrap_or(0);
        let (current, needed) = qq_farm_core::config::game_config::global()
            .get_level_exp_progress(level, exp);
        if let Some(obj) = data.as_object_mut() {
            obj.insert("levelProgress".to_string(), json!({ "current": current, "needed": needed }));
        }
    }
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
    // CommerceService 编排：构造（MallService, MysteryShopService, WarehouseService）
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
    let _id = resolve_id(&ctx, &headers, body.account_id.as_deref())?;
    let list = qq_farm_core::models::store::account_config::set_plant_blacklist(
        body.account_id.as_deref().unwrap_or(""),
        body.seed_ids,
    );
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
    // 全部 seed 列表（从 game_config 拿；运行时 available_seeds 走 worker）
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

async fn get_daily_gifts(
    ctx: State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    q: Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    get_daily_gift_overview(ctx, headers, q).await
}

async fn get_daily_gift_overview(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    let loop_ = get_loop(&ctx, &id)?;
    let task = loop_.task();
    let email = loop_.email();
    let share = loop_.share();
    let vip = loop_.qqvip();
    let month = loop_.monthcard();
    let mall = loop_.mall();

    let task_full = task.get_task_daily_state_like_app().await;
    let email_state = email.get_daily_state();
    let share_state = share.get_daily_state();
    let vip_state = vip.get_vip_daily_state();
    let month_state = month.get_month_card_daily_state();
    let free_state = mall.get_free_gift_daily_state();
    let growth = task.get_growth_task_state_like_app().await;
    let _ = id;
    let extract_bool = |v: &serde_json::Value, k: &str| {
        v.get(k).and_then(|x| x.as_bool()).unwrap_or(false)
    };
    let extract_i64 = |v: &serde_json::Value, k: &str| {
        v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
    };
    ok_data(json!({
        "date": chrono::Local::now().format("%Y-%m-%d").to_string(),
        "growth": {
            "key": growth.key,
            "label": "成长任务",
            "doneToday": growth.done_today,
            "completedCount": growth.completed_count,
            "totalCount": growth.total_count,
            "tasks": growth.tasks,
        },
        "gifts": [
            {
                "key": "task_claim",
                "label": "每日任务",
                "doneToday": task_full.done_today,
                "lastAt": task_full.last_claim_at,
                "completedCount": task_full.completed_count,
                "totalCount": task_full.total_count,
            },
            {
                "key": "email_rewards",
                "label": "邮箱奖励",
                "doneToday": extract_bool(&email_state, "doneToday"),
                "lastAt": extract_i64(&email_state, "lastCheckAt"),
            },
            {
                "key": "mall_free_gifts",
                "label": "商城免费礼包",
                "doneToday": free_state.done_today,
                "lastAt": free_state.last_claim_at,
            },
            {
                "key": "daily_share",
                "label": "分享礼包",
                "doneToday": extract_bool(&share_state, "doneToday"),
                "lastAt": extract_i64(&share_state, "lastClaimAt"),
            },
            {
                "key": "vip_daily_gift",
                "label": "会员礼包",
                "doneToday": vip_state.done_today,
                "lastAt": vip_state.last_claim_at,
            },
            {
                "key": "month_card_gift",
                "label": "月卡礼包",
                "doneToday": month_state.done_today,
                "lastAt": month_state.last_claim_at,
            },
        ],
    }))
}

async fn post_account_start(
    State(ctx): State<Arc<AdminContext>>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let acc = qq_farm_core::models::store::accounts::get_accounts()
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| ApiError::NotFound(format!("account not found: id")))?;
    let account = qq_farm_core::models::Account::from_store(&acc);
    ctx.engine.start_worker(account).map_err(|e| ApiError::Internal(e.to_string()))?;
    ok(json!({ "ok": true, "accountId": acc.id, "started": true }))
}

async fn post_account_stop(
    State(ctx): State<Arc<AdminContext>>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    ctx.engine.stop_worker(&id);
    ok(json!({ "ok": true, "accountId": id, "stopped": true }))
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

async fn get_config_seeds(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let gc = qq_farm_core::config::game_config::global();
    let data: Vec<serde_json::Value> = gc
        .get_all_seeds()
        .into_iter()
        .map(|s| {
            let item = gc.get_item_by_id(s.seed_id);
            let sells = item
                .as_ref()
                .and_then(|i| i.sells.as_ref())
                .and_then(|v| v.as_str())
                .map(|s| gc.parse_sells(s))
                .unwrap_or_default();
            let mut val = serde_json::to_value(&s).unwrap_or(json!({}));
            if let Some(obj) = val.as_object_mut() {
                obj.insert(
                    "priceId".into(),
                    json!(sells.first().map(|p| p.0).unwrap_or(0)),
                );
                if !obj.contains_key("image") {
                    obj.insert(
                        "image".into(),
                        json!(qq_farm_core::config::game_config::mapped_item_image(s.seed_id)),
                    );
                }
            }
            val
        })
        .collect();
    ok_data(data)
}

async fn get_config_fruits(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let gc = qq_farm_core::config::game_config::global();
    let data: Vec<serde_json::Value> = gc
        .get_all_items()
        .into_iter()
        .filter(|i| i.item_type == 6)
        .map(|fruit| {
            let plant = gc.get_plant_by_fruit_id(fruit.id);
            let sells = fruit
                .sells
                .as_ref()
                .and_then(|v| v.as_str())
                .map(|s| gc.parse_sells(s))
                .unwrap_or_default();
            json!({
                "id": fruit.id,
                "name": fruit.name,
                "type": fruit.item_type,
                "price": sells.first().map(|p| p.1).unwrap_or(0),
                "priceId": sells.first().map(|p| p.0).unwrap_or(0),
                "level": fruit.level.unwrap_or(0),
                "assetName": fruit.asset_name.clone().unwrap_or_default(),
                "desc": fruit.desc.clone().unwrap_or_default(),
                "rarity": fruit.rarity.unwrap_or(0),
                "maxCount": fruit.max_count.unwrap_or(9999),
                "plantId": plant.as_ref().map(|p| p.id),
                "seedId": plant.as_ref().and_then(|p| p.seed_id),
                "plantName": plant.as_ref().map(|p| p.name.clone()),
                "image": qq_farm_core::config::game_config::mapped_item_image(fruit.id),
            })
        })
        .collect();
    ok_data(data)
}

async fn get_config_items(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let gc = qq_farm_core::config::game_config::global();
    let data: Vec<serde_json::Value> = gc
        .get_all_items()
        .into_iter()
        .filter(|i| i.item_type != 5 && i.item_type != 6)
        .map(|item| {
            let sells = item
                .sells
                .as_ref()
                .and_then(|v| v.as_str())
                .map(|s| gc.parse_sells(s))
                .unwrap_or_default();
            json!({
                "id": item.id,
                "type": item.item_type,
                "name": item.name,
                "interactionType": item.interaction_type.clone().unwrap_or_default(),
                "priceId": sells.first().map(|p| p.0).unwrap_or(0),
                "price": sells.first().map(|p| p.1).unwrap_or(0),
                "level": item.level.unwrap_or(0),
                "assetName": item.asset_name.clone().unwrap_or_default(),
                "iconRes": item.icon_res.clone().unwrap_or_default(),
                "maxCount": item.max_count.unwrap_or(9999),
                "canUse": item.can_use.unwrap_or(0),
                "desc": item.desc.clone().unwrap_or_default(),
                "rarity": item.rarity.unwrap_or(0),
                "image": qq_farm_core::config::game_config::mapped_item_image(item.id),
            })
        })
        .collect();
    ok_data(data)
}

async fn get_config_item_types(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let cfg = qq_farm_core::config::game_config::load_item_types_config();
    ok(json!({ "ok": true, "itemTypes": cfg }))
}

async fn get_config_plants(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let gc = qq_farm_core::config::game_config::global();
    let data: Vec<serde_json::Value> = gc
        .get_all_plants()
        .into_iter()
        .map(|p| {
            let seed_id = p.seed_id.unwrap_or(0);
            let land_level = if seed_id > 0 {
                gc.get_item_by_id(seed_id)
                    .and_then(|i| i.level)
                    .unwrap_or_else(|| p.land_level_need.unwrap_or(0))
            } else {
                p.land_level_need.unwrap_or(0)
            };
            json!({
                "plantId": p.id,
                "id": p.id,
                "name": p.name,
                "plantName": p.name,
                "seedId": p.seed_id,
                "fruitId": p.fruit.as_ref().map(|f| f.id),
                "fruitCount": p.fruit.as_ref().map(|f| f.count).unwrap_or(0),
                "landLevelNeed": land_level,
                "seasons": p.seasons.unwrap_or(1),
                "growPhases": p.grow_phases.clone().unwrap_or_default(),
                "exp": p.exp.unwrap_or(0),
                "price": if seed_id > 0 { gc.get_seed_price(seed_id) } else { 0 },
                "image": if seed_id > 0 {
                    qq_farm_core::config::game_config::mapped_item_image(seed_id)
                } else {
                    String::new()
                },
            })
        })
        .collect();
    ok_data(data)
}

#[derive(Debug, Default, Deserialize)]
struct ConfigImageBody {
    #[serde(default)]
    image_base64: Option<String>,
    #[serde(default)]
    image_name: Option<String>,
}

fn maybe_save_image(body: &ConfigImageBody) -> Option<String> {
    let b64 = body.image_base64.as_deref()?.trim();
    if b64.is_empty() {
        return None;
    }
    let name = body
        .image_name
        .clone()
        .unwrap_or_else(|| format!("img-{}.png", chrono::Utc::now().timestamp_millis()));
    qq_farm_core::config::game_config::GameConfig::save_config_image_base64(&name, b64).ok()
}

async fn post_seed(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    let mut plant: qq_farm_core::config::game_config::Plant =
        serde_json::from_value(body.clone()).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    if plant.id <= 0 {
        return Err(ApiError::BadRequest("plant id required".into()));
    }
    let img_body: ConfigImageBody = serde_json::from_value(body).unwrap_or_default();
    if let Some(url) = maybe_save_image(&img_body) {
        plant.harvest_animation = Some(url);
    }
    qq_farm_core::config::game_config::global()
        .upsert_plant(plant)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    ok(json!({ "ok": true }))
}

async fn put_seed(
    State(_ctx): State<Arc<AdminContext>>,
    Path(id): Path<i64>,
    Json(mut body): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    body["id"] = json!(id);
    if body.get("seed_id").is_none() {
        body["seed_id"] = json!(id);
    }
    post_seed(State(_ctx), Json(body)).await
}

async fn delete_config_seed(
    State(_ctx): State<Arc<AdminContext>>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    let removed = qq_farm_core::config::game_config::global()
        .delete_plant_overlay(id)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    ok(json!({ "ok": true, "removed": removed }))
}

async fn post_fruit(
    State(ctx): State<Arc<AdminContext>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    let id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    put_fruit(State(ctx), Path(id), Json(body)).await
}

async fn put_fruit(
    State(_ctx): State<Arc<AdminContext>>,
    Path(id): Path<i64>,
    Json(mut body): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    body["id"] = json!(id);
    let mut item: qq_farm_core::config::game_config::Item =
        serde_json::from_value(body.clone()).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let img_body: ConfigImageBody = serde_json::from_value(body).unwrap_or_default();
    if let Some(url) = maybe_save_image(&img_body) {
        item.asset_name = Some(url);
    }
    qq_farm_core::config::game_config::global()
        .upsert_item(item)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    ok(json!({ "ok": true }))
}

async fn delete_config_fruit(
    State(_ctx): State<Arc<AdminContext>>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    delete_config_item(State(_ctx), Path(id)).await
}

async fn put_item(
    State(ctx): State<Arc<AdminContext>>,
    Path(id): Path<i64>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    put_fruit(State(ctx), Path(id), Json(body)).await
}

async fn delete_config_item(
    State(_ctx): State<Arc<AdminContext>>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    let removed = qq_farm_core::config::game_config::global()
        .delete_item_overlay(id)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    ok(json!({ "ok": true, "removed": removed }))
}

async fn get_config_image(Path(name): Path<String>) -> impl IntoResponse {
    match qq_farm_core::config::game_config::GameConfig::read_config_image(&name) {
        Some(bytes) => {
            let mime = if name.ends_with(".jpg") || name.ends_with(".jpeg") {
                "image/jpeg"
            } else if name.ends_with(".gif") {
                "image/gif"
            } else if name.ends_with(".webp") {
                "image/webp"
            } else {
                "image/png"
            };
            ([(header::CONTENT_TYPE, mime)], bytes).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
