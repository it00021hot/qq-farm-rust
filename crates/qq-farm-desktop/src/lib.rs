//! QQ Farm Tauri v2 桌面适配层。
//!
//! 仅依赖 `qq-farm-app` / `qq-farm-core`；不依赖 `qq-farm-server`，不把 Tauri 泄漏进 app。

mod assets;
mod commands;
mod error;
mod events;
#[cfg(target_os = "macos")]
mod menu;
mod paths;
mod shell;
mod state;
mod tray;
mod updater;

use std::sync::Arc;

use tauri::Manager;

use crate::state::DesktopState;

/// 桌面端进程入口（由 `main` / 移动端入口调用）。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    paths::prepare_data_dir();
    qq_farm_core::utils::logger::init();

    // RuntimeEngine / AppEvent 桥依赖当前线程的 Tokio runtime（与旧 GPUI 入口一致）。
    let runtime = Box::leak(Box::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("qq-farm-tokio")
            .build()
            .expect("tokio runtime"),
    ));
    let _enter = runtime.enter();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .register_uri_scheme_protocol("farmcfg", |_ctx, request| assets::handle_request(request))
        .on_menu_event(|app, event| shell::handle_menu_event(app, event.id()))
        .setup(|app| {
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
            tray::install(app.handle())?;
            shell::install_close_to_tray(app.handle());
            updater::setup(app.handle());

            let cfg_dir = qq_farm_core::config::paths::game_config_static_dir();
            tracing::info!(dir = %cfg_dir.display(), "game-config static dir");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // snapshot
            commands::snapshot::desktop_ready,
            commands::snapshot::get_snapshot,
            // account
            commands::account::list_accounts,
            commands::account::list_accounts_page,
            commands::account::upsert_account,
            commands::account::delete_account,
            commands::account::start_account,
            commands::account::stop_account,
            commands::account::remark_account,
            commands::account::wx_login_create,
            commands::account::wx_login_poll,
            commands::account::wx_login_confirm,
            commands::account::wx_login_code,
            commands::account::wx_login_destroy,
            commands::account::wx_quick_login_create,
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
            commands::farm::farm_get_automation,
            commands::farm::farm_set_automation,
            commands::farm::farm_get_logs,
            commands::farm::farm_clear_logs,
            commands::farm::farm_analytics,
            commands::farm::farm_get_plant_blacklist,
            commands::farm::farm_set_plant_blacklist,
            // friend
            commands::friend::friend_list,
            commands::friend::friend_sync,
            commands::friend::friend_clear_cache,
            commands::friend::friend_lands,
            commands::friend::friend_op,
            commands::friend::friend_interact_records,
            commands::friend::friend_blacklist_toggle,
            commands::friend::friend_known_gids,
            commands::friend::friend_set_known_gids,
            // activity
            commands::activity::activity_state,
            commands::activity::activity_snapshot,
            commands::activity::activity_claim_battle_pass,
            commands::activity::activity_light_constellation,
            commands::activity::activity_exchange_star_sand,
            commands::activity::activity_claim_solar_term,
            commands::activity::activity_claim_qingmei_seed,
            commands::activity::activity_qingmei_brew_start,
            commands::activity::activity_qingmei_brew_continue,
            commands::activity::activity_qingmei_brew_settle,
            // commerce
            commands::commerce::commerce_overview,
            commands::commerce::commerce_mall_catalog,
            commands::commerce::commerce_mall_purchase,
            commands::commerce::commerce_mystery_shop,
            commands::commerce::commerce_mystery_purchase,
            // settings
            commands::settings::get_settings,
            commands::settings::get_settings_panel,
            commands::settings::save_settings,
            commands::settings::get_offline_reminder,
            commands::settings::set_offline_reminder,
            commands::settings::test_offline_reminder,
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
