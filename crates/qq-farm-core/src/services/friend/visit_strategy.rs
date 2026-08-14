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
        "request timeout",
        "请求已中断",
        "连接关闭",
        "连接已在加密途中关闭",
        "worker exited",
    ];
    KEYWORDS.iter().any(|k| error_message.contains(k))
}

// =====================================================================
// 阶段 2E：1:1 补全原 TS visit-strategy.ts（time / blacklist / analyze / visit）
// =====================================================================

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::sleep;

use crate::proto::generated::gamepb::plantpb::LandInfo;
use crate::services::friend::api::FriendApi;
use crate::services::friend::gid_manager::GidManager;

// ============ 安静时段 ============

/// 解析 "HH:MM" 格式为分钟数（0-1439）；无效返回 None
#[must_use]
pub fn parse_time_to_minutes(time_str: &str) -> Option<u32> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let h: u32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}

/// 好友安静时段配置
#[derive(Debug, Clone, Default)]
pub struct FriendQuietHours {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

/// 全局安静时段（默认禁用，由 store / config 注入）
pub static FRIEND_QUIET_HOURS: PMutex<Option<FriendQuietHours>> = PMutex::new(None);

/// 设置全局安静时段（线程安全）
pub fn set_friend_quiet_hours(cfg: Option<FriendQuietHours>) {
    *FRIEND_QUIET_HOURS.lock() = cfg;
}

/// 当前是否在好友安静时段（测试 / 未指定账号时读全局注入）
#[must_use]
pub fn in_friend_quiet_hours(now_hhmm: Option<(u32, u32)>) -> bool {
    in_friend_quiet_hours_for(None, now_hhmm)
}

/// 按账号配置判断安静时段（对齐 TS `getFriendQuietHours(accountId)`）
#[must_use]
pub fn in_friend_quiet_hours_for(account_id: Option<&str>, now_hhmm: Option<(u32, u32)>) -> bool {
    let cfg = if let Some(id) = account_id {
        let snap = crate::models::store::account_config::get_friend_quiet_hours(Some(id));
        if !snap.enabled {
            return false;
        }
        FriendQuietHours {
            enabled: true,
            start: snap.start,
            end: snap.end,
        }
    } else {
        let cfg = FRIEND_QUIET_HOURS.lock().clone();
        match cfg {
            Some(c) if c.enabled => c,
            _ => return false,
        }
    };
    let (h, m) = now_hhmm.unwrap_or_else(|| {
        let t = chrono::Local::now();
        (t.format("%H").to_string().parse().unwrap_or(0), t.format("%M").to_string().parse().unwrap_or(0))
    });
    let cur = h * 60 + m;
    let start = match parse_time_to_minutes(&cfg.start) {
        Some(s) => s,
        None => return false,
    };
    let end = match parse_time_to_minutes(&cfg.end) {
        Some(e) => e,
        None => return false,
    };
    if start == end {
        return true;
    }
    if start < end {
        cur >= start && cur < end
    } else {
        cur >= start || cur < end
    }
}

// ============ 黑名单管理 ============

/// 全局好友黑名单（host_gid -> reason）—— 延迟初始化
pub fn friend_blacklist() -> &'static PMutex<std::collections::HashMap<i64, String>> {
    use std::sync::OnceLock;
    static BLACKLIST: OnceLock<PMutex<std::collections::HashMap<i64, String>>> = OnceLock::new();
    BLACKLIST.get_or_init(|| PMutex::new(std::collections::HashMap::new()))
}

/// 加入好友黑名单
pub fn add_friend_to_blacklist(friend_gid: i64, friend_name: &str, reason: &str) -> bool {
    if friend_gid == 0 {
        return false;
    }
    let mut map = friend_blacklist().lock();
    if map.contains_key(&friend_gid) {
        return false;
    }
    map.insert(friend_gid, reason.to_string());
    tracing::warn!(
        friend_gid,
        friend_name = %friend_name,
        reason = %reason,
        "好友已加入黑名单"
    );
    true
}

/// 移除黑名单
pub fn remove_from_blacklist(friend_gid: i64) -> bool {
    friend_blacklist().lock().remove(&friend_gid).is_some()
}

/// 是否在黑名单
#[must_use]
pub fn is_in_blacklist(friend_gid: i64) -> bool {
    friend_blacklist().lock().contains_key(&friend_gid)
}

/// 黑名单大小
#[must_use]
pub fn blacklist_size() -> usize {
    friend_blacklist().lock().len()
}

/// 检测"好友关系失效"错误（无效 / 不存在 / 删除 / 关系 / not found / invalid）
#[must_use]
pub fn is_invalid_friend_access_error(error_message: &str) -> bool {
    if error_message.is_empty() {
        return false;
    }
    if is_enter_farm_banned_error(error_message) || is_transient_network_error(error_message) {
        return false;
    }
    let lower = error_message.to_lowercase();
    let has_keyword = [
        "无效", "不存在", "删除", "关系", "not found", "invalid", "not friend", "friend",
    ]
    .iter()
    .any(|k| lower.contains(&k.to_lowercase()));
    has_keyword && parse_rpc_error_code(error_message) > 0
}

/// 处理"进入好友农场"错误
///
/// 返回 `{ handled, kind }`：`blacklist` / `invalid_removed` / `error`
#[must_use]
pub fn handle_friend_enter_error(
    friend_gid: i64,
    friend_name: &str,
    error_message: &str,
) -> FriendEnterErrorKind {
    if is_enter_farm_banned_error(error_message) {
        add_friend_to_blacklist(friend_gid, friend_name, error_message);
        return FriendEnterErrorKind::Blacklist;
    }
    if is_invalid_friend_access_error(error_message) {
        return FriendEnterErrorKind::InvalidRemoved;
    }
    FriendEnterErrorKind::Error
}

/// 进入好友农场错误分类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendEnterErrorKind {
    /// 已加入黑名单
    Blacklist,
    /// 关系失效
    InvalidRemoved,
    /// 普通错误（未处理）
    Error,
}

// ============ 好友土地分析 ============

/// 偷菜可偷信息
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StealableInfo {
    pub land_id: i64,
    pub plant_id: i64,
    pub name: String,
}

/// 好友土地分析结果
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResult {
    pub stealable: Vec<i64>,
    pub stealable_info: Vec<StealableInfo>,
    pub need_water: Vec<i64>,
    pub need_weed: Vec<i64>,
    pub need_bug: Vec<i64>,
    pub can_put_weed: Vec<i64>,
    pub can_put_bug: Vec<i64>,
}

