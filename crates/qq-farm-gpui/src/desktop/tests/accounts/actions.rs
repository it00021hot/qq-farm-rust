use super::{
    open_test_desktop, wait_for_account_batch_delete_popover, wait_for_account_row_delete_popover,
    Page,
};
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};
use serde_json::json;
use std::time::Duration;

#[gpui_kit::test]
async fn account_delete_confirmation_supports_escape_without_deleting(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = json!({
                "accounts": [{
                    "id": "1", "name": "Farm account", "platform": "qq",
                    "running": false, "wxAuthorized": false
                }]
            });
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("account-delete-1", cx);
        window.render_frame(cx);
        assert!(window.find("account-delete-confirm-message").visible());
    })
    .expect("open the row-anchored account confirmation");
    wait_for_account_row_delete_popover(cx, window).await;
    cx.update_window(window, |_, window, cx| {
        window.press("escape", cx);
        window.render_frame(cx);
        let state = desktop.read(cx);
        assert!(state.accounts.pending_delete.is_none());
        assert_eq!(state.page_raw["accounts"].as_array().map(Vec::len), Some(1));
    })
    .expect("Escape closes the confirmation without deleting the account");
}

#[gpui_kit::test]
async fn account_delete_confirmation_is_localized_and_cancellable(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.preferences.language = "zh-CN".into();
            this.page = Page::Accounts;
            this.page_raw = json!({
                "accounts": [{
                    "id": "1", "name": "Farm account", "platform": "qq",
                    "running": false, "wxAuthorized": false
                }]
            });
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("account-delete-1", cx);
        window.render_frame(cx);
        assert_eq!(window.find(("account-delete-confirm", 0_usize)).label(), Some("确认"));
        assert_eq!(window.find(("account-delete-cancel", 0_usize)).label(), Some("取消"));
    })
    .expect("open the Chinese row deletion popover");
    wait_for_account_row_delete_popover(cx, window).await;
    cx.update_window(window, |_, window, cx| {
        window.click(("account-delete-cancel", 0_usize), cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).accounts.pending_delete.is_none());
    })
    .expect("cancel account deletion without running the operation");
}

#[gpui_kit::test]
async fn account_delete_confirmation_runs_operation_only_after_confirm(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = json!({
                "accounts": [{
                    "id": "1", "name": "Farm account", "platform": "qq",
                    "running": false, "wxAuthorized": false
                }]
            });
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("account-delete-1", cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).accounts.success_feedback.is_none());
    })
    .expect("open the row deletion popover before confirming");
    wait_for_account_row_delete_popover(cx, window).await;
    cx.update_window(window, |_, window, cx| {
        window.click(("account-delete-confirm", 0_usize), cx);
        window.render_frame(cx);
    })
    .expect("confirm account deletion");
    cx.wait_for(window, Duration::from_secs(3), |_window, cx| {
        !desktop.read(cx).loading && desktop.read(cx).accounts.success_feedback.is_some()
    })
    .await;
    cx.update(|cx| {
        let desktop = desktop.read(cx);
        assert!(desktop.accounts.pending_delete.is_none());
        assert!(!desktop.loading);
    });
}

#[gpui_kit::test]
async fn account_batch_delete_cancel_preserves_current_page_selection(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let records: Vec<_> = (1..=12)
        .map(|id| {
            json!({
                "id": id.to_string(),
                "name": format!("Account {id}"),
                "platform": "qq",
                "running": false,
                "wxAuthorized": false
            })
        })
        .collect();
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = json!({"accounts": records});
            this.accounts.page_size = 10;
            this.accounts.page = 1;
            this.accounts.selected.extend(["1".into(), "11".into()]);
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).selected_visible_account_ids(), ["11".to_string()]);
        assert_eq!(window.find("account-batch-delete").label(), Some("批量删除"));
        window.click("account-batch-delete", cx);
        window.render_frame(cx);
    })
    .expect("open the Vue-style batch-delete confirmation for the current page");
    wait_for_account_batch_delete_popover(cx, window).await;
    cx.update_window(window, |_, window, cx| {
        let state = desktop.read(cx);
        assert!(state.accounts.pending_batch_delete);
        assert_eq!(state.selected_visible_account_ids(), ["11".to_string()]);
        window.click("account-batch-delete-cancel", cx);
        window.render_frame(cx);
        let state = desktop.read(cx);
        assert!(!state.accounts.pending_batch_delete);
        assert_eq!(state.selected_visible_account_ids(), ["11".to_string()]);
    })
    .expect("cancel without deleting or clearing selected rows");
}

#[gpui_kit::test]
async fn account_batch_delete_removes_the_selected_account_only_after_confirmation(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let account = qq_farm_core::models::store::accounts::add_or_update_account(
        qq_farm_core::models::store::accounts::AccountRecord {
            name: "Batch deletion probe".into(),
            platform: "qq".into(),
            username: "local".into(),
            ..Default::default()
        },
    );
    let account_id = account.id.clone();
    let record = json!({
        "id": account_id,
        "name": "Batch deletion probe",
        "platform": "qq",
        "running": false,
        "wxAuthorized": false
    });
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = json!({"accounts": [record]});
            this.accounts.page = 0;
            this.accounts.selected.insert(account.id);
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("account-batch-delete", cx);
        window.render_frame(cx);
        assert!(qq_farm_core::models::store::accounts::get_accounts()
            .iter()
            .any(|record| record.id == account_id));
    })
    .expect("opening the confirmation must not delete the account");
    wait_for_account_batch_delete_popover(cx, window).await;

    cx.update_window(window, |_, window, cx| {
        window.click("account-batch-delete-confirm", cx);
        window.render_frame(cx);
        assert!(!qq_farm_core::models::store::accounts::get_accounts()
            .iter()
            .any(|record| record.id == account_id));
        let state = desktop.read(cx);
        assert!(!state.accounts.pending_batch_delete);
        assert!(state.accounts.selected.is_empty());
    })
    .expect("confirm and delete only the captured selected account");
}
