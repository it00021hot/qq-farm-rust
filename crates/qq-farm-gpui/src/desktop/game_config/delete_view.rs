use super::super::*;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::popover::Popover;

impl FarmDesktop {
    pub(in crate::desktop) fn render_game_config_batch_delete_confirmation(
        &self,
        trigger: Button,
        cx: &mut Context<Self>,
    ) -> Popover {
        let language = self.preferences.language.clone();
        let change_entity = cx.entity();
        let desktop = change_entity.clone();

        Popover::new("game-config-batch-delete-confirmation")
            .anchor(gpui_kit::Anchor::BottomRight)
            .trigger(trigger)
            .on_open_change(move |open, _, cx| {
                change_entity.update(cx, |this, cx| {
                    this.game_config.pending_batch_delete = *open;
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
                    .id("game-config-batch-delete-popover")
                    .w(px(280.))
                    .gap_3()
                    .child(div().id("game-config-batch-delete-message").text_sm().child(message))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("game-config-batch-delete-cancel")
                                    .label(cancel)
                                    .ghost()
                                    .on_click(move |_, window, cx| {
                                        cancel_popover.update(cx, |state, cx| {
                                            state.dismiss(window, cx);
                                        });
                                        cancel_entity.update(cx, |this, cx| {
                                            this.game_config.pending_batch_delete = false;
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child(
                                Button::new("game-config-batch-delete-confirm")
                                    .label(confirm)
                                    .primary()
                                    .on_click(move |_, window, cx| {
                                        confirm_entity.update(cx, |this, cx| {
                                            this.confirm_selected_game_config_deletion(cx);
                                        });
                                        confirm_popover.update(cx, |state, cx| {
                                            state.dismiss(window, cx);
                                        });
                                    }),
                            ),
                    )
            })
    }

    pub(super) fn render_game_config_delete_confirmation(
        &self,
        kind: &str,
        id: i64,
        index: usize,
        trigger: Button,
        cx: &mut Context<Self>,
    ) -> Popover {
        let kind = kind.to_string();
        let language = self.preferences.language.clone();
        let change_entity = cx.entity();
        let desktop = change_entity.clone();

        Popover::new(("game-config-delete-confirmation", index))
            .anchor(gpui_kit::Anchor::BottomRight)
            .trigger(trigger)
            .on_open_change(move |open, _, cx| {
                change_entity.update(cx, |this, cx| {
                    this.game_config.pending_delete =
                        if *open { Some((kind.clone(), id)) } else { None };
                    cx.notify();
                });
            })
            .content(move |_, _, cx| {
                let popover = cx.entity();
                let confirm_entity = desktop.clone();
                let cancel_entity = desktop.clone();
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
                    .id(("config-delete-confirm-popover", index))
                    .w(px(260.))
                    .gap_3()
                    .child(div().id("config-delete-confirm-message").text_sm().child(message))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new(("config-delete-cancel", index))
                                    .label(cancel)
                                    .ghost()
                                    .on_click(move |_, window, cx| {
                                        cancel_popover.update(cx, |state, cx| {
                                            state.dismiss(window, cx);
                                        });
                                        cancel_entity.update(cx, |this, cx| {
                                            this.cancel_game_config_delete_confirmation(cx);
                                        });
                                    }),
                            )
                            .child(
                                Button::new(("config-delete-confirm", index))
                                    .label(confirm)
                                    .primary()
                                    .on_click(move |_, window, cx| {
                                        confirm_entity.update(cx, |this, cx| {
                                            this.delete_game_config(cx);
                                        });
                                        confirm_popover.update(cx, |state, cx| {
                                            state.dismiss(window, cx);
                                        });
                                    }),
                            ),
                    )
            })
    }

    pub(in crate::desktop) fn cancel_game_config_delete_confirmation(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.game_config.pending_delete = None;
        cx.notify();
    }
}
