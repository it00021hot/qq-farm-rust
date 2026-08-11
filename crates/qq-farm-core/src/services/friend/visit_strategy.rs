//! 拜访好友策略 —— 1:1 翻译原 `core/src/services/friend/visit-strategy.ts`。
//!
//! 核心：避免重复帮同一块地（recent help 去重）+ 错误分类。
//!
//! ## 阶段 1D 范围（本文件）
//!
//! - [`RecentHelp`] 状态机：in_flight / confirmed / noop
//! - [`prune_recent_help`] 清理过期 + LRU 限流（`HELP_CACHE_MAX = 2048`）
//! - [`get_help_snapshot_key`] 土地快照（用于检测"土地状态变化"）
//! - [`filter_recent_help`] 过滤掉已帮过的（除非快照变了）
//! - [`mark_recent_help`] / [`release_recent_help`]
//! - 错误检测（`is_enter_farm_banned_error` / `is_transient_network_error` / `parse_rpc_error_code`）
//!
//! ## 阶段 1D.2 范围（待办）
//!
//! - 完整策略（安静时段、好友 / 植物黑名单、选访问目标）
//! - gift/wish 流程

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex as PMutex;

// ============ 时间辅助 ============

/// 当前时间（毫秒，Unix epoch）—— 测试时可注入
pub type ClockMs = u64;

#[must_use]
pub fn now_ms() -> ClockMs {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ============ 常量（与原 TS 一致） ============

pub const HELP_IN_FLIGHT_TTL_MS: u64 = 15_000;
pub const HELP_RESULT_TTL_MS: u64 = 30_000;
pub const HELP_CACHE_MAX: usize = 2048;

// ============ RecentHelp 状态机 ============

/// 帮助状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpState {
    /// 进行中（已发请求，未确认）
    InFlight,
    /// 已确认（服务返回成功）
    Confirmed,
    /// NoOp（服务返回无操作，如地块不需要帮助）
    Noop,
}

/// 帮助记录
#[derive(Debug, Clone)]
pub struct RecentHelpEntry {
    pub state: HelpState,
    /// 土地快照（plant.id + phase + dry_num + weeds + insects 的拼字符串）
    pub snapshot_key: String,
    /// 过期时间（ms）
    pub expires_at: ClockMs,
}

/// RecentHelp 缓存
pub struct RecentHelpCache {
    inner: PMutex<HashMap<String, RecentHelpEntry>>,
}

impl Default for RecentHelpCache {
    fn default() -> Self {
        Self::new()
    }
}

impl RecentHelpCache {
    /// 创建
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: PMutex::new(HashMap::new()),
        }
    }

    /// 当前大小
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    /// 是否空
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }

    /// 拼 key
    #[must_use]
    pub fn make_key(host_gid: i64, land_id: i64) -> String {
        format!("{host_gid}:{land_id}")
    }

    /// 构造土地快照 key（用于检测土地状态变化）
    ///
    /// 格式：`landId:plantId:phase:dryNum:weeds:insects|...`（多块地用 `|` 分隔）
    ///
    /// 与原 TS `getHelpSnapshotKey(lands)` 一致
    #[must_use]
    pub fn make_snapshot_key(lands: &[LandSnapshot]) -> String {
        lands
            .iter()
            .map(|land| {
                let weeds = land
                    .weed_owners
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                let insects = land
                    .insect_owners
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{}:{}:{}:{}:{}:{}",
                    land.id, land.plant_id, land.phase, land.dry_num, weeds, insects
                )
            })
            .collect::<Vec<_>>()
            .join("|")
    }

    /// 清理过期条目 + LRU 限流
    ///
    /// 与原 TS `pruneRecentHelp(now)` 一致
    pub fn prune(&self, now: ClockMs) {
        let mut map = self.inner.lock();
        map.retain(|_, entry| entry.expires_at > now);
        // 简单 LRU：超出 HELP_CACHE_MAX 时按 key 顺序删除最早的
        // —— 严格 LRU 需要 LinkedHashMap；阶段 1D 用简化版（FIFO）
        while map.len() > HELP_CACHE_MAX {
            if let Some(first_key) = map.keys().next().cloned() {
                map.remove(&first_key);
            } else {
                break;
            }
        }
    }

    /// 过滤掉已帮过的 land ids
    ///
    /// 规则：
    /// - 如果 entry 不存在或已过期 → 保留
    /// - 如果 entry 存在但 snapshot_key 与当前不同 → 删除旧 entry + 保留
    /// - 否则（entry 存在且 snapshot_key 相同）→ 过滤掉
    ///
    /// 与原 TS `filterRecentHelp(hostGid, landIds, snapshotKey)` 一致
    pub fn filter(
        &self,
        host_gid: i64,
        land_ids: &[i64],
        snapshot_key: &str,
        now: ClockMs,
    ) -> Vec<i64> {
        self.prune(now);
        let mut map = self.inner.lock();
        // 去重 land_ids（保序）
        let mut seen = std::collections::HashSet::new();
        land_ids
            .iter()
            .filter(|&&id| id > 0)
            .filter(|&&id| seen.insert(id))
            .filter(|&&land_id| {
                let key = Self::make_key(host_gid, land_id);
                match map.get(&key) {
                    None => true, // 没记录 → 保留
                    Some(entry) if entry.expires_at <= now => true, // 过期 → 保留
                    Some(entry) if entry.snapshot_key != snapshot_key => {
                        // 快照变了 → 删旧 + 保留
                        map.remove(&key);
                        true
                    }
                    Some(_) => false, // 已帮过且快照一致 → 过滤
                }
            })
            .copied()
            .collect()
    }

    /// 标记一批 land 为已帮（in_flight / confirmed / noop）
    pub fn mark(
        &self,
        host_gid: i64,
        land_ids: &[i64],
        state: HelpState,
        ttl_ms: u64,
        snapshot_key: &str,
        now: ClockMs,
    ) {
        let mut map = self.inner.lock();
        for &land_id in land_ids {
            let key = Self::make_key(host_gid, land_id);
            map.insert(
                key,
                RecentHelpEntry {
                    state,
                    snapshot_key: snapshot_key.to_string(),
                    expires_at: now + ttl_ms,
                },
            );
        }
        drop(map);
        self.prune(now);
    }

    /// 释放一批 land（删除记录）
    pub fn release(&self, host_gid: i64, land_ids: &[i64]) {
        let mut map = self.inner.lock();
        for &land_id in land_ids {
            map.remove(&Self::make_key(host_gid, land_id));
        }
    }

    /// 获取某条记录
    #[must_use]
    pub fn get(&self, host_gid: i64, land_id: i64) -> Option<RecentHelpEntry> {
        self.inner.lock().get(&Self::make_key(host_gid, land_id)).cloned()
    }
}

