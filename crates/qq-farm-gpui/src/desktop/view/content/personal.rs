use super::super::super::*;
use super::state::ContentState;

impl FarmDesktop {
    pub(super) fn render_personal_content(
        &self,
        state: &ContentState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut content = Vec::new();
        if self.page == Page::Personal {
            content.push(self.render_personal_status(cx).into_any_element());
        }
        if state.daily_gifts_view {
            content.push(self.render_personal_daily_gifts(cx).into_any_element());
        }
        if state.personal_lands_view {
            content.push(self.render_personal_lands(cx).into_any_element());
        }
        if state.personal_bag {
            content.push(self.render_personal_bag(window, cx).into_any_element());
        }
        if state.personal_interaction_view {
            content.push(self.render_personal_interaction_items(cx).into_any_element());
        }
        if state.personal_illustrated_view {
            content.push(self.render_personal_illustrated(window, cx).into_any_element());
        }
        if state.personal_pets_view {
            content.push(self.render_personal_pets(cx).into_any_element());
        }
        content
    }
}
