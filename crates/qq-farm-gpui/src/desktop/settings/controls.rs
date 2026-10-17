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
                    self.append_planting_controls(column, cx).child(
                        Button::new("save-strategy-settings")
                            .label(crate::i18n::text(
                                &self.preferences.language,
                                "保存策略设置",
                                "Save strategy settings",
                            ))
                            .primary()
                            .loading(self.settings.strategy_saving)
                            .disabled(self.loading || self.selected_account.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.save_settings(cx))),
                    )
                }
                "automation" => self.append_automation_controls(controls, cx),
                "offlineReminder" => self.append_offline_reminder_controls(controls, cx),
                _ => controls,
            };
        }
        controls
    }
}
