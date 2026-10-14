use super::super::super::*;
use super::state::ContentState;

impl FarmDesktop {
    pub(super) fn render_commerce_content(
        &self,
        state: &ContentState,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut content = Vec::new();
        if state.mall_catalog_view || state.mystery_shop_view {
            if let Some((message, success)) = self.commerce.notice.as_ref() {
                content.push(
                    div()
                        .id("commerce-action-notice")
                        .text_sm()
                        .text_color(if *success { cx.theme().success } else { cx.theme().danger })
                        .child(message.clone())
                        .into_any_element(),
                );
            }
        }
        if state.mall_catalog_view {
            content.push(self.render_mall_shop_panel(cx).into_any_element());
        }
        if state.mystery_shop_view {
            content.push(self.render_mystery_shop_panel(cx).into_any_element());
        }
        content
    }
}
