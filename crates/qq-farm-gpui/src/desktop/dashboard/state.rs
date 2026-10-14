use crate::desktop::FarmDesktop;
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext as _, Context, Entity, ScrollHandle, Window};

pub(crate) struct DashboardState {
    pub(crate) log_keyword: Entity<InputState>,
    pub(crate) event_filter: String,
    pub(crate) module_filter: String,
    pub(crate) level_filter: String,
    pub(crate) logs_expanded: bool,
    pub(crate) logs_scroll: ScrollHandle,
    pub(crate) logs_follow_tail: bool,
    pub(crate) tick_generation: u64,
    pub(crate) bag_refresh_in_secs: u8,
    pub(crate) bag_refresh_generation: u64,
    pub(crate) bag_refresh_pending: bool,
    pub(crate) coupon_baseline: Option<i64>,
    pub(crate) status_revision: u64,
}

impl DashboardState {
    pub(in crate::desktop) fn new(
        language: &str,
        window: &mut Window,
        cx: &mut Context<FarmDesktop>,
    ) -> Self {
        let state = Self {
            log_keyword: cx.new(|cx| {
                InputState::new(window, cx).placeholder(crate::i18n::text(
                    language,
                    "关键词",
                    "Keyword",
                ))
            }),
            event_filter: String::new(),
            module_filter: String::new(),
            level_filter: String::new(),
            logs_expanded: false,
            logs_scroll: ScrollHandle::new(),
            logs_follow_tail: true,
            tick_generation: 0,
            bag_refresh_in_secs: 30,
            bag_refresh_generation: 0,
            bag_refresh_pending: false,
            coupon_baseline: None,
            status_revision: 0,
        };
        cx.observe(&state.log_keyword, |_, _, cx| cx.notify()).detach();
        state
    }

    pub(crate) fn logs_are_near_bottom(&self) -> bool {
        let offset = self.logs_scroll.offset().y.as_f32();
        let max_offset = self.logs_scroll.max_offset().y.as_f32();
        (offset + max_offset).abs() <= 50.
    }
}
