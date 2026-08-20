//! 活动窗口缓存（`ActivityService.List` 的 `activity_windows`）。
//!
//! 窗口表是全服日程，不是账号态；进程内一份即可。

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Instant;

use parking_lot::RwLock;

use crate::constants::ACTIVITY_WINDOWS_CACHE_TTL_MS;

/// 一条活动时间窗。
#[derive(Debug, Clone, Default)]
pub struct ActivityWindow {
    pub id: String,
    pub name: String,
    pub begin_time: i64,
    pub end_time: i64,
}

struct WindowsState {
    windows: HashMap<String, ActivityWindow>,
    loaded: bool,
    loaded_at: Option<Instant>,
}

impl WindowsState {
    fn empty() -> Self {
        Self { windows: HashMap::new(), loaded: false, loaded_at: None }
    }
}

static WINDOWS: LazyLock<RwLock<WindowsState>> =
    LazyLock::new(|| RwLock::new(WindowsState::empty()));

/// 用 List 回包替换缓存。
pub fn set_activity_windows(windows: Vec<ActivityWindow>) {
    let mut state = WINDOWS.write();
    let mut next = HashMap::with_capacity(windows.len());
    for window in windows {
        if window.id.is_empty() {
            continue;
        }
        next.insert(window.id.clone(), window);
    }
    state.loaded = !next.is_empty();
    state.loaded_at = Some(Instant::now());
    state.windows = next;
}

/// 缓存快照（活动目录用）。
#[must_use]
pub fn activity_windows_snapshot() -> Vec<ActivityWindow> {
    WINDOWS.read().windows.values().cloned().collect()
}

/// 缓存是否仍在 TTL 内。
#[must_use]
pub fn activity_windows_fresh() -> bool {
    let state = WINDOWS.read();
    match (state.loaded, state.loaded_at) {
        (true, Some(at)) => at.elapsed().as_millis() < u128::from(ACTIVITY_WINDOWS_CACHE_TTL_MS),
        _ => false,
    }
}

/// 是否已经成功写入过窗口。
#[must_use]
pub fn activity_windows_loaded() -> bool {
    WINDOWS.read().loaded
}

/// 按活动 ID 取一条窗口。
#[must_use]
pub fn activity_window_by_id(id: &str) -> Option<ActivityWindow> {
    if id.is_empty() {
        return None;
    }
    WINDOWS.read().windows.get(id).cloned()
}

#[cfg(test)]
pub fn clear_activity_windows_for_test() {
    *WINDOWS.write() = WindowsState::empty();
}
