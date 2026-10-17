/// 节令日历页状态（数据本体在 `page_raw`）。
#[derive(Default)]
pub(crate) struct SolarCalendarState {
    /// 写操作在途（claim / answer / season）
    pub(crate) pending: bool,
    pub(crate) action_generation: u64,
    /// 当前选中的季度 id
    pub(crate) season_id: i64,
    /// 正在答题的事件 id（0 = 未打开）
    pub(crate) question_id: i64,
    pub(crate) quiz_answer: Option<usize>,
    /// 本地时钟：serverTime + 本地流逝秒（1s tick）
    pub(crate) now_sec: i64,
    pub(crate) base_server_time: i64,
    pub(crate) base_at_sec: i64,
    pub(crate) tick_generation: u64,
    /// 已触发自动刷新的边界（避免同一秒重复刷新）
    pub(crate) refreshed_boundary: i64,
    /// 页面级错误（快照加载失败由通用错误横幅承担，这里给页内重试）
    pub(crate) error: Option<String>,
}
