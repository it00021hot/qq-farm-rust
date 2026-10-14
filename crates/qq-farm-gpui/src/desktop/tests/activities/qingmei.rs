use super::super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn qingmei_has_inventory_selection_then_round_quotes_and_brew_actions(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "qingmei".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "qingMei": {
                    "known": true,
                    "active": true,
                    "name": "青梅酿造",
                    "balanceKnown": true,
                    "balance": "12",
                    "dailySeed": {"claimed": false},
                    "started": false,
                    "finished": false,
                    "maxRounds": 2,
                    "actions": {
                        "start": {"enabled": true},
                        "continue": {"enabled": false},
                        "settle": {"enabled": false}
                    },
                    "ingredients": [
                        {"uid": "seed-a", "name": "青梅甲", "count": 3, "mutantTypes": ["香甜"]},
                        {"uid": "seed-b", "name": "青梅乙", "count": 7}
                    ],
                    "rules": {"title": "酿造规则", "paragraphs": ["选择原料并完成酿造。"]}
                }
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("qingmei-claim-seed").visible());
        assert!(window.find("qingmei-select-all").visible());
        assert!(window.find("qingmei-start-brew").visible());
        assert!(window.find(("qingmei-ingredient-toggle", 0_usize)).visible());
        assert!(window.try_find("qingmei-ingredient-select").is_none());
        window.click("qingmei-select-all", cx);
        window.render_frame(cx);
        assert!(window.find(("qingmei-count-down", 0_usize)).visible());
        window.click(("qingmei-count-down", 0_usize), cx);
    })
    .expect("select ingredients and adjust the selected recipe");

    cx.update(|cx| {
        let state = &desktop.read(cx).activities.qingmei_selected_ingredients;
        assert_eq!(state.get("seed-a"), Some(&2));
        assert_eq!(state.get("seed-b"), Some(&7));
        desktop.update(cx, |this, _| {
            this.page_raw["qingMei"]["started"] = serde_json::json!(true);
            this.page_raw["qingMei"]["currentRound"] = serde_json::json!(1);
            this.page_raw["qingMei"]["quotePrices"] = serde_json::json!(["6"]);
            this.page_raw["qingMei"]["quoteTotals"] = serde_json::json!(["36"]);
            this.page_raw["qingMei"]["actions"]["continue"]["enabled"] = serde_json::json!(true);
            this.page_raw["qingMei"]["actions"]["settle"]["enabled"] = serde_json::json!(false);
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("qingmei-continue-brew").visible());
        assert!(window.find("qingmei-settle-brew").visible());
        assert!(window.try_find("qingmei-select-all").is_none());
    })
    .expect("render round quotes and actions after brewing starts");
}

#[gpui_kit::test]
fn qingmei_brewing_controls_follow_the_selected_locale(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "qingmei".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "qingMei": {
                    "known": true,
                    "active": true,
                    "name": "Green plum brewing",
                    "balanceKnown": true,
                    "balance": 12,
                    "dailySeed": {"claimed": false},
                    "started": false,
                    "finished": false,
                    "maxRounds": 2,
                    "actions": {
                        "start": {"enabled": true},
                        "continue": {"enabled": false},
                        "settle": {"enabled": false}
                    },
                    "ingredients": [{"uid": "seed-a", "name": "Green plum", "count": 3}],
                    "rules": {"title": "Brewing rules", "paragraphs": ["Select ingredients."]}
                }
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("qingmei-claim-seed").label(), Some("Claim daily seed"));
        assert_eq!(window.find("qingmei-select-all").label(), Some("Select all"));
        assert_eq!(window.find("qingmei-start-brew").label(), Some("Start brewing"));
        window.click("qingmei-select-all", cx);
    })
    .expect("render Green Plum labels in English and select all ingredients");

    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page_raw["qingMei"]["started"] = serde_json::json!(true);
            this.page_raw["qingMei"]["currentRound"] = serde_json::json!(1);
            this.page_raw["qingMei"]["quoteTotals"] = serde_json::json!(["36"]);
            this.page_raw["qingMei"]["actions"]["continue"]["enabled"] = serde_json::json!(true);
            this.page_raw["qingMei"]["actions"]["settle"]["enabled"] = serde_json::json!(true);
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("qingmei-continue-brew").label(), Some("Continue brewing"));
        assert_eq!(window.find("qingmei-settle-brew").label(), Some("Settle brewing"));
        assert!(window.try_find("qingmei-select-all").is_none());
    })
    .expect("render enabled continuation and settlement actions in English");
}

#[gpui_kit::test]
fn qingmei_ingredient_controls_are_locked_while_a_brew_action_is_pending(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "qingmei".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "qingMei": {
                    "known": true,
                    "active": true,
                    "name": "青梅酿造",
                    "balanceKnown": true,
                    "balance": 12,
                    "dailySeed": {"claimed": false},
                    "started": false,
                    "finished": false,
                    "maxRounds": 2,
                    "actions": {
                        "start": {"enabled": true},
                        "continue": {"enabled": false},
                        "settle": {"enabled": false}
                    },
                    "ingredients": [{"uid": "seed-a", "name": "青梅", "count": 3}],
                    "rules": {"title": "酿造规则", "paragraphs": ["选择原料并完成酿造。"]}
                }
            });
            this.activities.qingmei_selected_ingredients.insert("seed-a".into(), 2);
            this.activities.pending_actions.insert("qingmei-start".into(), 1);
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("qingmei-select-all", cx);
        window.click(("qingmei-ingredient-toggle", 0_usize), cx);
        window.click(("qingmei-count-down", 0_usize), cx);
        window.click(("qingmei-count-up", 0_usize), cx);
        window.click(("qingmei-count-all", 0_usize), cx);
    })
    .expect("keep ingredient selection and quantity stable during brewing");
    cx.update(|cx| {
        assert_eq!(
            desktop.read(cx).activities.qingmei_selected_ingredients.get("seed-a"),
            Some(&2)
        );
        assert!(desktop.read(cx).activity_action_is_pending("qingmei-start"));
    });
}
