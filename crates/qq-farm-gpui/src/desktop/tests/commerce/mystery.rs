use super::*;

#[gpui_kit::test]
fn mystery_shop_empty_state_follows_vue_copy(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::MysteryShop;
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({"active": false, "npc": null});
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("mystery-shop-empty").label(), Some("当前没有活跃的神秘商人"));
        desktop.update(cx, |this, _| this.selected_account = None);
        window.render_frame(cx);
        assert_eq!(window.find("mystery-shop-empty").label(), Some("请先选择农场账号"));
    })
    .expect("mystery shop empty states use Vue copy");
}

#[gpui_kit::test]
fn mystery_shop_remains_available_as_a_mall_tab(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        // 神秘商店已并入商城 tab：侧边栏不再有独立入口
        assert!(window.try_find("mystery-shop").is_none());
        window.click("mall", cx);
        window.render_frame(cx);
        assert_eq!(window.find(("page-section", 2_usize)).label(), Some("神秘商人"));
        window.click(("page-section", 2_usize), cx);
        window.render_frame(cx);
    })
    .expect("open the mystery merchant through the Vue mall tab");

    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            assert_eq!(this.page, Page::Mall);
            assert_eq!(this.page_section, "mystery");
            assert_eq!(this.commerce.mall_slot_type, -1);
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "slotType": -1,
                "active": true,
                "activeTime": 1_790_000_000_000_i64,
                "expireTime": 1_790_003_600_000_i64,
                "npc": {
                    "id": 99,
                    "stock": 8,
                    "discountPercent": 25,
                    "originalPrice": 40,
                    "reward": {"id": 8, "name": "神秘种子", "count": 8, "image": "/game-config/seeds/8.webp"},
                    "price": {"id": 1, "name": "金币", "count": 30, "balance": 35}
                }
            });
            this.rows = crate::pages::page_data(Page::Mall, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("mystery-shop-refresh").visible());
        assert_eq!(window.find("mystery-original-price").label(), Some("原价 40"));
        assert_eq!(window.find("mystery-shop-buy").label(), Some("购买"));
        window.click("mystery-shop-buy", cx);
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        assert!(window.find("purchase-confirm").visible());
        assert_eq!(
            window.find("purchase-mystery-summary").label(),
            Some("确认花费 30 金币 购买 神秘种子 ×8？")
        );
        window.press("escape", cx);
    })
    .expect("show discount, stock, balance and a confirmation for the active mystery merchant");

    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page_raw["npc"]["price"]["balance"] = serde_json::json!(10);
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("mystery-shop-buy").label(), Some("购买"));
        window.click("mystery-shop-buy", cx);
        assert!(!window.has_active_dialog(cx));
    })
    .expect("disable mystery merchant purchases when the known balance is too low");

    cx.update(|cx| {
        desktop.update(cx, |this, _| this.page_raw["active"] = serde_json::json!(false));
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("mystery-shop-buy").is_none());
    })
    .expect("hide the product and purchase action when the mystery merchant is inactive");
}

#[gpui_kit::test]
fn mystery_shop_page_still_renders_without_sidebar_entry(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        // 侧边栏不再有独立入口（并入商城「神秘商人」tab），
        // 但持久化偏好直达该页时页面仍需正常渲染。
        assert!(window.try_find("mystery-shop").is_none());
        desktop.update(cx, |this, cx| this.set_page(Page::MysteryShop, cx));
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).page, Page::MysteryShop);
        assert_eq!(desktop.read(cx).commerce.mall_slot_type, -1);
        assert!(window.try_find(("page-section", 0_usize)).is_none());
    })
    .expect("render the mystery shop page without a sidebar entry");

    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "active": true,
                "activeTime": 1_790_000_000_000_i64,
                "expireTime": 1_790_003_600_000_i64,
                "npc": {
                    "id": 99,
                    "stock": 8,
                    "discountPercent": 25,
                    "originalPrice": 40,
                    "reward": {"id": 8, "name": "神秘种子", "count": 8},
                    "price": {"id": 1, "name": "金币", "count": 30, "balance": 35}
                }
            });
            this.rows = crate::pages::page_data(Page::MysteryShop, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("mystery-shop-refresh").visible());
        assert_eq!(window.find("mystery-shop-buy").label(), Some("购买"));
        window.click("mystery-shop-buy", cx);
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        assert!(window.find("purchase-confirm").visible());
        window.press("escape", cx);
    })
    .expect("render and purchase from the standalone mystery shop page");
}
