use crate::desktop::FarmDesktop;
use gpui_kit::Context;

impl FarmDesktop {
    pub(in crate::desktop) fn apply_settings_snapshot(
        &mut self,
        snapshot: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        self.settings.draft = snapshot;
        self.refresh_strategy_preview(cx);
        if let Some(handle) = self.window_handle {
            let _ = handle.update(cx, |_, window, cx| {
                self.sync_system_config_inputs(window, cx);
                self.sync_setting_selectors(window, cx);
            });
        }
    }
}
