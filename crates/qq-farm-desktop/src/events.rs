//! 订阅 RuntimeEvent → 刷新 UI。

use std::time::Duration;

use gpui::*;

use crate::app_state::AppState;

/// 启动事件监听与定时刷新。
pub fn spawn_event_listener(state: Entity<AppState>, cx: &mut App) {
    let (app, _, handle) = state.read(cx).bridge_parts();
    let weak = state.downgrade();
    let weak2 = state.downgrade();

    let (tx, rx) = std::sync::mpsc::channel::<()>();
    handle.spawn(async move {
        let mut events = app.subscribe_events();
        while events.recv().await.is_ok() {
            let _ = tx.send(());
        }
    });

    cx.spawn(async move |cx| {
        loop {
            let has_event = rx.try_recv().is_ok();
            if has_event {
                let _ = weak.update(cx, |state, cx| {
                    state.refresh_sync();
                    cx.notify();
                });
            }
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
        }
    })
    .detach();

    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_secs(3))
                .await;
            let _ = weak2.update(cx, |state, cx| {
                state.refresh_sync();
                cx.notify();
            });
        }
    })
    .detach();
}
