use super::*;
use gpui_kit::component::WindowExt;
use gpui_kit::TestSupportExt;

fn log_time(seconds: i64) -> String {
    chrono::DateTime::from_timestamp(seconds, 0)
        .map(|time| time.with_timezone(&chrono::Local).format("%Y/%m/%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "--".into())
}

impl FarmDesktop {
    pub(in crate::desktop) fn show_personal_pet_logs(&self, window: &mut Window, cx: &mut App) {
        let logs = self.farm.pets.logs.clone();
        let language = self.preferences.language.clone();
        let width = window.rem_size() * 48.;
        window.open_dialog(cx, move |dialog, _, cx| {
            let columns = [
                crate::i18n::text(&language, "好友", "Friend"),
                crate::i18n::text(&language, "时间", "Time"),
                crate::i18n::text(&language, "被偷数量", "Stolen"),
                crate::i18n::text(&language, "守护金币", "Guarded Gold"),
            ];
            dialog
                .title(crate::i18n::text(&language, "守护记录", "Guard Logs"))
                .w(width)
                .child(
                    div()
                        .v_flex()
                        .max_h(rems(28.))
                        .overflow_y_scrollbar()
                        .child(
                            h_flex()
                                .gap_3()
                                .pb_2()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .children(
                                    columns
                                        .into_iter()
                                        .map(|label| div().flex_1().font_medium().child(label)),
                                ),
                        )
                        .when(logs.is_empty(), |panel| {
                            panel.child(
                                div()
                                    .py_8()
                                    .text_center()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(crate::i18n::text(
                                        &language,
                                        "暂无守护记录",
                                        "No guard logs",
                                    )),
                            )
                        })
                        .children(logs.iter().enumerate().map(|(index, log)| {
                            let friend = log
                                .get("friendName")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or(crate::i18n::text(&language, "好友", "Friend"));
                            let timestamp = log
                                .get("timestamp")
                                .and_then(serde_json::Value::as_i64)
                                .unwrap_or_default();
                            let stolen = log
                                .get("stolenCount")
                                .and_then(serde_json::Value::as_i64)
                                .unwrap_or_default();
                            let protected = log
                                .get("protectedGold")
                                .and_then(serde_json::Value::as_i64)
                                .unwrap_or_default();
                            h_flex()
                                .id(format!("pet-log-row-{index}"))
                                .gap_3()
                                .py_2()
                                .text_sm()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .child(div().flex_1().child(friend.to_string()))
                                .child(div().flex_1().child(log_time(timestamp)))
                                .child(div().flex_1().child(stolen.to_string()))
                                .child(div().flex_1().child(protected.to_string()))
                                .test_support()
                        })),
                )
                .footer(
                    Button::new("pet-logs-close")
                        .label(crate::i18n::text(&language, "关闭", "Close"))
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                )
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn guard_record_timestamp_is_seconds_and_readable() {
        assert!(super::log_time(1_790_000_000).starts_with("2026/"));
        assert_eq!(super::log_time(i64::MAX), "--");
    }
}
