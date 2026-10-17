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
#[cfg(any(mobile, test))]
mod release;
#[cfg(desktop)]
mod shell;
mod state;
#[cfg(desktop)]
mod tray;
#[cfg(desktop)]
mod updater;

use std::sync::Arc;

use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::state::DesktopState;
use qq_farm_app::runtime_lease::{EngineLease, EngineLeaseError};

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
        .with_env(|env| rustls_platform_verifier::android::init_with_env(env, context))
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
        // 单实例保护必须最先注册：第二个进程只聚焦已有窗口后退出。
        // 没有它，双击两次 exe = 两个进程各自自动登录全部账号，
        // 服务端按"已在其他终端登录"互相顶号，形成跨进程互踢循环（2026-09-11 事故）。
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
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
            let data_dir = qq_farm_core::config::get_data_dir();
            let lease = match EngineLease::acquire_for(&data_dir, "tauri") {
                Ok(lease) => lease,
                Err(error) => {
                    if matches!(error, EngineLeaseError::AlreadyRunning) {
                        #[cfg(desktop)]
                        let owner = EngineLease::wake_existing_owner(&data_dir);
                        #[cfg(not(desktop))]
                        let _ = EngineLease::wake_existing_owner(&data_dir);
                        #[cfg(desktop)]
                        if owner.as_deref().is_some_and(|owner| owner != "tauri") {
                            let dialog_handle = app.handle().clone();
                            app.dialog()
                                .message("GPUI 桌面端正在使用此数据目录，已唤起现有窗口。")
                                .title("QQ Farm 已在运行")
                                .kind(MessageDialogKind::Info)
                                .show(move |_| dialog_handle.exit(0));
                            let window_handle = app.handle().clone();
                            std::thread::spawn(move || {
                                for _ in 0..30 {
                                    if let Some(window) = window_handle.get_webview_window("main") {
                                        let _ = window.hide();
                                        break;
                                    }
                                    std::thread::sleep(std::time::Duration::from_millis(100));
                                }
                            });
                            return Ok(());
                        }
                    }
                    return Err(Box::new(error).into());
                }
            };
            #[cfg(desktop)]
            if let Some(wake_receiver) = lease.take_wake_receiver() {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    while wake_receiver.recv().is_ok() {
                        for _ in 0..30 {
                            if app_handle.get_webview_window("main").is_some() {
                                crate::shell::show_main_window(&app_handle);
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                    }
                });
            }
            let max_workers =
                std::env::var("MAX_WORKERS").ok().and_then(|s| s.parse().ok()).unwrap_or(16);
            let app_ctx = Arc::new(qq_farm_app::bootstrap::assemble_app_context(
                max_workers,
                "https://game.qq.com",
            ));
            let desktop = DesktopState::with_lease(app_ctx, lease);
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
            commands::application::get_app_info,
            commands::application::check_app_update,
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
            commands::farm::farm_fertilize_all,
            // solar calendar
            commands::solar_calendar::solar_calendar_snapshot,
            commands::solar_calendar::solar_calendar_operate,
            // milk tea activity
            commands::milk_tea::milk_tea_snapshot,
            commands::milk_tea::milk_tea_operate,
            // share reward activity
            commands::share_reward::share_reward_snapshot,
            commands::share_reward::share_reward_operate,
            // friend
            commands::friend::friend_list,
            commands::friend::friend_lands,
            commands::friend::friend_op,
            commands::friend::friend_interact_records,
            commands::friend::friend_blacklist_toggle,
            commands::friend::friend_delete,
            // commerce
            commands::commerce::commerce_mall_catalog,
            commands::commerce::commerce_mall_purchase,
            commands::commerce::commerce_mystery_shop,
            commands::commerce::commerce_mystery_purchase,
            // pets
            commands::pets::pet_info,
            commands::pets::pet_deploy,
            commands::pets::pet_activate,
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
            commands::friend::illustrated_claim,
            commands::friend::seeds_purchase,
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

    app.run(|app_handle, event| {
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event {
            shell::show_main_window(app_handle);
        }
        if matches!(event, tauri::RunEvent::Exit) {
            if let Some(state) = app_handle.try_state::<DesktopState>() {
                tauri::async_runtime::block_on(qq_farm_app::lifecycle::shutdown(
                    &state.app,
                    std::time::Duration::from_secs(5),
                ));
            }
        }
    });
}

#[cfg(test)]
mod acl_tests {
    /// 防回归：`generate_handler!` 注册的每条 IPC 命令都必须在
    /// `permissions/desktop.toml` 的 ACL 白名单里，漏声明会在运行期报
    /// "Command xxx not allowed by ACL"。
    #[test]
    fn every_handler_command_is_allowed_by_acl() {
        let lib_src = include_str!("lib.rs");
        let mut handler: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for line in lib_src.lines() {
            let line = line.trim().trim_end_matches(',');
            if let Some((path, _)) = line.split_once("::") {
                // 形如 commands::farm::farm_get_logs
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
