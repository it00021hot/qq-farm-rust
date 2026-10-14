use std::collections::HashSet;

use super::super::super::*;

pub(crate) struct FriendScan {
    pub(crate) row: serde_json::Value,
    pub(crate) at_sec: i64,
}

pub(crate) struct WeatherState {
    pub(crate) search: Entity<InputState>,
    pub(crate) selected_gid: String,
    pub(crate) now_sec: i64,
    pub(crate) loaded_at_sec: i64,
    pub(crate) tick_generation: u64,
    pub(crate) operation_generation: u64,
    pub(crate) scanned_friends: HashMap<String, FriendScan>,
    pub(crate) deferred_gids: Vec<String>,
    pub(crate) scanning_gids: HashSet<String>,
    pub(crate) pending_actions: HashSet<String>,
    pub(crate) notice: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) rules_open: bool,
}

impl WeatherState {
    pub(in crate::desktop) fn new(
        language: &str,
        window: &mut Window,
        cx: &mut Context<FarmDesktop>,
    ) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::i18n::text(
                language,
                "搜索好友名称或 GID",
                "Search friends by name or GID",
            ))
        });
        cx.observe(&search, |_, _, cx| cx.notify()).detach();
        Self {
            search,
            selected_gid: String::new(),
            now_sec: 0,
            loaded_at_sec: 0,
            tick_generation: 0,
            operation_generation: 0,
            scanned_friends: HashMap::new(),
            deferred_gids: Vec::new(),
            scanning_gids: HashSet::new(),
            pending_actions: HashSet::new(),
            notice: None,
            error: None,
            rules_open: false,
        }
    }

    pub(in crate::desktop) fn invalidate_requests(&mut self) {
        self.operation_generation = self.operation_generation.wrapping_add(1);
        self.tick_generation = self.tick_generation.wrapping_add(1);
        self.pending_actions.clear();
        self.scanning_gids.clear();
    }

    pub(in crate::desktop) fn reset(&mut self, window: &mut Window, cx: &mut Context<FarmDesktop>) {
        self.invalidate_requests();
        self.selected_gid.clear();
        self.now_sec = 0;
        self.loaded_at_sec = 0;
        self.scanned_friends.clear();
        self.deferred_gids.clear();
        self.notice = None;
        self.error = None;
        self.rules_open = false;
        self.search.update(cx, |state, cx| state.set_value("", window, cx));
    }

    pub(super) fn merge_friend(&mut self, friend: &serde_json::Value, now_sec: i64) {
        let gid = friend.get("gid").map(value_to_action_string).unwrap_or_default();
        if !gid.is_empty() {
            self.scanned_friends.insert(gid, FriendScan { row: friend.clone(), at_sec: now_sec });
        }
    }
}
