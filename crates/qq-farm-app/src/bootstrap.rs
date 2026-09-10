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

/// 加载本地持久化状态（账号、用户、全局配置等）。
pub fn load_persisted_stores() {
    let _ = qq_farm_core::models::store::accounts::load_into_global();
    let () = qq_farm_core::models::user_store::auth::load_login_attempts();
    let () = qq_farm_core::models::user_store::auth::load_login_logs();
    let () = qq_farm_core::models::user_store::users::load_users();
    let _ = qq_farm_core::models::store::global_config::load_global_config();
    qq_farm_core::models::user_store::init();
}

/// 从环境变量 / `system_config` 构造网关模板。
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
    if let Some(mut sys) = qq_farm_core::models::store::global_config::get_system_config() {
        // 版本解析对齐 bot `resolveClientVersion`：保存的版本只有在其时间戳
        // 比默认值更新时才沿用，否则回默认（随 Login/Heartbeat 上报，旧版本
        // 可能被服务端冷落）；发生回退时持久化避免每次启动重复迁移。
        // bot 读取配置时优先使用 deviceInfo.clientVersion，然后将生效版本
        // 同步回顶层和设备字段。旧版 Rust 曾只解析顶层字段，导致两个版本
        // 不一致时 update_runtime_config 又把旧设备版本覆盖回运行时配置。
        let saved_version = if sys.device_info.client_version.trim().is_empty() {
            &sys.client_version
        } else {
            &sys.device_info.client_version
        };
        let (resolved_version, resolved_at) = qq_farm_core::config::resolve_client_version(
            saved_version,
            sys.client_version_updated_at,
        );
        let mut changed = resolved_version != sys.client_version
            || resolved_version != sys.device_info.client_version;
        sys.client_version = resolved_version;
        sys.client_version_updated_at = resolved_at;
        sys.device_info.client_version = sys.client_version.clone();
        let tz = qq_farm_core::config::normalize_time_zone(&sys.time_zone);
        if tz != sys.time_zone {
            sys.time_zone = tz;
            changed = true;
        }
        if changed {
            tracing::info!(
                version = %sys.client_version,
                "已升级过期的 client_version 配置"
            );
            qq_farm_core::models::store::global_config::set_system_config(sys.clone());
        }
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
            rt.device_info.user_agent
        };
        gateway_template.headers.insert("User-Agent".to_string(), ua);
        gateway_template.headers.insert("Origin".to_string(), gateway_origin.to_string());
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
        tsdk_wasm_path: std::env::var("TSDK_WASM_PATH").map_or_else(
            |_| qq_farm_core::config::get_resource_path(&["assets", "tsdk.wasm"]),
            PathBuf::from,
        ),
        data_root: qq_farm_core::config::get_data_dir(),
        ..Default::default()
    }));
    engine.spawn_event_bridge();
    engine.spawn_wx_keepalive();
    engine
        .qq_bot()
        .reconcile_background(qq_farm_core::models::store::global_config::gateway_qq_bot_config());
    let ctx = AppContext::new(engine);
    crate::qq_bot_bind::restore_saved_bindings(&ctx);
    ctx
}
