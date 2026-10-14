use std::collections::HashSet;

pub(crate) struct BagState {
    pub(crate) category: String,
    pub(crate) batch_mode: bool,
    pub(crate) selected_ids: HashSet<i64>,
    pub(crate) tick_generation: u64,
    pub(crate) refresh_generation: u64,
    pub(crate) refreshing: bool,
    pub(crate) error: Option<String>,
    pub(crate) notice: Option<(String, bool)>,
    pub(crate) notice_generation: u64,
    pub(crate) operations_generation: u64,
    pub(crate) selling_ids: HashSet<i64>,
    pub(crate) using_ids: HashSet<i64>,
    pub(crate) batch_selling: bool,
}

impl Default for BagState {
    fn default() -> Self {
        Self {
            category: "fruit".into(),
            batch_mode: Default::default(),
            selected_ids: Default::default(),
            tick_generation: Default::default(),
            refresh_generation: Default::default(),
            refreshing: Default::default(),
            error: Default::default(),
            notice: Default::default(),
            notice_generation: Default::default(),
            operations_generation: Default::default(),
            selling_ids: Default::default(),
            using_ids: Default::default(),
            batch_selling: Default::default(),
        }
    }
}
