//! QQ Farm GPUI 桌面端入口。

mod app_state;
mod bridge;
mod events;
mod shell;
mod ui;
mod views;

use std::sync::Arc;

use gpui::*;
use gpui_component::Root;

use crate::shell::ShellView;

fn main() {
    qq_farm_core::utils::logger::init();
    dotenvy::dotenv().ok();

    let runtime = Box::leak(Box::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("qq-farm-tokio")
            .build()
            .expect("tokio runtime"),
    ));
    let _enter = runtime.enter();
    let tokio_handle = runtime.handle().clone();

    let app_ctx = Arc::new(qq_farm_app::bootstrap::assemble_app_context(
        std::env::var("MAX_WORKERS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(16),
        "https://game.qq.com",
    ));

    Application::new().run(move |cx| {
        gpui_component::init(cx);

        let app_ctx = app_ctx.clone();
        let tokio_handle = tokio_handle.clone();

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(80.), px(60.)),
                        size: size(px(1280.), px(800.)),
                    })),
                    titlebar: Some(TitlebarOptions {
                        title: Some("QQ Farm".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    let state = cx.new(|cx| {
                        crate::app_state::AppState::new(
                            app_ctx.clone(),
                            tokio_handle.clone(),
                            window,
                            cx,
                        )
                    });
                    events::spawn_event_listener(state.clone(), cx);
                    let shell = cx.new(|cx| ShellView::new(state, window, cx));
                    cx.new(|cx| Root::new(shell, window, cx))
                },
            )
            .expect("open window");
        })
        .detach();
    });
}
