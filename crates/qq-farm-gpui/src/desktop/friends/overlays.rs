use super::super::*;

mod career;
mod lands;

use career::format_career_count;

impl FarmDesktop {
    pub(in crate::desktop) fn render_friend_detail(
        &self,
        gid: i64,
        detail: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let language = &self.preferences.language;
        let english = language == "en-US";
        let all_lands = detail
            .pointer("/lands")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let career = detail.get("career").filter(|career| !career.is_null());
        let mut panel = div()
            .v_flex()
            .gap_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background.opacity(0.55))
            .p_4();

        if let Some(career) = career {
            let harvest =
                career.get("harvest").and_then(serde_json::Value::as_i64).unwrap_or_default();
            let steal = career.get("steal").and_then(serde_json::Value::as_i64).unwrap_or_default();
            let ratio = if harvest <= 0 && steal > 0 {
                "--".to_string()
            } else if harvest <= 0 {
                "0%".to_string()
            } else {
                format!("{:.1}%", steal as f64 / harvest as f64 * 100.)
            };
            panel = panel.child(
                h_flex()
                    .items_center()
                    .flex_wrap()
                    .gap_4()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().group_box)
                    .px_3()
                    .py_2()
                    .child(div().font_medium().child(crate::i18n::text(
                        language,
                        "生涯统计",
                        "Career stats",
                    )))
                    .child(if english {
                        format!(
                            "Harvested {} · Stolen {} · Steal rate {ratio}",
                            format_career_count(harvest, true),
                            format_career_count(steal, true)
                        )
                    } else {
                        format!(
                            "收获 {} · 偷取 {} · 收偷比 {ratio}",
                            format_career_count(harvest, false),
                            format_career_count(steal, false)
                        )
                    }),
            );
        }

        panel.child(self.render_friend_lands(gid, &all_lands, cx)).into_any_element()
    }
}
