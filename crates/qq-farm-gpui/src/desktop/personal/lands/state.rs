#[derive(Default)]
pub(crate) struct LandsState {
    pub(crate) tick_generation: u64,
    pub(crate) now_sec: i64,
    pub(crate) loaded_at_sec: i64,
    pub(crate) refresh_generation: u64,
    pub(crate) refreshing: bool,
    pub(crate) error: Option<String>,
    pub(crate) operating: bool,
    pub(crate) finish_operation_after_refresh: bool,
    pub(crate) operation_generation: u64,
    pub(crate) operation_notice: Option<(String, bool)>,
    pub(crate) operation_notice_generation: u64,
}