/// 土地快照（用于 snapshot_key 构造）
#[derive(Debug, Clone, Default)]
pub struct LandSnapshot {
    pub id: i64,
    pub plant_id: i64,
    pub phase: i64,
    pub dry_num: i64,
    pub weed_owners: Vec<i64>,
    pub insect_owners: Vec<i64>,
}

impl LandSnapshot {
    /// 从 `LandInfo` 构造（farm 子模块的 LandInfo）
    ///
    /// —— 阶段 1D 不依赖 land-analysis，调用方自行构造 LandSnapshot
    pub fn from_land(land: &crate::proto::generated::gamepb::plantpb::LandInfo) -> Self {
        let plant = land.plant.as_ref();
        let phases = plant.map(|p| &p.phases);
        let phase = phases.and_then(|p| p.last()).map(|p| p.phase as i64).unwrap_or(0);
        Self {
            id: land.id,
            plant_id: plant.map(|p| p.id).unwrap_or(0),
            phase,
            dry_num: plant.map(|p| p.dry_num).unwrap_or(0),
            weed_owners: plant.map(|p| p.weed_owners.clone()).unwrap_or_default(),
            insect_owners: plant.map(|p| p.insect_owners.clone()).unwrap_or_default(),
        }
    }
}

// ============ 错误检测（与原 TS 1:1 翻译） ============

/// 检测"进入农场被封"错误（code 1002003）
#[must_use]
pub fn is_enter_farm_banned_error(error_message: &str) -> bool {
    error_message.contains("1002003")
}

