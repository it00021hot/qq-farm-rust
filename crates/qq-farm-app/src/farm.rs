//! 农场操作编排。

use std::sync::Arc;

use qq_farm_core::runtime::worker_loop::WorkerLoop;
use qq_farm_core::services::analytics::SortBy;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::session::AppContext;

/// 要求账号 worker 正在运行，返回 WorkerLoop。
pub fn require_worker_loop(ctx: &AppContext, account_id: &str) -> AppResult<Arc<WorkerLoop>> {
    if account_id.is_empty() {
        return Err(AppError::BadRequest("missing account id".to_string()));
    }
    ctx.engine
        .worker_loop(account_id)
        .ok_or(AppError::AccountNotRunning)
}

/// 面板状态 + 等级进度。
#[must_use]
pub fn panel_status_with_progress(ctx: &AppContext, account_id: &str) -> Value {
    let mut data = ctx.engine.panel_status(account_id);
    if let Some(status) = data.get("status") {
        let level = status.get("level").and_then(|v| v.as_i64()).unwrap_or(0);
        let exp = status.get("exp").and_then(|v| v.as_i64()).unwrap_or(0);
        let (current, needed) = qq_farm_core::config::game_config::global()
            .get_level_exp_progress(level, exp);
        if let Some(obj) = data.as_object_mut() {
            obj.insert(
                "levelProgress".to_string(),
                json!({ "current": current, "needed": needed }),
            );
        }
    }
    data
}

/// 钻石余额。
pub async fn diamond_balance(ctx: &AppContext, account_id: &str) -> AppResult<i64> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let gw = loop_.gateway();
    qq_farm_core::services::pay::PayService::new(gw.clone())
        .get_diamond_balance()
        .await
        .map(|b| b.max(0))
        .map_err(AppError::from_core)
}

/// 切换自动化单项。
pub fn set_automation(
    ctx: &AppContext,
    account_id: &str,
    key: &str,
    value: Value,
    extra: Value,
) -> AppResult<Value> {
    if account_id.is_empty() {
        return Err(AppError::BadRequest("Missing account id".into()));
    }
    let mut auto = serde_json::Map::new();
    if !key.is_empty() {
        auto.insert(key.to_string(), value);
    }
    if let Some(obj) = extra.as_object() {
        for (k, v) in obj {
            if k == "accountId" || k == "account_id" || k == "key" || k == "value" {
                continue;
            }
            auto.insert(k.clone(), v.clone());
        }
    }
    let snapshot = json!({ "automation": auto });
    qq_farm_core::models::store::account_config::apply_config_snapshot(
        snapshot,
        Some(account_id),
        true,
    );
    ctx.engine.reload_worker_config(account_id);
    Ok(json!(
        qq_farm_core::models::store::account_config::get_automation(Some(account_id))
    ))
}

/// 地块详情。
pub async fn lands(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let (lands, summary) = loop_
        .farm()
        .get_lands_detail()
        .await
        .map_err(AppError::from_core)?;
    Ok(json!({ "lands": lands, "summary": summary }))
}

/// 背包详情。
pub async fn bag(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let detail = loop_
        .warehouse()
        .get_bag_detail()
        .await
        .map_err(AppError::from_core)?;
    serde_json::to_value(detail).map_err(|e| AppError::Internal(e.to_string()))
}

/// 使用背包物品。
pub async fn bag_use(
    ctx: &AppContext,
    account_id: &str,
    item_id: i64,
    count: i64,
    uid: i64,
) -> AppResult<()> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_
        .warehouse()
        .use_item(item_id, count.max(1), uid)
        .await
        .map(|_| ())
        .map_err(AppError::from_core)
}

/// 出售背包物品。
pub async fn bag_sell(
    ctx: &AppContext,
    account_id: &str,
    items: &[(i64, i64, i64)],
) -> AppResult<()> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_
        .warehouse()
        .sell_items(items)
        .await
        .map(|_| ())
        .map_err(AppError::from_core)
}

/// 背包种子。
pub async fn bag_seeds(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let seeds = loop_
        .warehouse()
        .get_bag_seeds()
        .await
        .map_err(AppError::from_core)?;
    serde_json::to_value(seeds).map_err(|e| AppError::Internal(e.to_string()))
}

