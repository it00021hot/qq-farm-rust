use super::super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn illustrated_switches_books_and_preserves_progress_and_unlock_state(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| desktop.update(cx, |this, _| {
        this.page = Page::Personal;
        this.page_section = "illustrated".into();
        this.selected_account = Some("7".into());
        this.page_raw = serde_json::json!({"illustrated": {
            "crop":{"level":2,"progress":40,"nextLevelProgress":80,"items":[
                {"seedId":1,"name":"苹果","rewardCategory":2,"progress":40,"unlocked":true},
                {"seedId":2,"name":"梨","rewardCategory":3,"progress":0,"unlocked":false}
            ]},
            "mutant":{"level":1,"progress":10,"nextLevelProgress":40,"currentBuffs":[
                {"id":1,"name":"变异几率","value":15,"valueType":"probability"}
            ],"items":[{"seedId":3,"name":"金苹果","rewardCategory":4,"progress":10,"unlocked":true}]}
        }});
    }));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("illustrated-level").visible());
        assert!(window.find("illustrated-progress").visible());
        assert_eq!(window.find("illustrated-level").label(), Some("等级 2"));
        assert_eq!(window.find("illustrated-progress").label(), Some("40 / 80"));
        assert!(window.find("illustrated-item-crop-1").visible());
        assert!(window.find("illustrated-item-crop-2").visible());
        window.click("illustrated-book-mutant", cx);
        window.render_frame(cx);
        assert!(window.try_find("illustrated-item-crop-1").is_none());
        assert!(window.find("illustrated-item-mutant-3").visible());
        assert_eq!(window.find("illustrated-level").label(), Some("等级 1"));
        assert_eq!(window.find("illustrated-buff-0").label(), Some("变异几率 +15%"));
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("illustrated-level").label(), Some("Lv 1"));
    })
    .expect("virtualized crop and mutant books");
}

#[gpui_kit::test]
fn illustrated_rewards_dialog_lists_level_states_and_claim_gate(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Personal;
            this.page_section = "illustrated".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({"illustrated": {
                "crop": {
                    "level": 5, "progress": 60, "nextLevelProgress": 100, "claimable": true,
                    "rewards": [{"itemId": 1002, "name": "点券", "count": 200}],
                    "levels": [
                        {"level": 4, "progress": 40, "claimed": true, "canClaim": false,
                            "rewards": [{"itemId": 1002, "name": "点券", "count": 200}]},
                        {"level": 5, "progress": 80, "claimed": false, "canClaim": true,
                            "rewards": [{"itemId": 1002, "name": "点券", "count": 200}]},
                        {"level": 6, "progress": 120, "claimed": false, "canClaim": false,
                            "rewards": []}
                    ],
                    "items": []
                },
                "mutant": {"level": 1, "progress": 0, "nextLevelProgress": 0, "items": []}
            }});
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("illustrated-claim").visible(), "可领取时头部显示领取按钮");
        window.click("illustrated-details", cx);
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        assert_eq!(window.find("illustrated-level-5").label(), Some("5 级 · 80 点券 ×200 可领取"));
        assert_eq!(window.find("illustrated-level-4").label(), Some("4 级 · 40 点券 ×200 已领取"));
        assert_eq!(window.find("illustrated-level-6").label(), Some("6 级 · 120 未达成"));
        assert!(window.find("illustrated-claim-confirm").visible());
        window.press("escape", cx);
    })
    .expect("rewards dialog lists level states and gates the claim button");
}

/// 图鉴网格必须能被滚轮滚动：注入 200 项后下滚，首行应被虚拟化、尾部行应进入视口。
#[gpui_kit::test]
fn illustrated_grid_scrolls_to_reach_later_rows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let items: Vec<serde_json::Value> = (1..=200)
        .map(|i| {
            serde_json::json!({
                "seedId": i, "name": format!("作物{i}"),
                "rewardCategory": 2, "progress": 0, "unlocked": false,
            })
        })
        .collect();
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Personal;
            this.page_section = "illustrated".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({"illustrated": {
                "crop": {"level": 1, "progress": 0, "nextLevelProgress": 10, "items": items},
                "mutant": {"level": 1, "progress": 0, "nextLevelProgress": 10, "items": []},
            }});
        })
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("illustrated-item-crop-1").visible());
        assert!(window.try_find("illustrated-item-crop-200").is_none());
        // 滚到底（对齐 gpui-base 自身测试模式：handle 驱动滚动）。
        // 若列表尺寸退化为 Infer（长到内容高度、无内部溢出），此滚动将不产生位移，
        // 首行不会被虚拟化 —— 即用户报告的「图鉴页滚动不了」回归。
        desktop.update(cx, |this, _| {
            this.farm.illustrated.grid_scroll_handle.scroll_to_bottom();
        });
        window.render_frame(cx);
        let early_gone = window.try_find("illustrated-item-crop-1").is_none();
        let late_mounted = window.try_find("illustrated-item-crop-200").is_some();
        assert!(
            early_gone && late_mounted,
            "滚到底后首行应被虚拟化、尾行应挂载: early_gone={early_gone} late_mounted={late_mounted}"
        );
    })
    .expect("illustrated grid scrolls to the bottom");
}