/// 偷菜可偷的植物信息（plant_id, name）
pub fn get_plant_name(plant_id: i64) -> Option<String> {
    let cfg = crate::config::game_config::global();
    let name = cfg.get_plant_name(plant_id);
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// 是否活动植物（用于"仅偷活动植物"）
///
/// 阶段 2E 简化版：通过模块级 `ACTIVITY_PLANTS` HashSet 判断（默认 false）。
/// 当 `mark_activity_plant()` 被调用后该函数会返回 true。
#[must_use]
pub fn is_activity_plant(land: &LandInfo) -> bool {
    let plant_id = match land.plant.as_ref() {
        Some(p) => p.id,
        None => return false,
    };
    activity_plants().lock().unwrap().contains(&plant_id)
}

/// 标记活动植物（在偷到带活动积分的植物时调用）
pub fn mark_activity_plant(plant_id: i64) {
    activity_plants().lock().unwrap().insert(plant_id);
}

// 模块级活动植物集合（共享给 is_activity_plant / mark_activity_plant）
use std::sync::Mutex as StdMutex;
static ACTIVITY_PLANTS: std::sync::OnceLock<StdMutex<std::collections::HashSet<i64>>> =
    std::sync::OnceLock::new();

#[allow(dead_code)]
fn activity_plants() -> &'static StdMutex<std::collections::HashSet<i64>> {
    ACTIVITY_PLANTS.get_or_init(|| StdMutex::new(std::collections::HashSet::new()))
}

/// 植物黑名单（按 account_id 隔离）
pub fn plant_blacklist() -> &'static PMutex<std::collections::HashMap<String, Vec<i64>>> {
    use std::sync::OnceLock;
    static MAP: OnceLock<PMutex<std::collections::HashMap<String, Vec<i64>>>> = OnceLock::new();
    MAP.get_or_init(|| PMutex::new(std::collections::HashMap::new()))
}

/// 设置植物黑名单
pub fn set_plant_blacklist(account_id: &str, seeds: Vec<i64>) {
    plant_blacklist().lock().insert(account_id.to_string(), seeds);
}

/// 获取植物黑名单
#[must_use]
pub fn get_plant_blacklist(account_id: &str) -> Vec<i64> {
    plant_blacklist()
        .lock()
        .get(account_id)
        .cloned()
        .unwrap_or_default()
}

/// 好友黑名单（按 account_id 隔离）
pub fn account_friend_blacklist() -> &'static PMutex<std::collections::HashMap<String, Vec<i64>>> {
    use std::sync::OnceLock;
    static MAP: OnceLock<PMutex<std::collections::HashMap<String, Vec<i64>>>> = OnceLock::new();
    MAP.get_or_init(|| PMutex::new(std::collections::HashMap::new()))
}

/// 设置好友黑名单
pub fn set_account_friend_blacklist(account_id: &str, gids: Vec<i64>) {
    account_friend_blacklist().lock().insert(account_id.to_string(), gids);
}

/// 获取好友黑名单
#[must_use]
pub fn get_account_friend_blacklist(account_id: &str) -> Vec<i64> {
    account_friend_blacklist()
        .lock()
        .get(account_id)
        .cloned()
        .unwrap_or_default()
}

/// 阶段枚举（与原 TS PlantPhase 对齐）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlantPhase {
    Seed = 0,
    Sprout = 1,
    Growing = 2,
    Ripe = 3,
    Dead = 4,
}

impl PlantPhase {
    /// 对齐 TS / `land_analysis` 的 proto 阶段：1 SEED、2 GERMINATION、
    /// 3–5 生长、6 MATURE、7 DEAD。
    #[must_use]
    pub fn from_i32(v: i32) -> Self {
        match v {
            2 => Self::Sprout,
            3 | 4 | 5 => Self::Growing,
            6 => Self::Ripe,
            7 => Self::Dead,
            _ => Self::Seed,
        }
    }
}

/// 获取土地当前阶段（按 begin_time 取当前阶段，对齐 TS `getCurrentPhase`）
#[must_use]
pub fn get_current_phase(land: &LandInfo) -> Option<PlantPhase> {
    let plant = land.plant.as_ref()?;
    if plant.phases.is_empty() {
        return None;
    }
    crate::services::farm::land_analysis::PlantPhase::from_phases(&plant.phases).map(|p| match p {
        crate::services::farm::land_analysis::PlantPhase::Seed => PlantPhase::Seed,
        crate::services::farm::land_analysis::PlantPhase::Sprout => PlantPhase::Sprout,
        crate::services::farm::land_analysis::PlantPhase::Growing => PlantPhase::Growing,
        crate::services::farm::land_analysis::PlantPhase::Ripe => PlantPhase::Ripe,
        crate::services::farm::land_analysis::PlantPhase::Dead => PlantPhase::Dead,
    })
}

/// 是否"被占领的从地块"（共享主地块的从地）
#[must_use]
pub fn is_occupied_slave_land(land: &LandInfo) -> bool {
    land.master_land_id > 0 && land.master_land_id != land.id
}

/// 分析好友土地
///
/// 与原 TS `analyzeFriendLands(lands, myGid, friendName, options)` 一致
#[must_use]
pub fn analyze_friend_lands(
    lands: &[LandInfo],
    my_gid: i64,
    plant_blacklist: &[i64],
    steal_activity_only: bool,
) -> AnalyzeResult {
    let mut result = AnalyzeResult::default();
    let land_ids: HashSet<i64> = lands.iter().map(|l| l.id).collect();
    for land in lands {
        if is_occupied_slave_land(land) {
            continue;
        }
        let plant = match land.plant.as_ref() {
            Some(p) => p,
            None => continue,
        };
        if plant.phases.is_empty() {
            continue;
        }
        let phase = match get_current_phase(land) {
            Some(p) => p,
            None => continue,
        };
        let id = land.id;

        if phase == PlantPhase::Ripe {
            if plant.stealable {
                let plant_id = plant.id;
                // 蔬菜黑名单按 seed_id 过滤（1:1 对齐 TS `visit-strategy.ts`）
                let seed_id = crate::config::game_config::global()
                    .get_plant_by_id(plant_id)
                    .and_then(|p| p.seed_id)
                    .unwrap_or(0);
                if !plant_blacklist.is_empty() && seed_id > 0 && plant_blacklist.contains(&seed_id) {
                    continue;
                }
                if steal_activity_only && !is_activity_plant(land) {
                    continue;
                }
                result.stealable.push(id);
                result.stealable_info.push(StealableInfo {
                    land_id: id,
                    plant_id,
                    name: get_plant_name(plant_id).unwrap_or_else(|| "未知".to_string()),
                });
            }
            continue;
        }

        if phase == PlantPhase::Dead {
            continue;
        }

        // 帮助操作
        if plant.dry_num > 0 {
            result.need_water.push(id);
        }
        if !plant.weed_owners.is_empty() {
            result.need_weed.push(id);
        }
        if !plant.insect_owners.is_empty() {
            result.need_bug.push(id);
        }

        // 捣乱操作：每块地最多 2 个草/虫，且我没放过
        let weed_count = plant.weed_owners.len();
        let insect_count = plant.insect_owners.len();
        let i_put_weed = plant.weed_owners.contains(&my_gid);
        let i_put_bug = plant.insect_owners.contains(&my_gid);
        if weed_count < 2 && !i_put_weed {
            result.can_put_weed.push(id);
        }
        if insect_count < 2 && !i_put_bug {
            result.can_put_bug.push(id);
        }
    }
    let _ = land_ids; // 保持参数引用
    result
}

