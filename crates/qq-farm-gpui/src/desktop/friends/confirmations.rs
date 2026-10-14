use super::super::*;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::popover::Popover;

impl FarmDesktop {
    pub(in crate::desktop) fn render_friend_confirmation(
        &self,
        gid: i64,
        action: &'static str,
        name: String,
        index: usize,
        trigger: Button,
        cx: &mut Context<Self>,
    ) -> Popover {
        let language = self.preferences.language.clone();
        let english = language == "en-US";
        let message = match action {
            "bad" => crate::i18n::text(&language, "确定执行此操作吗？", "Confirm this operation?")
                .to_string(),
            "blacklist" if english => format!("Add \"{name}\" to blacklist?"),
            "blacklist" => format!("确定将「{name}」加入黑名单吗？"),
            "unblacklist" if english => format!("Remove \"{name}\" from blacklist?"),
            "unblacklist" => format!("确定将「{name}」移出黑名单吗？"),
            "delete" => crate::i18n::text(
                &language,
                "删除后将加入黑名单，不再自动互动",
                "The friend will be blacklisted and no longer auto-interacted",
            )
            .to_string(),
            _ => crate::i18n::text(&language, "确认操作吗？", "Confirm this action?").to_string(),
        };
        let confirm_label = crate::i18n::text(&language, "确认", "Confirm");
        let cancel_label = crate::i18n::text(&language, "取消", "Cancel");
        let desktop = cx.entity();

        // Popover open state is keyed by this id, so every action on a row
        // needs its own; a shared id would toggle all three confirmations at
        // once and stack their surfaces.
        Popover::new(format!("friend-confirmation-{index}-{action}"))
            .anchor(gpui_kit::Anchor::BottomRight)
            .trigger(trigger)
            .content(move |_, _, cx| {
                let popover = cx.entity();
                let confirm_entity = desktop.clone();
                let cancel_popover = popover.clone();
                let confirm_popover = popover;
                let message = div().id("friend-confirm-message").text_sm().child(message.clone());
                #[cfg(test)]
                let message = message.test_support();
                let content = v_flex()
                    .id(format!("friend-confirm-content-{index}-{action}"))
                    .w(px(280.))
                    .gap_3()
                    .child(message)
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new(("friend-confirm-cancel", index))
                                    .label(cancel_label)
                                    .ghost()
                                    .on_click(move |_, window, cx| {
                                        cancel_popover.update(cx, |state, cx| {
                                            state.dismiss(window, cx);
                                        });
                                    }),
                            )
                            .child(
                                Button::new(("friend-confirm-confirm", index))
                                    .label(confirm_label)
                                    .primary()
                                    .on_click(move |_, window, cx| {
                                        confirm_entity.update(cx, |this, cx| {
                                            this.confirm_friend_action(gid, action, cx);
                                        });
                                        confirm_popover.update(cx, |state, cx| {
                                            state.dismiss(window, cx);
                                        });
                                    }),
                            ),
                    );
                #[cfg(test)]
                let content = content.test_support();
                content
            })
    }
}
