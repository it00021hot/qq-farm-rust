use super::super::support::open_test_desktop;
use crate::desktop::personal::{fertilize_candidate_count, visible_farm_operations};
use crate::desktop::Page;
use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn personal_farm_renders_career_summary_and_merged_land_cards(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Personal;
            this.page_section = "lands".into();
            this.selected_account = Some("7".into());
            this.farm.lands.now_sec = 100;
            this.farm.lands.loaded_at_sec = 100;
            this.page_raw = serde_json::json!({
                "lands": {
                    "lands": [
                        {"id": 1, "status": "growing", "unlocked": true,
                            "plantName": "太阳花", "matureInSec": 30, "totalGrowTime": 60,
                            "seedImage": "/game-config/seed/1.webp", "plantSize": 2,
                            "occupiedLandIds": [1, 2, 5, 6], "level": 2, "currentSeason": 1,
                            "totalSeason": 2, "needWater": true,
                            "mutantConfigIds": [31], "purpleCrystalResonanceExpBonus": 250,
                            "interactionEffects": [{"itemName": "彩虹肥料"}],
                            "needInteractionCleanup": true},
                        {"id": 2, "status": "growing", "occupiedByMaster": true},
                        {"id": 3, "status": "empty", "unlocked": true, "couldUpgrade": true}
                    ],
                    "summary": {"harvestable": 1, "growing": 2, "empty": 3, "dead": 4},
                    "career": {"harvest": 12345, "steal": 1234}
                }
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("career-harvest").visible());
        assert!(window.find("career-steal").visible());
        assert!(window.find("career-ratio").visible());
        assert!(window.find("land-summary-harvestable").visible());
        assert!(window.find("farm-land-card-1").visible());
        assert!(window.try_find("farm-land-card-2").is_none());
        assert!(window.find("farm-land-card-3").visible());
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("clear").label(), Some("Maintain"));
        assert_eq!(window.find("plant").label(), Some("Plant"));
        assert_eq!(window.find("upgrade").label(), Some("Upgrade Lands"));
        assert_eq!(window.find("all").label(), Some("Run All"));
    })
    .expect("render personal farm with career, summary and merged land footprint");
}

#[gpui_kit::test]
fn personal_farm_keeps_the_mutation_badge_when_only_the_purple_bonus_is_named(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Personal;
            this.page_section = "lands".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "lands": {
                    "lands": [{
                        "id": 1,
                        "status": "growing",
                        "unlocked": true,
                        "plantName": "太阳花",
                        "mutantEffects": [{"description": "经验加成"}],
                        "purpleCrystalResonanceExpBonus": 250
                    }]
                }
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("farm-land-mutation-badge").visible());
        assert!(window.find("farm-land-plant-1").visible());
    })
    .expect("a purple bonus keeps the Vue mutation badge even when its name list is empty");
}

#[test]
fn farm_toolbar_only_shows_operations_with_actionable_lands() {
    assert!(visible_farm_operations(&serde_json::Value::Null).is_empty());
    let operations = visible_farm_operations(&serde_json::json!({
        "lands": {"lands": [
            {"id": 1, "unlocked": true, "status": "harvestable", "needWater": true},
            {"id": 2, "unlocked": true, "status": "empty", "couldUpgrade": true},
            {"id": 3, "unlocked": false, "status": "locked", "couldUnlock": true}
        ]}
    }));
    assert_eq!(
        operations,
        vec![
            ("harvest", "收获"),
            ("clear", "一键务农"),
            ("plant", "种植"),
            ("upgrade", "升级土地"),
            ("all", "一键全收"),
        ]
    );
}

#[test]
fn farm_toolbar_ignores_harvestable_slave_cells_of_merged_plants() {
    let operations = visible_farm_operations(&serde_json::json!({
        "lands": {"lands": [
            {"id": 1, "unlocked": true, "status": "growing",
                "plantSize": 2, "occupiedLandIds": [1, 2]},
            {"id": 2, "unlocked": true, "status": "harvestable", "occupiedByMaster": true}
        ]}
    }));
    assert!(operations.is_empty());
}

