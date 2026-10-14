use super::super::Page;
use super::support::open_test_desktop;
use crate::pages::DisplayRow;
use gpui_kit::component::WindowExt;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};
use std::time::Duration;

async fn wait_for_friend_confirmation_popover(
    cx: &mut TestAppContext,
    window: gpui_kit::AnyWindowHandle,
) {
    cx.wait_for(window, Duration::from_secs(1), |window, _| {
        window.try_find(("friend-confirm-cancel", 0_usize)).is_some_and(|button| button.visible())
    })
    .await;
}

mod loading;
mod operations;
mod presentation;
mod realtime;
mod visitors;
