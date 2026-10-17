use super::*;

#[gpui_kit::test]
fn mall_empty_and_no_account_states_follow_vue_copy(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Mall;
            this.page_section = "normal".into();
            this.commerce.mall_slot_type = 1;
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "diamond": 0,
                "catalog": {
                    "membership": {"isSvip": false, "remainingDays": 0},
                    "refreshCountdown": 0,
                    "goods": []
                }
            });
            this.rows = crate::pages::page_data(Page::Mall, this.page_raw.clone()).rows;
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("mall-empty").label(), Some("暂无商品"));
        desktop.update(cx, |this, _| this.selected_account = None);
        window.render_frame(cx);
        assert_eq!(window.find("mall-no-account").label(), Some("请先选择农场账号"));
    })
    .expect("mall empty and no-account states use Vue copy");
}

#[gpui_kit::test]
fn mall_catalog_uses_reward_cards_and_vue_purchase_status_labels(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Mall;
            this.page_section = "normal".into();
            this.commerce.mall_slot_type = 1;
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "diamond": 125,
                "catalog": {
                    "membership": {"isSvip": false, "remainingDays": 0},
                    "refreshCountdown": 12,
                    "goods": [
                        {
                            "id": 31,
                            "name": "打折番茄种子",
                            "rewards": [{"name": "番茄种子", "image": "/game-config/seeds/1.webp"}],
                            "price": {"id": 1, "count": 80, "name": "钻石"},
                            "originalPrice": 100,
                            "discountText": "8 折",
                            "limit": {"bought": 1, "max": 2},
                            "purchaseStatus": "available",
                            "purchasable": true,
                            "isFree": false
                        },
                        {
                            "id": 32,
                            "name": "每日免费礼包",
                            "rewards": [],
                            "purchaseStatus": "sold_out",
                            "purchasable": false,
                            "isFree": true
                        }
                    ]
                }
            });
            this.rows = crate::pages::page_data(Page::Mall, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("mall-buy-32-1").visible());
        assert_eq!(window.find("mall-name-31-0").label(), Some("打折番茄种子"));
        assert_eq!(window.find("mall-original-price-31-0").label(), Some("原价 100"));
        assert_eq!(window.find("mall-buy-31-0").label(), Some("购买"));
        assert_eq!(window.find("mall-buy-32-1").label(), Some("奖励已领取"));
        assert_eq!(window.find("refresh-mall").label(), Some("刷新"));
        assert_ne!(window.find("refresh-mall").disabled(), Some(true));
        window.click("mall-buy-31-0", cx);
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        assert_eq!(window.find("purchase-original-price").label(), Some("原价 100"));
        assert!(window.find("purchase-confirm").visible());
        assert!(window.find("decrement").visible());
        assert!(window.find("increment").visible());
        window.press("escape", cx);
    })
    .expect("render mall reward cards and open the selected purchase");
}
#[gpui_kit::test]
fn mall_available_status_still_requires_vue_purchasable_guard(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Mall;
            this.page_section = "normal".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "catalog": {"goods": [
                    {"id": 20, "name": "未提供购买权限", "purchaseStatus": "available"},
                    {"id": 21, "name": "不可购买", "purchaseStatus": "available", "purchasable": false},
                    {"id": 22, "name": "可购买", "purchaseStatus": "available", "purchasable": true}
                ]}
            });
            this.rows = crate::pages::page_data(Page::Mall, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("mall-buy-20-0").label(), Some("购买"));
        assert_eq!(window.find("mall-buy-21-1").label(), Some("购买"));
        window.click("mall-buy-20-0", cx);
        window.render_frame(cx);
        assert!(!window.has_active_dialog(cx));
        window.click("mall-buy-21-1", cx);
        window.render_frame(cx);
        assert!(!window.has_active_dialog(cx));
        window.click("mall-buy-22-2", cx);
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        window.press("escape", cx);
    })
    .expect("Vue shows the available label but only opens purchase when purchasable is true");
}
#[gpui_kit::test]
fn mall_tabs_reuse_loaded_catalogs_and_isolate_them_by_account(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let svip_snapshot = serde_json::json!({
        "slotType": 4,
        "diamond": 90,
        "catalog": {"goods": [{"id": 44, "name": "会员礼包"}]}
    });
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Mall;
            this.page_section = "normal".into();
            this.commerce.mall_slot_type = 1;
            this.selected_account = Some("7".into());
            this.commerce.store_catalog("7", 4, svip_snapshot.clone());
        });
    });

    cx.update_window(window, |_, window, cx| {
        let revision = desktop.read(cx).request_revision;
        desktop.update(cx, |this, cx| this.set_page_section("svip", window, cx));
        assert_eq!(desktop.read(cx).request_revision, revision);
        assert_eq!(desktop.read(cx).page_raw, svip_snapshot);
        assert_eq!(desktop.read(cx).commerce.mall_slot_type, 4);

        desktop.update(cx, |this, cx| this.set_page_section("normal", window, cx));
        assert_eq!(desktop.read(cx).request_revision, revision + 1);
        assert!(desktop.read(cx).page_raw.is_null());

        desktop.update(cx, |this, _| {
            this.commerce.store_catalog(
                "8",
                1,
                serde_json::json!({"slotType": 1, "catalog": {"goods": []}}),
            );
        });
        let revision = desktop.read(cx).request_revision;
        desktop.update(cx, |this, cx| this.set_page_section("svip", window, cx));
        assert_eq!(desktop.read(cx).request_revision, revision + 1);
        assert!(desktop.read(cx).page_raw.is_null());
    })
    .expect("reuse a loaded tab only for its account and request tabs without a cached catalog");
}
#[gpui_kit::test]
fn mall_and_confirmation_controls_follow_the_english_locale(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Mall;
            this.page_section = "normal".into();
            this.commerce.mall_slot_type = 1;
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "catalog": {
                    "membership": {"isSvip": false},
                    "goods": [
                        {"id": 31, "name": "Tomato seed", "rewards": [], "price": {"count": 8, "name": "Coins"}, "purchaseStatus": "available", "purchasable": true},
                        {"id": 32, "name": "Daily pack", "rewards": [], "isFree": true, "purchaseStatus": "sold_out", "purchasable": false}
                    ]
                }
            });
            this.rows = crate::pages::page_data(Page::Mall, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("mall-buy-31-0").label(), Some("Buy"));
        assert_eq!(window.find("mall-buy-32-1").label(), Some("Reward claimed"));
        window.click("mall-buy-31-0", cx);
        window.render_frame(cx);
        assert_eq!(window.find("purchase-confirm").label(), Some("Buy"));
        assert_eq!(window.find("purchase-cancel").label(), Some("Cancel"));
        assert_eq!(
            desktop.read(cx).commerce.purchase_count.read(cx).presentation().placeholder().as_ref(),
            "Quantity"
        );
        window.press("escape", cx);
    })
    .expect("mall status and purchase confirmation controls use the selected locale");

    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page_section = "mystery".into();
            this.commerce.mall_slot_type = -1;
            this.page_raw = serde_json::json!({
                "active": true,
                "npc": {
                    "stock": 2,
                    "reward": {"name": "Mystery seed", "count": 1},
                    "price": {"name": "Coins", "count": 8, "balance": 10}
                }
            });
            this.rows = crate::pages::page_data(Page::Mall, this.page_raw.clone()).rows;
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("mystery-shop-refresh").label(), Some("Refresh"));
        assert_eq!(window.find("mystery-shop-buy").label(), Some("Buy"));
        window.click("mystery-shop-buy", cx);
        window.render_frame(cx);
        assert_eq!(window.find("purchase-confirm").label(), Some("Buy"));
        window.press("escape", cx);
    })
    .expect("mystery merchant and purchase controls use the selected locale");
}
