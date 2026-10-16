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
            // 购买结果走全局 toast
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
