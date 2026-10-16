use super::{open_test_desktop, Page};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};
use serde_json::json;

#[gpui_kit::test]
fn account_success_feedback_is_not_rendered_in_page_after_toast_migration(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = json!({"accounts": []});
            this.accounts.success_feedback = Some("添加成功".into());
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        // 操作结果已迁移为全局 toast：页面内不再渲染成功横幅（字段保留供保存流程读写）
        assert_eq!(desktop.read(cx).accounts.success_feedback.as_deref(), Some("添加成功"));
        assert!(window.try_find("account-success-feedback").is_none());
    })
    .expect("account success feedback is toast-only after migration");
}

#[gpui_kit::test]
fn account_toolbar_matches_vue_and_keeps_lifecycle_actions_on_each_row(cx: &mut TestAppContext) {
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
        assert!(window.try_find("account-run").is_none());
        assert!(window.find("account-add-toolbar").visible());
        assert!(window.find("account-batch-delete").visible());
        assert!(window.find("refresh").visible());
        assert!(window.find("account-edit-1").visible());
        assert!(window.find("account-toggle-1").visible());
        assert!(window.find("account-delete-1").visible());
    })
    .expect("expose only the Vue account-page controls and preserve each row operation");
}

#[test]
fn account_login_status_and_validation_messages_are_localized() {
    assert_eq!(
        super::super::super::accounts::account_status_text("en-US", "等待微信扫码"),
        "Waiting for a WeChat scan"
    );
    assert_eq!(
        super::super::super::accounts::account_status_text("en-US", "小明 · 请在电脑微信中确认"),
        "小明 · Confirm in WeChat for desktop"
    );
    assert_eq!(
        super::super::super::accounts::localized_account_message(
            "en-US",
            "请输入登录 code 或粘贴完整登录链接"
        ),
        "Enter a login code or paste a full login URL"
    );
    assert_eq!(
        super::super::super::accounts::account_status_text("zh-CN", "等待微信扫码"),
        "等待微信扫码"
    );
}

#[test]
fn account_last_online_uses_vue_field_precedence_and_fallbacks() {
    let record = serde_json::json!({
        "lastOnlineAt": 1_700_000_000,
        "updatedAt": 1_600_000_000,
        "updated_at": 1_500_000_000,
    });
    assert_eq!(
        super::super::super::accounts::account_last_online_timestamp(&record),
        1_700_000_000
    );

    let record = serde_json::json!({ "lastOnlineAt": null, "updatedAt": "1600000000" });
    assert_eq!(
        super::super::super::accounts::account_last_online_timestamp(&record),
        1_600_000_000
    );

    let record = serde_json::json!({ "updated_at": 1_500_000_000 });
    assert_eq!(
        super::super::super::accounts::account_last_online_timestamp(&record),
        1_500_000_000
    );

    assert_eq!(
        super::super::super::accounts::account_last_online_timestamp(&serde_json::json!({})),
        0
    );
}
