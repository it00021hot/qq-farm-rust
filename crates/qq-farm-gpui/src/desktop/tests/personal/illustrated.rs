use super::super::support::open_test_desktop;
use crate::desktop::Page;
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
