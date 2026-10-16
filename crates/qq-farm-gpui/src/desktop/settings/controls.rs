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
                // 策略 tab：整行分组卡片竖排（对齐 Tauri 分隔线 + 网格布局）
                "strategy" => {
                    let column = div().v_flex().gap_3().w_full();
                    self.append_planting_controls(column, cx)
                }
                "automation" => self.append_automation_controls(controls, cx),
                "offlineReminder" => self.append_offline_reminder_controls(controls, cx),
                _ => controls,
            };
        }
        controls
    }
}
