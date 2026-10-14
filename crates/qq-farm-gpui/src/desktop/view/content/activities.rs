use super::super::super::*;
use super::state::ContentState;

impl FarmDesktop {
    pub(super) fn render_activity_content(
        &self,
        state: &ContentState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut content = Vec::new();
        if let Some((message, success)) = self.activities.action_notice.as_ref() {
            content.push(
                div()
                    .id("activity-action-notice")
                    .text_sm()
                    .text_color(if *success { cx.theme().success } else { cx.theme().danger })
                    .child(message.clone())
                    .into_any_element(),
            );
        }
        if state.activity_directory {
            content.push(self.render_activity_directory(cx).into_any_element());
        }
        if state.charity_view {
            content.push(self.render_charity_panel(cx).into_any_element());
        }
        if state.activity_shop_view {
            content.push(self.render_activity_shop_panel(cx).into_any_element());
        }
        if state.pet_diary_view {
            content.push(self.render_pet_diary_panel(cx).into_any_element());
        }
        if state.autumn_view {
            content.push(self.render_autumn_panel(window, cx).into_any_element());
        }
        if state.qixi_view {
            content.push(self.render_qixi_panel(window, cx).into_any_element());
        }
        if state.qingmei_view {
            content.push(self.render_qingmei_panel(cx).into_any_element());
        }
        if state.constellation_view {
            content.push(self.render_constellation_panel(cx).into_any_element());
        }
        if state.solar_terms_view {
            content.push(self.render_solar_terms_panel(cx).into_any_element());
        }
        if state.season_view {
            content.push(self.render_season_pass_panel(cx).into_any_element());
        }
        if state.weather_view {
            content.push(self.render_weather_panel(window, cx).into_any_element());
        }
        content
    }
}
