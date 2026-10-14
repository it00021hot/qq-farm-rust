use super::super::super::*;
use super::presentation::{metric_card, reward_label, reward_line_plain, text_at};

fn global_reward_threshold(target: &str, language: &str) -> Option<String> {
    (!target.is_empty()).then(|| {
        if language == "en-US" {
            format!("Unlocks at {target} love")
        } else {
            format!("{target} 爱心解锁")
        }
    })
}

pub(super) fn metrics(
    charity: &serde_json::Value,
    language: &str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let settlement = charity.get("settlement").unwrap_or(&serde_json::Value::Null);
    let global = charity.get("globalProgress").unwrap_or(&serde_json::Value::Null);
    let eligible = settlement.get("eligible").and_then(serde_json::Value::as_bool) == Some(true);
    let donated = text_at(global, "/donated");
    let target = text_at(global, "/target");
    let love = charity.get("love").unwrap_or(&serde_json::Value::Null);
    let love_label = reward_label(love, crate::i18n::text(language, "当前爱心", "Current love"));
    let settlement_reward = settlement.get("reward");

    h_flex()
        .flex_wrap()
        .gap_3()
        .child(metric_card(&love_label, text_at(charity, "/loveBalance"), Some(love), cx))
        .child(metric_card(
            crate::i18n::text(language, "累计捐赠", "Total donated"),
            text_at(charity, "/donatedLove"),
            None,
            cx,
        ))
        .child(
            metric_card(
                crate::i18n::text(language, "结算资格", "Settlement"),
                text_at(settlement, "/requiredLove"),
                settlement_reward,
                cx,
            )
            .child(
                div()
                    .text_xs()
                    .text_color(if eligible {
                        cx.theme().success
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(if eligible {
                        crate::i18n::text(language, "已获得", "Qualified")
                    } else {
                        crate::i18n::text(language, "未达成", "Not yet")
                    }),
            ),
        )
        .child(metric_card(
            crate::i18n::text(language, "全服捐赠", "Server-wide"),
            format!("{donated} / {target}"),
            None,
            cx,
        ))
}

pub(super) fn global_progress(
    charity: &serde_json::Value,
    language: &str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let global = charity.get("globalProgress").unwrap_or(&serde_json::Value::Null);
    let donated = text_at(global, "/donated");
    let target = text_at(global, "/target");
    let ratio = target
        .parse::<f64>()
        .ok()
        .filter(|target| *target > 0.)
        .map(|target| (donated.parse::<f64>().unwrap_or_default() / target).clamp(0., 1.) as f32)
        .unwrap_or_default();
    let global_reward = global.get("reward").unwrap_or(&serde_json::Value::Null);
    let reward_target = global.get("rewardTarget").map(value_to_action_string).unwrap_or_default();
    let reward_threshold = global_reward_threshold(&reward_target, language);

    div()
        .v_flex()
        .gap_3()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .p_4()
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(div().font_medium().child(crate::i18n::text(
                    language,
                    "全服共同目标",
                    "Community goal",
                )))
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(reward_line_plain(global_reward, "", cx))
                        .when_some(reward_threshold, |row, threshold| {
                            row.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(threshold),
                            )
                        }),
                ),
        )
        .child(
            div()
                .h(px(8.))
                .w_full()
                .rounded_full()
                .bg(cx.theme().border)
                .child(div().h_full().w(relative(ratio)).rounded_full().bg(cx.theme().primary)),
        )
        .child(
            h_flex()
                .justify_between()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .justify_end()
                .child(format!("{:.1}%", ratio * 100.)),
        )
}

#[cfg(test)]
mod tests {
    #[test]
    fn global_reward_threshold_uses_the_vue_unlock_wording() {
        assert_eq!(super::global_reward_threshold("150", "zh-CN"), Some("150 爱心解锁".to_owned()));
        assert_eq!(
            super::global_reward_threshold("150", "en-US"),
            Some("Unlocks at 150 love".to_owned())
        );
        assert_eq!(super::global_reward_threshold("", "en-US"), None);
    }
}
