//! `qq-farm-server` —— HTTP + Socket.IO 服务入口。
//!
//! 通过 `qq_farm_app::bootstrap::assemble_app_context` 组装引擎，再挂 HTTP / Socket。

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{routing::get, Router};
use tracing::info;

use qq_farm_server::{config::ServerConfig, context::AdminContext, middleware, routes, socket};

#[tokio::main]
async fn main() -> Result<()> {
    qq_farm_core::utils::logger::init();
    dotenvy::dotenv().ok();
    let server_cfg = ServerConfig::from_env();

    let app_ctx = qq_farm_app::bootstrap::assemble_app_context(
        server_cfg.max_workers,
        &server_cfg.gateway_origin,
    );
    let ctx = Arc::new(AdminContext::from_app(app_ctx));
    let (sio_layer, io) = socket::setup_socketio(ctx.clone());
    socket::spawn_socket_forwarder(io, ctx.clone());

    let accounts = qq_farm_core::models::store::accounts::get_accounts();
    if accounts.is_empty() {
        info!("未发现账号，请访问管理面板添加账号");
    } else {
        info!(count = accounts.len(), "发现账号");
        let mut started = 0u32;
        for acc in accounts {
            if acc.has_wx_auth() {
                continue;
            }
            if acc.code.trim().is_empty() {
                continue;
            }
            let models_acc = qq_farm_core::models::AccountSession::from_store(&acc);
            if let Err(e) = ctx.engine.start_worker(models_acc) {
                tracing::warn!(account_id = %acc.id, "启动 worker 失败: {e}");
            } else {
                started += 1;
            }
        }
        info!(started, "已启动填了 Code 的 QQ 账号");
        ctx.engine.schedule_wx_authorized_start();
    }

    let app = Router::new()
        .route("/health", get(qq_farm_server::health))
        .route("/ws", get(socket::ws_handler))
        .merge(routes::build(ctx.clone()))
        .nest_service(
            "/game-config",
            tower_http::services::ServeDir::new(qq_farm_core::config::game_config_static_dir()),
        )
        .with_state(ctx.clone())
        .layer(sio_layer)
        .layer(axum::middleware::from_fn(middleware::cors_layer));

    let port = server_cfg.admin_port;
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!(%addr, "qq-farm-server 启动");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
