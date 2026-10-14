use gpui_kit::{component::input::InputState, Entity};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct PetsState {
    pub(crate) food_inputs: HashMap<i64, Entity<InputState>>,
    pub(crate) logs: Vec<serde_json::Value>,
    pub(crate) activating: bool,
    pub(crate) activation_refresh_pending: bool,
    pub(crate) refresh_generation: u64,
    pub(crate) refreshing: bool,
    pub(crate) operation_generation: u64,
    pub(crate) notice: Option<(String, bool)>,
    pub(crate) notice_generation: u64,
}
