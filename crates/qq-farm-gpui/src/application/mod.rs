use crate::{desktop::FarmDesktop, runtime::UiPreferences, theme, tray};
use gpui_kit::component::theme::{Theme, ThemeMode};
use gpui_kit::{
    application as gpui_application, init, open_window, px, size, AppContext as _, TitlebarOptions,
    WindowBounds, WindowOptions,
};
use qq_farm_app::{runtime_lease::EngineLease, AppContext};
use std::sync::Arc;
use std::time::Duration;

mod commands;
pub(crate) use commands::{configure_commands, register_window_commands, set_application_menus};

pub(crate) fn run(
    ui_app: Arc<AppContext>,
    ui_runtime: Arc<tokio::runtime::Runtime>,
    ui_lease: Arc<EngineLease>,
    ui_preferences: UiPreferences,
) {
    let application = gpui_application()
        .with_assets(gpui_kit::assets::AllAssets)
        .with_http_client(Arc::new(crate::http::ReqwestHttpClient::new(&ui_runtime)));
    application.on_reopen(restore_main_window);
    application.run(move |cx| {
        init(cx);
        theme::init(cx);
        let quit_app = Arc::clone(&ui_app);
        let quit_runtime = Arc::clone(&ui_runtime);
        cx.on_app_quit(move |_| {
            let app = Arc::clone(&quit_app);
            let job = quit_runtime.spawn(async move {
                qq_farm_app::lifecycle::shutdown(&app, Duration::from_millis(150)).await;
            });
            async move {
                let _ = job.await;
            }
        })
        .detach();
        let preferences = ui_preferences.clone();
        configure_commands(cx, &ui_preferences.language);
        let (tray_sender, tray_receiver) = futures::channel::mpsc::unbounded();
        if let Some(wake_receiver) = ui_lease.take_wake_receiver() {
            let wake_sender = tray_sender.clone();
            std::thread::spawn(move || {
                while wake_receiver.recv().is_ok() {
                    if wake_sender.unbounded_send(tray::Action::ShowWindow).is_err() {
                        break;
                    }
                }
            });
        }
        let tray = tray::install(tray_sender, &preferences.language);
        let close_to_tray = tray.is_some();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(
                size(
                    px(preferences.window_width.max(1100) as f32),
                    px(preferences.window_height.max(720) as f32),
                ),
                cx,
            )),
            window_min_size: Some(size(px(1100.), px(720.))),
            app_id: Some("com.qqfarm.gpui".into()),
            titlebar: Some(TitlebarOptions { title: Some("QQ Farm".into()), ..Default::default() }),
            ..Default::default()
        };
        let window_preferences = preferences.clone();
        let (window_handle, view) = open_window(options, cx, move |window, cx| {
            let theme_mode = match window_preferences.appearance.as_str() {
                "dark" => ThemeMode::Dark,
                "light" => ThemeMode::Light,
                _ => ThemeMode::from(window.appearance()),
            };
            Theme::change(theme_mode, None, cx);
            cx.new(|cx| {
                FarmDesktop::new(
                    ui_app.clone(),
                    ui_runtime.clone(),
                    ui_lease,
                    tray,
                    false,
                    window_preferences,
                    tray_receiver,
                    window,
                    cx,
                )
            })
        })
        .expect("open QQ Farm window");
        view.update(cx, |this, _| {
            this.window_handle = Some(window_handle);
        });
        register_window_commands(&view, window_handle, cx);
        let close_view = view.downgrade();
        let _ = window_handle.update(cx, |_, window, app| {
            window.on_window_should_close(app, move |_window, cx| {
                if close_to_tray {
                    #[cfg(target_os = "macos")]
                    cx.hide();
                    #[cfg(not(target_os = "macos"))]
                    _window.minimize_window();
                } else {
                    let _ = close_view.update(cx, |this, cx| this.request_quit(cx));
                }
                false
            });
        });
    });
}

pub(crate) fn restore_main_window(cx: &mut gpui_kit::App) {
    cx.activate(true);
    if let Some(handle) = cx.windows().first().copied() {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }
}
