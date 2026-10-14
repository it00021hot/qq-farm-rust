use super::super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{point, px, AppContext as GpuiContextExt, ScrollDelta, TestAppContext};

#[gpui_kit::test]
fn bag_defaults_to_fruit_hides_system_items_and_selects_sellable_rows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Personal;
            this.page_section = "bag".into();
            this.selected_account = Some("7".into());
            this.farm.bag.category = "fruit".into();
            this.farm.bag.batch_mode = true;
            this.page_raw = serde_json::json!({
                "bag": {"items": [
                    {"id": 1002, "itemType": 17, "name": "点券", "count": 100},
                    {"id": 7, "itemType": 17, "name": "苹果苹果苹果苹果苹果苹果苹果苹果", "count": 4,
                        "sellable": true, "price": 8, "priceId": 1005, "priceUnit": " 金豆"},
                    {"id": 8, "itemType": 5, "name": "种子", "count": 2},
                    {"id": 9, "itemType": 11, "name": "肥料", "count": 3}
                ]}
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("bag-item-7-0").visible());
        assert_eq!(
            window.find("bag-item-name-7-0").label(),
            Some("苹果苹果苹果苹果苹果苹果苹果苹果")
        );
        assert!(window.try_find("bag-item-1002-0").is_none());
        assert!(window.try_find("bag-item-8-0").is_none());
        assert_eq!(window.find("bag-category-all").label(), Some("全部 (3)"));
        assert_eq!(window.find("bag-category-fruit").label(), Some("果实 (1)"));
        assert_eq!(window.find("bag-category-seed").label(), Some("种子 (1)"));
        assert_eq!(window.find("bag-category-tool").label(), Some("道具 (1)"));
        assert_eq!(window.find("bag-batch-toggle").label(), Some("取消"));
        assert_eq!(window.find("bag-select-all").label(), Some("全选"));
        assert_eq!(window.find("bag-sell-selected").label(), Some("批量出售 (0)"));
        window.click("bag-select-7-0", cx);
        window.render_frame(cx);
        assert_eq!(window.find("bag-sell-selected").label(), Some("批量出售 (1)"));
        window.click("bag-item-7-0", cx);
        window.render_frame(cx);
        assert_eq!(window.find("bag-sell-selected").label(), Some("批量出售 (0)"));
        window.click("bag-item-7-0", cx);
        window.render_frame(cx);
        assert_eq!(window.find("bag-sell-selected").label(), Some("批量出售 (1)"));
        window.click("bag-select-7-0", cx);
        window.render_frame(cx);
        assert_eq!(window.find("bag-sell-selected").label(), Some("批量出售 (0)"));

        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("bag-category-all").label(), Some("All (3)"));
        assert_eq!(window.find("bag-category-fruit").label(), Some("Fruit (1)"));
        assert_eq!(window.find("bag-category-seed").label(), Some("Seed (1)"));
        assert_eq!(window.find("bag-category-tool").label(), Some("Tool (1)"));
        assert_eq!(window.find("bag-batch-toggle").label(), Some("Cancel"));
        assert_eq!(window.find("bag-select-all").label(), Some("Select All"));
        assert_eq!(window.find("bag-sell-selected").label(), Some("Batch Sell (0)"));
    })
    .expect("render warehouse categories, hidden items and sellable checkbox selection");
}

#[gpui_kit::test]
fn warehouse_search_scopes_select_all_and_the_sale_count(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Personal;
            this.page_section = "bag".into();
            this.selected_account = Some("7".into());
            this.farm.bag.batch_mode = true;
            this.page_raw = serde_json::json!({"bag":{"items":[
                {"id":7,"itemType":17,"name":"apple","count":4,"sellable":true},
                {"id":8,"itemType":17,"name":"pear","count":3,"sellable":true}
            ]}});
        })
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("page-search", cx);
        window.input("apple", cx);
        window.render_frame(cx);
        window.click("bag-select-all", cx);
        window.render_frame(cx);
        assert!(window.try_find("bag-item-8-1").is_none());
        assert_eq!(window.find("bag-sell-selected").label(), Some("批量出售 (1)"));
        desktop.update(cx, |this, _| {
            assert_eq!(this.farm.bag.selected_ids, std::collections::HashSet::from([7]));
        });
    })
    .expect("search must not select hidden warehouse items");
}

#[gpui_kit::test]
fn large_warehouse_grid_mounts_visible_cards_and_scrolls_to_later_items(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let items = (0..120)
        .map(|index| {
            serde_json::json!({
                "id": 2_000 + index,
                "itemType": 17,
                "name": format!("果实 {index}"),
                "count": index + 1,
                "sellable": true
            })
        })
        .collect::<Vec<_>>();
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Personal;
            this.page_section = "bag".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({"bag":{"items":items}});
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("personal-bag-grid-viewport").visible());
        assert!(window.find("bag-item-2000-0").visible());
        assert!(window.try_find("bag-item-2119-119").is_none());
        window.scroll(
            "personal-bag-grid-viewport",
            ScrollDelta::Pixels(point(px(0.), px(-100_000.))),
            cx,
        );
        window.render_frame(cx);
        assert!(window.try_find("bag-item-2027-27").is_some());
    })
    .expect("warehouse grid should virtualize rows and mount cards when scrolled");
}

#[gpui_kit::test]
fn bag_without_original_stock_reports_a_notice_without_submitting(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Personal;
            this.page_section = "bag".into();
            this.selected_account = Some("7".into());
            this.run_bag_action(7, 0, true, cx);
            assert!(!this.loading);
            assert!(this.farm.bag.selling_ids.is_empty());
            assert_eq!(
                this.farm.bag.notice.as_ref().map(|(message, _)| message.as_str()),
                Some("未找到可出售物品")
            );
            this.farm.bag.selected_ids.insert(7);
            this.run_bag_batch_sell(cx);
            assert!(!this.farm.bag.batch_selling);
            assert_eq!(
                this.farm.bag.notice.as_ref().map(|(message, _)| message.as_str()),
                Some("请先勾选可出售物品")
            );
        })
    });
}

#[gpui_kit::test]
fn empty_warehouse_category_uses_vue_no_data_copy(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Personal;
            this.page_section = "bag".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({"bag": {"items": []}});
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("bag-empty").label(), Some("无数据"));
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("bag-empty").label(), Some("No Data"));
        window.click("page-search", cx);
        window.input("apple", cx);
        window.render_frame(cx);
        assert_eq!(window.find("bag-empty").label(), Some("No matching items"));
    })
    .expect("warehouse empty state should follow Vue's noData copy and search wording");
}
