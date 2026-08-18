//! Activity Center 路由 — 对齐原 `controllers/admin/activity-center-routes.ts`。

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::context::{ok_data, AdminContext, ApiResult};
use crate::routes::resolve_account_id_required as resolve_account_id;

/// 构造 activity-center 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/activity-center", get(get_snapshot))
        .route("/api/activity-center/snapshot", get(get_snapshot))
        .route("/api/activity-center/season", get(get_season))
        .route("/api/activity-center/shop", get(get_shop))
        .route("/api/activity-center/solar-terms", get(get_solar_terms))
        .route("/api/activity-center/qingmei", get(get_qingmei))
        .route("/api/activity-center/pass/claim", post(claim_battle_pass))
        .route("/api/activity-center/constellation/light", post(light_constellation))
        .route("/api/activity-center/shop/exchange", post(exchange_star_sand))
        .route("/api/activity-center/solar-terms/{term_id}/claim", post(claim_solar_term))
        .route("/api/activity-center/qingmei/daily-seed/claim", post(claim_qingmei_seed))
        .route("/api/activity-center/qingmei/brew/start", post(start_qingmei_brew))
        .route("/api/activity-center/qingmei/brew/continue", post(continue_qingmei_brew))
        .route("/api/activity-center/qingmei/brew/settle", post(settle_qingmei_brew))
}

