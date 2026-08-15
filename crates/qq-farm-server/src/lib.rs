//! `qq-farm-server` lib —— 暴露给 E2E 测试和嵌入使用。
//!
//! 主要导出：
//! - [`build_test_app`]：构造测试用 axum Router（含 AdminContext + CORS）
//! - [`TestApp`]：测试上下文配置

use std::sync::Arc;

use axum::{routing::get, Json, Router};
use tower_http::services::ServeDir;
use serde_json::json;

pub mod config;
pub mod context;
pub mod middleware;
pub mod routes;
pub mod sessions;
pub mod socket;

pub use config::ServerConfig;
pub use context::AdminContext;

/// 测试用上下文配置
#[derive(Debug, Clone, Default)]
pub struct TestApp {
    /// 自定义 server_url（默认 "ws://localhost:0"）
    pub server_url: Option<String>,
    /// 自定义 platform（默认 "test"）
    pub platform: Option<String>,
}

/// 构造测试用 axum Router（包含完整路由 + middleware + health）
pub async fn build_test_app(cfg: TestApp) -> Router {
    use std::sync::Once;
    use qq_farm_core::runtime::engine::{EngineConfig, GatewayConfigTemplate, RuntimeEngine};

    // persist_global 会写 accounts.json；测试必须隔离，避免污染开发 data/
    static INIT_DATA_DIR: Once = Once::new();
    INIT_DATA_DIR.call_once(|| {
        let dir = std::env::temp_dir().join(format!("qq-farm-e2e-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("FARM_DATA_DIR", &dir);
    });

    let _ = cfg;
    let engine = Arc::new(RuntimeEngine::assemble(EngineConfig {
        max_workers: 4,
        gateway_template: GatewayConfigTemplate {
            server_url: cfg
                .server_url
                .clone()
                .unwrap_or_else(|| "ws://localhost:0".to_string()),
            platform: cfg.platform.clone().unwrap_or_else(|| "test".to_string()),
            os: "linux".to_string(),
            client_version: "0.1.0-test".to_string(),
            headers: std::collections::HashMap::new(),
        },
        ..Default::default()
    }));

    let ctx = Arc::new(AdminContext::new(engine));
    ctx.engine.spawn_event_bridge();

    // 启动时尝试加载状态（即便失败也不影响 E2E）
    let _ = qq_farm_core::models::store::accounts::load_into_global();
    let _ = qq_farm_core::models::user_store::auth::load_login_attempts();
    let _ = qq_farm_core::models::user_store::auth::load_login_logs();
    let _ = qq_farm_core::models::user_store::users::load_users();
    let _ = qq_farm_core::models::user_store::users::load_cards();
    let _ = qq_farm_core::models::user_store::card_claim::load_card_claim_records();
    // 初始化默认 admin 账号（admin/admin）—— 但只在内存中无 admin 时
    qq_farm_core::models::user_store::init();

    // 重置所有登录尝试：避免多测试用例间 127.0.0.1 rate limit 互相干扰
    qq_farm_core::models::user_store::auth::reset_all_login_attempts();

    // 注意：测试用卡密由各测试通过 `qq_farm_core::models::user_store::users::create_card` 自助生成（mint_test_card helper）。

    Router::new()
        .route("/health", get(health))
        .route("/ws", get(socket::ws_handler))
        .merge(routes::build(ctx.clone()))
        .nest_service(
            "/game-config",
            ServeDir::new(qq_farm_core::config::game_config_static_dir()),
        )
        .with_state(ctx)
        .layer(axum::middleware::from_fn(middleware::cors_layer))
}

/// 健康检查
pub async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "qq-farm-server",
        "stage": "2G",
        "routes": "all"
    }))
}
