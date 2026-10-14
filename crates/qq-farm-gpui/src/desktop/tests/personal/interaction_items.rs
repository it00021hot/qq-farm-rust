use super::super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn self_interaction_picker_defaults_to_first_item_and_validates_land_input(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Personal;
            this.page_section = "interactionItems".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({"interactionItems": {"items": [
                {"itemId":44,"name":"雨水瓶","count":2,"saleConditionSatisfiedCount":1,"description":"浇灌自家农场"},
                {"itemId":55,"name":"彩虹","count":3}
            ]}});
            this.sync_personal_interaction_picker(window, cx);
        });
        window.render_frame(cx);
        assert!(window.find("personal-interaction-item-44").visible());
        assert!(window.find("personal-interaction-picker").visible());
        assert!(window.find("interaction-land-ids").visible());
        let refresh_revision = desktop.read(cx).request_revision;
        window.click("personal-interaction-refresh", cx);
        assert_eq!(
            desktop.read(cx).request_revision,
            refresh_revision,
            "interaction item refresh stays within its own section"
        );
        window.click("personal-interaction-use", cx);
        desktop.update(cx, |this, cx| {
            assert_eq!(this.farm.interaction.picker.read(cx).selected_value(), Some(&44));
            assert!(!this.farm.interaction.using);
            assert!(this
                .farm
                .interaction
                .notice
                .as_ref()
                .is_some_and(|(message, _)| message.contains("1,2,3")));
        });
        assert_eq!(
            window.find("personal-interaction-sellable-44").label(),
            Some("可出售: 1")
        );
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("personal-interaction-use").label(), Some("Use"));
        assert_eq!(window.find("personal-interaction-sellable-44").label(), Some("Sellable: 1"));
        assert_eq!(
            desktop.read(cx).farm.interaction.land_ids.read(cx).presentation().placeholder().as_ref(),
            "e.g. 1,2,3"
        );
    }).expect("self interaction list, picker and validation");
}
