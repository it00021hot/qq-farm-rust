use nsh_base::theme;
use nsh_core::prelude::*;

use crate::features::{self, load_accounts, AccountRow, Workspace};
use crate::i18n::text_key;
use crate::navigation::{Section, SECTIONS};
use crate::ui::choice;

const SECTION_KEY: &str = "qq-farm.section";
const ACCOUNT_KEY: &str = "qq-farm.account";
const SIDEBAR_WIDTH: f32 = 224.0;

pub(crate) fn workspace() -> impl IntoElement {
    let section = signal(Section::from_id(&stored(SECTION_KEY)).unwrap_or(Section::Dashboard));
    let account_id = signal(stored_account());
    let pending = signal(None::<String>);
    let command_failure = signal(None::<String>);
    let rows = load_accounts();
    remember(section, account_id);
    keep_selected_account(account_id, rows);
    let workspace = Workspace { section, account_id, rows, pending, command_failure };
    view().flex_1().flex_row().child(sidebar(workspace)).child(features::content(workspace))
}

fn sidebar(workspace: Workspace) -> impl IntoElement {
    nsh_base::radio_group()
        .w(px(SIDEBAR_WIDTH))
        .gap_1()
        .px_3()
        .py_4()
        .bg(move || theme().sidebar)
        .child(
            text(move || text_key("shell.brand"))
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .px_3()
                .py_2(),
        )
        .child(
            text(move || text_key("nav.group"))
                .text_sm()
                .text_color(move || theme().muted_foreground)
                .px_3()
                .pt_4(),
        )
        .child(each(
            move || SECTIONS.to_vec(),
            |section: &Section| section.id().to_owned(),
            move |section, _| {
                let section = section.get_untracked();
                choice(
                    move || workspace.section.get() == section,
                    move || text_key(section.label_key()),
                )
                .on_change(move |_: &bool| workspace.section.set(section))
            },
        ))
}

fn remember(section: Signal<Section>, account_id: Signal<Option<String>>) {
    effect(move || {
        store(SECTION_KEY, section.get().id());
    });
    effect(move || match account_id.get() {
        Some(id) => store(ACCOUNT_KEY, &id),
        None => {
            nsh_core::storage::Storage::remove_item(ACCOUNT_KEY);
        }
    });
}

fn store(key: &str, value: &str) {
    if nsh_core::storage::Storage::get_item(key).as_deref() == Some(value) {
        return;
    }
    if let Err(error) = nsh_core::storage::Storage::set_item(key, value) {
        log::warn!("preference {key} was not saved: {error}");
    }
}

fn keep_selected_account(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) {
    effect(move || {
        let Some(list) = rows.get() else {
            return;
        };
        let current = account_id.get();
        let valid = current.as_ref().is_some_and(|id| list.iter().any(|row| &row.id == id));
        if valid {
            return;
        }
        let next = list.first().map(|row| row.id.clone());
        if next != current {
            account_id.set(next);
        }
    });
}

fn stored(key: &str) -> String {
    nsh_core::storage::Storage::get_item(key).unwrap_or_default()
}

fn stored_account() -> Option<String> {
    nsh_core::storage::Storage::get_item(ACCOUNT_KEY).filter(|id| !id.is_empty())
}