// ============ 好友列表缓存 ============

const DEFAULT_FRIENDS_LIST_CACHE_TTL_MS: u64 = 60_000;
const MIN_FRIENDS_LIST_CACHE_TTL_MS: u64 = 10_000;

/// 全局好友列表缓存
#[derive(Debug, Clone, Default)]
pub struct FriendsListCache {
    pub friends: Vec<FriendSummary>,
    pub time_ms: u64,
}

impl FriendsListCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_ttl_ms(&self, configured_ttl_sec: i64) -> u64 {
        if configured_ttl_sec <= 0 {
            return DEFAULT_FRIENDS_LIST_CACHE_TTL_MS;
        }
        let ms = (configured_ttl_sec as u64) * 1000;
        ms.max(MIN_FRIENDS_LIST_CACHE_TTL_MS)
    }
}

/// 好友摘要
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendSummary {
    pub gid: i64,
    pub name: String,
    pub avatar_url: String,
    pub level: i64,
    pub gold: i64,
    pub plant: Option<FriendPlantSummary>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendPlantSummary {
    pub steal_num: i64,
    pub dry_num: i64,
    pub weed_num: i64,
    pub insect_num: i64,
}

/// 把 proto `GameFriend` 映射成面板 DTO（对齐 visit-strategy.ts）
#[must_use]
pub fn game_friend_to_summary(f: crate::proto::generated::gamepb::friendpb::GameFriend) -> FriendSummary {
    let gid = f.gid;
    let name = if !f.remark.trim().is_empty() {
        f.remark
    } else if !f.name.trim().is_empty() {
        f.name
    } else {
        format!("GID:{gid}")
    };
    FriendSummary {
        gid,
        name,
        avatar_url: f.avatar_url.trim().to_string(),
        level: f.level,
        gold: f.gold,
        plant: f.plant.map(|p| FriendPlantSummary {
            steal_num: p.steal_plant_num,
            dry_num: p.dry_num,
            weed_num: p.weed_num,
            insect_num: p.insect_num,
        }),
    }
}

/// 全局好友列表缓存 state
pub static FRIENDS_LIST_CACHE: PMutex<Option<FriendsListCache>> = PMutex::new(None);

/// 清空好友列表缓存
pub fn clear_friends_list_cache() {
    *FRIENDS_LIST_CACHE.lock() = None;
}

// ============ 批量操作与帮助结果 ============

/// 批量操作 fallback（先尝试批量，失败则逐个）
pub async fn run_batch_with_fallback<F, S, FutB, FutS>(
    ids: &[i64],
    batch_fn: F,
    single_fn: S,
) -> usize
where
    F: FnOnce(Vec<i64>) -> FutB,
    S: Fn(i64) -> FutS,
    FutB: std::future::Future<Output = Result<(), crate::error::Error>>,
    FutS: std::future::Future<Output = Result<(), crate::error::Error>>,
{
    let target: Vec<i64> = ids.iter().copied().filter(|&i| i > 0).collect();
    if target.is_empty() {
        return 0;
    }
    if (batch_fn(target.clone()).await).is_ok() {
        return target.len();
    }
    let mut ok = 0usize;
    for id in target {
        if (single_fn(id).await).is_ok() {
            ok += 1;
        }
        sleep(Duration::from_millis(100)).await;
    }
    ok
}

// ============ FarmingOutcome（与原 TS 一致） ============

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct FarmingOutcome {
    pub effect: FarmingEffect,
    pub operation_count: i64,
    pub land_count: usize,
    pub land_ids: Vec<i64>,
    pub operation_limits: Vec<serde_json::Value>,
    pub code: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FarmingEffect {
    #[default]
    Noop,
    Confirmed,
    Uncertain,
}

#[must_use]
pub fn empty_farming_outcome(effect: FarmingEffect) -> FarmingOutcome {
    FarmingOutcome {
        effect,
        operation_count: 0,
        land_count: 0,
        land_ids: Vec::new(),
        operation_limits: Vec::new(),
        code: 0,
    }
}

#[must_use]
pub fn merge_farming_outcomes(outcomes: &[FarmingOutcome]) -> FarmingOutcome {
    let confirmed: Vec<&FarmingOutcome> = outcomes
        .iter()
        .filter(|o| o.effect == FarmingEffect::Confirmed)
        .collect();
    let mut land_ids: Vec<i64> = confirmed
        .iter()
        .flat_map(|o| o.land_ids.iter().copied())
        .collect();
    land_ids.sort_unstable();
    land_ids.dedup();
    let mut operation_limits: Vec<serde_json::Value> = confirmed
        .iter()
        .flat_map(|o| o.operation_limits.iter().cloned())
        .collect();
    operation_limits.sort_by_key(|v| serde_json::to_string(v).unwrap_or_default());
    operation_limits.dedup_by(|a, b| serde_json::to_string(a).unwrap_or_default() == serde_json::to_string(b).unwrap_or_default());

    let effect = if !confirmed.is_empty() {
        FarmingEffect::Confirmed
    } else if outcomes.iter().any(|o| o.effect == FarmingEffect::Uncertain) {
        FarmingEffect::Uncertain
    } else {
        FarmingEffect::Noop
    };

    let operation_count: i64 = confirmed.iter().map(|o| o.operation_count).sum();

    FarmingOutcome {
        effect,
        operation_count,
        land_count: land_ids.len(),
        land_ids,
        operation_limits,
        code: 0,
    }
}

// ============ 偷菜结果 ============

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct StealResult {
    pub ok: usize,
    pub stolen_infos: Vec<StealableInfo>,
    pub score_gained: i64,
}

// ============ 访问结果 ============

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct VisitResult {
    pub acted: bool,
    pub entered: bool,
}

// ============ 检查可操作（10008 偷菜 / 10003 捣乱） ============

/// 实际帮好友 farm（带 RecentHelp 去重 + 批量 fallback）
pub async fn run_farming_with_fallback(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    host_gid: i64,
    ids: &[i64],
    _stop_when_exp_limit: bool,
    snapshot_key: &str,
) -> FarmingOutcome {
    let target = recent_help.filter(host_gid, ids, snapshot_key, now_ms());
    if target.is_empty() {
        return empty_farming_outcome(FarmingEffect::Noop);
    }
    recent_help.mark(
        host_gid,
        &target,
        HelpState::InFlight,
        HELP_IN_FLIGHT_TTL_MS,
        snapshot_key,
        now_ms(),
    );
    match api.help_farm(host_gid, target.clone()).await {
        Ok(confirmed_lands) => {
            let confirmed_ids: Vec<i64> = confirmed_lands.iter().map(|l| l.id).collect();
            recent_help.mark(
                host_gid,
                &confirmed_ids,
                HelpState::Confirmed,
                HELP_RESULT_TTL_MS,
                snapshot_key,
                now_ms(),
            );
            // 释放未确认的
            let unconfirmed: Vec<i64> = target
                .iter()
                .copied()
                .filter(|id| !confirmed_ids.contains(id))
                .collect();
            recent_help.release(host_gid, &unconfirmed);
            FarmingOutcome {
                effect: FarmingEffect::Confirmed,
                operation_count: confirmed_ids.len() as i64,
                land_count: confirmed_ids.len(),
                land_ids: confirmed_ids,
                operation_limits: Vec::new(),
                code: 0,
            }
        }
        Err(_) => {
            recent_help.release(host_gid, &target);
            let mut outcomes = Vec::new();
            for land_id in target {
                recent_help.mark(
                    host_gid,
                    &[land_id],
                    HelpState::InFlight,
                    HELP_IN_FLIGHT_TTL_MS,
                    snapshot_key,
                    now_ms(),
                );
                let outcome = match api.help_farm(host_gid, vec![land_id]).await {
                    Ok(land) => FarmingOutcome {
                        effect: FarmingEffect::Confirmed,
                        operation_count: land.len() as i64,
                        land_count: land.len(),
                        land_ids: land.iter().map(|l| l.id).collect(),
                        operation_limits: Vec::new(),
                        code: 0,
                    },
                    Err(_) => {
                        recent_help.release(host_gid, &[land_id]);
                        empty_farming_outcome(FarmingEffect::Uncertain)
                    }
                };
                outcomes.push(outcome);
                sleep(Duration::from_millis(100)).await;
            }
            merge_farming_outcomes(&outcomes)
        }
    }
}

// ============ 拜访好友主流程 ============

/// 拜访好友（帮 + 偷 + 捣乱，按 automation flag 分派）
pub async fn visit_friend(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    friend: &FriendSummary,
    total_actions: &mut TotalActions,
    my_gid: i64,
    account_id: &str,
) -> VisitResult {
    use crate::services::automation::is_automation_on;
    let friend_gid = friend.gid;
    let friend_name = friend.name.clone();

    // 1. enter
    let enter_reply = match api.enter_farm(friend_gid).await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("{e}");
            let kind = handle_friend_enter_error(friend_gid, &friend_name, &msg);
            if kind != FriendEnterErrorKind::Error {
                return VisitResult {
                    acted: false,
                    entered: false,
                };
            }
            tracing::warn!(friend_gid, error = %msg, "进入好友农场失败");
            return VisitResult {
                acted: false,
                entered: false,
            };
        }
    };

    let lands = enter_reply.lands.clone();
    if lands.is_empty() {
        let _ = api.leave_farm(friend_gid).await;
        return VisitResult {
            acted: false,
            entered: true,
        };
    }

    let plant_blacklist = get_plant_blacklist(account_id);
    let friend_blacklist = get_account_friend_blacklist(account_id);
    if friend_blacklist.contains(&friend_gid) {
        let _ = api.leave_farm(friend_gid).await;
        return VisitResult {
            acted: false,
            entered: true,
        };
    }
    let status = analyze_friend_lands(&lands, my_gid, &plant_blacklist, false);
    let snapshot_key = RecentHelpCache::make_snapshot_key(
        &lands.iter().map(LandSnapshot::from_land).collect::<Vec<_>>(),
    );

    let mut actions: Vec<String> = Vec::new();

    // 1. 帮助操作（锄草/除虫/浇水）
    let help_enabled = is_automation_on("friend_help");
    if help_enabled {
        let all_help_ids: Vec<i64> = status
            .need_weed
            .iter()
            .chain(status.need_bug.iter())
            .chain(status.need_water.iter())
            .copied()
            .collect::<HashSet<i64>>()
            .into_iter()
            .collect();
        if !all_help_ids.is_empty() {
            let outcome = run_farming_with_fallback(
                api,
                recent_help,
                friend_gid,
                &all_help_ids,
                false,
                &snapshot_key,
            )
            .await;
            if outcome.land_count > 0 {
                let mut parts = Vec::new();
                if !status.need_weed.is_empty() {
                    parts.push(format!("草{}", status.need_weed.len()));
                }
                if !status.need_bug.is_empty() {
                    parts.push(format!("虫{}", status.need_bug.len()));
                }
                if !status.need_water.is_empty() {
                    parts.push(format!("水{}", status.need_water.len()));
                }
                actions.push(format!(
                    "一键务农{}块/{}项({})",
                    outcome.land_count,
                    outcome.operation_count,
                    parts.join("/")
                ));
                total_actions.farming += outcome.land_count;
            }
        }
    }

    // 2. 偷菜操作
    if is_automation_on("friend_steal") && !status.stealable.is_empty() {
        let steal_result = steal_lands_with_reward_log(
            api,
            recent_help,
            friend_gid,
            &status.stealable,
            &status.stealable_info,
            None,
        )
        .await;
        if steal_result.ok > 0 {
            let plant_names: Vec<String> = steal_result
                .stolen_infos
                .iter()
                .filter_map(|i| {
                    if i.name.is_empty() {
                        None
                    } else {
                        Some(i.name.clone())
                    }
                })
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            let score_hint = if steal_result.score_gained > 0 {
                format!("+积分x{}", steal_result.score_gained)
            } else {
                String::new()
            };
            actions.push(format!(
                "偷{}{}{}",
                steal_result.ok,
                if plant_names.is_empty() {
                    String::new()
                } else {
                    format!("({})", plant_names.join("/"))
                },
                score_hint
            ));
            total_actions.steal += steal_result.ok;
        }
    }

    // 3. 捣乱（放草 + 放虫）
    if is_automation_on("friend_bad")
        && (!status.can_put_weed.is_empty() || !status.can_put_bug.is_empty())
    {
        if !status.can_put_weed.is_empty() {
            let n = api.put_weeds(friend_gid, status.can_put_weed.clone()).await.unwrap_or(0);
            if n > 0 {
                actions.push(format!("放草{n}"));
                total_actions.put_weed += n;
            }
        }
        if !status.can_put_bug.is_empty() {
            let n = api.put_insects(friend_gid, status.can_put_bug.clone()).await.unwrap_or(0);
            if n > 0 {
                actions.push(format!("放虫{n}"));
                total_actions.put_bug += n;
            }
        }
    }

    if !actions.is_empty() {
        tracing::info!(
            friend_gid,
            friend_name = %friend_name,
            actions = ?actions,
            "完成好友拜访"
        );
        crate::services::panel_log::log(
            account_id,
            "好友",
            format!("{friend_name}: {}", actions.join("/")),
            Some(serde_json::json!({
                "module": "friend",
                "event": "照顾好友",
                "friendName": friend_name,
                "friendGid": friend_gid,
                "actions": actions,
            })),
        );
    }

    let _ = api.leave_farm(friend_gid).await;
    VisitResult {
        acted: !actions.is_empty(),
        entered: true,
    }
}

