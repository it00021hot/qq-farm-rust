use super::super::*;

mod automation;
mod common;
mod offline_reminder;
mod planting;
mod strategy_timing;

impl FarmDesktop {
    pub(in crate::desktop) fn append_settings_controls(
        &self,
        mut controls: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.page == Page::Settings {
            controls = match self.page_section.as_str() {
                "strategy" => self.append_planting_controls(controls, cx),
                "automation" => self.append_automation_controls(controls, cx),
                "offlineReminder" => self.append_offline_reminder_controls(controls, cx),
                _ => controls,
            };
        }
        controls
    }
}