#[test]
fn farm_operation_visibility_uses_vue_number_and_truthiness_coercion() {
    let mature_in_future = serde_json::json!({
        "lands": {"lands": [{
            "id": 1,
            "unlocked": true,
            "status": "growing",
            "matureInSec": "30",
            "plantName": "玉米"
        }]}
    });
    assert!(visible_farm_operations(&mature_in_future).is_empty());

    let empty_string_name = serde_json::json!({
        "lands": {"lands": [{
            "id": 1,
            "unlocked": true,
            "status": "growing",
            "matureInSec": "0",
            "plantName": ""
        }]}
    });
    assert!(visible_farm_operations(&empty_string_name).is_empty());

    let zero_mature_numeric_string = serde_json::json!({
        "lands": {"lands": [{
            "id": 1,
            "unlocked": true,
            "status": "growing",
            "matureInSec": "0",
            "plantName": "玉米"
        }]}
    });
    assert_eq!(
        visible_farm_operations(&zero_mature_numeric_string),
        vec![("harvest", "收获"), ("all", "一键全收")]
    );
}

#[gpui_kit::test]
fn farm_operation_uses_local_pending_state_and_feedback(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Personal;
            this.page_section = "lands".into();
            this.selected_account = Some("missing-worker".into());
            this.page_raw = serde_json::json!({
                "lands": {"lands": [{"id": 1, "status": "harvestable", "unlocked": true}]}
            });
            this.farm.lands.operating = true;
            this.farm.lands.operation_notice = Some(("操作失败".into(), false));
            assert!(this.farm.lands.operating);
            assert!(!this.loading);
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("farm-operation-feedback").visible());
        window.click("all", cx);
        assert_eq!(desktop.read(cx).farm.lands.operation_generation, 0);
    })
    .expect("show local farm busy and error states without a page-wide loading state");

    cx.update(|cx| {
        let state = desktop.read(cx);
        assert!(state.farm.lands.operating);
        assert!(!state.loading);
        assert!(state.farm.lands.operation_notice.as_ref().is_some_and(|(_, success)| !success));
    });
}

#[gpui_kit::test]
fn personal_offline_state_hides_business_actions_and_rows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Personal;
            this.page_section = "bag".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({"connected":false});
        })
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("personal-unavailable").visible());
        assert_eq!(window.find("personal-unavailable").label(), Some("账号未运行，请先启动账号"));
        assert!(window.try_find("bag-batch-toggle").is_none());
        assert!(window.try_find("bag-category-fruit").is_none());
        desktop.update(cx, |this, _| this.selected_account = None);
        window.render_frame(cx);
        assert_eq!(window.find("personal-unavailable").label(), Some("请选择账号"));
    })
    .expect("offline farm must not expose mutation controls");
}

#[test]
fn fertilize_candidates_count_growing_crops_only() {
    assert_eq!(fertilize_candidate_count(&serde_json::Value::Null), 0);
    let raw = serde_json::json!({
        "lands": {"lands": [
            {"id": 1, "unlocked": true, "status": "growing", "matureInSec": 600},
            {"id": 2, "unlocked": true, "status": "growing", "matureInSec": 0},
            {"id": 3, "unlocked": true, "status": "harvestable", "matureInSec": 600},
            {"id": 4, "unlocked": false, "status": "growing", "matureInSec": 600},
            {"id": 5, "unlocked": true, "status": "growing", "matureInSec": 600,
                "occupiedByMaster": true}
        ]}
    });
    assert_eq!(fertilize_candidate_count(&raw), 1);
}

#[gpui_kit::test]
fn fertilize_all_dialog_offers_fertilizer_types(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Personal;
            this.page_section = "lands".into();
            this.selected_account = Some("missing-worker".into());
            this.page_raw = serde_json::json!({
                "lands": {"lands": [
                    {"id": 1, "status": "growing", "unlocked": true, "matureInSec": 600},
                    {"id": 2, "status": "harvestable", "unlocked": true}
                ]}
            });
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("fertilize-all", cx);
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        assert_eq!(window.find("fertilize-all-count").label(), Some("可施肥 1 株作物"));
        assert_eq!(window.find("fertilize-all-normal").label(), Some("普通化肥"));
        assert_eq!(window.find("fertilize-all-organic").label(), Some("有机化肥"));
        window.click("fertilize-all-cancel", cx);
        window.render_frame(cx);
        assert!(!window.has_active_dialog(cx));
        assert_eq!(desktop.read(cx).farm.lands.fertilize_all_generation, 0);
    })
    .expect("one-click fertilize dialog shows candidate count and both fertilizer types");
}

#[gpui_kit::test]
fn fertilize_all_button_stays_inert_without_candidates(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Personal;
            this.page_section = "lands".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "lands": {"lands": [
                    {"id": 2, "status": "harvestable", "unlocked": true}
                ]}
            });
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("fertilize-all", cx);
        window.render_frame(cx);
        assert!(!window.has_active_dialog(cx));
        assert_eq!(desktop.read(cx).farm.lands.fertilize_all_generation, 0);
    })
    .expect("no candidate crops -> fertilize-all cannot open its confirmation");
}
