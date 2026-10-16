use super::super::super::*;
use super::super::auth_status::AccountAuthStatus;
use super::super::{
    account_last_online_timestamp, account_value, format_account_timestamp, AccountColumnId,
};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::StatefulInteractiveElement;

impl FarmDesktop {
    pub(super) fn render_account_column_cell(
        &self,
        column: AccountColumnId,
        row_index: usize,
        record: &serde_json::Value,
        stretch: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let id = account_value(record, &["id"]);
        match column {
            AccountColumnId::Index => super::account_cell_frame(stretch, column.width())
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child((row_index + 1).to_string()),
            AccountColumnId::Name => super::account_cell_frame(stretch, column.width())
                .text_sm()
                .truncate()
                .font_medium()
                .child(account_value(record, &["name"])),
            AccountColumnId::Platform => {
                let platform = account_value(record, &["platform"]);
                let platform_name = match platform.as_str() {
                    "wx" if self.preferences.language == "en-US" => "WeChat",
                    "wx" => "微信",
                    "qq" => "QQ",
                    _ => platform.as_str(),
                };
                super::account_cell_frame(stretch, column.width())
                    .text_sm()
                    .child(platform_name.to_string())
            }
            AccountColumnId::RunStatus => {
                let running =
                    record.get("running").and_then(serde_json::Value::as_bool).unwrap_or(false);
                let label = if self.preferences.language == "en-US" {
                    if running {
                        "Running"
                    } else {
                        "Stopped"
                    }
                } else if running {
                    "运行中"
                } else {
                    "已停止"
                };
                let color = if running { cx.theme().green } else { cx.theme().muted_foreground };
                super::account_cell_frame(stretch, column.width()).child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(cx.theme().group_box)
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(color)
                        .child(label),
                )
            }
            AccountColumnId::AuthStatus => {
                let status = AccountAuthStatus::from_record(record);
                let color = match status {
                    AccountAuthStatus::Authorized => cx.theme().green,
                    AccountAuthStatus::Unauthorized | AccountAuthStatus::RescanRecommended => {
                        cx.theme().warning
                    }
                };
                let label = status.label(&self.preferences.language);
                let tag = div()
                    .id(format!("account-auth-status-{id}"))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(cx.theme().group_box)
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(color)
                    .child(label);
                let tooltip = status.tooltip(&self.preferences.language).map(str::to_owned);
                super::account_cell_frame(stretch, column.width()).child(tag.when_some(
                    tooltip,
                    |tag, tooltip| {
                        tag.tooltip(move |window, cx| {
                            Tooltip::new(tooltip.clone()).build(window, cx)
                        })
                    },
                ))
            }
            AccountColumnId::LastOnline => {
                let label = format_account_timestamp(account_last_online_timestamp(record));
                let tooltip = label.clone();
                super::account_cell_frame(stretch, column.width()).flex().items_center().child(
                    div()
                        .id(format!("account-last-online-{id}"))
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_center()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(label)
                        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx)),
                )
            }
            AccountColumnId::Actions => {
                let running =
                    record.get("running").and_then(serde_json::Value::as_bool).unwrap_or(false);
                let lifecycle_pending = self.accounts.lifecycle.is_pending(&id);
                let record_for_edit = record.clone();
                let id_for_lifecycle = id.clone();
                let delete_trigger = Button::new(format!("account-delete-{id}"))
                    .label(crate::i18n::text(&self.preferences.language, "删除", "Delete"))
                    .danger()
                    .outline()
                    .disabled(self.loading);
                super::account_cell_frame(stretch, column.width())
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .child(
                        Button::new(format!("account-edit-{id}"))
                            .label(crate::i18n::text(&self.preferences.language, "编辑", "Edit"))
                            .primary()
                            .outline()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_account_form(Some(record_for_edit.clone()), window, cx)
                            })),
                    )
                    .child(
                        Button::new(format!("account-toggle-{id}"))
                            .label(if self.preferences.language == "en-US" {
                                if running {
                                    "Stop"
                                } else {
                                    "Start"
                                }
                            } else if running {
                                "停止"
                            } else {
                                "启动"
                            })
                            .when(running, |button| button.warning())
                            .when(!running, |button| button.success())
                            .outline()
                            .loading(lifecycle_pending)
                            .disabled(lifecycle_pending || self.loading)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_account_lifecycle(id_for_lifecycle.clone(), !running, cx)
                            })),
                    )
                    .child(self.render_account_delete_confirmation(
                        id,
                        row_index,
                        delete_trigger,
                        cx,
                    ))
            }
        }
    }
}
