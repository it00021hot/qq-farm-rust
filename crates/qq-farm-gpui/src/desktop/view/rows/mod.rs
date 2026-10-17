use super::super::*;

mod commerce;
mod dashboard;
mod friends;
mod game_config;
mod personal;
mod settings;

impl FarmDesktop {
    pub(super) fn render_business_row(
        &self,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match self.page {
            Page::Dashboard => self.render_dashboard_business_row(row, index, cx),
            Page::Activities => div().into_any_element(),
            Page::SolarCalendar => div().into_any_element(),
            Page::MilkTea => div().into_any_element(),
            Page::ShareReward => div().into_any_element(),
            Page::Personal => self.render_personal_business_row(row, index, cx),
            Page::Friends => self.render_friends_business_row(row, index, cx),
            Page::Analytics => div().into_any_element(),
            Page::Mall => self.render_commerce_business_row(row, index, cx),
            Page::MysteryShop => div().into_any_element(),
            Page::Settings => self.render_settings_business_row(row, index, cx),
            Page::GameConfig => self.render_game_config_business_row(row, index, cx),
            Page::Accounts => div().into_any_element(),
        }
    }
}
