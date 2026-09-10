//! QQ Farm Tauri v2 桌面适配层。
//!
//! 仅依赖 `qq-farm-app` / `qq-farm-core`；不把 Tauri 泄漏进 app。

mod assets;
mod commands;
mod error;
mod events;
#[cfg(target_os = "macos")]
mod menu;
mod paths;
#[cfg(desktop)]
mod shell;
mod state;
#[cfg(desktop)]
mod tray;
#[cfg(desktop)]
mod updater;

use std::sync::Arc;

use tauri::Manager;

use crate::state::DesktopState;

#[cfg(target_os = "android")]
use jni::objects::{JClass, JObject};

/// Initialize rustls' Android system certificate verifier before any async
/// network request can be created by reqwest.
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_qqfarm_rust_MainActivity_initRustlsVerifier<'caller>(
    mut unowned_env: jni::EnvUnowned<'caller>,
    _class: JClass<'caller>,
    context: JObject<'caller>,
) {
    use jni::errors::LogErrorAndDefault;

    unowned_env
        .with_env(|env| {
            rustls_platform_verifier::android::init_with_env(env, context)
        })
        .resolve::<LogErrorAndDefault>();
}

/// 桌面端进程入口（由 `main` / 移动端入口调用）。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    paths::prepare_data_dir();
    #[cfg(not(target_os = "android"))]
    qq_farm_core::utils::logger::init();

    // 单一 Tokio runtime：业务 `tokio::spawn` 与 Tauri async_runtime 共用，避免双 runtime。
    let runtime = Box::leak(Box::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("qq-farm-tokio")
            .build()
            .expect("tokio runtime"),
    ));
    let handle = runtime.handle().clone();
    tauri::async_runtime::set(handle.clone());
    // setup / sync IPC 线程上的 `tokio::spawn` 需要当前线程已 enter。
    let _enter = handle.enter();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .register_uri_scheme_protocol("farmcfg", |_ctx, request| assets::handle_request(request));

    #[cfg(desktop)]
    let builder = builder.on_menu_event(|app, event| shell::handle_menu_event(app, event.id()));

    let app = builder
        .setup(|app| {
            #[cfg(target_os = "android")]
            {
                paths::prepare_android_data_dir(app.handle());
                qq_farm_core::utils::logger::init();
            }
            paths::apply_bundled_resource_env(app.handle());
            let max_workers =
                std::env::var("MAX_WORKERS").ok().and_then(|s| s.parse().ok()).unwrap_or(16);
            let app_ctx = Arc::new(qq_farm_app::bootstrap::assemble_app_context(
                max_workers,
                "https://game.qq.com",
            ));
            let desktop = DesktopState::new(app_ctx);
            events::spawn_event_bridge(app.handle().clone(), desktop.clone());
            desktop.app.engine.schedule_wx_authorized_start();
            app.manage(desktop);

            #[cfg(target_os = "macos")]
            menu::install(app.handle())?;
            #[cfg(desktop)]
            {
                tray::install(app.handle())?;
                shell::install_close_to_tray(app.handle());
                updater::setup(app.handle());
            }

            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_decorations(false);
                let _ = window.set_shadow(true);

                // 显式把窗口放到主显示器中央并设默认大小，覆盖 config 的 center 选项
                // （config 的 center 在 set_decorations 之后偶尔会被重置；这里用主显示器尺寸重算）
                if let Ok(Some(monitor)) = window.primary_monitor() {
                    let mon_size = monitor.size();
                    let mon_pos = monitor.position();
                    let scale = monitor.scale_factor();
                    // 1440 x 900 逻辑像素（与 tauri.conf.json 一致）
                    let win_w = 1440.0_f64;
                    let win_h = 900.0_f64;
                    let mon_w_logical = mon_size.width as f64 / scale;
                    let mon_h_logical = mon_size.height as f64 / scale;
                    let mon_x_logical = mon_pos.x as f64 / scale;
                    let mon_y_logical = mon_pos.y as f64 / scale;
                    let x = mon_x_logical + (mon_w_logical - win_w) / 2.0;
                    let y = mon_y_logical + (mon_h_logical - win_h) / 2.0;
                    use tauri::{LogicalPosition, LogicalSize};
                    let _ = window.set_size(LogicalSize::new(win_w, win_h));
                    let _ = window.set_position(LogicalPosition::new(x, y));
                    tracing::info!(
                        win_w,
                        win_h,
                        x,
                        y,
                        mon_w_logical,
                        mon_h_logical,
                        scale,
                        "主窗口已居中并设定尺寸"
                    );
                } else {
                    // 拿不到主显示器时退化用 tauri.conf.json 的 center 选项
                    let _ = window.center();
                }
            }

            let cfg_dir = qq_farm_core::config::paths::game_config_static_dir();
            tracing::info!(dir = %cfg_dir.display(), "game-config static dir");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // snapshot
            commands::snapshot::desktop_ready,
            commands::snapshot::get_snapshot,
            // account
            commands::account::list_accounts_page,
            commands::account::upsert_account,
            commands::account::delete_account,
            commands::account::start_account,
            commands::account::stop_account,
            commands::account::wx_login_create,
            commands::account::wx_login_poll,
            commands::account::wx_login_confirm,
            commands::account::wx_login_code,
            commands::account::wx_quick_login_create,
            commands::account::wx_quick_login_detect,
            commands::account::wx_quick_login_authorize,
            commands::account::wx_quick_login_confirm,
            // qq login (NapCat)
            commands::qq_login::get_qq_login_settings,
            commands::qq_login::save_qq_login_settings,
            commands::qq_login::qq_login_create_task,
            commands::qq_login::qq_login_task_status,
            commands::qq_login::qq_login_miniapp_code,
            commands::qq_login::qq_login_cancel_task,
            // farm
            commands::farm::farm_status_detail,
            commands::farm::farm_diamond,
            commands::farm::farm_lands,
            commands::farm::farm_operate,
            commands::farm::farm_bag,
            commands::farm::farm_bag_sell,
            commands::farm::farm_bag_use,
            commands::farm::farm_seeds,
            commands::farm::farm_daily_gifts,
            commands::farm::farm_get_logs,
            commands::farm::farm_clear_logs,
            commands::farm::farm_analytics,
            commands::farm::farm_get_plant_blacklist,
            commands::farm::farm_set_plant_blacklist,
            commands::farm::farm_fertilizer_check_and_buy,
            // friend
            commands::friend::friend_list,
            commands::friend::friend_lands,
            commands::friend::friend_op,
            commands::friend::friend_interact_records,
            commands::friend::friend_blacklist_toggle,
            commands::friend::friend_delete,
            // activity
            commands::activity::activity_snapshot,
            commands::activity::activity_claim_battle_pass,
            commands::activity::activity_light_constellation,
            commands::activity::activity_exchange_star_sand,
            commands::activity::activity_claim_solar_term,
            commands::activity::activity_claim_qingmei_seed,
            commands::activity::activity_qingmei_brew_start,
            commands::activity::activity_qingmei_brew_continue,
            commands::activity::activity_qingmei_brew_settle,
            commands::activity::activity_claim_qixi_bridge,
            commands::activity::activity_gift_qixi_sachet,
            commands::activity::activity_get_charity,
            commands::activity::activity_claim_charity_seeds,
            commands::activity::activity_donate_charity_love,
            commands::activity::activity_claim_charity_daily_gift,
            commands::activity::activity_claim_charity_progress_reward,
            // weather
            commands::weather::weather_snapshot,
            commands::weather::weather_friends,
            commands::weather::weather_friends_scan,
            commands::weather::weather_exchange_collector,
            commands::weather::weather_collect,
            commands::weather::weather_summon,
            commands::weather::weather_mischief_frog,
            commands::weather::weather_mischief_cloud,
            commands::weather::weather_advance_research,
            // commerce
            commands::commerce::commerce_mall_catalog,
            commands::commerce::commerce_mall_purchase,
            commands::commerce::commerce_mystery_shop,
            commands::commerce::commerce_mystery_purchase,
            // pets
            commands::pets::pet_info,
            commands::pets::pet_deploy,
            commands::pets::pet_withdraw,
            commands::pets::pet_food_use,
            commands::pets::pet_protect_logs,
            commands::pets::dog_skill_gifts_status,
            commands::pets::dog_skill_gifts_claim,
            // interaction items + illustrated
            commands::friend::friend_interaction_items,
            commands::friend::friend_interaction_items_use,
            commands::friend::farm_interaction_items,
            commands::friend::farm_interaction_items_use,
            commands::friend::illustrated_snapshot,
            // settings
            commands::settings::get_settings_panel,
            commands::settings::save_settings,
            commands::settings::get_offline_reminder,
            commands::settings::set_offline_reminder,
            commands::settings::test_offline_reminder,
            commands::settings::get_qq_bot_bind_status,
            commands::settings::start_qq_bot_bind,
            commands::settings::poll_qq_bot_bind,
            commands::settings::unbind_qq_bot,
            commands::settings::get_device_presets,
            commands::settings::get_system_config,
            commands::settings::set_system_config,
            commands::settings::reset_system_config,
            // config
            commands::config::config_list_seeds,
            commands::config::config_list_fruits,
            commands::config::config_list_items,
            commands::config::config_list_plants,
            commands::config::config_list_item_types,
            commands::config::config_add,
            commands::config::config_modify,
            commands::config::config_delete,
        ])
        .build(tauri::generate_context!())
        .expect("error while building qq-farm-desktop");

    #[cfg(target_os = "macos")]
    app.run(|app_handle, event| {
        if let tauri::RunEvent::Reopen { .. } = event {
            shell::show_main_window(app_handle);
        }
    });
    #[cfg(not(target_os = "macos"))]
    app.run(|_, _| {});
}

