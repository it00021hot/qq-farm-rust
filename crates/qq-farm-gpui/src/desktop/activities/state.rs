use crate::desktop::FarmDesktop;
use gpui_kit::component::combobox::{ComboboxEvent, ComboboxState};
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext as _, Context, Entity, Window};
use std::collections::{HashMap, HashSet};

pub(crate) struct ActivitiesState {
    pub(crate) pet_diary_friend_gid: String,
    pub(crate) pet_diary_friend_selector:
        Entity<super::pet_diary_view::friend_picker::PetDiaryFriendSelector>,
    pub(crate) pet_diary_friend_loading: bool,
    pub(crate) pet_diary_friend_error: Option<String>,
    pub(crate) pet_diary_friend_request_generation: u64,
    pub(crate) pet_diary_friends_loading: bool,
    pub(crate) pet_diary_friends_request_generation: u64,
    pub(crate) pet_diary_exchange_count: Entity<InputState>,
    pub(crate) weather: super::weather::WeatherState,
    pub(crate) qixi_selected_friend_gid: String,
    pub(crate) qixi_friend_search: Entity<InputState>,
    pub(crate) qixi_gift_count: Entity<InputState>,
    pub(crate) qixi_friends_loading: bool,
    pub(crate) qixi_friends_request_generation: u64,
    pub(crate) qixi_rules_expanded: bool,
    pub(crate) star_sand_category: String,
    pub(crate) charity_donation_pending: bool,
    pub(crate) charity_rules_expanded: bool,
    pub(crate) constellation_group_id: String,
    pub(crate) solar_term_id: String,
    pub(crate) stellar_section: String,
    pub(crate) activity_tick_generation: u64,
    pub(crate) pet_diary_goods_id: Entity<InputState>,
    pub(crate) qingmei_selected_ingredients: HashMap<String, i64>,
    pub(crate) autumn_mode: String,
    pub(crate) autumn_choice_id: String,
    pub(crate) autumn_log_tab: i64,
    pub(crate) autumn_logs_load_pending: bool,
    pub(crate) pending_actions: HashMap<String, u64>,
    pub(crate) action_sequence: u64,
    pub(crate) action_notice: Option<(String, bool)>,
    pub(crate) action_notice_generation: u64,
    pub(crate) view_revision: u64,
    pub(crate) pet_diary_battle_target: Option<(String, String, String)>,
    pub(crate) pet_diary_log_kind: String,
    pub(crate) pet_diary_records: HashMap<String, serde_json::Value>,
    pub(crate) pet_diary_notice: Option<String>,
    pub(crate) pet_diary_warning: Option<String>,
    pub(crate) pet_diary_expanded_sections: HashSet<String>,
}

impl ActivitiesState {
    pub(in crate::desktop) fn new(
        language: &str,
        window: &mut Window,
        cx: &mut Context<FarmDesktop>,
    ) -> Self {
        let pet_diary_friend_selector = cx.new(|cx| {
            ComboboxState::new(
                super::pet_diary_view::friend_picker::PetDiaryFriendDelegate::empty(language),
                Vec::new(),
                window,
                cx,
            )
            .searchable(true)
        });
        cx.subscribe(&pet_diary_friend_selector, |this, _, event, cx| {
            if let ComboboxEvent::Change(values) = event {
                this.activities.pet_diary_friend_gid = values.first().cloned().unwrap_or_default();
                this.activities.pet_diary_battle_target = None;
                cx.notify();
            }
        })
        .detach();
        let pet_diary_exchange_count = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .placeholder(crate::i18n::text(language, "数量", "Quantity"));
            state.set_value("1", window, cx);
            state.set_min(Some(1.), window, cx);
            state
        });
        let qixi_gift_count = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(crate::i18n::text(
                language,
                "赠送数量",
                "Gift quantity",
            ));
            state.set_value("1", window, cx);
            state
        });
        let state = Self {
            pet_diary_friend_gid: String::new(),
            pet_diary_friend_selector,
            pet_diary_friend_loading: false,
            pet_diary_friend_error: None,
            pet_diary_friend_request_generation: 0,
            pet_diary_friends_loading: false,
            pet_diary_friends_request_generation: 0,
            pet_diary_exchange_count,
            weather: super::weather::WeatherState::new(language, window, cx),
            qixi_selected_friend_gid: String::new(),
            qixi_friend_search: cx.new(|cx| {
                InputState::new(window, cx).placeholder(crate::i18n::text(
                    language,
                    "搜索好友",
                    "Search friends",
                ))
            }),
            qixi_gift_count,
            qixi_friends_loading: false,
            qixi_friends_request_generation: 0,
            qixi_rules_expanded: true,
            star_sand_category: "__all__".into(),
            charity_donation_pending: false,
            charity_rules_expanded: true,
            constellation_group_id: String::new(),
            solar_term_id: String::new(),
            stellar_section: "season".into(),
            activity_tick_generation: 0,
            pet_diary_goods_id: cx.new(|cx| {
                InputState::new(window, cx).placeholder(crate::i18n::text(
                    language,
                    "商品编号",
                    "Item ID",
                ))
            }),
            qingmei_selected_ingredients: HashMap::new(),
            autumn_mode: "wish".into(),
            autumn_choice_id: String::new(),
            autumn_log_tab: 1,
            autumn_logs_load_pending: false,
            pending_actions: HashMap::new(),
            action_sequence: 0,
            action_notice: None,
            action_notice_generation: 0,
            view_revision: 0,
            pet_diary_battle_target: None,
            pet_diary_log_kind: "interact".into(),
            pet_diary_records: HashMap::new(),
            pet_diary_notice: None,
            pet_diary_warning: None,
            pet_diary_expanded_sections: [
                "nurture", "hunt", "charm", "battle", "seeds", "shop", "story", "solar",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        };
        cx.observe(&state.pet_diary_exchange_count, |_, _, cx| cx.notify()).detach();
        cx.observe(&state.qixi_gift_count, |_, _, cx| cx.notify()).detach();
        cx.observe(&state.qixi_friend_search, |_, _, cx| cx.notify()).detach();
        cx.observe(&state.pet_diary_goods_id, |_, _, cx| cx.notify()).detach();
        state
    }
}
