//! 活动中心：当前活动的目录入口（节令日历 / 奶茶活动 / 快乐不独享）。

use super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn render_activities(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.preferences.language.as_str();
        let cards = [
            (
                Page::SolarCalendar,
                "activity-center-solar-calendar",
                "activity-center-open-solar-calendar",
                "节令日历",
                "Solar Calendar",
                "节气奖励、答题与季度奖励",
                "Solar-term rewards, quiz and season rewards",
            ),
            (
                Page::MilkTea,
                "activity-center-milk-tea",
                "activity-center-open-milk-tea",
                "奶茶活动",
                "Milk Tea",
                "店铺、工坊、雇佣、工作与兑换",
                "Shop, workshop, hiring, work and exchange",
            ),
            (
                Page::ShareReward,
                "activity-center-share-reward",
                "activity-center-open-share-reward",
                "快乐不独享",
                "Share & Reward",
                "分享、每日奖励与档位奖励",
                "Sharing, daily and tier rewards",
            ),
        ];
        div().id("activity-center").test_support().v_flex().gap_3().w_full().children(cards.map(
            |(page, card_id, button_id, zh_title, en_title, zh_desc, en_desc)| {
                h_flex()
                    .id(card_id)
                    .test_support()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .p_4()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().group_box)
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .p_2()
                                    .rounded(cx.theme().radius)
                                    .bg(cx.theme().accent)
                                    .child(Icon::new(page.icon())),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_semibold()
                                            .child(crate::i18n::text(language, zh_title, en_title)),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(crate::i18n::text(language, zh_desc, en_desc)),
                                    ),
                            ),
                    )
                    .child(
                        Button::new(button_id)
                            .label(crate::i18n::text(language, "进入", "Open"))
                            .on_click(cx.listener(move |this, _, _, cx| this.set_page(page, cx))),
                    )
            },
        ))
    }
}
