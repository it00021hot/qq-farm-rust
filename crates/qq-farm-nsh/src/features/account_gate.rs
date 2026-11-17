use nsh_core::prelude::*;

use super::accounts::{account_is_running, AccountRow};
use crate::i18n::text_key;
use crate::ui::{button, notice, ButtonVariant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountGate {
    Missing,
    Loading,
    Failed,
    Stopped,
    Ready,
}

pub(crate) fn account_gate(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> AccountGate {
    let Some(id) = account_id.get() else {
        return AccountGate::Missing;
    };
    if rows.error().is_some() {
        return AccountGate::Failed;
    }
    let Some(rows) = rows.get() else {
        return AccountGate::Loading;
    };
    if account_is_running(&rows, &id) {
        AccountGate::Ready
    } else {
        AccountGate::Stopped
    }
}

pub(crate) fn gate_notice(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
    stopped_key: &'static str,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || account_gate(account_id, rows) == AccountGate::Missing,
            move || notice(move || text_key("dashboard.empty")),
        ))
        .child(when(
            move || account_gate(account_id, rows) == AccountGate::Loading,
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || account_gate(account_id, rows) == AccountGate::Failed,
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        rows.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || rows.refetch()),
                    )
            },
        ))
        .child(when(
            move || account_gate(account_id, rows) == AccountGate::Stopped,
            move || notice(move || text_key(stopped_key)),
        ))
}
