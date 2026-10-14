use super::super::super::*;

pub(super) fn card(
    this: &FarmDesktop,
    charity: &serde_json::Value,
    can_donate: bool,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let balance = charity
        .get("loveBalance")
        .map(value_to_action_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "0".into());
    let confirming = this.activities.charity_donation_pending;
    let pending = this.activity_action_is_pending("charity-donate");
    let language = &this.preferences.language;

    div()
        .v_flex()
        .flex_1()
        .min_w(px(220.))
        .gap_3()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .p_4()
        .child(div().font_medium().child(crate::i18n::text(language, "爱心捐赠", "Donate love")))
        .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
            if language == "en-US" {
                format!("Donates all {balance} love at once and clears the balance")
            } else {
                format!("本次将捐赠全部 {balance} 份爱心（会一次性清空当前余额）")
            },
        ))
        .when(confirming, |card| {
            card.child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        Button::new("charity-donate-cancel")
                            .label(crate::i18n::text(language, "取消", "Cancel"))
                            .ghost()
                            .flex_1()
                            .min_w_0()
                            .disabled(pending)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.activities.charity_donation_pending = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("charity-donate-confirm")
                            .label(crate::i18n::text(language, "确认捐赠", "Confirm donation"))
                            .danger()
                            .flex_1()
                            .min_w_0()
                            .loading(pending)
                            .disabled(this.loading || !can_donate)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.activities.charity_donation_pending = false;
                                this.run_activity_action("charity-donate", cx);
                            })),
                    ),
            )
        })
        .child(
            Button::new("charity-donate")
                .label(if confirming {
                    crate::i18n::text(language, "确认捐赠", "Confirm donation")
                } else {
                    crate::i18n::text(language, "捐赠全部爱心", "Donate all love")
                })
                .when(confirming, |button| button.danger())
                .when(!confirming, |button| button.primary())
                .loading(pending)
                .disabled(this.loading || !can_donate)
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.activities.charity_donation_pending {
                        this.activities.charity_donation_pending = false;
                        this.run_activity_action("charity-donate", cx);
                    } else {
                        this.activities.charity_donation_pending = true;
                        cx.notify();
                    }
                })),
        )
}
