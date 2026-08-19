//! Daily gifts 路由 — 每日礼包概览。

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    routing::get,
    Router,
};
use serde::Deserialize;

use crate::context::{ok_data, AdminContext, ApiResult};
use crate::routes::resolve_id;

/// 构造 daily-gifts 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new().route("/api/daily-gifts", get(get_daily_gifts))
}

#[derive(Debug, Deserialize)]
struct AccountQuery {
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

async fn get_daily_gifts(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_id(&ctx, &headers, q.account_id.as_deref())?;
    let data = qq_farm_app::farm::daily_gift_overview(&ctx.app_context(), &id)
        .await
        .map_err(crate::context::ApiError::from)?;
    ok_data(data)
}
