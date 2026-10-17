use super::*;
use gpui_kit::assets::IconName;

impl FarmDesktop {
    pub(super) fn render_content_toolbar(&self, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        h_flex()
            .justify_between()
            .items_center()
            .px_6()
            .py_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("toggle-sidebar")
                            .icon(IconName::PanelLeft)
                            .accessibility_label(crate::i18n::text(language, "侧边栏", "Sidebar"))
                            .ghost()
                            .on_click(cx.listener(|this, _, window, cx| {
                                // Dispatch the command directly: re-dispatching the action through
                                // `focus_handle` from inside a click listener would synchronously
                                // re-enter this entity's lease and panic.
                                this.dispatch_ui_command(&ToggleSidebar, window, cx)
                            })),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_semibold()
                            .child(self.page.title_for(&self.preferences.language)),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("refresh")
                            .icon(IconName::RefreshCw)
                            .label(crate::i18n::text(language, "刷新", "Refresh"))
                            .ghost()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.dispatch_ui_command(&RefreshPage, window, cx)
                            })),
                    )
                    .when(self.page == Page::Accounts, |row| {
                        let selected_ids = self.selected_visible_account_ids();
                        let delete_trigger = Button::new("account-batch-delete")
                            .label(crate::i18n::text(language, "批量删除", "Batch Delete"))
                            .danger()
                            .outline()
                            .disabled(selected_ids.is_empty() || self.loading);
                        row.child(
                            Button::new("account-add-toolbar")
                                .label(crate::i18n::text(language, "新增账号", "Add Account"))
                                .primary()
                                .outline()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_account_form(None, window, cx)
                                })),
                        )
                        .child(self.render_account_batch_delete_confirmation(delete_trigger, cx))
                    })
                    .when(self.page == Page::GameConfig, |row| {
                        let selected = self
                            .game_config
                            .selected_ids
                            .iter()
                            .filter(|(section, _)| section == &self.page_section)
                            .count();
                        let delete_trigger = Button::new("game-config-batch-delete")
                            .label(crate::i18n::text(language, "批量删除", "Batch Delete"))
                            .danger()
                            .outline()
                            .disabled(selected == 0 || self.loading);
                        row.child(
                            Button::new("game-config-add")
                                .label(crate::i18n::text(language, "新增", "Add"))
                                .primary()
                                .outline()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_game_config_form(None, window, cx);
                                })),
                        )
                        .child(
                            self.render_game_config_batch_delete_confirmation(delete_trigger, cx),
                        )
                        .when(selected > 0, |row| {
                            row.child(
                                Button::new("game-config-clear-selection")
                                    .label(crate::i18n::text(
                                        language,
                                        "清除选择",
                                        "Clear selection",
                                    ))
                                    .ghost()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.game_config
                                            .selected_ids
                                            .retain(|(section, _)| section != &this.page_section);
                                        cx.notify();
                                    })),
                            )
                        })
                    }),
            )
    }
}