/// 拜访好友 - 仅偷菜
pub async fn visit_friend_for_steal(
    api: &FriendApi,
    _recent_help: &RecentHelpCache,
    friend: &FriendSummary,
    total_actions: &mut TotalActions,
    my_gid: i64,
    account_id: &str,
) -> Option<VisitResult> {
    let friend_gid = friend.gid;
    let friend_name = friend.name.clone();

    let enter_reply = match api.enter_farm(friend_gid).await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("{e}");
            let kind = handle_friend_enter_error(friend_gid, &friend_name, &msg);
            if kind != FriendEnterErrorKind::Error {
                return Some(VisitResult {
                    acted: false,
                    entered: false,
                });
            }
            return Some(VisitResult {
                acted: false,
                entered: false,
            });
        }
    };

    let lands = enter_reply.lands.clone();
    if lands.is_empty() {
        let _ = api.leave_farm(friend_gid).await;
        return Some(VisitResult {
            acted: false,
            entered: true,
        });
    }

    let plant_blacklist =
        crate::models::store::account_config::get_plant_blacklist(Some(account_id));
    let has_stealable_before_filter = lands.iter().any(|land| {
        if is_occupied_slave_land(land) {
            return false;
        }
        let plant = match land.plant.as_ref() {
            Some(p) if !p.phases.is_empty() && p.stealable => p,
            _ => return false,
        };
        matches!(get_current_phase(land), Some(PlantPhase::Ripe)) && {
            let _ = plant;
            true
        }
    });
    let status = analyze_friend_lands(&lands, my_gid, &plant_blacklist, false);

    if has_stealable_before_filter && status.stealable.is_empty() {
        let _ = api.leave_farm(friend_gid).await;
        return None;
    }

    let mut actions: Vec<String> = Vec::new();
    if !status.stealable.is_empty() {
        let steal_result = steal_lands_with_reward_log(
            api,
            _recent_help,
            friend_gid,
            &status.stealable,
            &status.stealable_info,
            None,
        )
        .await;
        if steal_result.ok > 0 {
            let plant_names: Vec<String> = steal_result
                .stolen_infos
                .iter()
                .map(|i| i.name.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            let names = plant_names.join("/");
            actions.push(if names.is_empty() {
                format!("偷{}", steal_result.ok)
            } else {
                format!("偷{}({names})", steal_result.ok)
            });
            total_actions.steal += steal_result.ok;
            crate::services::stats::record_operation_for(
                account_id,
                "steal",
                steal_result.ok as i64,
            );
            crate::utils::random::random_delay(500, 800).await;
        }
    }

    if !actions.is_empty() {
        crate::services::panel_log::log(
            account_id,
            "好友",
            format!("{}: {}", friend_name, actions.join("/")),
            Some(serde_json::json!({
                "module": "friend",
                "event": "visit_friend",
                "result": "ok",
                "friendName": friend_name,
                "friendGid": friend_gid,
                "actions": actions,
            })),
        );
    }

    let _ = api.leave_farm(friend_gid).await;
    Some(VisitResult {
        acted: !actions.is_empty(),
        entered: true,
    })
}