/// 种子目录（静态配置）。
#[must_use]
pub fn seeds_catalog() -> Value {
    let cfg = qq_farm_core::config::game_config::global();
    let seeds: Vec<Value> = cfg
        .get_all_plants()
        .into_iter()
        .filter_map(|p| {
            p.seed_id.map(|sid| {
                json!({
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
    json!(seeds)
}

/// 农场手动操作。
pub async fn operate(ctx: &AppContext, account_id: &str, op: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let farm = loop_.farm();
    let op = op.to_lowercase();
    let result: Result<Value, qq_farm_core::error::Error> = match op.as_str() {
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
        "fertilize" => farm.op_fertilize().await.map(|r| {
            json!({ "ok": true, "op": "fertilize", "normal": r.normal, "organic": r.organic })
        }),
        "plant" => farm
            .op_plant()
            .await
            .map(|n| json!({ "ok": true, "op": "plant", "count": n })),
        "remove" | "clear" => farm
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
    result.map_err(AppError::from_core)
}

/// 种植分析排名。
#[must_use]
pub fn analytics(sort_by: Option<&str>) -> Value {
    let sort = sort_by.map_or(SortBy::Exp, SortBy::from_str_opt);
    let rankings = qq_farm_core::services::analytics::get_plant_rankings(sort);
    json!(rankings)
}

/// 偷菜作物黑名单。
#[must_use]
pub fn plant_blacklist(account_id: &str) -> Value {
    let id = if account_id.is_empty() {
        None
    } else {
        Some(account_id)
    };
    json!(qq_farm_core::models::store::account_config::get_plant_blacklist(id))
}

pub fn set_plant_blacklist(account_id: &str, seed_ids: Vec<i64>) -> Value {
    json!(qq_farm_core::models::store::account_config::set_plant_blacklist(
        account_id, seed_ids
    ))
}

pub fn add_plant_blacklist(account_id: &str, seed_id: i64) -> Value {
    let mut current =
        qq_farm_core::models::store::account_config::get_plant_blacklist(Some(account_id));
    if !current.contains(&seed_id) {
        current.push(seed_id);
    }
    set_plant_blacklist(account_id, current)
}

pub fn remove_plant_blacklist(account_id: &str, seed_id: i64) -> Value {
    let cur = qq_farm_core::models::store::account_config::get_plant_blacklist(Some(account_id));
    let next: Vec<i64> = cur.into_iter().filter(|x| *x != seed_id).collect();
    set_plant_blacklist(account_id, next)
}

/// 购买化肥。
pub async fn fertilizer_buy(
    ctx: &AppContext,
    account_id: &str,
    fertilizer_type: &str,
    count: i32,
) -> AppResult<i32> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let kind = match fertilizer_type {
        "normal" => qq_farm_core::services::mall::MallFertilizerKind::Normal,
        _ => qq_farm_core::services::mall::MallFertilizerKind::Organic,
    };
    Ok(loop_
        .mall()
        .auto_buy_fertilizer(true, kind, count)
        .await)
}

/// 检查并购买双化肥。
pub async fn fertilizer_check_and_buy(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let mystery =
        qq_farm_core::services::mystery_shop::MysteryShopService::new(loop_.gateway().clone());
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
    Ok(json!({
        "organicBought": summary.organic_bought,
        "normalBought": summary.normal_bought,
        "organicCurrentHours": summary.organic_current_hours,
        "normalCurrentHours": summary.normal_current_hours,
        "error": summary.error,
    }))
}

/// 每日礼包概览（从 farm 路由提取的聚合逻辑）。
pub async fn daily_gift_overview(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
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

    let extract_bool = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_bool()).unwrap_or(false);
    let extract_i64 = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);

    Ok(json!({
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

/// 从引擎读取全局日志（最近 limit 条，新→旧）。
#[must_use]
pub fn engine_global_logs(ctx: &AppContext, account_id: Option<&str>, limit: usize) -> Value {
    let state = ctx.engine.runtime_state();
    let logs = state.global_logs.lock().clone();
    let mut filtered: Vec<_> = if let Some(id) = account_id.filter(|s| !s.is_empty()) {
        logs.into_iter()
            .filter(|l| l.account_id.as_deref() == Some(id))
            .collect()
    } else {
        logs
    };
    filtered.sort_by(|a, b| b.ts.cmp(&a.ts));
    filtered.truncate(limit.max(1));
    json!(filtered)
}

/// 账号日志。
#[must_use]
pub fn account_logs(ctx: &AppContext, account_id: Option<&str>, limit: usize) -> Value {
    let logs = ctx.engine.runtime_state().account_logs.lock().clone();
    let mut filtered: Vec<_> = if let Some(id) = account_id.filter(|s| !s.is_empty()) {
        logs.into_iter().filter(|l| l.account_id == id).collect()
    } else {
        logs
    };
    filtered = filtered.into_iter().rev().take(limit.max(1)).collect();
    json!(filtered)
}

/// 清空全局日志（可选按账号）。
pub fn clear_global_logs(ctx: &AppContext, account_id: Option<&str>) {
    let state = ctx.engine.runtime_state();
    let mut logs = state.global_logs.lock();
    if let Some(id) = account_id.filter(|s| !s.is_empty()) {
        logs.retain(|l| l.account_id.as_deref() != Some(id));
    } else {
        logs.clear();
    }
}
