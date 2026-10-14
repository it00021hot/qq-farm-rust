mod catalog;
mod forms;
mod persistence;

use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::TestAppContext;
use std::time::Duration;

async fn wait_for_sheet(cx: &mut TestAppContext, window: gpui_kit::AnyWindowHandle) {
    cx.wait_for(window, Duration::from_secs(1), |window, _| {
        window.find("sheet-content").bounds().right() <= window.viewport_size().width
    })
    .await;
}

async fn wait_for_delete_popover(cx: &mut TestAppContext, window: gpui_kit::AnyWindowHandle) {
    let mut previous_y = None;
    let mut stable_frames = 0;
    cx.wait_for(window, Duration::from_secs(1), |window, _| {
        let Some(button) =
            window.try_find(("config-delete-cancel", 0_usize)).filter(|button| button.visible())
        else {
            return false;
        };
        let y = button.bounds().origin.y.as_f32();
        if previous_y.is_some_and(|previous: f32| (previous - y).abs() < 0.1) {
            stable_frames += 1;
        } else {
            stable_frames = 0;
        }
        previous_y = Some(y);
        stable_frames >= 3
    })
    .await;
}

async fn wait_for_batch_delete_popover(cx: &mut TestAppContext, window: gpui_kit::AnyWindowHandle) {
    cx.wait_for(window, Duration::from_secs(1), |window, _| {
        window.try_find("game-config-batch-delete-cancel").is_some_and(|button| button.visible())
    })
    .await;
}
