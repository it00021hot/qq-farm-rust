use super::{open_test_desktop, Page};
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};
use qq_farm_core::models::store::accounts::{self, AccountRecord};
use std::time::Duration;

#[gpui_kit::test]
async fn editing_account_remark_persists_and_legacy_form_uses_vue_name_fallback(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let account = accounts::add_or_update_account(AccountRecord {
        name: "Legacy account".into(),
        code: "unchanged-code".into(),
        platform: "qq".into(),
        username: "local".into(),
        ..Default::default()
    });
    let legacy_record = serde_json::json!({
        "id": account.id,
        "name": "Legacy account",
        "code": "unchanged-code",
        "platform": "qq",
        "running": false,
        "wxAuthorized": false
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.open_account_form(Some(legacy_record), window, cx);
            assert_eq!(this.accounts.remark.read(cx).value(), "Legacy account");
            this.accounts
                .remark
                .update(cx, |state, cx| state.set_value("Updated notes", window, cx));
            this.save_account(cx);
        });
        window.render_frame(cx);
    })
    .expect("save the edited account remark through the account sheet");

    cx.wait_for(window, Duration::from_secs(3), |_, cx| {
        let state = desktop.read(cx);
        !state.accounts.form_open && state.accounts.success_feedback.is_some()
    })
    .await;

    let saved = accounts::get_accounts()
        .into_iter()
        .find(|saved| saved.id == account.id)
        .expect("account remains in the store");
    assert_eq!(saved.remark, "Updated notes");

    let persisted: serde_json::Value = serde_json::from_slice(
        &std::fs::read(accounts::accounts_file()).expect("read persisted account store"),
    )
    .expect("parse persisted account store");
    assert_eq!(persisted["accounts"][0]["remark"], serde_json::json!("Updated notes"));
}

#[gpui_kit::test]
fn account_login_url_hint_updates_live_and_follows_locale(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = serde_json::json!({"accounts": []});
            cx.notify();
        });
        window.render_frame(cx);
        window.click("account-add-toolbar", cx);
    })
    .expect("open the account form from its page toolbar");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.accounts.code.update(cx, |state, cx| {
                state.set_value(
                    "https://game.qq.com/prod/ws?code=abc&platform=wx&os=mac&ver=1.2",
                    window,
                    cx,
                );
                cx.emit(gpui_kit::component::input::InputEvent::Change);
            });
            cx.notify();
        });
        window.render_frame(cx);
    })
    .expect("paste a login URL into the account form");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("account-login-url-hint").label(),
            Some("已从 URL 识别：平台 微信 / os mac / ver 1.2")
        );
        assert_eq!(desktop.read(cx).accounts.platform, "wx");
    })
    .expect("show the live URL detection hint and selected platform");

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.preferences.language = "en-US".into();
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("account-login-url-hint").label(),
            Some("Detected from URL: Platform WeChat / os mac / ver 1.2")
        );
        desktop.update(cx, |this, cx| {
            this.accounts.code.update(cx, |state, cx| state.set_value("plain-code", window, cx));
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("account-login-url-hint").is_none());
    })
    .expect("localize the hint and clear it when the input becomes a manual code");
}

#[gpui_kit::test]
fn local_wechat_authorization_displays_its_detected_avatar_with_a_fallback(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.open_account_form(None, window, cx);
            this.accounts.login_tab = "wx".into();
            this.accounts.wx_mode = "local".into();
            this.accounts.wx_quick_profile = Some((
                1234,
                "authorize-uuid".into(),
                "Milo".into(),
                "https://example.invalid/avatar.png".into(),
            ));
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("wx-local-avatar").visible());
        assert!(window.try_find("wx-local-avatar-fallback").is_none());
    })
    .expect("show the detected desktop WeChat avatar");

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.accounts.wx_quick_profile =
                Some((1234, "authorize-uuid".into(), "Milo".into(), String::new()));
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("wx-local-avatar-fallback").visible());
        assert!(window.try_find("wx-local-avatar").is_none());
    })
    .expect("keep a useful WeChat avatar placeholder when no image URL is available");
}
