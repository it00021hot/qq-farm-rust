/// 快乐不独享页状态（数据在 `page_raw`）。
#[derive(Default)]
pub(crate) struct ShareRewardState {
    pub(crate) pending: bool,
    pub(crate) generation: u64,
    pub(crate) error: Option<String>,
    /// 日志 tab：1 = 我收到的 / 0 = 我送出的
    pub(crate) log_tab: i64,
    /// 已加载的日志（tail 接口返回，随 tab 切换刷新）
    pub(crate) logs: Vec<serde_json::Value>,
}
