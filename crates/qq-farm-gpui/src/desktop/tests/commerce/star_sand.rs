use super::*;

#[gpui_kit::test]
fn star_sand_shop_filters_product_cards_by_category(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);

    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "shop".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "capabilities": {"exchange": true},
                "shop": {
                    "name": "星砂商店",
                    "balanceKnown": true,
                    "balance": "17",
                    "categories": ["种子", "道具"],
                    "goods": [
                        {"id": 1, "name": "星砂种子", "category": "种子", "item": {}, "cost": {"count": 3}, "exchangeable": true},
                        {"id": 2, "name": "星砂道具", "category": "道具", "item": {}, "cost": {"count": 5}, "exchangeable": true}
                    ]
                }
            });
            this.rows = crate::pages::page_data(Page::Activities, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("star-sand-category-all").label(), Some("全部"));
        assert!(window.find(("star-sand-category", 0_usize)).visible());
        assert!(window.find(("star-sand-exchange", 0_usize)).visible());
        assert!(window.find(("star-sand-exchange", 1_usize)).visible());

        window.click(("star-sand-category", 0_usize), cx);
        window.render_frame(cx);
        assert!(window.find(("star-sand-exchange", 0_usize)).visible());
        assert!(window.try_find(("star-sand-exchange", 1_usize)).is_none());

        window.click(("star-sand-exchange", 0_usize), cx);
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        assert_eq!(window.find("purchase-confirm").label(), Some("确认兑换"));
        let pending = desktop.read(cx).commerce.pending_purchase.clone().unwrap();
        assert_eq!(pending["activityGoods"], true);
        assert_eq!(pending["fallbackItem"], serde_json::json!({}));
        assert_eq!(pending["fallbackGoodsName"], "星砂种子");
    })
    .expect("filter star-sand goods and retain reward data in the exchange confirmation");
}
#[gpui_kit::test]
fn star_sand_shop_filters_object_categories_by_category_id(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);

    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "shop".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "shop": {
                    "categories": [
                        {"id": "seed-id", "name": "种子"},
                        {"id": "tool-id", "name": "道具"}
                    ],
                    "goods": [
                        {"id": 1, "name": "种子奖励", "categoryId": "seed-id", "item": {}, "exchangeable": true},
                        {"id": 2, "name": "道具奖励", "categoryName": "tool-id", "item": {}, "exchangeable": true}
                    ]
                }
            });
            this.rows = crate::pages::page_data(Page::Activities, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(("star-sand-category", 0_usize)).label(), Some("种子"));
        assert_eq!(window.find(("star-sand-category", 1_usize)).label(), Some("道具"));
        assert!(window.find(("star-sand-exchange", 0_usize)).visible());
        assert!(window.find(("star-sand-exchange", 1_usize)).visible());

        window.click(("star-sand-category", 0_usize), cx);
        window.render_frame(cx);
        assert!(window.find(("star-sand-exchange", 0_usize)).visible());
        assert!(window.try_find(("star-sand-exchange", 1_usize)).is_none());

        window.click(("star-sand-category", 1_usize), cx);
        window.render_frame(cx);
        assert!(window.try_find(("star-sand-exchange", 0_usize)).is_none());
        assert!(window.find(("star-sand-exchange", 1_usize)).visible());
    })
    .expect("object category labels and alternate goods category fields filter like Vue");
}
#[gpui_kit::test]
fn star_sand_shop_requires_explicit_exchangeable_and_shows_vue_reason(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);

    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "shop".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "shop": {
                    "goods": [
                        {"id": 1, "name": "不可兑换", "item": {}, "exchangeable": false},
                        {"id": 2, "name": "缺少门控字段", "item": {}},
                        {"id": 3, "name": "可兑换", "item": {}, "exchangeable": true}
                    ]
                }
            });
            this.rows = crate::pages::page_data(Page::Activities, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(("star-sand-exchange", 0_usize)).label(), Some("不可兑换"));
        assert_eq!(window.find(("star-sand-exchange", 1_usize)).label(), Some("不可兑换"));
        assert_eq!(window.find(("star-sand-exchange", 2_usize)).label(), Some("兑换"));
        window.click(("star-sand-exchange", 0_usize), cx);
        window.render_frame(cx);
        assert!(!window.has_active_dialog(cx));
        window.click(("star-sand-exchange", 1_usize), cx);
        window.render_frame(cx);
        assert!(!window.has_active_dialog(cx));
        window.click(("star-sand-exchange", 2_usize), cx);
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        window.press("escape", cx);
    })
    .expect("Vue marks missing/false exchangeable values unavailable and enables explicit true");
}
