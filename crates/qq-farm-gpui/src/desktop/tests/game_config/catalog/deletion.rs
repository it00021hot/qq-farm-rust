use super::*;

#[gpui_kit::test]
async fn game_config_batch_delete_requires_confirmation_and_cancel_preserves_selection(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.game_config.selected_ids.insert(("seeds".into(), 987_654_321));
            cx.notify();
        });
        window.render_frame(cx);
        window.click("game-config-batch-delete", cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).game_config.pending_batch_delete);
    })
    .expect("open the batch-delete confirmation without deleting immediately");
    wait_for_batch_delete_popover(cx, window).await;

    cx.update_window(window, |_, window, cx| {
        window.click("game-config-batch-delete-cancel", cx);
        window.render_frame(cx);
        let state = desktop.read(cx);
        assert!(!state.game_config.pending_batch_delete);
        assert!(state.game_config.selected_ids.contains(&("seeds".into(), 987_654_321)));
    })
    .expect("cancel batch deletion and retain the selected rows");
}
#[gpui_kit::test]
async fn game_config_batch_delete_only_removes_the_selected_record_after_confirmation(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let seed_id = 987_654_325;
    qq_farm_app::config::upsert_game_config(
        qq_farm_app::config::GameConfigEntry::Plant(qq_farm_core::config::game_config::Plant {
            id: seed_id,
            seed_id: Some(seed_id),
            name: "Batch delete confirmation probe".into(),
            ..Default::default()
        }),
        None,
    )
    .expect("create an isolated seed configuration for the confirmation flow");

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({
                "seeds": [{"seedId": seed_id, "name": "Batch delete confirmation probe"}]
            });
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            this.game_config.selected_ids.insert(("seeds".into(), seed_id));
            cx.notify();
        });
        window.render_frame(cx);
        window.click("game-config-batch-delete", cx);
        window.render_frame(cx);
        let config = qq_farm_core::config::game_config::GameConfig::new();
        config.load();
        assert!(
            config.get_plant_by_seed_id(seed_id).is_some(),
            "triggering delete must only open confirmation"
        );
    })
    .expect("open confirmation without deleting the configuration");
    wait_for_batch_delete_popover(cx, window).await;

    cx.update_window(window, |_, window, cx| {
        window.click("game-config-batch-delete-confirm", cx);
        window.render_frame(cx);
        let config = qq_farm_core::config::game_config::GameConfig::new();
        config.load();
        assert!(
            config.get_plant_by_seed_id(seed_id).is_none(),
            "confirmation should delete the selected configuration"
        );
        assert!(!desktop.read(cx).game_config.pending_batch_delete);
        assert!(desktop.read(cx).game_config.selected_ids.is_empty());
    })
    .expect("confirm and delete the selected configuration");
}
#[gpui_kit::test]
async fn game_config_row_delete_confirmation_dismisses_with_escape(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({
                "seeds": [{"seedId": 987_654_397, "name": "Escape probe"}]
            });
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            cx.notify();
        });
    });
    cx.simulate_window_resize(window, size(px(1440.), px(900.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("config-delete", 0_usize), cx);
        window.render_frame(cx);
    })
    .expect("open the row delete confirmation");
    wait_for_delete_popover(cx, window).await;
    cx.update_window(window, |_, window, cx| window.press("escape", cx))
        .expect("dismiss the confirmation with Escape");
    cx.wait_for(window, Duration::from_secs(1), |_, cx| {
        desktop.read(cx).game_config.pending_delete.is_none()
    })
    .await;
}
#[gpui_kit::test]
async fn game_config_row_delete_confirmation_removes_the_local_override(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let seed_id = 987_654_398;
    qq_farm_app::config::upsert_game_config(
        qq_farm_app::config::GameConfigEntry::Plant(qq_farm_core::config::game_config::Plant {
            id: seed_id,
            name: "Row delete probe".into(),
            seed_id: Some(seed_id),
            ..Default::default()
        }),
        None,
    )
    .expect("write a temporary seed overlay for the row delete workflow");

    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({
                "seeds": [{"seedId": seed_id, "name": "Row delete probe"}]
            });
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            cx.notify();
        });
    });
    cx.simulate_window_resize(window, size(px(1440.), px(900.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("config-delete", 0_usize), cx);
        window.render_frame(cx);
    })
    .expect("open the row delete confirmation");
    wait_for_delete_popover(cx, window).await;
    cx.update_window(window, |_, window, cx| {
        window.click(("config-delete-confirm", 0_usize), cx);
        window.render_frame(cx);
    })
    .expect("confirm removal of the local override");

    cx.wait_for(window, Duration::from_secs(2), |_, cx| {
        desktop.read(cx).game_config.success_feedback.is_some()
            && desktop.read(cx).game_config.pending_delete.is_none()
    })
    .await;
    let reloaded = qq_farm_core::config::game_config::GameConfig::new();
    reloaded.load();
    assert!(reloaded.get_plant_by_seed_id(seed_id).is_none());
}
