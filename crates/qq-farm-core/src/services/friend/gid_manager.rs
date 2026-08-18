//! 好友 GID 缓存管理。
//!
//! 对应原 `core/src/services/friend/gid-manager.ts`（310 行）。
//!
//! ## 阶段 1D 范围
//!
//! - 内存缓存（好友 GID 列表）
//! - 黑名单（不访问的 GID）
//! - 去重 + 排序

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::broadcast;

const DEFAULT_SYNC_INTERVAL: Duration = Duration::from_secs(600);
const DEFAULT_RETRY_INTERVAL: Duration = Duration::from_secs(60);

/// 好友 GID 管理器
pub struct GidManager {
    /// 缓存的好友 GID
    cached: Arc<RwLock<Vec<i64>>>,
    /// 上次同步时间
    last_sync: Arc<RwLock<Option<Instant>>>,
    /// 同步间隔
    sync_interval: Duration,
    /// 重试间隔
    retry_interval: Duration,
    /// 同步事件订阅
    event_tx: broadcast::Sender<GidEvent>,
}

/// GidManager 事件
#[derive(Debug, Clone)]
pub enum GidEvent {
    /// 好友列表已更新
    Synced { count: usize },
    /// 黑名单已更新
    BlacklistChanged { count: usize },
    /// 同步失败
    SyncFailed { message: String },
}

impl GidManager {
    /// 创建
    #[must_use]
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(64);
        Self {
            cached: Arc::new(RwLock::new(Vec::new())),
            last_sync: Arc::new(RwLock::new(None)),
            sync_interval: DEFAULT_SYNC_INTERVAL,
            retry_interval: DEFAULT_RETRY_INTERVAL,
            event_tx,
        }
    }

    /// 订阅事件
    pub fn subscribe(&self) -> broadcast::Receiver<GidEvent> {
        self.event_tx.subscribe()
    }

    /// 当前缓存的 GID 列表（只读快照）
    #[must_use]
    pub fn cached(&self) -> Vec<i64> {
        self.cached.read().clone()
    }

    /// 缓存数量
    #[must_use]
    pub fn count(&self) -> usize {
        self.cached.read().len()
    }

    /// 上次同步时间距今
    #[must_use]
    pub fn since_last_sync(&self) -> Option<Duration> {
        self.last_sync.read().map(|t| t.elapsed())
    }

    /// 是否需要重新同步（达到 sync_interval）
    #[must_use]
    pub fn needs_sync(&self) -> bool {
        self.since_last_sync().map_or(true, |d| d >= self.sync_interval)
    }

    /// 更新缓存（拉取新数据后调用）
    pub fn update(&self, gids: Vec<i64>) {
        // 去重 + 排序
        let unique: HashSet<i64> = gids.into_iter().collect();
        let mut sorted: Vec<i64> = unique.into_iter().collect();
        sorted.sort_unstable();
        let count = sorted.len();
        *self.cached.write() = sorted;
        *self.last_sync.write() = Some(Instant::now());
        let _ = self.event_tx.send(GidEvent::Synced { count });
    }

    /// 标记同步失败
    pub fn clear_cache(&self) {
        *self.cached.write() = Vec::new();
    }

    pub fn mark_sync_failed(&self, message: String) {
        let _ = self.event_tx.send(GidEvent::SyncFailed { message });
    }

    /// 同步间隔
    #[must_use]
    pub fn sync_interval(&self) -> Duration {
        self.sync_interval
    }

    /// 重试间隔
    #[must_use]
    pub fn retry_interval(&self) -> Duration {
        self.retry_interval
    }
}

impl Default for GidManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty() {
        let m = GidManager::new();
        assert_eq!(m.count(), 0);
        assert!(m.needs_sync());
    }

    #[test]
    fn update_dedupes_and_sorts() {
        let m = GidManager::new();
        m.update(vec![3, 1, 2, 2, 1]);
        assert_eq!(m.cached(), vec![1, 2, 3]);
        assert_eq!(m.count(), 3);
    }

    #[test]
    fn update_marks_recent_sync() {
        let m = GidManager::new();
        m.update(vec![1, 2, 3]);
        assert!(!m.needs_sync());
        assert!(m.since_last_sync().unwrap() < Duration::from_secs(1));
    }

    #[test]
    fn events_emitted() {
        let m = GidManager::new();
        let mut rx = m.subscribe();
        m.update(vec![1, 2, 3]);
        let event = rx.try_recv().expect("event");
        match event {
            GidEvent::Synced { count } => assert_eq!(count, 3),
            _ => panic!("expected Synced"),
        }
    }
}