/// 拜访好友 - 仅帮助
pub async fn visit_friend_for_help(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    friend: &FriendSummary,
    total_actions: &mut TotalActions,
    _my_gid: i64,
    _account_id: &str,
    ignore_exp_limit: bool,
    help_auto_disabled: &std::sync::atomic::AtomicBool,
) -> Option<VisitResult> {
    let friend_gid = friend.gid;
    let friend_name = friend.name.clone();
    let stop_when_exp_limit =
        crate::services::automation::is_automation_on_for(_account_id, "friend_help_exp_limit")
            && !ignore_exp_limit;
    if stop_when_exp_limit && help_auto_disabled.load(std::sync::atomic::Ordering::Acquire) {
        return Some(VisitResult {
            acted: false,
            entered: false,
        });
    }

    let enter_reply = match api.enter_farm(friend_gid).await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("{e}");
            let kind = handle_friend_enter_error(friend_gid, &friend_name, &msg);
            if kind != FriendEnterErrorKind::Error {
                return Some(VisitResult {
                    acted: false,
                    entered: false,
                });
            }
            return Some(VisitResult {
                acted: false,
                entered: false,
            });
        }
    };

    let lands = enter_reply.lands.clone();
    if lands.is_empty() {
        let _ = api.leave_farm(friend_gid).await;
        return Some(VisitResult {
            acted: false,
            entered: true,
        });
    }

    let status = analyze_friend_lands(&lands, _my_gid, &[], false);
    let snapshot_key = RecentHelpCache::make_snapshot_key(
        &lands.iter().map(LandSnapshot::from_land).collect::<Vec<_>>(),
    );

    let mut actions: Vec<String> = Vec::new();
    let all_help_ids: Vec<i64> = status
        .need_weed
        .iter()
        .chain(status.need_bug.iter())
        .chain(status.need_water.iter())
        .copied()
        .collect::<HashSet<i64>>()
        .into_iter()
        .collect();
    if !all_help_ids.is_empty() {
        let before_exp = crate::services::status::status_data_for(_account_id).exp;
        let outcome = run_farming_with_fallback(
            api,
            recent_help,
            friend_gid,
            &all_help_ids,
            stop_when_exp_limit,
            &snapshot_key,
        )
        .await;
        if outcome.land_count > 0 {
            actions.push(format!("帮{}块", outcome.land_count));
            total_actions.farming += outcome.land_count;
            crate::services::stats::record_operation_for(
                _account_id,
                "helpFarming",
                outcome.land_count as i64,
            );
            if stop_when_exp_limit {
                tokio::time::sleep(Duration::from_millis(200)).await;
                let after_exp = crate::services::status::status_data_for(_account_id).exp;
                if after_exp <= before_exp {
                    help_auto_disabled.store(true, std::sync::atomic::Ordering::Release);
                    crate::services::panel_log::log(
                        _account_id,
                        "好友",
                        "今日帮助经验已达上限，自动停止帮忙",
                        Some(serde_json::json!({
                            "module": "friend",
                            "event": "friend_cycle",
                            "result": "ok",
                        })),
                    );
                }
            }
        }
    }

    if !actions.is_empty() {
        crate::services::panel_log::log(
            _account_id,
            "好友",
            format!("{}: {}", friend_name, actions.join("/")),
            Some(serde_json::json!({
                "module": "friend",
                "event": "visit_friend",
                "result": "ok",
                "friendName": friend_name,
                "friendGid": friend_gid,
                "actions": actions,
            })),
        );
    }

    let _ = api.leave_farm(friend_gid).await;
    Some(VisitResult {
        acted: !actions.is_empty(),
        entered: true,
    })
}

