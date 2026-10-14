mod actions;
mod filters;
mod forms;
mod navigation;
mod presentation;
mod search;
mod table;

use super::super::Page;
use super::support::open_test_desktop;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AnyWindowHandle, AppContext as _, TestAppContext};
use std::time::Duration;

async fn choose_filter_option(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    select_id: &'static str,
    down_count: usize,
) {
    cx.update_window(window, |_, window, cx| {
        window.within(select_id).click("input", cx);
        for _ in 0..down_count {
            window.press("down", cx);
        }
        window.press("enter", cx);
    })
    .expect("open the account filter select and confirm its option");
    cx.run_until_parked();
}

async fn wait_for_account_batch_delete_popover(
    cx: &mut TestAppContext,
    window: gpui_kit::AnyWindowHandle,
) {
    cx.wait_for(window, Duration::from_secs(1), |window, _| {
        window.try_find("account-batch-delete-cancel").is_some_and(|button| button.visible())
    })
    .await;
}

async fn wait_for_account_row_delete_popover(
    cx: &mut TestAppContext,
    window: gpui_kit::AnyWindowHandle,
) {
    cx.wait_for(window, Duration::from_secs(1), |window, _| {
        window.try_find(("account-delete-confirm", 0_usize)).is_some_and(|button| button.visible())
    })
    .await;
}
