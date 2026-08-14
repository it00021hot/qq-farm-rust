//! `qq-farm-server` —— HTTP + Socket.IO 服务入口。
//!
//! 对齐原 bot `runtime-engine.start()`：
//! 加载 store / system_config → assemble engine → Socket.IO → 自动启动全部账号。

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{routing::get, Router};
use qq_farm_core::config::{
    get_runtime_config, sanitize_gateway_url, update_runtime_config, DEFAULT_CLIENT_VERSION,
    DEFAULT_GATEWAY_URL,
};
use qq_farm_core::runtime::engine::{EngineConfig, GatewayConfigTemplate, RuntimeEngine};
use tracing::info;

use qq_farm_server::{context::AdminContext, middleware, routes, socket};

#[tokio::main]
async fn main() -> Result<()> {
    qq_farm_core::utils::logger::init();
    dotenvy::dotenv().ok();

    // 先加载持久化状态，再 assemble（gateway 要用 system_config）
    let _ = qq_farm_core::models::store::accounts::load_into_global();
    let _ = qq_farm_core::models::user_store::auth::load_login_attempts();
    let _ = qq_farm_core::models::user_store::auth::load_login_logs();
    let _ = qq_farm_core::models::user_store::users::load_users();
    let _ = qq_farm_core::models::user_store::users::load_cards();
    let _ = qq_farm_core::models::store::global_config::load_global_config();
    let _ = qq_farm_core::models::user_store::card_claim::load_card_claim_records();
    qq_farm_core::models::user_store::init();

    let mut gateway_template = GatewayConfigTemplate {
        server_url: std::env::var("FARM_SERVER_URL")
            .unwrap_or_else(|_| DEFAULT_GATEWAY_URL.to_string()),
        platform: std::env::var("FARM_PLATFORM").unwrap_or_else(|_| "qq".to_string()),
        os: std::env::var("FARM_OS").unwrap_or_else(|_| "Windows".to_string()),
        client_version: std::env::var("FARM_CLIENT_VERSION")
            .unwrap_or_else(|_| DEFAULT_CLIENT_VERSION.to_string()),
        headers: std::collections::HashMap::new(),
    };
    if let Some(sys) = qq_farm_core::models::store::global_config::get_system_config() {
        update_runtime_config(&sys);
        if !sys.server_url.is_empty() {
            gateway_template.server_url = sys.server_url;
        }
        if !sys.platform.is_empty() {
            gateway_template.platform = sys.platform;
        }
        if !sys.os.is_empty() {
            gateway_template.os = sys.os;
        }
        if !sys.client_version.is_empty() {
            gateway_template.client_version = sys.client_version;
        }
    }

    gateway_template.server_url = sanitize_gateway_url(&gateway_template.server_url);
    let rt = get_runtime_config();
    if gateway_template.headers.is_empty() {
        let ua = if rt.device_info.user_agent.is_empty() {
            qq_farm_core::config::DeviceInfo::windows_pc().user_agent
        } else {
            rt.device_info.user_agent.clone()
        };
        gateway_template
            .headers
            .insert("User-Agent".to_string(), ua);
        gateway_template.headers.insert(
            "Origin".to_string(),
            "https://gate-obt.nqf.qq.com".to_string(),
        );
    }
    info!(
        server_url = %gateway_template.server_url,
        client_version = %gateway_template.client_version,
        platform = %gateway_template.platform,
        "网关配置"
    );

    let engine = Arc::new(RuntimeEngine::assemble(EngineConfig {
        max_workers: 16,
        gateway_template,
        tsdk_wasm_path: std::env::var("TSDK_WASM_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| qq_farm_core::config::get_resource_path(&["assets", "tsdk.wasm"])),
        data_root: qq_farm_core::config::get_data_dir(),
        ..Default::default()
    }));
    engine.spawn_event_bridge();

    let ctx = Arc::new(AdminContext::new(engine));
    let (sio_layer, io) = socket::setup_socketio(ctx.clone());
    socket::spawn_socket_forwarder(io, ctx.clone());

    let accounts = qq_farm_core::models::store::accounts::get_accounts();
    if accounts.is_empty() {
        info!("未发现账号，请访问管理面板添加账号");
    } else {
        info!(count = accounts.len(), "发现账号，正在启动");
        for acc in accounts {
            if acc.code.trim().is_empty() {
                continue;
            }
            let models_acc = qq_farm_core::models::Account::from_store(&acc);
            if let Err(e) = ctx.engine.start_worker(models_acc) {
                tracing::warn!(account_id = %acc.id, "启动 worker 失败: {e}");
            }
        }
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

    let port: u16 = std::env::var("ADMIN_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3007);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!(%addr, "qq-farm-server 启动");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
