//! 农场操作编排。

use std::sync::Arc;

use qq_farm_core::runtime::worker_loop::WorkerLoop;
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
