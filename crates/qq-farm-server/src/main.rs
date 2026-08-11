//! `qq-farm-server` —— HTTP + WebSocket 服务入口。
//!
//! 阶段 0：仅占位启动，监听端口 + 健康检查路由。
//! 阶段 4：迁移原项目 `core/src/controllers/admin/*` 的全部 HTTP API。

use std::net::SocketAddr;

use anyhow::Result;
use axum::{routing::get, Json, Router};
use serde_json::json;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    qq_farm_core::utils::logger::init();
    dotenvy::dotenv().ok();

    let app = Router::new().route("/health", get(health));

    let port: u16 = std::env::var("ADMIN_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3007);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!(%addr, "qq-farm-server 启动 (阶段 0 占位)");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// 健康检查
async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "qq-farm-server",
        "stage": "0",
    }))
}
