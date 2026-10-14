use super::*;

#[gpui_kit::test]
fn purchase_confirmation_resets_quantity_and_uses_a_stepper(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.commerce.purchase_count.update(cx, |state, cx| {
                state.set_value("42", window, cx);
            });
            this.open_purchase_confirmation(
                serde_json::json!({"id": 4, "name": "测试商品"}),
                window,
                cx,
            );
            assert_eq!(this.commerce.purchase_count.read(cx).value(), "1");
        });

        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        assert!(window.find("decrement").visible());
        assert!(window.find("increment").visible());
        assert!(window.find("purchase-confirm").visible());
        window.press("escape", cx);
    })
    .expect("open a purchase confirmation with its reset quantity");
    cx.update_window(window, |_, window, cx| {
        assert!(!window.has_active_dialog(cx));
    })
    .expect("Escape closes the purchase confirmation");
    cx.update(|cx| assert!(desktop.read(cx).commerce.pending_purchase.is_none()));
}
#[gpui_kit::test]
fn purchase_pending_does_not_set_page_loading(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Mall;
            this.page_section = "normal".into();
            this.selected_account = Some("7".into());
        });
    });
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.open_purchase_confirmation(
                serde_json::json!({"id": 1, "name": "种子包", "price": {"id": 1, "count": 5}}),
                window,
                cx,
            );
        });
        desktop.update(cx, |this, cx| {
            this.commerce.purchase_pending = true;
            cx.notify();
        });
        window.render_frame(cx);
        let state = desktop.read(cx);
        assert!(state.commerce.purchase_pending);
        assert!(state.commerce.pending_purchase.is_some());
        assert!(!state.loading);
    })
    .expect("mark the purchase confirmation busy without blocking the page");
}