/// 总操作计数器（与原 TS `totalActions` 一致）
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct TotalActions {
    pub farming: usize,
    pub steal: usize,
    pub put_weed: usize,
    pub put_bug: usize,
}

// ============ 偷菜（含积分收集 + 自动出售） ============

/// 偷好友菜（带积分收集 + 推送缩减 + 重试）
pub async fn steal_lands_with_reward_log(
    api: &FriendApi,
    _recent_help: &RecentHelpCache,
    friend_gid: i64,
    land_ids: &[i64],
    stealable_info: &[StealableInfo],
    _session: Option<()>,
) -> StealResult {
    let mut result = StealResult::default();
    if land_ids.is_empty() {
        return result;
    }
    let pending: Vec<i64> = land_ids.to_vec();
    let info_list: Vec<StealableInfo> = stealable_info.to_vec();
    let mut pending_ref: Vec<i64> = pending.clone();
    let info_list_ref: Vec<StealableInfo> = info_list.clone();

    // 第一次尝试
    match api.steal_farm(friend_gid, pending_ref.clone()).await {
        Ok(()) => {
            result.ok = pending_ref.len();
            result.stolen_infos = info_list_ref.clone();
            return result;
        }
        Err(_) => {
            // 失败：逐块重试
            let to_retry = pending_ref.clone();
            for land_id in to_retry {
                match api.steal_farm(friend_gid, vec![land_id]).await {
                    Ok(()) => {
                        result.ok += 1;
                        if let Some(info) = info_list_ref.iter().find(|i| i.land_id == land_id) {
                            result.stolen_infos.push(info.clone());
                        }
                    }
                    Err(_) => {
                        // 不可偷，移除
                        pending_ref.retain(|&x| x != land_id);
                    }
                }
                sleep(Duration::from_millis(100)).await;
            }
        }
    }
    result
}

// ============ 面板手动好友操作 ============

/// 面板手动好友操作
///
/// op: `water` / `weed` / `bug` / `steal` / `farming` / `bad`
pub async fn do_friend_operation(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    friend_gid: i64,
    op: crate::models::types::FriendOperation,
) -> serde_json::Value {
    if friend_gid == 0 {
        return serde_json::json!({"ok": false, "message": "无效好友ID", "opType": op.as_str()});
    }

    let op_str = op.as_str();

    // 1. enter
    let enter_reply = match api.enter_farm(friend_gid).await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("{e}");
            let kind = handle_friend_enter_error(friend_gid, &format!("GID:{friend_gid}"), &msg);
            match kind {
                FriendEnterErrorKind::Blacklist => {
                    return serde_json::json!({"ok": true, "opType": op_str, "count": 0, "message": "好友已自动加入黑名单"});
                }
                FriendEnterErrorKind::InvalidRemoved => {
                    return serde_json::json!({"ok": true, "opType": op_str, "count": 0, "message": "好友 GID 已失效"});
                }
                FriendEnterErrorKind::Error => {
                    return serde_json::json!({"ok": false, "opType": op_str, "count": 0, "message": format!("进入好友农场失败: {msg}")});
                }
            }
        }
    };

    let result = match op {
        crate::models::types::FriendOperation::Steal => {
            do_steal_op(api, recent_help, friend_gid, &enter_reply.lands).await
        }
        crate::models::types::FriendOperation::Farming
        | crate::models::types::FriendOperation::Water
        | crate::models::types::FriendOperation::Weed
        | crate::models::types::FriendOperation::Insecticide => {
            do_farm_op(
                api,
                recent_help,
                friend_gid,
                op,
                &enter_reply.lands,
            )
            .await
        }
        crate::models::types::FriendOperation::Bad => {
            do_bad_op(api, friend_gid, &enter_reply.lands).await
        }
        crate::models::types::FriendOperation::Fertilize => {
            // 暂未对接 Fertilize 单地操作
            serde_json::json!({"ok": true, "opType": op_str, "count": 0, "message": "施肥功能暂未对接"})
        }
    };
    let _ = api.leave_farm(friend_gid).await;
    result
}

async fn do_steal_op(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    friend_gid: i64,
    lands: &[LandInfo],
) -> serde_json::Value {
    let status = analyze_friend_lands(lands, 0, &[], false);
    if status.stealable.is_empty() {
        return serde_json::json!({"ok": true, "opType": "steal", "count": 0, "message": "没有可偷取土地"});
    }
    let result = steal_lands_with_reward_log(
        api,
        recent_help,
        friend_gid,
        &status.stealable,
        &status.stealable_info,
        None,
    )
    .await;
    let msg = if result.ok > 0 {
        let score_hint = if result.score_gained > 0 {
            format!("，获得积分x{}", result.score_gained)
        } else {
            String::new()
        };
        format!("偷取完成 {} 块{}", result.ok, score_hint)
    } else {
        "偷取失败或无可偷".to_string()
    };
    serde_json::json!({"ok": true, "opType": "steal", "count": result.ok, "message": msg})
}