#[cfg(test)]
mod acl_tests {
    /// 防回归：`generate_handler!` 注册的每条 IPC 命令都必须在
    /// `permissions/desktop.toml` 的 ACL 白名单里，漏声明会在运行期报
    /// "Command xxx not allowed by ACL"（小红花四条命令曾漏过）。
    #[test]
    fn every_handler_command_is_allowed_by_acl() {
        let lib_src = include_str!("lib.rs");
        let mut handler: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for line in lib_src.lines() {
            let line = line.trim().trim_end_matches(',');
            if let Some((path, _)) = line.split_once("::") {
                // 形如 commands::activity::activity_snapshot
                if path == "commands" {
                    if let Some(name) = line.rsplit("::").next() {
                        if name.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                            && !name.is_empty()
                        {
                            handler.insert(name.to_string());
                        }
                    }
                }
            }
        }
        let acl_src = include_str!("../permissions/desktop.toml");
        let mut acl: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for line in acl_src.lines() {
            let line = line.trim().trim_end_matches(',');
            if let Some(name) = line.strip_prefix('"').and_then(|l| l.strip_suffix('"')) {
                acl.insert(name);
            }
        }
        assert!(!handler.is_empty(), "handler 命令解析失败");
        assert!(!acl.is_empty(), "ACL 白名单解析失败");
        let missing: Vec<&String> = handler.iter().filter(|c| !acl.contains(c.as_str())).collect();
        assert!(
            missing.is_empty(),
            "以下 IPC 命令未在 permissions/desktop.toml 声明，运行期会被 ACL 拦截: {missing:?}"
        );
    }
}
