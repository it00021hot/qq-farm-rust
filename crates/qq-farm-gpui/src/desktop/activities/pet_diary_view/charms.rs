use super::super::super::*;
use gpui_kit::component::WindowExt;
use serde_json::Value;

use super::common::{picture, section_card, text};

pub(super) fn render(this: &FarmDesktop, snapshot: &Value, cx: &mut Context<FarmDesktop>) -> Div {
    let language = &this.preferences.language;
    let english = language == "en-US";
    let charms = snapshot.get("charms").unwrap_or(&Value::Null);
    let free_remaining =
        charms.get("freeRefreshRemaining").and_then(Value::as_i64).unwrap_or_default();
    let can_refresh = charms.get("canRefresh").and_then(Value::as_bool) == Some(true);
    let can_choose = charms.get("canChoose").and_then(Value::as_bool) == Some(true);
    let pool = charms.get("pool").and_then(Value::as_array).cloned().unwrap_or_default();
    let equipped = charms.get("equipped").and_then(Value::as_array).cloned().unwrap_or_default();
    let refresh_cost = charms
        .pointer("/refreshCost/count")
        .map(|value| super::common::text(Some(value), "30"))
        .unwrap_or_else(|| "30".into());
    let refresh_balance = super::common::text(
        charms.get("refreshBalance"),
        crate::i18n::text(language, "未知", "Unknown"),
    );
    let paid_refresh_title = crate::i18n::text(language, "付费刷新锦囊", "Paid charm refresh");
    let paid_refresh_ok = crate::i18n::text(language, "消耗点券刷新", "Refresh using coupons");
    let cancel = crate::i18n::text(language, "取消", "Cancel");

    let mut panel = section_card(this, "夺宝锦囊", "charm", cx);
    if !this.activities.pet_diary_expanded_sections.contains("charm") {
        return panel;
    }
    panel = panel.child(
        h_flex()
            .items_center()
            .flex_wrap()
            .gap_3()
            .child(
                Button::new("pet-diary-refresh-charm")
                    .label(if english {
                        format!("Refresh charm pool ({free_remaining} free)")
                    } else {
                        format!("刷新锦囊池（免费 {free_remaining} 次）")
                    })
                    .ghost()
                    .loading(this.pet_diary_action_is_pending("refreshCharm"))
                    .disabled(this.loading || !can_refresh)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if free_remaining > 0 {
                            this.run_pet_diary_action(
                                "refreshCharm",
                                serde_json::json!({"payment": "free"}),
                                cx,
                            );
                        } else {
                            let confirm_entity = cx.entity();
                            let cancel_entity = confirm_entity.clone();
                            let description = if english {
                                format!("Free refreshes are used up. This costs {refresh_cost} coupons (balance {refresh_balance}); no diamonds will be used. Continue?")
                            } else {
                                format!(
                                    "免费刷新已用完，本次将消耗 {refresh_cost} 点券（余额 {refresh_balance}），不会使用钻石。是否继续？"
                                )
                            };
                            window.open_alert_dialog(cx, move |alert, _, _| {
                                let description = description.clone();
                                let confirm_entity = confirm_entity.clone();
                                let cancel_entity = cancel_entity.clone();
                                alert
                                    .confirm()
                                    .title(paid_refresh_title)
                                    .description(description)
                                    .ok_text(paid_refresh_ok)
                                    .cancel_text(cancel)
                                    .on_ok(move |_, _, cx| {
                                        confirm_entity.update(cx, |this, cx| {
                                            this.confirm_pet_diary_paid_refresh(cx)
                                        });
                                        true
                                    })
                                    .on_cancel(move |_, _, cx| {
                                        cancel_entity.update(cx, |_, cx| cx.notify());
                                        true
                                    })
                            });
                        }
                    })),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(text(charms.get("refreshNote"), "")),
            ),
    );

    if pool.is_empty() {
        panel =
            panel.child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                crate::i18n::text(language, "暂无待选锦囊", "No charms available to equip"),
            ));
    } else {
        panel = panel.child(h_flex().flex_wrap().gap_3().children(pool.iter().enumerate().map(
            |(index, charm)| {
                let id = text(charm.get("id"), "");
                let name = text(charm.get("name"), crate::i18n::text(language, "锦囊", "Charm"));
                let description = charm
                    .get("shortDescription")
                    .filter(|value| value.as_str().is_some_and(|value| !value.trim().is_empty()))
                    .or_else(|| charm.get("description"));
                let image = charm.get("image").and_then(Value::as_str).unwrap_or_default();
                let mut card = div()
                    .v_flex()
                    .w(px(220.))
                    .gap_2()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .p_3()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(if image.is_empty() {
                                div().size(px(26.)).into_any_element()
                            } else {
                                picture(image, 26., cx)
                            })
                            .child(div().font_medium().child(name)),
                    );
                if description.is_some() {
                    card = card.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(text(description, "")),
                    );
                }
                if can_choose {
                    card = card.child(
                        Button::new(("pet-diary-equip-charm", index))
                            .label(crate::i18n::text(language, "装备", "Equip"))
                            .primary()
                            .loading(this.pet_diary_action_is_pending("equipCharm"))
                            .disabled(this.loading)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_pet_diary_action(
                                    "equipCharm",
                                    serde_json::json!({"charmId": id}),
                                    cx,
                                )
                            })),
                    );
                }
                card
            },
        )));
    }

    if !equipped.is_empty() {
        panel = panel.child(
            h_flex()
                .items_center()
                .flex_wrap()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(language, "已装备：", "Equipped:")),
                )
                .children(equipped.iter().map(|charm| {
                    let name =
                        text(charm.get("name"), crate::i18n::text(language, "锦囊", "Charm"));
                    let remaining = charm
                        .get("remaining")
                        .and_then(Value::as_array)
                        .map(|values| {
                            values
                                .iter()
                                .map(|value| text(Some(value), ""))
                                .collect::<Vec<_>>()
                                .join("/")
                        })
                        .filter(|value| !value.is_empty());
                    div().rounded_full().bg(cx.theme().group_box).px_2().py_1().text_xs().child(
                        if let Some(remaining) = remaining {
                            if english {
                                format!("{name} ({remaining} remaining)")
                            } else {
                                format!("{name}（剩 {remaining} 次）")
                            }
                        } else {
                            name
                        },
                    )
                })),
        );
    }
    panel
}
