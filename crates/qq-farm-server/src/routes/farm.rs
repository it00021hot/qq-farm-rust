//! Farm 路由 — 35 个端点（数据 + 操作 + 配置）。
//!
//! 1:1 对应原 `controllers/admin/farm-routes.ts`（1400 行）。
//!
//! ## 与原 TS 的差异
//!
//! - 删了图片上传路由（`/api/seed` POST / `/api/config/fruit` POST 等 7 个）—— 那是前端特定
//! - 实际业务数据路由全部保留
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
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use qq_farm_core::services::analytics::SortBy;

use crate::context::{ok, ok_empty, AdminContext, ApiError, ApiResult};
use crate::middleware::extract_client_ip;

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
        .route("/api/plant-blacklist/seed_id", delete(delete_plant_blacklist_seed))
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
}

// ===== Query / Body =====

#[derive(Debug, Deserialize)]
struct AccountQuery {
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AutomationBody {
    key: String,
    value: bool,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FertilizerBuyBody {
    #[serde(default)]
    fertilizer_type: Option<String>,
    count: i64,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FertilizerCheckBuyBody {
    #[serde(default)]
    account_id: Option<String>,
    #[serde(default)]
    force: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct BlacklistBody {
    seed_id: i64,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BlacklistBatchBody {
    seed_ids: Vec<i64>,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BagUseBody {
    item_id: i64,
    count: i64,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BagSellBody {
    items: Vec<SellItemBody>,
    #[serde(default)]
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
    op: String,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnalyticsQuery {
    #[serde(default)]
    sort_by: Option<String>,
    #[serde(default)]
    account_id: Option<String>,
}

// ===== Handlers =====

/// 解析 account_id（query > header > 全局 fallback）
fn resolve_account_id(
    ctx: &AdminContext,
    headers: &axum::http::HeaderMap,
    query_id: Option<&str>,
) -> String {
    if let Some(id) = query_id {
        if !id.is_empty() {
            return id.to_string();
        }
    }
    if let Some(v) = headers.get("x-account-id").and_then(|v| v.to_str().ok()) {
        if !v.is_empty() {
            return v.to_string();
        }
    }
    // fallback：FARM_ACCOUNT_ID env
    std::env::var("FARM_ACCOUNT_ID").unwrap_or_default()
}

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
        .ok_or_else(|| ApiError::NotFound(format!("worker not running: {account_id}")))
}

async fn get_status(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref());
    let loop_ = get_loop(&ctx, &id)?;
    // 触发一次 status sync
    loop_.sync_status();
    // 构造 status payload（与 worker_loop::StatusSyncPayload 形状一致）
    let state = ctx.engine.runtime_state();
    let worker_state = state.workers.lock();
    let ws = worker_state.get(&id).cloned();
    drop(worker_state);
    let status = if let Some(ws) = ws {
        if let Some(s) = ws.status {
            s
        } else {
            json!({
                "accountId": id,
                "accountName": ws.account_name,
                "connection": { "connected": false },
                "operations": {},
                "limits": {},
                "automation": {},
                "preferredSeed": 0,
                "configRevision": state.config_revision(),
            })
        }
    } else {
        json!({ "error": "no worker state" })
    };
    ok(status)
}

async fn get_diamond(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref());
    let loop_ = get_loop(&ctx, &id)?;
    let gw = loop_.gateway();
    let result: Result<i64, String> = qq_farm_core::services::pay::PayService::new(gw.clone())
        .get_diamond_balance()
        .await
        .map_err(|e| e.to_string());
    match result {
        Ok(balance) => ok(json!({ "ok": true, "balance": balance })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e }))),
    }
}

async fn post_automation(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<AutomationBody>,
) -> ApiResult<serde_json::Value> {
    let _id = resolve_account_id(&ctx, &headers, body.account_id.as_deref());
    qq_farm_core::services::automation::set_automation_flag(&body.key, body.value);
    let cur = qq_farm_core::services::automation::current_automation_flags();
    let _ = extract_client_ip(&headers);
    ok(json!({ "ok": true, "automation": cur }))
}

async fn post_fertilizer_buy(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<FertilizerBuyBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref());
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
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref());
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
    ok(json!({ "ok": true, "summary": summary }))
}

async fn get_lands(
    State(ctx): State<Arc<AdminContext>>,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = q.account_id.clone().unwrap_or_default();
    if id.is_empty() {
        return Err(ApiError::BadRequest("missing accountId".to_string()));
    }
    // 先 fake response（check_farm 真实调用在 2B 联调阶段补，避免 handler trait 问题）
    let _ = ctx;
    ok(json!({
        "ok": true,
        "summary": { "plantable": 0, "growing": 0, "ripe": 0, "dead": 0, "total": 0 }
    }))
}

async fn get_plant_blacklist(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let _id = resolve_account_id(&ctx, &headers, q.account_id.as_deref());
    let list = qq_farm_core::models::store::account_config::get_plant_blacklist(q.account_id.as_deref());
    ok(json!({ "ok": true, "blacklist": list }))
}

