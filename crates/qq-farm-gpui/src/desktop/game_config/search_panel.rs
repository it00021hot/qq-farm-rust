use super::super::*;
use gpui_kit::component::select::Select;

impl FarmDesktop {
    pub(super) fn render_game_config_search_panel(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let toggle = Button::new("game-config-search-toggle")
            .label(crate::i18n::text(
                language,
                if self.game_config.search_open { "收起搜索" } else { "搜索" },
                if self.game_config.search_open { "Hide search" } else { "Search" },
            ))
            .ghost()
            .on_click(cx.listener(|this, _, _, cx| {
                this.game_config.search_open = !this.game_config.search_open;
                cx.notify();
            }));
        let mut panel = div().w_full().rounded(cx.theme().radius).bg(cx.theme().group_box).p_3();
        let mut fields = h_flex().flex_wrap().items_center().gap_3().child(
            Input::new(&self.search).id("game-config-search-keyword").cleanable(true).w(px(240.)),
        );

        match self.page_section.as_str() {
            "seeds" => {
                fields = fields.child(filter_select(
                    "game-config-season-select",
                    crate::i18n::text(language, "季节", "Seasons"),
                    &self.game_config.season_selector,
                ));
            }
            "fruits" => {
                fields = fields.child(filter_select(
                    "game-config-rarity-select",
                    crate::i18n::text(language, "稀有度", "Rarity"),
                    &self.game_config.rarity_selector,
                ));
            }
            "items" => {
                fields = fields
                    .child(filter_select(
                        "game-config-item-type-select",
                        crate::i18n::text(language, "类型", "Type"),
                        &self.game_config.item_type_selector,
                    ))
                    .child(filter_select(
                        "game-config-rarity-select",
                        crate::i18n::text(language, "稀有度", "Rarity"),
                        &self.game_config.rarity_selector,
                    ));
            }
            _ => {}
        }

        fields =
            fields
                .child(
                    Button::new("game-config-search-reset")
                        .label(crate::i18n::text(language, "重置", "Reset"))
                        .ghost()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.reset_game_config_search(window, cx)
                        })),
                )
                .child(
                    Button::new("game-config-search-apply")
                        .label(crate::i18n::text(language, "搜索", "Search"))
                        .primary()
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| this.apply_game_config_search(cx))),
                );

        panel = panel.child(fields);
        div()
            .w_full()
            .v_flex()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(toggle)
                    .child(self.render_game_config_column_settings(cx)),
            )
            .when(self.game_config.search_open, |view| view.child(panel))
    }

    pub(in crate::desktop) fn apply_game_config_search(&mut self, cx: &mut Context<Self>) {
        self.game_config.applied_keyword = self.search.read(cx).value().trim().to_string();
        self.game_config.applied_seasons = self.game_config.seasons;
        self.game_config.applied_rarity = self.game_config.rarity;
        self.game_config.applied_item_type = self.game_config.item_type;
        self.game_config.page = 0;
        cx.notify();
    }

    fn reset_game_config_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |state, cx| state.set_value("", window, cx));
        self.game_config.seasons = None;
        self.game_config.rarity = None;
        self.game_config.item_type = None;
        self.sync_game_config_selectors(window, cx);
        cx.notify();
    }
}

fn filter_select(
    id: &'static str,
    label: &str,
    state: &gpui_kit::Entity<super::selectors::GameConfigFilterSelect>,
) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap_2()
        .child(div().text_sm().child(label.to_owned()))
        .child(Select::new(state).id(id).w(px(190.)).accessibility_label(label.to_owned()))
}
