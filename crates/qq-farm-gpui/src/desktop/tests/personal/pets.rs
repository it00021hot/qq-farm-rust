use super::super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn pets_render_real_ownership_gates_food_input_and_guard_records(cx: &mut TestAppContext) {
    use gpui_kit::component::WindowExt;
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let mut previous_focus = None;
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Personal;
            this.page_section = "pets".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "pets":{"activeDogId":1,"protectDuration":93600,"maxProtectDuration":864000,
                    "dogs":[
                        {"id":1,"name":"值守犬","active":true,"owned":true,
                            "skillDescription":"技能描述1234567890ABCDEFGHIJKLMNOP"},
                        {"id":2,"name":"可上场犬","active":false,"owned":true},
                        {"id":3,"name":"有卡犬","active":false,"owned":false,"activatable":true},
                        {"id":4,"name":"无卡犬","active":false,"owned":false,"activatable":false}
                    ],
                    "foods":[{"id":90004,"name":"普通狗粮","count":5,"duration":86400}]},
                "petGiftStatus":{"pending":2}
            });
            this.farm.pets.logs = vec![serde_json::json!({"friendName":"好友甲","timestamp":1790000000,"stolenCount":2,"protectedGold":100})];
            this.sync_personal_pet_food_inputs(window, cx);
        });
        window.render_frame(cx);
        assert!(window.find("pet-protection-remaining").visible());
        assert_eq!(
            window.find("pet-skill-1").label(),
            Some("技能描述1234567890ABCDEFGHIJKLMNOP")
        );
        assert!(window.find("pet-deploy-2").visible());
        assert!(window.find("pet-activate-3").visible());
        assert!(window.try_find("pet-deploy-1").is_none());
        assert!(window.try_find("pet-deploy-4").is_none());
        assert!(window.try_find("pet-activate-4").is_none());
        assert!(window.find("pet-withdraw").visible());
        assert!(window.find("claim-pet-gifts").visible());
        for id in [
            "claim-pet-gifts",
            "pet-protect-logs",
            "pet-refresh",
            "pet-withdraw",
            "pet-deploy-2",
            "pet-food-use-90004",
        ] {
            assert_ne!(window.find(id).disabled(), Some(true), "{id} stays available");
        }
        desktop.update(cx, |this, cx| {
            this.farm.pets.activating = true;
            this.farm.pets.activation_refresh_pending = true;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("pet-activate-3").visible());
        for id in [
            "claim-pet-gifts",
            "pet-protect-logs",
            "pet-refresh",
            "pet-withdraw",
            "pet-deploy-2",
            "pet-food-use-90004",
        ] {
            assert_ne!(window.find(id).disabled(), Some(true), "{id} stays available");
        }
        let refresh_revision = desktop.read(cx).request_revision;
        window.click("pet-refresh", cx);
        assert_eq!(
            desktop.read(cx).request_revision,
            refresh_revision,
            "pet refresh stays scoped and does not reload the whole page"
        );
        assert!(!desktop.read(cx).farm.pets.activation_refresh_pending);
        assert!(!desktop.read(cx).farm.pets.activating);
        desktop.update(cx, |this, cx| {
            this.farm.pets.activating = false;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("increment", cx);
        desktop.update(cx, |this, cx| {
            assert_eq!(this.farm.pets.food_inputs[&90004].read(cx).value(), "2");
        });
        window.click("page-search", cx);
        previous_focus = window.focused(cx);
        desktop.update(cx, |this, cx| this.show_personal_pet_logs(window, cx));
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        assert!(window.find("pet-log-row-0").visible());
        window.press("escape", cx);
    })
    .expect("show pet records and request keyboard dismissal");
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(!window.has_active_dialog(cx));
        assert_eq!(
            window.focused(cx),
            previous_focus,
            "closing records must restore the previous focus"
        );
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("pet-refresh").label(), Some("Refresh"));
        assert_eq!(window.find("claim-pet-gifts").label(), Some("Claim"));
        assert_eq!(window.find("pet-food-use-90004").label(), Some("Feed"));
        assert_eq!(window.find("pet-protect-logs").label(), Some("Guard Logs"));
    })
    .expect(
        "pet capability gates and editable food quantity remain available after dismissing records",
    );
}

#[gpui_kit::test]
fn pet_food_empty_state_remains_visible_when_pet_info_is_unavailable(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Personal;
            this.page_section = "pets".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({"connected":true});
        });
        window.render_frame(cx);
        assert!(window.find("personal-pets-empty").visible());
        assert!(window.find("personal-pet-food-empty").visible());
    })
    .expect("pet food remains visible with the pet information empty state");
}
