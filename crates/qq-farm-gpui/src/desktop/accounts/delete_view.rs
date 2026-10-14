use super::super::*;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::popover::Popover;

impl FarmDesktop {
    pub(in crate::desktop) fn render_account_batch_delete_confirmation(
        &self,
        trigger: Button,
        cx: &mut Context<Self>,
    ) -> Popover {
        let language = self.preferences.language.clone();
        let state_entity = cx.entity();
        let desktop = state_entity.clone();
        Popover::new("account-batch-delete-confirmation")
            .anchor(gpui_kit::Anchor::BottomRight)
            .trigger(trigger)
            .on_open_change(move |open, _, cx| {
                state_entity.update(cx, |this, cx| {
                    this.accounts.pending_batch_delete = *open;
                    cx.notify();
                });
            })
            .content(move |_, _, cx| {
                let popover = cx.entity();
                let cancel_entity = desktop.clone();
                let confirm_entity = desktop.clone();
                let cancel_popover = popover.clone();
                let confirm_popover = popover;
                let cancel = crate::i18n::text(&language, "取消", "Cancel");
                let confirm = crate::i18n::text(&language, "删除", "Delete");
                let message = crate::i18n::text(
                    &language,
                    "确认删除吗？",
                    "Are you sure you want to delete?",
                );
                v_flex()
                    .id("account-batch-delete-popover")
                    .w(px(280.))
                    .gap_3()
                    .child(div().id("account-batch-delete-message").text_sm().child(message))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("account-batch-delete-cancel")
                                    .label(cancel)
                                    .ghost()
                                    .on_click(move |_, window, cx| {
                                        cancel_popover.update(cx, |state, cx| {
                                            state.dismiss(window, cx);
                                        });
                                        cancel_entity.update(cx, |this, cx| {
                                            this.accounts.pending_batch_delete = false;
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child(
                                Button::new("account-batch-delete-confirm")
                                    .label(confirm)
                                    .primary()
                                    .on_click(move |_, window, cx| {
                                        confirm_entity.update(cx, |this, cx| {
                                            this.confirm_account_batch_delete(cx);
                                        });
                                        confirm_popover.update(cx, |state, cx| {
                                            state.dismiss(window, cx);
                                        });
                                    }),
                            ),
                    )
            })
    }

    pub(in crate::desktop) fn render_account_delete_confirmation(
        &self,
        id: String,
        row_index: usize,
        trigger: Button,
        cx: &mut Context<Self>,
    ) -> Popover {
        let language = self.preferences.language.clone();
        let confirm = crate::i18n::text(&language, "确认", "Confirm");
        let cancel = crate::i18n::text(&language, "取消", "Cancel");
        let state_entity = cx.entity();
        let desktop = state_entity.clone();
        Popover::new(("account-delete-confirmation", row_index))
            .anchor(gpui_kit::Anchor::BottomRight)
            .trigger(trigger)
            .on_open_change(move |open, _, cx| {
                state_entity.update(cx, |this, cx| {
                    this.accounts.pending_delete = if *open { Some(id.clone()) } else { None };
                    cx.notify();
                });
            })
            .content(move |_, _, cx| {
                let popover = cx.entity();
                let cancel_entity = desktop.clone();
                let confirm_entity = desktop.clone();
                let cancel_popover = popover.clone();
                let confirm_popover = popover;
                let description =
                    crate::i18n::text(&language, "确认删除该账号吗？", "Delete this account?");
                let description =
                    div().id("account-delete-confirm-message").text_sm().child(description);
                #[cfg(test)]
                let description = description.test_support();
                v_flex()
                    .id(("account-delete-confirm-popover", row_index))
                    .w(px(260.))
                    .gap_3()
                    .child(description)
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new(("account-delete-cancel", row_index))
                                    .label(cancel)
                                    .ghost()
                                    .on_click(move |_, window, cx| {
                                        cancel_popover.update(cx, |state, cx| {
                                            state.dismiss(window, cx);
                                        });
                                        cancel_entity.update(cx, |this, cx| {
                                            this.cancel_account_delete_confirmation(cx);
                                        });
                                    }),
                            )
                            .child(
                                Button::new(("account-delete-confirm", row_index))
                                    .label(confirm)
                                    .primary()
                                    .on_click(move |_, window, cx| {
                                        confirm_entity.update(cx, |this, cx| {
                                            this.confirm_delete_account(cx);
                                        });
                                        confirm_popover.update(cx, |state, cx| {
                                            state.dismiss(window, cx);
                                        });
                                    }),
                            ),
                    )
            })
    }

    pub(in crate::desktop) fn cancel_account_delete_confirmation(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.accounts.pending_delete = None;
        cx.notify();
    }
}
