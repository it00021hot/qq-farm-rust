use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn render_row_actions(
        &self,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut actions = h_flex().gap_1();
        actions = self.append_account_row_actions(actions, row, index, cx);
        actions = self.append_friend_row_actions(actions, row, index, cx);
        actions = self.append_settings_row_actions(actions, row, index, cx);
        actions = self.append_commerce_row_actions(actions, row, index, cx);
        actions = self.append_activity_row_actions(actions, row, index, cx);
        actions = self.append_game_config_row_actions(actions, row, index, cx);
        actions
    }
}