#[derive(Debug, Deserialize)]
struct AccountQuery {
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExchangeBody {
    #[serde(default)]
    goods_id: Value,
    #[serde(default)]
    count: Value,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QingMeiStartBody {
    #[serde(default)]
    ingredients: Option<Value>,
    #[serde(default)]
    count: Option<Value>,
}

fn activity_app_result(result: qq_farm_app::AppResult<Value>) -> ApiResult<Value> {
    match result {
        Ok(s) => ok_data(s),
        Err(qq_farm_app::AppError::Core(e)) => Ok(Json(activity_error_json(&e))),
        Err(e) => Ok(Json(activity_error_json_from_app(&e))),
    }
}

fn activity_error_json_from_app(err: &qq_farm_app::AppError) -> Value {
    activity_error_json(&qq_farm_core::error::Error::Protocol(err.to_string()))
}

async fn get_snapshot(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    activity_app_result(qq_farm_app::activity::snapshot(&ctx.app_context(), &id).await)
}

async fn get_season(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    activity_app_result(qq_farm_app::activity::season(&ctx.app_context(), &id).await)
}

async fn claim_battle_pass(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<Value> {
    let id = resolve_account_id(&ctx, &headers, None)?;
    activity_app_result(qq_farm_app::activity::claim_battle_pass(&ctx.app_context(), &id).await)
}

async fn light_constellation(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<Value> {
    let id = resolve_account_id(&ctx, &headers, None)?;
    activity_app_result(qq_farm_app::activity::light_constellation(&ctx.app_context(), &id).await)
}

async fn exchange_star_sand(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<ExchangeBody>,
) -> ApiResult<Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref())?;
    activity_app_result(
        qq_farm_app::activity::exchange_star_sand(
            &ctx.app_context(),
            &id,
            &body.goods_id,
            &body.count,
        )
        .await,
    )
}

async fn claim_solar_term(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(term_id): Path<String>,
) -> ApiResult<Value> {
    let id = resolve_account_id(&ctx, &headers, None)?;
    activity_app_result(
        qq_farm_app::activity::claim_solar_term(&ctx.app_context(), &id, &term_id).await,
    )
}

fn activity_error_json(err: &qq_farm_core::error::Error) -> Value {
    let raw = err.to_string();
    let protocol_code = raw
        .split("code=")
        .nth(1)
        .and_then(|s| s.split(|c: char| c.is_whitespace() || c == ',' || c == ')').next())
        .unwrap_or("")
        .to_string();
    let business_code = raw
        .rsplit("business error: ")
        .next()
        .unwrap_or(&raw)
        .split(':')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    let (code, message) = match protocol_code.as_str() {
        "1034014" => ("1034014", "今日青梅种子已经领取，无需重复领取"),
        "1034038" => {
            ("1034038", "当前没有可点亮或可领取的星宿奖励，可能已经领取过，请稍后或明天再来看看")
        }
        "1034001" => ("1034001", "当前活动暂不可操作，请稍后再试"),
        "1034002" => ("1034002", "活动尚未开放或已经结束"),
        _ if business_code == "INVALID_EXCHANGE_COUNT"
            || raw.contains("INVALID_EXCHANGE_COUNT") =>
        {
            ("INVALID_EXCHANGE_COUNT", "兑换数量必须是正十进制整数")
        }
        _ if business_code == "INVALID_SHOP_GOODS_ID" || raw.contains("INVALID_SHOP_GOODS_ID") => {
            ("INVALID_SHOP_GOODS_ID", "商品信息无效，请刷新商店后重试")
        }
        _ if business_code == "SHOP_GOODS_NOT_FOUND" || raw.contains("SHOP_GOODS_NOT_FOUND") => {
            ("SHOP_GOODS_NOT_FOUND", "该商品已不在当前商店目录中，请刷新后重试")
        }
        _ if business_code == "SHOP_GOODS_UNAVAILABLE"
            || raw.contains("SHOP_GOODS_UNAVAILABLE") =>
        {
            ("SHOP_GOODS_UNAVAILABLE", "该商品当前不可兑换，请刷新商店后重试")
        }
        _ if business_code == "SHOP_BALANCE_UNAVAILABLE"
            || raw.contains("SHOP_BALANCE_UNAVAILABLE") =>
        {
            ("SHOP_BALANCE_UNAVAILABLE", "暂时无法确认星砂余额，请稍后重试")
        }
        _ if business_code == "INSUFFICIENT_STAR_SAND"
            || raw.contains("INSUFFICIENT_STAR_SAND") =>
        {
            ("INSUFFICIENT_STAR_SAND", "星砂余额不足，无法完成本次兑换")
        }
        _ if business_code == "SHOP_RESPONSE_INVALID" || raw.contains("SHOP_RESPONSE_INVALID") => {
            ("SHOP_RESPONSE_INVALID", "商店数据已经变化，请刷新页面后重试")
        }
        _ if business_code == "SHOP_UNAVAILABLE"
            || raw.contains("SHOP_UNAVAILABLE")
            || raw.contains("当前赛季未发现活动商店") =>
        {
            ("SHOP_UNAVAILABLE", "星砂商店暂未开放，请稍后再来看看")
        }
        _ if raw.contains("当前没有可领取的游记奖励") => {
            ("NO_PASS_REWARD", "当前没有可领取的游记奖励，请完成新的游记等级后再试")
        }
        _ if raw.contains("指定节令当前不可领取") => {
            ("SOLAR_TERM_UNAVAILABLE", "当前节令奖励暂不可领取，请在开放后再试")
        }
        _ if raw.contains("服务端未发现星座活动") || raw.contains("CONSTELLATION") => {
            ("CONSTELLATION_UNAVAILABLE", "观星礼录活动暂未开放或已经结束")
        }
        _ if raw.contains("服务端未发现可用游记") => {
            ("PASS_UNAVAILABLE", "千星游记活动暂未开放或已经结束")
        }
        _ if raw.contains("服务端未发现指定节令") => {
            ("SOLAR_TERM_NOT_FOUND", "未找到该节令活动，请刷新页面后再试")
        }
        _ if raw.contains("当前赛季数据为空") => {
            ("SEASON_UNAVAILABLE", "当前活动数据暂未开放，请稍后刷新重试")
        }
        _ if raw.contains("termId 必须") => {
            ("INVALID_SOLAR_TERM", "节令信息已失效，请刷新页面后重试")
        }
        _ if raw.contains("账号未运行") || raw.contains("账号已离线") => {
            ("ACCOUNT_OFFLINE", "当前账号尚未运行，请先启动账号后再试")
        }
        _ if raw.contains("API Timeout")
            || raw.contains("请求超时")
            || raw.contains("request timeout") =>
        {
            ("ACTIVITY_TIMEOUT", "活动服务响应超时，请稍后重试")
        }
        _ if raw.contains("连接未打开")
            || raw.contains("账号尚未登录")
            || raw.contains("requires Online")
            || raw.contains("ws not connected")
            || raw.contains("connection phase") =>
        {
            ("GAME_OFFLINE", "游戏连接尚未就绪，请稍后重试")
        }
        _ if raw.contains("请求队列已满") || raw.contains("request queue full") => {
            ("ACTIVITY_BUSY", "活动操作过于频繁，请稍后再试")
        }
        _ if raw.contains("发送失败")
            || raw.contains("请求被中断")
            || raw.contains("send failed") =>
        {
            ("ACTIVITY_REQUEST_INTERRUPTED", "活动请求未能完成，请稍后重试")
        }
        _ if raw.contains("不匹配的活动 ID")
            || raw.contains("未知操作类型")
            || raw.contains("回包缺少动态状态") =>
        {
            ("ACTIVITY_DATA_CHANGED", "活动数据已经更新，请刷新页面后再试")
        }
        _ if !business_code.is_empty()
            && business_code.chars().all(|c| c.is_ascii_uppercase() || c == '_') =>
        {
            let msg = raw
                .rsplit("business error: ")
                .next()
                .and_then(|s| s.split_once(':'))
                .map(|(_, m)| m.trim())
                .filter(|m| !m.is_empty())
                .unwrap_or("活动操作失败，请刷新页面后重试");
            return json!({ "ok": false, "error": msg, "errorCode": business_code });
        }
        _ => (
            if protocol_code.is_empty() {
                "ACTIVITY_OPERATION_FAILED"
            } else {
                protocol_code.as_str()
            },
            "活动操作失败，请刷新页面后重试",
        ),
    };
    json!({ "ok": false, "error": message, "errorCode": code })
}

async fn get_shop(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    activity_app_result(qq_farm_app::activity::shop(&ctx.app_context(), &id).await)
}

async fn get_solar_terms(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    activity_app_result(qq_farm_app::activity::solar_terms(&ctx.app_context(), &id).await)
}

async fn get_qingmei(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref())?;
    activity_app_result(qq_farm_app::activity::qingmei(&ctx.app_context(), &id).await)
}

async fn claim_qingmei_seed(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<Value> {
    let id = resolve_account_id(&ctx, &headers, None)?;
    activity_app_result(qq_farm_app::activity::claim_qingmei_seed(&ctx.app_context(), &id).await)
}

async fn start_qingmei_brew(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<QingMeiStartBody>,
) -> ApiResult<Value> {
    let id = resolve_account_id(&ctx, &headers, None)?;
    let input = body.ingredients.or(body.count).unwrap_or(Value::Null);
    activity_app_result(
        qq_farm_app::activity::start_qingmei_brew(&ctx.app_context(), &id, input).await,
    )
}

async fn continue_qingmei_brew(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<Value> {
    let id = resolve_account_id(&ctx, &headers, None)?;
    activity_app_result(qq_farm_app::activity::continue_qingmei_brew(&ctx.app_context(), &id).await)
}

async fn settle_qingmei_brew(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<Value> {
    let id = resolve_account_id(&ctx, &headers, None)?;
    activity_app_result(qq_farm_app::activity::settle_qingmei_brew(&ctx.app_context(), &id).await)
}
