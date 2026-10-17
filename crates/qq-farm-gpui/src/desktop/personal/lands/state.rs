#[derive(Default)]
pub(crate) struct LandsState {
    pub(crate) tick_generation: u64,
    pub(crate) now_sec: i64,
    pub(crate) loaded_at_sec: i64,
    pub(crate) refresh_generation: u64,
    pub(crate) refreshing: bool,
    pub(crate) error: Option<String>,
    pub(crate) operating: bool,
    /// 一键施肥在途（独立于巡田操作按钮）
    pub(crate) fertilize_all_pending: bool,
    pub(crate) fertilize_all_generation: u64,
    pub(crate) finish_operation_after_refresh: bool,
    pub(crate) operation_generation: u64,
    pub(crate) operation_notice: Option<(String, bool)>,
    pub(crate) operation_notice_generation: u64,
    /// 土地详情弹窗（bot e5f3552 LandDetailModal）
    pub(crate) pending_detail: Option<serde_json::Value>,
    pub(crate) detail_generation: u64,
    pub(crate) detail_dialog_open: bool,
    /// 详情弹窗的余额对比（level/gold/coupon/goldBean；打开时拉取）
    pub(crate) detail_balances: Option<serde_json::Value>,
    pub(crate) detail_balances_loading: bool,
}
