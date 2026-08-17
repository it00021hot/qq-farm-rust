//! 异步桥接辅助（业务经 qq-farm-app）。

use std::sync::Arc;

use gpui::*;
use qq_farm_app::AppContext;

use crate::app_state::AppState;
use crate::views::{humanize_error, is_soft_business_message};

/// 在 tokio 上执行异步门面，成功后更新 AppState。
pub fn run_async<T, Fut, F>(
    state: &Entity<AppState>,
    cx: &mut App,
    make_fut: F,
    on_ok: impl FnOnce(&mut AppState, T, &mut Context<AppState>) + 'static,
) where
    T: Send + 'static,
    Fut: std::future::Future<Output = Result<T, String>> + Send + 'static,
    F: FnOnce(Arc<AppContext>, String) -> Fut + Send + 'static,
{
    let weak = state.downgrade();
    let (app, account_id, handle) = state.read(cx).bridge_parts();
    let fut = make_fut(app, account_id);
    cx.spawn(async move |cx| {
        let result = handle
            .spawn(fut)
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        let _ = weak.update(cx, |state, cx| match result {
            Ok(v) => {
                state.clear_toast();
                on_ok(state, v, cx);
                cx.notify();
            }
            Err(e) => {
                let msg = humanize_error(&e);
                if is_soft_business_message(&msg) {
                    state.flash_warning(msg, cx);
                } else {
                    state.flash_error(msg, cx);
                }
            }
        });
    })
    .detach();
}

/// 同步门面调用。
pub fn run_sync(
    state: &Entity<AppState>,
    cx: &mut App,
    f: impl FnOnce(&AppContext, &str, &mut AppState, &mut Context<AppState>) -> Result<(), String>,
) {
    let (app, account_id, _) = state.read(cx).bridge_parts();
    state.update(cx, |state, cx| match f(&app, &account_id, state, cx) {
        Ok(()) => {
            state.refresh_accounts();
            cx.notify();
        }
        Err(e) => {
            let msg = humanize_error(&e);
            if is_soft_business_message(&msg) {
                state.flash_warning(msg, cx);
            } else {
                state.flash_error(msg, cx);
            }
        }
    });
}
