use super::{
    bag::BagState, illustrated::IllustratedState, interaction_items::InteractionItemsState,
    lands::LandsState, pets::PetsState,
};
use crate::desktop::{Context, FarmDesktop, Window};

pub(crate) struct FarmState {
    pub(crate) lands: LandsState,
    pub(crate) bag: BagState,
    pub(crate) interaction: InteractionItemsState,
    pub(crate) illustrated: IllustratedState,
    pub(crate) pets: PetsState,
}

impl FarmState {
    pub(in crate::desktop) fn new(
        language: &str,
        window: &mut Window,
        cx: &mut Context<FarmDesktop>,
    ) -> Self {
        Self {
            lands: LandsState::default(),
            bag: BagState::default(),
            interaction: InteractionItemsState::new(language, window, cx),
            illustrated: IllustratedState::default(),
            pets: PetsState::default(),
        }
    }
}
