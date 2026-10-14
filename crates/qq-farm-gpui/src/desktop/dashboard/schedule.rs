use super::super::*;
use super::format::{check_row, int_at};

impl FarmDesktop {
    pub(super) fn render_schedule_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let status = self.page_raw.get("status").unwrap_or(&serde_json::Value::Null);
        let language = self.preferences.language.as_str();
        let online = int_at(status, &["runStatus"]) == 1;
        let next_checks = status.get("nextChecks").unwrap_or(&serde_json::Value::Null);
        let schedule_card = div()
            .id("dashboard-schedule-card")
            .v_flex()
            .flex_1()
            .min_w(px(230.))
            .gap_3()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_4()
            .child(div().font_medium().child(crate::i18n::text(
                language,
                "下次巡查倒计时",
                "Next check countdown",
            )))
            .children([
                check_row(
                    language,
                    crate::i18n::text(language, "🌱 下次农场巡查", "🌱 Next farm check"),
                    next_checks,
                    "farmRemainSec",
                    "farmQuiet",
                    online,
                    cx,
                ),
                check_row(
                    language,
                    crate::i18n::text(language, "🤝 下次帮助", "🤝 Next help"),
                    next_checks,
                    "helpRemainSec",
                    "helpQuiet",
                    online,
                    cx,
                ),
                check_row(
                    language,
                    crate::i18n::text(language, "🏃 下次偷菜", "🏃 Next steal"),
                    next_checks,
                    "stealRemainSec",
                    "stealQuiet",
                    online,
                    cx,
                ),
            ])
            .test_support();

        schedule_card.into_any_element()
    }
}
