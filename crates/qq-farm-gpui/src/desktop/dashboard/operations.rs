use super::super::*;
use super::format::{int_at, number_i64, operation_label};

impl FarmDesktop {
    pub(super) fn render_operations_card(&self, cx: &mut Context<Self>) -> Div {
        let status = self.page_raw.get("status").unwrap_or(&serde_json::Value::Null);
        let language = self.preferences.language.as_str();
        let online = int_at(status, &["runStatus"]) == 1;
        let operations = status
            .get("operations")
            .and_then(serde_json::Value::as_object)
            .map(|operations| {
                operations
                    .iter()
                    .filter(|(key, _)| !matches!(key.as_str(), "upgrade" | "levelUp"))
                    .map(|(key, count)| (key.clone(), number_i64(count).unwrap_or_default()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let operations_card = div()
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
                "今日统计",
                "Today's stats",
            )))
            .when(!online, |card| {
                card.child(
                    div()
                        .id("dashboard-operations-offline")
                        .flex_1()
                        .min_h(px(120.))
                        .v_flex()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .py_6()
                        .text_center()
                        .text_color(cx.theme().muted_foreground)
                        .child(div().text_2xl().child("📡"))
                        .child(crate::i18n::text(language, "账号未登录", "Not logged in"))
                        .child(div().text_xs().child(crate::i18n::text(
                            language,
                            "账号未运行，请先启动账号",
                            "Account is not running. Please start it first.",
                        )))
                        .test_support(),
                )
            })
            .when(online, |card| {
                card.child(div().flex().flex_wrap().gap_2().children(
                    operations.clone().into_iter().map(|(key, count)| {
                        let label = operation_label(&key, language);
                        div()
                            .h_flex()
                            .justify_between()
                            .gap_4()
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().background)
                            .px_3()
                            .py_2()
                            .child(label)
                            .child(count.to_string())
                    }),
                ))
            });

        operations_card
    }
}
