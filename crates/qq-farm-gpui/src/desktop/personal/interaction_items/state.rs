use crate::desktop::FarmDesktop;
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext as _, Context, Entity, Window};

pub(crate) struct InteractionItemsState {
    pub(crate) land_ids: Entity<InputState>,
    pub(crate) picker: Entity<super::InteractionItemSelect>,
    pub(crate) using: bool,
    pub(crate) refreshing: bool,
    pub(crate) generation: u64,
    pub(crate) refresh_generation: u64,
    pub(crate) notice: Option<(String, bool)>,
    pub(crate) notice_generation: u64,
}

impl InteractionItemsState {
    pub(in crate::desktop) fn new(
        language: &str,
        window: &mut Window,
        cx: &mut Context<FarmDesktop>,
    ) -> Self {
        let picker = cx.new(|cx| {
            super::InteractionItemSelect::new(
                gpui_kit::component::select::SearchableVec::new(Vec::new()),
                None,
                window,
                cx,
            )
        });
        cx.subscribe(&picker, |_, _, event, cx| {
            let gpui_kit::component::select::SelectEvent::Confirm(_) = event;
            cx.notify();
        })
        .detach();
        Self {
            land_ids: cx.new(|cx| {
                InputState::new(window, cx).placeholder(crate::i18n::text(
                    language,
                    "例如： 1,2,3",
                    "e.g. 1,2,3",
                ))
            }),
            picker,
            using: false,
            refreshing: false,
            generation: 0,
            refresh_generation: 0,
            notice: None,
            notice_generation: 0,
        }
    }
}
