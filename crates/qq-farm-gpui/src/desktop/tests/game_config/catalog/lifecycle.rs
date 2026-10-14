use super::*;

#[gpui_kit::test]
fn leaving_game_config_resets_page_local_table_and_editor_state(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.game_config.form_open = true;
            this.game_config.selected_ids.insert(("seeds".into(), 501));
            this.game_config.pending_delete = Some(("seed".into(), 501));
            this.game_config.seasons = Some(2);
            this.game_config.page = 3;
            this.game_config.page_size = 50;
            let seed_id = this.game_config.columns.get_mut("seeds").unwrap();
            seed_id[0].checked = false;
            seed_id[0].pin = crate::desktop::game_config::columns::GameConfigColumnPin::Left;
            this.set_page(Page::Dashboard, cx);
        });
        window.render_frame(cx);
        let state = desktop.read(cx);
        assert_eq!(state.page, Page::Dashboard);
        assert!(!state.game_config.form_open);
        assert!(state.game_config.selected_ids.is_empty());
        assert!(state.game_config.pending_delete.is_none());
        assert!(!state.game_config.pending_batch_delete);
        assert_eq!(state.game_config.seasons, None);
        assert_eq!(state.game_config.page, 0);
        assert_eq!(state.game_config.page_size, 10);
        assert!(state.game_config.columns["seeds"].iter().all(|column| column.checked));
        assert_eq!(
            state.game_config.columns["seeds"][0].pin,
            crate::desktop::game_config::columns::GameConfigColumnPin::Unfixed
        );
    })
    .expect("leaving the catalog unmounts its local filter, selection, and sheet state");
}
