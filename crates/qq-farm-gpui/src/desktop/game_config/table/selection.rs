use super::super::super::*;

impl FarmDesktop {
    pub(super) fn set_game_config_selection(
        &mut self,
        section: &str,
        id: i64,
        selected: bool,
        cx: &mut Context<Self>,
    ) {
        let key = (section.to_string(), id);
        if selected {
            self.game_config.selected_ids.insert(key);
        } else {
            self.game_config.selected_ids.remove(&key);
        }
        cx.notify();
    }
}
