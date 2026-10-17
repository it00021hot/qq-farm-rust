use super::*;

impl FarmDesktop {
    pub(super) fn render_root(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let sidebar = self.render_sidebar(cx).into_any_element();
        let content = self.render_content(window, cx);
        let layout = self.render_workspace_layout(sidebar, content, cx);
        h_flex()
            .id("farm-root")
            .relative()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, action: &RefreshPage, window, cx| {
                this.dispatch_ui_command(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &FocusSearch, window, cx| {
                this.dispatch_ui_command(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &ToggleSidebar, window, cx| {
                this.dispatch_ui_command(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &OpenSettings, window, cx| {
                this.dispatch_ui_command(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &SaveChanges, window, cx| {
                this.dispatch_ui_command(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &CheckForUpdates, window, cx| {
                this.dispatch_ui_command(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &NextPage, window, cx| {
                this.dispatch_ui_command(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &PreviousPage, window, cx| {
                this.dispatch_ui_command(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &CancelOverlay, window, cx| {
                this.dispatch_ui_command(action, window, cx)
            }))
            .on_action(cx.listener(|this, action: &QuitApp, window, cx| {
                this.dispatch_ui_command(action, window, cx)
            }))
            .items_stretch()
            .size_full()
            .child(layout)
            .child(self.render_toast(cx))
    }
}

impl Render for FarmDesktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_root(window, cx)
    }
}
