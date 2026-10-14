use crate::desktop::FarmDesktop;
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext as _, Context, Entity, Window};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FriendFeedbackLevel {
    Success,
    Info,
    Error,
}

pub(crate) struct FriendsState {
    pub(crate) interaction_gid: Entity<InputState>,
    pub(crate) interaction_land_ids: Entity<InputState>,
    pub(crate) interaction_picker:
        Entity<crate::desktop::personal::interaction_items::InteractionItemSelect>,
    pub(crate) selected_interaction_item_id: Option<i64>,
    pub(crate) interaction_using: bool,
    pub(crate) interaction_generation: u64,
    pub(crate) interaction_items_refresh_loading: bool,
    pub(crate) interaction_items_refresh_generation: u64,
    pub(crate) records_refresh_loading: bool,
    pub(crate) records_refresh_generation: u64,
    pub(crate) detail: Option<(i64, serde_json::Value)>,
    pub(crate) detail_loading_gid: Option<i64>,
    pub(crate) detail_error: Option<(i64, String)>,
    pub(crate) detail_loaded_at: i64,
    pub(crate) countdown_now_sec: i64,
    pub(crate) countdown_generation: u64,
    pub(crate) record_filter: Option<i64>,
    pub(crate) page: usize,
    pub(crate) delete_loading: HashMap<i64, u64>,
    pub(crate) delete_generation: u64,
    pub(crate) operation_loading: HashMap<(i64, &'static str), u64>,
    pub(crate) operation_generation: u64,
    pub(crate) realtime_refresh_generation: u64,
    pub(crate) steal_all_loading: bool,
    pub(crate) steal_all_generation: u64,
    pub(crate) steal_all_current_gid: Option<i64>,
    pub(crate) blacklist_loading: bool,
    pub(crate) blacklist_generation: u64,
    pub(crate) feedback: Option<(String, FriendFeedbackLevel)>,
    pub(crate) feedback_generation: u64,
}

impl FriendsState {
    pub(in crate::desktop) fn new(
        language: &str,
        window: &mut Window,
        cx: &mut Context<FarmDesktop>,
    ) -> Self {
        let interaction_picker = cx.new(|cx| {
            crate::desktop::personal::interaction_items::InteractionItemSelect::new(
                gpui_kit::component::select::SearchableVec::new(Vec::new()),
                None,
                window,
                cx,
            )
        });
        cx.subscribe(&interaction_picker, |this, _, event, cx| {
            let gpui_kit::component::select::SelectEvent::Confirm(selected) = event;
            if let Some(item_id) = selected {
                this.select_friend_interaction_item(*item_id, cx);
            }
        })
        .detach();
        Self {
            interaction_gid: cx.new(|cx| {
                InputState::new(window, cx).placeholder(crate::i18n::text(
                    language,
                    "好友 GID",
                    "Friend GID",
                ))
            }),
            interaction_land_ids: cx.new(|cx| {
                InputState::new(window, cx).placeholder(crate::i18n::text(
                    language,
                    "例如： 1,2,3",
                    "e.g. 1,2,3",
                ))
            }),
            interaction_picker,
            selected_interaction_item_id: None,
            interaction_using: false,
            interaction_generation: 0,
            interaction_items_refresh_loading: false,
            interaction_items_refresh_generation: 0,
            records_refresh_loading: false,
            records_refresh_generation: 0,
            detail: None,
            detail_loading_gid: None,
            detail_error: None,
            detail_loaded_at: 0,
            countdown_now_sec: 0,
            countdown_generation: 0,
            record_filter: None,
            page: 0,
            delete_loading: HashMap::new(),
            delete_generation: 0,
            operation_loading: HashMap::new(),
            operation_generation: 0,
            realtime_refresh_generation: 0,
            steal_all_loading: false,
            steal_all_generation: 0,
            steal_all_current_gid: None,
            blacklist_loading: false,
            blacklist_generation: 0,
            feedback: None,
            feedback_generation: 0,
        }
    }
}
