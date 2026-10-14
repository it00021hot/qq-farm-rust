use super::super::*;
use super::search::{game_config_filters_match, matches_vue_keyword};

impl FarmDesktop {
    pub(in crate::desktop) fn append_game_config_controls(
        &self,
        mut controls: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.page == Page::GameConfig {
            controls = controls.child(self.render_game_config_search_panel(cx));
        }
        controls
    }

    /// Catalog rows matching the applied keyword and filters. The table slices
    /// rows into pages itself, so the pagination footer needs this pre-slice
    /// count.
    pub(in crate::desktop) fn game_config_filtered_total(&self) -> usize {
        let query = self.game_config.applied_keyword.trim().to_lowercase();
        self.rows
            .iter()
            .filter(|row| {
                game_config_filters_match(
                    row,
                    &self.page_section,
                    self.game_config.applied_seasons,
                    self.game_config.applied_rarity,
                    self.game_config.applied_item_type,
                ) && (query.is_empty() || matches_vue_keyword(row, &query))
            })
            .count()
    }
}
