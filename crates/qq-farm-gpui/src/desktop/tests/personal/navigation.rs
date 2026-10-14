use super::super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn changing_personal_section_starts_a_load_for_the_selected_tab(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Personal;
            this.page_section = "lands".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "connected": true,
                "lands": {"lands": [{"id": 1}]},
                "bag": {"items": [{"id": 2}]}
            });
            this.farm.bag.category = "tool".into();
            this.farm.bag.batch_mode = true;
        });
        let initial_revision = desktop.read(cx).request_revision;
        desktop.update(cx, |this, cx| this.set_page_section("bag", window, cx));
        assert_eq!(desktop.read(cx).page_section, "bag");
        assert!(desktop.read(cx).page_raw.get("bag").is_none());
        assert_eq!(desktop.read(cx).farm.bag.category, "fruit");
        assert!(!desktop.read(cx).farm.bag.batch_mode);
        assert_eq!(
            desktop.read(cx).request_revision,
            initial_revision + 1,
            "opening another Vue tab starts that section's load"
        );

        let same_section_revision = desktop.read(cx).request_revision;
        desktop.update(cx, |this, cx| this.set_page_section("bag", window, cx));
        assert_eq!(
            desktop.read(cx).request_revision,
            same_section_revision,
            "selecting the active tab does not reload its data"
        );

        desktop.update(cx, |this, cx| {
            this.farm.illustrated.book = "mutant".into();
            this.farm.interaction.land_ids.update(cx, |state, cx| {
                state.set_value("1,2", window, cx);
            });
        });
        desktop.update(cx, |this, cx| this.set_page_section("illustrated", window, cx));
        assert_eq!(desktop.read(cx).farm.illustrated.book, "crop");
        assert!(desktop.read(cx).page_raw.get("illustrated").is_none());

        desktop.update(cx, |this, cx| this.set_page_section("interactionItems", window, cx));
        assert!(desktop.read(cx).farm.interaction.land_ids.read(cx).value().is_empty());
    })
    .expect("personal tabs load only when opened");
}

#[gpui_kit::test]
fn personal_section_failure_shows_a_banner_and_retry_restarts_the_load(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Personal;
            this.page_section = "lands".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "connected": true,
                "lands": {"lands": [{"id": 1}]},
                "sectionErrors": {"lands": "地块服务暂不可用"}
            });
            this.rows = crate::pages::page_data(Page::Personal, this.page_raw.clone()).rows;
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("personal-unavailable").is_none());
        assert!(window.find("personal-section-error").visible());
        assert_eq!(window.find("personal-section-error").label(), Some("地块服务暂不可用"));
        assert_ne!(window.find("personal-section-retry").disabled(), Some(true));
        assert!(
            window.try_find("empty-state-label").is_none(),
            "the failing section shows its own banner, not the empty state"
        );
        let before = desktop.read(cx).request_revision;
        window.click("personal-section-retry", cx);
        assert!(
            desktop.read(cx).request_revision > before,
            "the retry button restarts the page load"
        );
        desktop.update(cx, |this, _| this.loading = true);
        window.render_frame(cx);
        let pending = desktop.read(cx).request_revision;
        window.click("personal-section-retry", cx);
        window.render_frame(cx);
        assert_eq!(
            desktop.read(cx).request_revision,
            pending,
            "the retry button ignores clicks while a load is pending"
        );
    })
    .expect("a failed personal section keeps a banner with a working retry");
}