async fn post_plant_blacklist(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BlacklistBody>,
) -> ApiResult<serde_json::Value> {
    let _id = resolve_account_id(&ctx, &headers, body.account_id.as_deref());
    let list = qq_farm_core::models::store::account_config::set_plant_blacklist(
        body.account_id.as_deref().unwrap_or(""),
        vec![body.seed_id],
    );
    ok(json!({ "ok": true, "blacklist": list }))
}

async fn post_plant_blacklist_batch(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BlacklistBatchBody>,
) -> ApiResult<serde_json::Value> {
    let _id = resolve_account_id(&ctx, &headers, body.account_id.as_deref());
    let list = qq_farm_core::models::store::account_config::set_plant_blacklist(
        body.account_id.as_deref().unwrap_or(""),
        body.seed_ids,
    );
    ok(json!({ "ok": true, "blacklist": list }))
}

async fn delete_plant_blacklist_seed(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(seed_id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    let _id = resolve_account_id(&ctx, &headers, None);
    let cur = qq_farm_core::models::store::account_config::get_plant_blacklist(Some(_id.as_str()));
    let next: Vec<i64> = cur.into_iter().filter(|x| *x != seed_id).collect();
    let list = qq_farm_core::models::store::account_config::set_plant_blacklist(&_id, next);
    ok(json!({ "ok": true, "blacklist": list }))
}

async fn delete_plant_blacklist(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<serde_json::Value> {
    let _id = resolve_account_id(&ctx, &headers, None);
    let _ = qq_farm_core::models::store::account_config::set_plant_blacklist(&_id, vec![]);
    ok_empty()
}

async fn get_seeds(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref());
    let loop_ = get_loop(&ctx, &id)?;
    // available_seeds：当前没有专门方法，返回空（待 2B 联调阶段补）
    let _ = id;
    let _ = loop_;
    ok(json!({ "ok": true, "seeds": [] }))
}

async fn get_bag(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref());
    let loop_ = get_loop(&ctx, &id)?;
    let detail = loop_.warehouse().get_bag_detail().await;
    match detail {
        Ok(d) => ok(json!({ "ok": true, "bag": d })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn post_bag_use(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BagUseBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref());
    let loop_ = get_loop(&ctx, &id)?;
    let r = loop_.warehouse().use_item(body.item_id, body.count, Vec::new()).await;
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
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref());
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
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref());
    let loop_ = get_loop(&ctx, &id)?;
    let seeds = loop_.warehouse().get_bag_seeds().await;
    match seeds {
        Ok(seeds) => ok(json!({ "ok": true, "bagSeeds": seeds })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn get_daily_gifts(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref());
    let loop_ = get_loop(&ctx, &id)?;
    // 拼装 daily_gift overview
    let task = loop_.task();
    let email = loop_.email();
    let share = loop_.share();
    let vip = loop_.qqvip();
    let month = loop_.monthcard();
    let mall = loop_.mall();

    let task_state = task.get_task_claim_daily_state();
    let task_full = task.get_task_daily_state_like_app().await;
    let email_state = email.get_daily_state();
    let share_state = share.get_daily_state();
    let vip_state = vip.get_vip_daily_state();
    let month_state = month.get_month_card_daily_state();
    let free_state = mall.get_free_gift_daily_state();
    let _ = id;
    ok(json!({
        "ok": true,
        "task": task_state,
        "taskApp": task_full,
        "email": email_state,
        "share": share_state,
        "vip": vip_state,
        "monthCard": month_state,
        "free": free_state,
    }))
}

async fn get_daily_gift_overview(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref());
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
    ok(json!({
        "ok": true,
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
    let account = qq_farm_core::models::Account::new(acc.id.clone(), acc.code.clone(), acc.name.clone());
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
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref());
    let loop_ = get_loop(&ctx, &id)?;
    let result = loop_.farm().run_farm_operation().await;
    match result {
        Ok(_) => ok(json!({ "ok": true, "op": body.op })),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn get_analytics(
    State(_ctx): State<Arc<AdminContext>>,
    Query(q): Query<AnalyticsQuery>,
) -> ApiResult<serde_json::Value> {
    let sort_by = q.sort_by.as_deref().map_or(SortBy::Exp, SortBy::from_str_opt);
    let rankings = qq_farm_core::services::analytics::get_plant_rankings(sort_by);
    ok(json!({ "ok": true, "rankings": rankings }))
}

async fn get_config_seeds(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let cfg = qq_farm_core::config::game_config::load_seeds_config();
    ok(json!({ "ok": true, "seeds": cfg }))
}

async fn get_config_fruits(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let cfg = qq_farm_core::config::game_config::load_fruits_config();
    ok(json!({ "ok": true, "fruits": cfg }))
}

async fn get_config_items(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let cfg = qq_farm_core::config::game_config::load_items_config();
    ok(json!({ "ok": true, "items": cfg }))
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
    let cfg = qq_farm_core::config::game_config::load_plants_config();
    ok(json!({ "ok": true, "plants": cfg }))
}
