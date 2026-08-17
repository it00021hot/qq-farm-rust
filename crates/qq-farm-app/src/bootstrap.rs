//! 共享启动：加载 store + 组装 RuntimeEngine（server / desktop 共用）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use qq_farm_core::config::{
    get_runtime_config, sanitize_gateway_url, update_runtime_config, DeviceInfo,
    DEFAULT_CLIENT_VERSION, DEFAULT_GATEWAY_URL,
};
use qq_farm_core::runtime::engine::{EngineConfig, GatewayConfigTemplate, RuntimeEngine};

use crate::session::AppContext;

/// 加载本地持久化状态（账号、用户、卡密、全局配置等）。
pub fn load_persisted_stores() {
    let _ = qq_farm_core::models::store::accounts::load_into_global();
    let _ = qq_farm_core::models::user_store::auth::load_login_attempts();
    let _ = qq_farm_core::models::user_store::auth::load_login_logs();
    let _ = qq_farm_core::models::user_store::users::load_users();
    let _ = qq_farm_core::models::user_store::users::load_cards();
    let _ = qq_farm_core::models::store::global_config::load_global_config();
    let _ = qq_farm_core::models::user_store::card_claim::load_card_claim_records();
    qq_farm_core::models::user_store::init();
}

/// 从环境变量 / system_config 构造网关模板。
#[must_use]
pub fn gateway_template_from_env(gateway_origin: &str) -> GatewayConfigTemplate {
    let mut gateway_template = GatewayConfigTemplate {
        server_url: std::env::var("FARM_SERVER_URL")
            .unwrap_or_else(|_| DEFAULT_GATEWAY_URL.to_string()),
        platform: std::env::var("FARM_PLATFORM").unwrap_or_else(|_| "qq".to_string()),
        os: std::env::var("FARM_OS").unwrap_or_else(|_| "Windows".to_string()),
        client_version: std::env::var("FARM_CLIENT_VERSION")
            .unwrap_or_else(|_| DEFAULT_CLIENT_VERSION.to_string()),
        headers: HashMap::new(),
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
            DeviceInfo::windows_pc().user_agent
        } else {
            rt.device_info.user_agent.clone()
        };
        gateway_template
            .headers
            .insert("User-Agent".to_string(), ua);
        gateway_template
            .headers
            .insert("Origin".to_string(), gateway_origin.to_string());
    }
    gateway_template
}

/// 组装引擎并返回 [`AppContext`]。
#[must_use]
pub fn assemble_app_context(max_workers: usize, gateway_origin: &str) -> AppContext {
    load_persisted_stores();
    let gateway_template = gateway_template_from_env(gateway_origin);
    let engine = Arc::new(RuntimeEngine::assemble(EngineConfig {
        max_workers,
        gateway_template,
        tsdk_wasm_path: std::env::var("TSDK_WASM_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                qq_farm_core::config::get_resource_path(&["assets", "tsdk.wasm"])
            }),
        data_root: qq_farm_core::config::get_data_dir(),
        ..Default::default()
    }));
    engine.spawn_event_bridge();
    AppContext::new(engine)
}
