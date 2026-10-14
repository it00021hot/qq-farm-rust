use super::super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn open_activity_record(
        &mut self,
        record: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(section) = super::routing::activity_section_for_record_with_stellar_fallback(
            record,
            self.activities.stellar_section.as_str(),
        ) else {
            return;
        };
        let autumn_mode = super::routing::activity_autumn_mode_for_record(record);
        if let Some(mode) = autumn_mode {
            self.activities.autumn_mode = mode.into();
            self.activities.autumn_choice_id.clear();
            self.activities.autumn_log_tab = 1;
            self.activities.autumn_logs_load_pending = mode == "happy";
        }
        self.set_page_section(section, window, cx);
    }
}
