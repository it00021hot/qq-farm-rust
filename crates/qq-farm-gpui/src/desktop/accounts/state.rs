use super::helpers::account_login_detected_platform;
use super::search_selectors::{self, AccountFilterOption, AccountFilterSelect, AccountFilterValue};
use crate::desktop::FarmDesktop;
use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::component::select::{SearchableVec, SelectEvent};
use gpui_kit::component::IndexPath;
use gpui_kit::{AppContext as _, Context, Entity, Window};
use std::collections::HashSet;
use std::path::PathBuf;

pub(crate) struct AccountsState {
    pub(crate) records: Vec<(String, String, bool)>,
    pub(crate) avatars: std::collections::HashMap<String, String>,
    pub(crate) platform_selector: Entity<AccountFilterSelect>,
    pub(crate) run_status_selector: Entity<AccountFilterSelect>,
    pub(crate) auth_status_selector: Entity<AccountFilterSelect>,
    pub(crate) keyword: Entity<InputState>,
    pub(crate) name: Entity<InputState>,
    pub(crate) code: Entity<InputState>,
    pub(crate) remark: Entity<TextareaState>,
    pub(crate) keyword_applied: String,
    pub(crate) search_open: bool,
    pub(crate) platform_filter: Option<String>,
    pub(crate) running_filter: Option<i64>,
    pub(crate) auth_filter: Option<bool>,
    pub(crate) platform_draft: Option<String>,
    pub(crate) running_draft: Option<i64>,
    pub(crate) auth_draft: Option<bool>,
    pub(crate) page: usize,
    pub(crate) page_size: usize,
    pub(crate) selected: HashSet<String>,
    pub(crate) columns: Vec<super::AccountColumnSetting>,
    pub(crate) column_picker_open: bool,
    pub(crate) lifecycle: crate::desktop::accounts::AccountLifecycleState,
    pub(crate) form_open: bool,
    pub(crate) editing_id: Option<String>,
    pub(crate) platform: String,
    pub(crate) login_tab: String,
    pub(crate) form_error: Option<String>,
    pub(crate) form_saving: bool,
    pub(crate) wx_mode: String,
    pub(crate) wx_status: String,
    pub(crate) wx_error: Option<String>,
    pub(crate) wx_task_id: Option<String>,
    pub(crate) wx_session_id: Option<String>,
    pub(crate) wx_quick_profile: Option<(u16, String, String, String)>,
    pub(crate) wx_qr_path: Option<PathBuf>,
    pub(crate) wx_generation: u64,
    pub(crate) pending_delete: Option<String>,
    pub(crate) pending_batch_delete: bool,
    pub(crate) success_feedback: Option<String>,
    pub(crate) feedback_generation: u64,
}

impl AccountsState {
    pub(in crate::desktop) fn new(
        directory: super::AccountDirectory,
        language: &str,
        window: &mut Window,
        cx: &mut Context<FarmDesktop>,
    ) -> Self {
        let super::AccountDirectory { records, avatars } = directory;
        let platform_options = search_selectors::platform_options(language);
        let platform_selector = cx.new(|cx| {
            AccountFilterSelect::new(
                SearchableVec::new(platform_options),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        cx.subscribe(
            &platform_selector,
            |this, _, event: &SelectEvent<SearchableVec<AccountFilterOption>>, cx| {
                if let SelectEvent::Confirm(Some(AccountFilterValue::Platform(value))) = event {
                    this.accounts.platform_draft = value.clone();
                    cx.notify();
                }
            },
        )
        .detach();

        let run_status_options = search_selectors::run_status_options(language);
        let run_status_selector = cx.new(|cx| {
            AccountFilterSelect::new(
                SearchableVec::new(run_status_options),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        cx.subscribe(
            &run_status_selector,
            |this, _, event: &SelectEvent<SearchableVec<AccountFilterOption>>, cx| {
                if let SelectEvent::Confirm(Some(AccountFilterValue::RunStatus(value))) = event {
                    this.accounts.running_draft = *value;
                    cx.notify();
                }
            },
        )
        .detach();

        let auth_status_options = search_selectors::auth_status_options(language);
        let auth_status_selector = cx.new(|cx| {
            AccountFilterSelect::new(
                SearchableVec::new(auth_status_options),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        cx.subscribe(
            &auth_status_selector,
            |this, _, event: &SelectEvent<SearchableVec<AccountFilterOption>>, cx| {
                if let SelectEvent::Confirm(Some(AccountFilterValue::AuthStatus(value))) = event {
                    this.accounts.auth_draft = *value;
                    cx.notify();
                }
            },
        )
        .detach();

        let state = Self {
            records,
            avatars,
            platform_selector,
            run_status_selector,
            auth_status_selector,
            keyword: cx.new(|cx| {
                InputState::new(window, cx).placeholder(crate::i18n::text(
                    language,
                    "请输入账号名称、QQ 号或备注",
                    "Search account name, QQ number, or notes",
                ))
            }),
            name: cx.new(|cx| {
                InputState::new(window, cx).placeholder(crate::i18n::text(
                    language,
                    "留空默认「账号ID」",
                    "Optional, defaults to account#{id}",
                ))
            }),
            code: cx.new(|cx| {
                InputState::new(window, cx).placeholder(crate::i18n::text(
                    language,
                    "可粘贴裸 Code，或完整 URL（含 platform/os/ver/code）",
                    "Paste bare code or full URL (platform/os/ver/code)",
                ))
            }),
            remark: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .placeholder(crate::i18n::text(language, "备注", "Remark"))
                    .auto_grow(2, 4)
            }),
            keyword_applied: String::new(),
            search_open: false,
            platform_filter: None,
            running_filter: None,
            auth_filter: None,
            platform_draft: None,
            running_draft: None,
            auth_draft: None,
            page: 0,
            page_size: 10,
            selected: HashSet::new(),
            columns: super::default_account_columns().into(),
            column_picker_open: false,
            lifecycle: crate::desktop::accounts::AccountLifecycleState::default(),
            form_open: false,
            editing_id: None,
            platform: "qq".into(),
            login_tab: "code".into(),
            form_error: None,
            form_saving: false,
            wx_mode: "local".into(),
            wx_status: String::new(),
            wx_error: None,
            wx_task_id: None,
            wx_session_id: None,
            wx_quick_profile: None,
            wx_qr_path: None,
            wx_generation: 0,
            pending_delete: None,
            pending_batch_delete: false,
            success_feedback: None,
            feedback_generation: 0,
        };
        cx.observe(&state.keyword, |_, _, cx| cx.notify()).detach();
        cx.observe(&state.code, |_, _, cx| cx.notify()).detach();
        cx.subscribe(
            &state.code,
            |this, input, event: &gpui_kit::component::input::InputEvent, cx| {
                if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                    if let Some(platform) =
                        account_login_detected_platform(input.read(cx).value().as_ref())
                    {
                        this.accounts.platform = platform;
                        cx.notify();
                    }
                }
            },
        )
        .detach();
        state
    }
}
