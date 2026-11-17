use nsh_base::{theme, MenuEvent};
use nsh_core::prelude::*;

use crate::features::{
    self, account_label, load_accounts, platform_label, selected_account, AccountRow, Workspace,
};
use crate::i18n::text_key;
use crate::navigation::{Section, SECTIONS};
use crate::ui::{button, choice, ButtonVariant};

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
    view()
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
        .child(account_menu(workspace))
        .child(
            text(move || text_key("nav.group"))
                .text_sm()
                .text_color(move || theme().muted_foreground)
                .px_3()
                .pt_4(),
        )
        .child(nsh_base::radio_group().gap_1().child(each(
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
        )))
}

fn account_menu(workspace: Workspace) -> impl IntoElement {
    let open = signal(false);
    let reference = element_ref();
    nsh_base::menu()
        .open(move || Some(open.get()))
        .placement(Placement::Bottom)
        .align(Align::Start)
        .offset(px(4.0))
        .trigger(
            view().child(
                button(move || switcher_label(workspace), ButtonVariant::Secondary)
                    .w(px(200.0))
                    .justify_start()
                    .element_ref(&reference)
                    .on_press(move || open.update(|value| *value = !*value)),
            ),
        )
        .on_open_change(move |opened: &MenuEvent| {
            if let MenuEvent::OpenChange(value, _) = opened {
                open.set(*value);
            }
        })
        .on_close(move || reference.focus())
        .popup_style(|style| {
            style
                .min_w(px(200.0))
                .p_1()
                .rounded(move || theme().radius)
                .border_1()
                .border_color(move || theme().border)
                .bg(move || theme().popover)
                .text_color(move || theme().popover_foreground)
                .shadow_md()
        })
        .popup_starting_style(|style| style.opacity(0.0))
        .popup_ending_style(|style| style.opacity(0.0))
        .child(dynamic(move || account_items(workspace, open)))
}

fn switcher_label(workspace: Workspace) -> SharedString {
    let Some(id) = workspace.account_id.get() else {
        return text_key("shell.noAccount");
    };
    workspace
        .rows
        .get()
        .as_ref()
        .and_then(|rows| selected_account(rows, Some(&id)))
        .map(|row| SharedString::from(account_label(&row)))
        .unwrap_or_else(|| text_key("shell.noAccount"))
}

fn account_items(workspace: Workspace, open: Signal<bool>) -> View {
    let rows = workspace.rows.get().unwrap_or_default();
    view()
        .gap_1()
        .children(rows.into_iter().map(move |row| account_item(workspace, open, row)))
        .child(manage_item(workspace, open))
}

fn account_item(workspace: Workspace, open: Signal<bool>, row: AccountRow) -> impl IntoElement {
    let id = row.id.clone();
    let checked_id = id.clone();
    let title = SharedString::from(account_label(&row));
    let label = title.clone();
    let platform = row.platform.clone();
    nsh_base::menu_item()
        .accessibility_label(move || Some(label.clone()))
        .role(Some(nsh_core::AccessibilityRole::MenuItemRadio))
        .checked(move || Some(workspace.account_id.get().as_deref() == Some(checked_id.as_str())))
        .min_h(px(32.0))
        .px_2()
        .py_1()
        .rounded(px(4.0))
        .flex_row()
        .items_center()
        .gap_2()
        .child(text(title).text_sm().flex_1().truncate())
        .child(when(
            {
                let platform = platform.clone();
                move || !platform.is_empty()
            },
            {
                let platform = platform.clone();
                move || {
                    text(platform_label(&platform))
                        .text_sm()
                        .text_color(move || theme().muted_foreground)
                }
            },
        ))
        .on_select(move || {
            workspace.account_id.set(Some(id.clone()));
            open.set(false);
        })
}

fn manage_item(workspace: Workspace, open: Signal<bool>) -> impl IntoElement {
    nsh_base::menu_item()
        .accessibility_label(move || Some(text_key("shell.manage")))
        .min_h(px(32.0))
        .px_2()
        .py_1()
        .rounded(px(4.0))
        .flex_row()
        .items_center()
        .child(text(move || text_key("shell.manage")).text_sm())
        .on_select(move || {
            workspace.section.set(Section::Accounts);
            open.set(false);
        })
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