async fn do_farm_op(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    friend_gid: i64,
    op: crate::models::types::FriendOperation,
    lands: &[LandInfo],
) -> serde_json::Value {
    let status = analyze_friend_lands(lands, 0, &[], false);
    let land_ids: Vec<i64> = match op {
        crate::models::types::FriendOperation::Farming => status
            .need_weed
            .iter()
            .chain(status.need_bug.iter())
            .chain(status.need_water.iter())
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect(),
        crate::models::types::FriendOperation::Water => status.need_water,
        crate::models::types::FriendOperation::Weed => status.need_weed,
        crate::models::types::FriendOperation::Insecticide => status.need_bug,
        _ => Vec::new(),
    };
    if land_ids.is_empty() {
        return serde_json::json!({"ok": true, "opType": op.as_str(), "count": 0, "message": "没有需要照顾的土地"});
    }
    let snapshot_key = RecentHelpCache::make_snapshot_key(
        &lands.iter().map(LandSnapshot::from_land).collect::<Vec<_>>(),
    );
    let outcome = run_farming_with_fallback(
        api,
        recent_help,
        friend_gid,
        &land_ids,
        false,
        &snapshot_key,
    )
    .await;
    serde_json::json!({
        "ok": true,
        "opType": op.as_str(),
        "count": outcome.land_count,
        "landCount": outcome.land_count,
        "operationCount": outcome.operation_count,
        "message": format!("一键务农完成 {} 块 / {} 项操作", outcome.land_count, outcome.operation_count),
    })
}

pub async fn do_bad_op(
    api: &FriendApi,
    friend_gid: i64,
    lands: &[LandInfo],
) -> serde_json::Value {
    let status = analyze_friend_lands(lands, 0, &[], false);
    if status.can_put_bug.is_empty() && status.can_put_weed.is_empty() {
        return serde_json::json!({"ok": true, "opType": "bad", "count": 0, "bugCount": 0, "weedCount": 0, "message": "没有可捣乱土地"});
    }
    let weed_count = if !status.can_put_weed.is_empty() {
        api.put_weeds(friend_gid, status.can_put_weed.clone())
            .await
            .unwrap_or(0)
    } else {
        0
    };
    let bug_count = if !status.can_put_bug.is_empty() {
        api.put_insects(friend_gid, status.can_put_bug.clone())
            .await
            .unwrap_or(0)
    } else {
        0
    };
    serde_json::json!({
        "ok": true,
        "opType": "bad",
        "count": bug_count + weed_count,
        "bugCount": bug_count,
        "weedCount": weed_count,
        "message": format!("捣乱完成 虫{}/草{}", bug_count, weed_count),
    })
}

// ============ FriendApi 别名（用于上层调用） ============

/// 上层调用便利方法：使用 GidManager + FriendApi 拉取好友列表
pub async fn get_friends_list_via(
    api: &FriendApi,
    _gid_manager: &GidManager,
    my_gid: i64,
) -> Vec<FriendSummary> {
    let friends = match api.get_all_game_friends().await {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };
    friends
        .into_iter()
        .filter(|f| f.gid != my_gid && f.name != "小小农夫" && f.remark != "小小农夫")
        .map(game_friend_to_summary)
        .collect()
}

