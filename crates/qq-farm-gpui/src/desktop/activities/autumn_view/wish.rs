use super::super::super::*;

pub(crate) fn selected_autumn_choice(autumn: &serde_json::Value, selected: &str) -> String {
    let choices = autumn.pointer("/wish/choices").and_then(serde_json::Value::as_array);
    if choices.is_some_and(|choices| {
        choices
            .iter()
            .any(|choice| choice.get("id").map(value_to_action_string).as_deref() == Some(selected))
    }) {
        return selected.to_string();
    }

    choices
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("id"))
        .map(value_to_action_string)
        .unwrap_or_default()
}

pub(super) fn render(
    desktop: &FarmDesktop,
    activity: &serde_json::Value,
    window: &mut Window,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let language = &desktop.preferences.language;
    let day = super::shared::number(activity.get("day")).unwrap_or_default();
    let remaining = super::shared::number(activity.get("remaining")).unwrap_or_default();
    let can_draw = activity.get("canDraw").and_then(serde_json::Value::as_bool) == Some(true);
    let can_claim = activity.get("canClaim").and_then(serde_json::Value::as_bool) == Some(true);
    let pending = activity.get("pending").filter(|pending| pending.is_object());
    let mut panel = div().v_flex().gap_4();

    panel = panel.child(
        h_flex()
            .gap_2()
            .child(super::shared::status_chip(
                crate::i18n::text(language, &format!("第 {day} 天"), &format!("Day {day}")),
                cx,
            ))
            .child(super::shared::status_chip_tinted(
                crate::i18n::text(
                    language,
                    &format!("今日剩余祈愿 {remaining} 次"),
                    &format!("Wishes left today: {remaining}"),
                ),
                cx.theme().primary,
                cx.theme().primary.opacity(0.12),
                cx,
            )),
    );

    let choices = activity
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|choice| {
            let id = choice.get("id").map(value_to_action_string).unwrap_or_default();
            (!id.is_empty()).then(|| {
                let name = super::shared::text(choice, "name").unwrap_or_else(|| id.clone());
                (id, name)
            })
        })
        .collect::<Vec<_>>();
    let choice_ids = choices.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
    let selected_index = choice_ids
        .iter()
        .position(|id| id == &desktop.activities.autumn_choice_id)
        .or_else(|| (!choice_ids.is_empty()).then_some(0));
    let choice_focus_handles = choice_ids
        .iter()
        .map(|id| {
            let radio_id = format!("autumn-wish-choice-{id}");
            window.use_keyed_state(radio_id, cx, |_, cx| cx.focus_handle()).read(cx).clone()
        })
        .collect::<Vec<_>>();
    let choice_count = choices.len();
    let choices_enabled = can_draw && !desktop.loading;
    let desktop_entity = cx.entity();
    let mut choice_row = h_flex().flex_wrap().gap_3();
    for (index, (id, name)) in choices.into_iter().enumerate() {
        let selected = selected_index == Some(index);
        let radio_id = format!("autumn-wish-choice-{id}");
        let action_id = id.clone();
        let navigable_choice_ids = choice_ids.clone();
        let target_focus_handles = choice_focus_handles.clone();
        let radio_disabled = !can_draw || desktop.loading;
        let radio_focus_handle = choice_focus_handles[index].clone();
        let pointer_focus_handle = radio_focus_handle.clone();
        let radio_desktop = desktop_entity.clone();
        let focused = radio_focus_handle.is_focused(window);
        let indicator = div()
            .flex()
            .size_4()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .border_color(if selected { cx.theme().primary } else { cx.theme().border })
            .bg(cx.theme().background)
            .child(if selected {
                div().size_2().rounded_full().bg(cx.theme().primary)
            } else {
                div().size_2()
            });
        choice_row = choice_row.child(
            gpui_kit::base::Radio::new(radio_id)
                .checked(selected)
                .disabled(radio_disabled)
                .tab_stop(selected)
                .tab_index(if selected { 0 } else { -1 })
                .track_focus(&radio_focus_handle)
                .set_position(index + 1, choice_count)
                .accessibility_label(name.clone())
                .on_change(move |checked, _, _, cx| {
                    if checked {
                        radio_desktop.update(cx, |this, cx| {
                            this.activities.autumn_choice_id = action_id.clone();
                            cx.notify();
                        });
                    }
                })
                .on_mouse_down(
                    gpui_kit::MouseButton::Left,
                    cx.listener(move |_, _, window, cx| {
                        if !radio_disabled {
                            window.focus(&pointer_focus_handle, cx);
                        }
                    }),
                )
                .on_key_down(cx.listener(
                    move |this, event: &gpui_kit::KeyDownEvent, window, cx| {
                        if !choices_enabled
                            || event.keystroke.modifiers.modified()
                            || navigable_choice_ids.is_empty()
                        {
                            return;
                        }
                        let current = navigable_choice_ids
                            .iter()
                            .position(|id| id == &this.activities.autumn_choice_id)
                            .unwrap_or(index);
                        let next = match event.keystroke.key.as_str() {
                            "right" | "down" => Some((current + 1) % navigable_choice_ids.len()),
                            "left" | "up" => Some(
                                (current + navigable_choice_ids.len() - 1)
                                    % navigable_choice_ids.len(),
                            ),
                            "home" => Some(0),
                            "end" => Some(navigable_choice_ids.len() - 1),
                            _ => None,
                        };
                        let Some(next) = next else {
                            return;
                        };
                        let Some(next_id) = navigable_choice_ids.get(next).cloned() else {
                            return;
                        };

                        this.activities.autumn_choice_id = next_id;
                        window.focus(&target_focus_handles[next], cx);
                        cx.stop_propagation();
                        cx.notify();
                    },
                ))
                .h_flex()
                .items_center()
                .gap_2()
                .rounded(cx.theme().radius * 0.5)
                .border_1()
                .border_color(if focused { cx.theme().primary } else { cx.theme().background })
                .bg(cx.theme().background)
                .px_2()
                .py_1()
                .text_color(if radio_disabled {
                    cx.theme().muted_foreground
                } else {
                    cx.theme().foreground
                })
                .child(indicator)
                .child(div().text_sm().child(name)),
        );
    }
    let choice_group = gpui_kit::base::RadioGroup::new("autumn-wish-choice-group")
        .axis(gpui_kit::Axis::Horizontal)
        .child(choice_row);
    panel = panel.child(choice_group).child(
        Button::new("autumn-draw")
            .label(if can_draw {
                crate::i18n::text(language, "抽签祈愿", "Draw a wish")
            } else if pending.is_some() {
                crate::i18n::text(language, "先领取待领奖励", "Claim the pending reward first")
            } else {
                crate::i18n::text(language, "今日祈愿已完成", "Today's wishes are complete")
            })
            .primary()
            .loading(desktop.activity_action_is_pending("autumn-draw"))
            .disabled(
                desktop.loading
                    || desktop.selected_account.is_none()
                    || !can_draw
                    || desktop.activities.autumn_choice_id.is_empty(),
            )
            .on_click(cx.listener(|this, _, _, cx| this.run_activity_action("autumn-draw", cx))),
    );

    if let Some(pending) = pending {
        let pending_day = super::shared::number(pending.get("day")).unwrap_or_default();
        let title = super::shared::text(pending, "text")
            .unwrap_or_else(|| crate::i18n::text(language, "祈愿结果", "Wish result").into());
        let mut rewards = h_flex().flex_wrap().gap_3();
        for reward in
            pending.get("rewards").and_then(serde_json::Value::as_array).into_iter().flatten()
        {
            rewards = rewards.child(super::shared::item_card(reward, language, cx));
        }
        let mut pending_card = super::shared::surface(cx)
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().font_medium().child(title))
                    .child(super::shared::status_chip(
                        crate::i18n::text(
                            language,
                            &format!("第 {pending_day} 天"),
                            &format!("Day {pending_day}"),
                        ),
                        cx,
                    )),
            )
            .child(rewards);
        pending_card = pending_card.child(
            Button::new("autumn-claim")
                .label(crate::i18n::text(language, "领取奖励", "Claim reward"))
                .primary()
                .loading(desktop.activity_action_is_pending("autumn-claim"))
                .disabled(desktop.loading || desktop.selected_account.is_none() || !can_claim)
                .on_click(
                    cx.listener(|this, _, _, cx| this.run_activity_action("autumn-claim", cx)),
                ),
        );
        panel = panel.child(pending_card);
    }

    let mut reward_days = h_flex().flex_wrap().gap_2();
    let mut has_reward_days = false;
    for entry in
        activity.get("rewardDays").and_then(serde_json::Value::as_array).into_iter().flatten()
    {
        has_reward_days = true;
        let day = super::shared::number(entry.get("day")).unwrap_or_default();
        let reward = entry.get("reward").unwrap_or(&serde_json::Value::Null);
        reward_days = reward_days.child(
            super::shared::surface(cx)
                .items_center()
                .child(super::shared::status_chip(
                    crate::i18n::text(language, &format!("第 {day} 天"), &format!("Day {day}")),
                    cx,
                ))
                .child(super::shared::item_card(reward, language, cx)),
        );
    }
    if has_reward_days {
        panel = panel
            .child(div().text_sm().font_medium().child(crate::i18n::text(
                language,
                "连续祈愿奖励",
                "Consecutive wish rewards",
            )))
            .child(reward_days);
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::selected_autumn_choice;
    use serde_json::json;

    #[test]
    fn autumn_wish_uses_first_choice_by_default_and_preserves_valid_selection() {
        let autumn = json!({"wish": {"choices": [{"id": 12}, {"id": 34}]}});

        assert_eq!(selected_autumn_choice(&autumn, ""), "12");
        assert_eq!(selected_autumn_choice(&autumn, "34"), "34");
        assert_eq!(selected_autumn_choice(&autumn, "99"), "12");
        assert_eq!(selected_autumn_choice(&json!({"wish": {"choices": []}}), "12"), "");
    }
}
