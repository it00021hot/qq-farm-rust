use super::super::*;
use gpui_kit::component::resizable::{h_resizable, resizable_panel, ResizableState};

impl FarmDesktop {
    pub(in crate::desktop) fn render_workspace_layout(
        &self,
        sidebar: AnyElement,
        content: Div,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.sidebar_collapsed {
            return h_flex()
                .size_full()
                .child(div().w(rems(4.)).h_full().flex_shrink_0().child(sidebar))
                .child(content)
                .into_any_element();
        }
        h_resizable("farm-workspace-layout")
            .with_state(&self.sidebar_layout)
            .child(
                resizable_panel()
                    .size(px(self.preferences.sidebar_width.clamp(200, 288) as f32))
                    .size_range(px(200.)..px(288.))
                    .child(sidebar),
            )
            .child(resizable_panel().child(content))
            .on_resize(cx.listener(|this, layout: &Entity<ResizableState>, _, cx| {
                if let Some(width) = layout.read(cx).sizes().first() {
                    this.preferences.sidebar_width =
                        width.as_f32().round().clamp(200., 288.) as u32;
                    this.persist_ui_preferences();
                    cx.notify();
                }
            }))
            .into_any_element()
    }
}