/// 从错误消息中解析 RPC 错误码
#[must_use]
pub fn parse_rpc_error_code(error_message: &str) -> i32 {
    // 正则 /code=(\d+)/i
    if let Some(start) = error_message.find("code=") {
        let rest = &error_message[start + 5..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse().unwrap_or(0)
    } else {
        0
    }
}

/// 检测瞬态网络错误（用于重试判断）
#[must_use]
pub fn is_transient_network_error(error_message: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "连接未打开",
        "请求超时",
        "请求已中断",
        "连接关闭",
        "连接已在加密途中关闭",
        "worker exited",
    ];
    KEYWORDS.iter().any(|k| error_message.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== RecentHelp 状态机测试 =====

    #[test]
    fn make_key_format() {
        assert_eq!(RecentHelpCache::make_key(100, 1), "100:1");
        assert_eq!(RecentHelpCache::make_key(-1, 1), "-1:1");
    }

    #[test]
    fn snapshot_key_basic() {
        let lands = vec![
            LandSnapshot {
                id: 1, plant_id: 10, phase: 2, dry_num: 0, weed_owners: vec![], insect_owners: vec![],
            },
            LandSnapshot {
                id: 2, plant_id: 10, phase: 3, dry_num: 1, weed_owners: vec![100], insect_owners: vec![],
            },
        ];
        let key = RecentHelpCache::make_snapshot_key(&lands);
        assert_eq!(key, "1:10:2:0::|2:10:3:1:100:");
    }

    #[test]
    fn filter_empty_cache_returns_all() {
        let cache = RecentHelpCache::new();
        let lands = vec![1, 2, 3];
        let snap = "x";
        let result = cache.filter(100, &lands, snap, 1000);
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn filter_removes_already_helped_with_same_snapshot() {
        let cache = RecentHelpCache::new();
        cache.mark(100, &[1, 2], HelpState::Confirmed, 30_000, "snap1", 1000);
        let result = cache.filter(100, &[1, 2, 3], "snap1", 1100);
        assert_eq!(result, vec![3]);
    }

    #[test]
    fn filter_keeps_when_snapshot_changed() {
        let cache = RecentHelpCache::new();
        cache.mark(100, &[1], HelpState::Confirmed, 30_000, "snap_old", 1000);
        // 同一块地但 snapshot 变了 → 删旧 + 保留
        let result = cache.filter(100, &[1], "snap_new", 1100);
        assert_eq!(result, vec![1]);
        assert!(cache.get(100, 1).is_none()); // 旧 entry 已被清掉
    }

    #[test]
    fn filter_expired_entries_pass_through() {
        let cache = RecentHelpCache::new();
        cache.mark(100, &[1], HelpState::Confirmed, 100, "snap", 1000);
        // 1500ms 后查 → 已过期
        let result = cache.filter(100, &[1], "snap", 1500);
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn filter_dedupes_input() {
        let cache = RecentHelpCache::new();
        let result = cache.filter(100, &[1, 1, 2, 2, 3], "snap", 1000);
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn filter_skips_non_positive_ids() {
        let cache = RecentHelpCache::new();
        let result = cache.filter(100, &[0, -1, 1, 2], "snap", 1000);
        assert_eq!(result, vec![1, 2]);
    }

    #[test]
    fn mark_and_release() {
        let cache = RecentHelpCache::new();
        cache.mark(100, &[1, 2], HelpState::InFlight, 15_000, "snap", 1000);
        assert_eq!(cache.len(), 2);
        cache.release(100, &[1]);
        assert_eq!(cache.len(), 1);
        assert!(cache.get(100, 1).is_none());
        assert!(cache.get(100, 2).is_some());
    }

    #[test]
    fn prune_removes_expired() {
        let cache = RecentHelpCache::new();
        cache.mark(100, &[1], HelpState::Confirmed, 100, "snap", 1000);
        cache.mark(200, &[2], HelpState::Confirmed, 200, "snap", 1000);
        cache.prune(1100);
        // 1 已过期（100 ttl → 1100 过期），2 仍在（200 ttl → 1200 过期）
        assert!(cache.get(100, 1).is_none());
        assert!(cache.get(200, 2).is_some());
    }

    #[test]
    fn prune_lru_caps_size() {
        let cache = RecentHelpCache::new();
        // 填到 HELP_CACHE_MAX + 10
        for i in 0..(HELP_CACHE_MAX + 10) {
            cache.mark(100, &[i as i64], HelpState::Confirmed, 30_000, "snap", 0);
        }
        cache.prune(0);
        // 限流到 HELP_CACHE_MAX
        assert_eq!(cache.len(), HELP_CACHE_MAX);
    }

    #[test]
    fn different_host_gids_isolated() {
        let cache = RecentHelpCache::new();
        cache.mark(100, &[1], HelpState::Confirmed, 30_000, "snap", 1000);
        // gid 200 的 1 号地未帮过
        let result = cache.filter(200, &[1], "snap", 1100);
        assert_eq!(result, vec![1]);
    }

    // ===== 错误检测测试 =====

    #[test]
    fn enter_farm_banned_error_detected() {
        assert!(is_enter_farm_banned_error("gate error: code=1002003 禁止进入"));
        assert!(!is_enter_farm_banned_error("some other error"));
        assert!(!is_enter_farm_banned_error(""));
    }

    #[test]
    fn parse_rpc_error_code_extracts() {
        assert_eq!(parse_rpc_error_code("error: code=1002003 msg"), 1002003);
        assert_eq!(parse_rpc_error_code("error: code=42"), 42);
        assert_eq!(parse_rpc_error_code("no code here"), 0);
        assert_eq!(parse_rpc_error_code("code=99999999 at position"), 99999999);
    }

    #[test]
    fn transient_network_error_detected() {
        assert!(is_transient_network_error("连接未打开"));
        assert!(is_transient_network_error("请求超时: foo"));
        assert!(is_transient_network_error("连接关闭 (code=1006)"));
        assert!(is_transient_network_error("worker exited"));
        assert!(!is_transient_network_error("业务错误"));
        assert!(!is_transient_network_error(""));
    }
}
