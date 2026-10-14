use gpui_kit::component::searchable_list::SearchableListItem;
use gpui_kit::component::select::{SearchableVec, SelectState};
use gpui_kit::component::IndexPath;
use gpui_kit::{App, SharedString, Window};

use super::super::FarmDesktop;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AccountFilterValue {
    Platform(Option<String>),
    RunStatus(Option<i64>),
    AuthStatus(Option<bool>),
}

#[derive(Clone)]
pub(crate) struct AccountFilterOption {
    value: AccountFilterValue,
    label: String,
}

impl SearchableListItem for AccountFilterOption {
    type Value = AccountFilterValue;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

pub(crate) type AccountFilterSelect = SelectState<SearchableVec<AccountFilterOption>>;

pub(crate) fn platform_options(language: &str) -> Vec<AccountFilterOption> {
    vec![
        option(AccountFilterValue::Platform(None), text(language, "全部", "All")),
        option(AccountFilterValue::Platform(Some("qq".into())), "QQ".into()),
        option(AccountFilterValue::Platform(Some("wx".into())), text(language, "微信", "WeChat")),
    ]
}

pub(crate) fn run_status_options(language: &str) -> Vec<AccountFilterOption> {
    vec![
        option(AccountFilterValue::RunStatus(None), text(language, "全部", "All")),
        option(AccountFilterValue::RunStatus(Some(0)), text(language, "已停止", "Stopped")),
        option(AccountFilterValue::RunStatus(Some(1)), text(language, "运行中", "Running")),
        option(AccountFilterValue::RunStatus(Some(2)), text(language, "异常", "Error")),
    ]
}

pub(crate) fn auth_status_options(language: &str) -> Vec<AccountFilterOption> {
    vec![
        option(AccountFilterValue::AuthStatus(None), text(language, "全部", "All")),
        option(AccountFilterValue::AuthStatus(Some(true)), text(language, "已授权", "Authorized")),
        option(
            AccountFilterValue::AuthStatus(Some(false)),
            text(language, "未授权", "Unauthorized"),
        ),
    ]
}

fn option(value: AccountFilterValue, label: String) -> AccountFilterOption {
    AccountFilterOption { value, label }
}

fn text(language: &str, chinese: &str, english: &str) -> String {
    crate::i18n::text(language, chinese, english).to_owned()
}

fn selected_index(
    options: &[AccountFilterOption],
    value: &AccountFilterValue,
) -> Option<IndexPath> {
    options.iter().position(|option| option.value == *value).map(IndexPath::new)
}

impl FarmDesktop {
    pub(in crate::desktop) fn sync_account_filter_selectors(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) {
        let platform_options = platform_options(&self.preferences.language);
        let platform = AccountFilterValue::Platform(self.accounts.platform_draft.clone());
        self.accounts.platform_selector.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(platform_options.clone()), window, cx);
            select.set_selected_index(selected_index(&platform_options, &platform), window, cx);
        });

        let run_status_options = run_status_options(&self.preferences.language);
        let run_status = AccountFilterValue::RunStatus(self.accounts.running_draft);
        self.accounts.run_status_selector.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(run_status_options.clone()), window, cx);
            select.set_selected_index(selected_index(&run_status_options, &run_status), window, cx);
        });

        let auth_status_options = auth_status_options(&self.preferences.language);
        let auth_status = AccountFilterValue::AuthStatus(self.accounts.auth_draft);
        self.accounts.auth_status_selector.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(auth_status_options.clone()), window, cx);
            select.set_selected_index(
                selected_index(&auth_status_options, &auth_status),
                window,
                cx,
            );
        });
    }
}