#[allow(dead_code)]
fn _silence_unused(_: &Arc<FriendApi>) {}

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
    fn game_friend_dto_has_name_avatar_plant() {
        let f = crate::proto::generated::gamepb::friendpb::GameFriend {
            gid: 9,
            name: "张三".into(),
            avatar_url: "http://avatar".into(),
            level: 10,
            gold: 100,
            plant: Some(crate::proto::generated::gamepb::friendpb::Plant {
                steal_plant_num: 2,
                dry_num: 1,
                weed_num: 0,
                insect_num: 3,
                ..Default::default()
            }),
            ..Default::default()
        };
        let v = serde_json::to_value(game_friend_to_summary(f)).unwrap();
        assert_eq!(v["gid"], 9);
        assert_eq!(v["name"], "张三");
        assert_eq!(v["avatarUrl"], "http://avatar");
        assert_eq!(v["plant"]["stealNum"], 2);
        assert_eq!(v["plant"]["dryNum"], 1);
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
        assert!(is_transient_network_error("request timeout: foo"));
        assert!(is_transient_network_error("连接关闭 (code=1006)"));
        assert!(is_transient_network_error("worker exited"));
        assert!(!is_transient_network_error("业务错误"));
        assert!(!is_transient_network_error(""));
    }

    // ===== 阶段 2E 补全测试 =====

    #[test]
    fn parse_time_to_minutes_basic() {
        assert_eq!(parse_time_to_minutes("00:00"), Some(0));
        assert_eq!(parse_time_to_minutes("12:30"), Some(12 * 60 + 30));
        assert_eq!(parse_time_to_minutes("23:59"), Some(23 * 60 + 59));
        assert_eq!(parse_time_to_minutes("24:00"), None);
        assert_eq!(parse_time_to_minutes("12:60"), None);
        assert_eq!(parse_time_to_minutes("12"), None);
        assert_eq!(parse_time_to_minutes(""), None);
    }

    #[test]
    fn in_friend_quiet_hours_disabled_by_default() {
        // 默认未启用
        *FRIEND_QUIET_HOURS.lock() = None;
        assert!(!in_friend_quiet_hours(Some((10, 0))));
    }

    #[test]
    fn in_friend_quiet_hours_within_window() {
        *FRIEND_QUIET_HOURS.lock() = Some(FriendQuietHours {
            enabled: true,
            start: "22:00".to_string(),
            end: "08:00".to_string(),
        });
        // 跨天：22:00 - 次日 08:00
        assert!(in_friend_quiet_hours(Some((23, 0))));
        assert!(in_friend_quiet_hours(Some((7, 30))));
        assert!(!in_friend_quiet_hours(Some((10, 0))));
    }

    #[test]
    fn in_friend_quiet_hours_same_window_means_all_day() {
        *FRIEND_QUIET_HOURS.lock() = Some(FriendQuietHours {
            enabled: true,
            start: "00:00".to_string(),
            end: "00:00".to_string(),
        });
        // 起止相同 → 全天静默
        assert!(in_friend_quiet_hours(Some((10, 0))));
    }

    #[test]
    fn blacklist_add_and_remove() {
        add_friend_to_blacklist(100, "alice", "test");
        assert!(is_in_blacklist(100));
        assert_eq!(blacklist_size(), 1);
        // 重复 add 不会增加
        add_friend_to_blacklist(100, "alice", "test");
        assert_eq!(blacklist_size(), 1);
        assert!(remove_from_blacklist(100));
        assert!(!is_in_blacklist(100));
    }

    #[test]
    fn blacklist_add_zero_returns_false() {
        assert!(!add_friend_to_blacklist(0, "zero", ""));
    }

    #[test]
    fn invalid_friend_access_error_basic() {
        assert!(!is_invalid_friend_access_error(""));
        // banned 错误不算
        assert!(!is_invalid_friend_access_error("code=1002003"));
        // transient 也不算
        assert!(!is_invalid_friend_access_error("连接未打开"));
        // 含 code=xxx + invalid 关键字
        assert!(is_invalid_friend_access_error("code=42 invalid friend"));
    }

    #[test]
    fn handle_friend_enter_error_classifies() {
        // 1002003 → blacklist
        let k = handle_friend_enter_error(200, "bob", "code=1002003");
        assert_eq!(k, FriendEnterErrorKind::Blacklist);
        assert!(is_in_blacklist(200));
        // invalid → InvalidRemoved
        let k2 = handle_friend_enter_error(300, "carol", "code=42 invalid friend");
        assert_eq!(k2, FriendEnterErrorKind::InvalidRemoved);
        // 普通 → Error
        let k3 = handle_friend_enter_error(400, "dave", "连接未打开");
        assert_eq!(k3, FriendEnterErrorKind::Error);
    }

    #[test]
    fn empty_farming_outcome_defaults() {
        let o = empty_farming_outcome(FarmingEffect::Noop);
        assert_eq!(o.effect, FarmingEffect::Noop);
        assert_eq!(o.land_count, 0);
        assert!(o.land_ids.is_empty());
    }

    #[test]
    fn merge_farming_outcomes_aggregates() {
        let outcomes = vec![
            FarmingOutcome {
                effect: FarmingEffect::Confirmed,
                operation_count: 2,
                land_count: 1,
                land_ids: vec![1],
                operation_limits: vec![],
                code: 0,
            },
            FarmingOutcome {
                effect: FarmingEffect::Confirmed,
                operation_count: 3,
                land_count: 1,
                land_ids: vec![2],
                operation_limits: vec![],
                code: 0,
            },
            FarmingOutcome {
                effect: FarmingEffect::Uncertain,
                operation_count: 0,
                land_count: 0,
                land_ids: vec![],
                operation_limits: vec![],
                code: 0,
            },
        ];
        let merged = merge_farming_outcomes(&outcomes);
        assert_eq!(merged.effect, FarmingEffect::Confirmed);
        assert_eq!(merged.operation_count, 5);
        assert_eq!(merged.land_count, 2);
        assert_eq!(merged.land_ids, vec![1, 2]);
    }

    #[test]
    fn merge_farming_outcomes_only_uncertain() {
        let outcomes = vec![FarmingOutcome {
            effect: FarmingEffect::Uncertain,
            operation_count: 0,
            land_count: 0,
            land_ids: vec![],
            operation_limits: vec![],
            code: 0,
        }];
        let merged = merge_farming_outcomes(&outcomes);
        assert_eq!(merged.effect, FarmingEffect::Uncertain);
    }

    #[test]
    fn merge_farming_outcomes_dedup_land_ids() {
        let outcomes = vec![
            FarmingOutcome {
                effect: FarmingEffect::Confirmed,
                operation_count: 1,
                land_count: 1,
                land_ids: vec![1, 2],
                operation_limits: vec![],
                code: 0,
            },
            FarmingOutcome {
                effect: FarmingEffect::Confirmed,
                operation_count: 1,
                land_count: 1,
                land_ids: vec![2, 3],
                operation_limits: vec![],
                code: 0,
            },
        ];
        let merged = merge_farming_outcomes(&outcomes);
        // 2 只出现 1 次（去重）
        assert_eq!(merged.land_ids, vec![1, 2, 3]);
    }

    #[test]
    fn plant_blacklist_per_account() {
        set_plant_blacklist("acc1", vec![100, 200]);
        set_plant_blacklist("acc2", vec![300]);
        assert_eq!(get_plant_blacklist("acc1"), vec![100, 200]);
        assert_eq!(get_plant_blacklist("acc2"), vec![300]);
        assert_eq!(get_plant_blacklist("acc3"), Vec::<i64>::new());
    }

    #[test]
    fn account_friend_blacklist_per_account() {
        set_account_friend_blacklist("acc1", vec![11, 22]);
        set_account_friend_blacklist("acc2", vec![33]);
        assert_eq!(get_account_friend_blacklist("acc1"), vec![11, 22]);
        assert_eq!(get_account_friend_blacklist("acc2"), vec![33]);
        assert_eq!(get_account_friend_blacklist("acc3"), Vec::<i64>::new());
    }

    #[test]
    fn friends_list_cache_ttl_basic() {
        let c = FriendsListCache::new();
        assert_eq!(c.get_ttl_ms(0), 60_000);
        assert_eq!(c.get_ttl_ms(120), 120_000);
        // 最小 10s
        assert_eq!(c.get_ttl_ms(1), 10_000);
    }

    #[test]
    fn is_activity_plant_unknown_returns_false() {
        // 没标记过的 plant_id → false
        use crate::proto::generated::gamepb::plantpb::PlantInfo;
        let land = LandInfo {
            id: 1,
            plant: Some(PlantInfo {
                id: 9999999,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(!is_activity_plant(&land));
    }

    #[test]
    fn mark_activity_plant_makes_it_active() {
        use crate::proto::generated::gamepb::plantpb::PlantInfo;
        mark_activity_plant(8888);
        let land = LandInfo {
            id: 1,
            plant: Some(PlantInfo {
                id: 8888,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(is_activity_plant(&land));
    }

    #[test]
    fn plant_phase_from_proto_mature_is_ripe() {
        assert_eq!(PlantPhase::from_i32(6), PlantPhase::Ripe);
        assert_eq!(PlantPhase::from_i32(7), PlantPhase::Dead);
        assert_eq!(PlantPhase::from_i32(3), PlantPhase::Growing);
        assert_eq!(PlantPhase::from_i32(1), PlantPhase::Seed);
    }
}
