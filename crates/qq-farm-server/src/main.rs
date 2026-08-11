//! `qq-farm-server` —— HTTP + WebSocket 服务入口。
//!
//! ## 阶段
//!
//! - 0：占位启动 + 健康检查
//! - 2A：集成 `controllers/admin/farm-routes.ts` 全部 35 端点
//!
//! ## 启动流程
//!
//! 1. 加载配置（system_config）
//! 2. 构造 `RuntimeEngine`（含 runtime_state + relogin_reminder）
//! 3. 构造 `AdminContext`
//! 4. 挂载路由 + CORS 中间件
//! 5. 监听端口

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{routing::get, Json, Router};
use qq_farm_core::runtime::engine::{EngineConfig, GatewayConfigTemplate, RuntimeEngine};
use serde_json::json;
use tracing::info;

mod context;
mod middleware;
mod routes;

use crate::context::AdminContext;

#[tokio::main]
async fn main() -> Result<()> {
    qq_farm_core::utils::logger::init();
    dotenvy::dotenv().ok();

    // 构造 runtime engine
    let engine = Arc::new(RuntimeEngine::assemble(EngineConfig {
        max_workers: 16,
        gateway_template: GatewayConfigTemplate {
            server_url: std::env::var("FARM_SERVER_URL")
                .unwrap_or_else(|_| "https://game.qq.com".to_string()),
            platform: "qq".to_string(),
            os: std::env::var("FARM_OS").unwrap_or_else(|_| "linux".to_string()),
            client_version: std::env::var("FARM_CLIENT_VERSION")
                .unwrap_or_else(|_| "1.0.0".to_string()),
            headers: std::collections::HashMap::new(),
        },
        ..Default::default()
    }));

    let ctx = Arc::new(AdminContext::new(engine));

    // 路由
    let app = Router::new()
        .route("/health", get(health))
        .merge(routes::build())
        .with_state(ctx)
        .layer(axum::middleware::from_fn(middleware::cors_layer));

    let port: u16 = std::env::var("ADMIN_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3007);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!(%addr, "qq-farm-server 启动 (阶段 2A: farm routes)");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// 健康检查
async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "qq-farm-server",
        "stage": "2A",
        "routes": "farm"
    }))
}
