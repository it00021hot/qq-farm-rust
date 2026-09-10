//! 好友护主犬缓存 —— 按天记录每位好友当前上场的狗，避免每轮巡查靠 Enter 试探。
//!
//! 对齐 bot `core/src/services/friend/pet-cache.ts`（HEAD=8dae528）。
//!
//! 数据只有一个来源：`VisitService.Enter` 回包的 `brief_dog_info.dog_id`
//! （visitpb.proto field 3），由 [`crate::services::friend::api::FriendApi::enter_farm`]
//! 统一写透（对应 bot `recordFriendDogFromEnterReply`，见 pet-cache.ts:135-140）。
//! 因此所有进入好友农场的调用都顺手写入这里（偷菜、帮忙、捣乱、宠物同步、
//! 面板手动操作），真正额外花 RPC 的只有 [`crate::services::friend::pet_sync`]
//! 的每日补齐。
//!
//! 新鲜度按「系统日期」（[`crate::utils::time::today_system_date_key`]）判定：
//! 好友随时可以换狗或让狗粮吃完，所以跨日的记录一律视为未知，
//! 由每日同步重新确认（跨日条目在加载与运行期都会被丢弃，文件不会无限增长）。
//!
//! 落盘文件：`friend-pet-<sha256(account_id)>.json`（模式同
//! `friend-bad-state-<hash>.json`）。写盘有 2 秒防抖（`FLUSH_DEBOUNCE_MS`），
//! 且只在结论变化或首次确认时才排队（pet-cache.ts:127-129）；
//! 停机前调用 [`flush_friend_pet_cache_now`] 立即落盘。

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::proto::generated::gamepb::visitpb::EnterReply;

/// 护主犬 ID（bot pet-cache.ts:18 `PROTECT_DOG_ID = 90021`）
pub const PROTECT_DOG_ID: i64 = 90021;

/// 缓存文件版本
const CACHE_VERSION: i64 = 1;

/// 写盘防抖（bot pet-cache.ts:21 `FLUSH_DEBOUNCE_MS = 2000`）
pub const FLUSH_DEBOUNCE_MS: u64 = 2000;

/// 三态判定（bot pet-cache.ts:24）：
/// 当天确认上场护主犬 / 上场了别的狗或没有上场狗 / 今天还没确认过
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendDogState {
    /// 当天确认上场的是护主犬
    Protect,
    /// 当天确认上场的是别的狗，或没有上场狗
    Other,
    /// 今天还没确认过（含跨日作废）
    Unknown,
}

/// 单条缓存（bot `FriendDogEntry`，pet-cache.ts:26-30）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DogEntry {
    dog_id: i64,
    date: String,
    checked_at: i64,
}

/// 落盘结构：`{version, last_full_sync_date, entries: {gid: entry}}`
#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheFile {
    version: i64,
    #[serde(default)]
    last_full_sync_date: String,
    #[serde(default)]
    entries: HashMap<String, DogEntry>,
}

/// 账号内存态（进程内 per-account）
#[derive(Debug, Default)]
struct AccountState {
    entries: HashMap<i64, DogEntry>,
    last_full_sync_date: String,
    /// 有未落盘的变更
    dirty: bool,
    /// 防抖代际：新一次排队会让旧任务失效，一串变更只落一次盘
    flush_seq: u64,
}

fn accounts() -> &'static Mutex<HashMap<String, Arc<Mutex<AccountState>>>> {
    static MAP: OnceLock<Mutex<HashMap<String, Arc<Mutex<AccountState>>>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_file_path(account_id: &str) -> std::path::PathBuf {
    use sha2::{Digest, Sha256};
    let key = if account_id.is_empty() { "default" } else { account_id };
    let token = hex::encode(Sha256::digest(key.as_bytes()));
    crate::config::paths::get_data_file(&format!("friend-pet-{token}.json"))
}

/// 惰性加载：首次访问该账号时读文件；跨日条目加载时直接丢掉；
/// 解析失败按空处理并 log warn（bot pet-cache.ts:49-78）。
fn load_state(account_id: &str) -> Arc<Mutex<AccountState>> {
    let key = account_id.to_string();
    if let Some(state) = accounts().lock().get(&key) {
        return state.clone();
    }
    let state = Arc::new(Mutex::new(AccountState::default()));
    {
        let today = crate::utils::time::today_system_date_key();
        let path = cache_file_path(&key);
        let file_existed = path.exists();
        // 解析失败/文件缺失都会走 fallback；用标记区分出「文件损坏」告警
        let mut used_fallback = false;
        let file = crate::services::json_db::read_json_with_default::<CacheFile, _>(&path, || {
            used_fallback = true;
            CacheFile::default()
        });
        if used_fallback && file_existed {
            tracing::warn!(account_id = %key, "读取好友宠物缓存失败，按空缓存处理");
        }
        if file.version == CACHE_VERSION {
            let mut guard = state.lock();
            for (raw_gid, entry) in file.entries {
                let Ok(gid) = raw_gid.parse::<i64>() else {
                    continue;
                };
                if gid <= 0 || entry.date != today {
                    // 跨日记录没有价值，加载时直接丢掉（pet-cache.ts:62-63）
                    continue;
                }
                guard.entries.insert(gid, entry);
            }
            // 跨日的全量同步标记同样作废（pet-cache.ts:75）
            if file.last_full_sync_date == today {
                guard.last_full_sync_date = file.last_full_sync_date;
            }
        }
    }
    accounts().lock().insert(key, state.clone());
    state
}

/// 立即把内存态原子写入文件（bot pet-cache.ts:80-93 `flushCache`）
fn flush_locked(account_id: &str, state: &AccountState) {
    let file = CacheFile {
        version: CACHE_VERSION,
        last_full_sync_date: state.last_full_sync_date.clone(),
        entries: state.entries.iter().map(|(gid, e)| (gid.to_string(), e.clone())).collect(),
    };
    if let Err(e) =
        crate::services::json_db::write_json_file_atomic(cache_file_path(account_id), &file)
    {
        tracing::warn!(account_id, error = %e, "保存好友宠物缓存失败");
    }
}

/// 排队一次防抖落盘：tokio 上下文里 2 秒后写盘；无运行时（纯同步测试）只标脏，
/// 由 [`flush_friend_pet_cache_now`] 兜底（bot pet-cache.ts:95-97 `scheduleFlush`）。
fn schedule_flush(account_id: &str, state: &Arc<Mutex<AccountState>>) {
    let seq = {
        let mut guard = state.lock();
        guard.dirty = true;
        guard.flush_seq += 1;
        guard.flush_seq
    };
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let state = state.clone();
    let key = account_id.to_string();
    handle.spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(FLUSH_DEBOUNCE_MS)).await;
        let mut guard = state.lock();
        if guard.dirty && guard.flush_seq == seq {
            flush_locked(&key, &guard);
            guard.dirty = false;
        }
    });
}

/// 跨日清理：丢掉不是今天的条目与全量同步标记（bot pet-cache.ts:99-114）。
pub fn drop_stale_entries(account_id: &str) {
    let state = load_state(account_id);
    let today = crate::utils::time::today_system_date_key();
    let changed = {
        let mut guard = state.lock();
        let before = guard.entries.len();
        guard.entries.retain(|_, entry| entry.date == today);
        let date_cleared =
            !guard.last_full_sync_date.is_empty() && guard.last_full_sync_date != today;
        if date_cleared {
            guard.last_full_sync_date.clear();
        }
        guard.entries.len() != before || date_cleared
    };
    if changed {
        schedule_flush(account_id, &state);
    }
}

/// 记录一位好友当前上场的狗；`dog_id == 0` 表示没有上场狗，同样是有效结论
/// （bot pet-cache.ts:119-130）。write-through：仅当结论变化或首次确认才写盘。
pub fn record_friend_dog(account_id: &str, gid: i64, dog_id: i64) {
    if gid <= 0 {
        return;
    }
    drop_stale_entries(account_id);
    let state = load_state(account_id);
    let today = crate::utils::time::today_system_date_key();
    let next_dog_id = dog_id.max(0);
    let changed = {
        let mut guard = state.lock();
        let previous = guard.entries.get(&gid);
        let changed = previous.is_none() || previous.is_some_and(|e| e.dog_id != next_dog_id);
        guard
            .entries
            .insert(gid, DogEntry { dog_id: next_dog_id, date: today, checked_at: now_ms_i64() });
        changed
    };
    if changed {
        schedule_flush(account_id, &state);
    }
}

/// 从 Enter 回包顺手记录（零额外 RPC）：没有上场狗时服务端不下发
/// `brief_dog_info`，缺省即 dog_id 0（bot pet-cache.ts:135-140）。
pub fn record_friend_dog_from_enter_reply(account_id: &str, gid: i64, enter_reply: &EnterReply) {
    let dog_id = enter_reply.brief_dog_info.as_ref().map_or(0, |d| d.dog_id);
    record_friend_dog(account_id, gid, dog_id);
}

/// 三态判定（bot pet-cache.ts:142-149）
#[must_use]
pub fn get_friend_dog_state(account_id: &str, gid: i64) -> FriendDogState {
    if gid <= 0 {
        return FriendDogState::Unknown;
    }
    drop_stale_entries(account_id);
    let state = load_state(account_id);
    let dog_id = state.lock().entries.get(&gid).map_or(-1_i64, |e| e.dog_id);
    if dog_id < 0 {
        FriendDogState::Unknown
    } else if dog_id == PROTECT_DOG_ID {
        FriendDogState::Protect
    } else {
        FriendDogState::Other
    }
}

/// 当天是否已有结论（含「没有上场狗」这一结论）
#[must_use]
pub fn is_friend_dog_known_today(account_id: &str, gid: i64) -> bool {
    get_friend_dog_state(account_id, gid) != FriendDogState::Unknown
}

/// 当天上场的狗 ID；未知返回 0
#[must_use]
pub fn get_friend_dog_id(account_id: &str, gid: i64) -> i64 {
    if gid <= 0 {
        return 0;
    }
    drop_stale_entries(account_id);
    let state = load_state(account_id);
    let dog_id = state.lock().entries.get(&gid).map_or(0, |e| e.dog_id);
    dog_id
}

/// 当天全量同步是否已完成（bot pet-cache.ts:168-171）
#[must_use]
pub fn is_full_sync_done_today(account_id: &str) -> bool {
    drop_stale_entries(account_id);
    let state = load_state(account_id);
    let done = state.lock().last_full_sync_date == crate::utils::time::today_system_date_key();
    done
}

/// 停机前把防抖里的待写落盘，避免丢掉当天已确认的结论
/// （bot pet-cache.ts:176-179 `flushFriendPetCacheNow`）。
pub fn flush_friend_pet_cache_now(account_id: &str) {
    let state = load_state(account_id);
    let mut guard = state.lock();
    flush_locked(account_id, &guard);
    guard.dirty = false;
}

/// 标记当天全量同步完成（bot pet-cache.ts:181-184，立即落盘）
pub fn mark_full_sync_done(account_id: &str) {
    let state = load_state(account_id);
    let mut guard = state.lock();
    guard.last_full_sync_date = crate::utils::time::today_system_date_key();
    flush_locked(account_id, &guard);
    guard.dirty = false;
}

/// 缓存统计（对齐 bot `getFriendPetCacheStats`，pet-cache.ts:186-199；
/// 返回 `{date, known, protect, fullSyncDone}`，serde camelCase）。
#[must_use]
pub fn get_friend_pet_cache_stats(account_id: &str) -> serde_json::Value {
    drop_stale_entries(account_id);
    let state = load_state(account_id);
    let guard = state.lock();
    let protect = guard.entries.values().filter(|e| e.dog_id == PROTECT_DOG_ID).count();
    serde_json::json!({
        "date": crate::utils::time::today_system_date_key(),
        "known": guard.entries.len(),
        "protect": protect,
        "fullSyncDone": guard.last_full_sync_date == crate::utils::time::today_system_date_key(),
    })
}

/// 仅供断开与测试使用：丢掉内存态，下次访问重新从文件加载（文件保留，
/// bot pet-cache.ts:204-208 `resetFriendPetCacheMemory`）。
pub fn clear_friend_pet_cache(account_id: &str) {
    accounts().lock().remove(account_id);
}

fn now_ms_i64() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 临时 FARM_DATA_DIR 隔离数据目录；Drop 恢复环境变量。
    /// `#[serial(farm_data_dir)]` 与仓库内其它数据目录测试互斥。
    struct DataDirGuard(Option<String>);
    impl Drop for DataDirGuard {
        fn drop(&mut self) {
            match self.0.clone() {
                Some(v) => std::env::set_var("FARM_DATA_DIR", v),
                None => std::env::remove_var("FARM_DATA_DIR"),
            }
        }
    }

    fn temp_data_dir() -> DataDirGuard {
        let tmp = std::env::temp_dir().join(format!(
            "qq-farm-pet-cache-{}-{}",
            std::process::id(),
            crate::services::friend::visit_strategy::now_ms()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("mkdir");
        let prev = std::env::var("FARM_DATA_DIR").ok();
        std::env::set_var("FARM_DATA_DIR", &tmp);
        DataDirGuard(prev)
    }

    fn fresh_account() -> String {
        format!("pet-cache-test-{}", crate::services::friend::visit_strategy::now_ms())
    }

    #[test]
    #[serial_test::serial(farm_data_dir)]
    fn tri_state_and_write_through() {
        let _dir = temp_data_dir();
        let acc = fresh_account();
        assert_eq!(get_friend_dog_state(&acc, 100), FriendDogState::Unknown);
        // dog_id 0 也是有效结论 = 无上场狗
        record_friend_dog(&acc, 100, 0);
        assert_eq!(get_friend_dog_state(&acc, 100), FriendDogState::Other);
        assert!(is_friend_dog_known_today(&acc, 100));
        record_friend_dog(&acc, 200, PROTECT_DOG_ID);
        assert_eq!(get_friend_dog_state(&acc, 200), FriendDogState::Protect);
        assert_eq!(get_friend_dog_id(&acc, 200), PROTECT_DOG_ID);
        // gid<=0 一律未知
        assert_eq!(get_friend_dog_state(&acc, 0), FriendDogState::Unknown);
    }

    #[test]
    #[serial_test::serial(farm_data_dir)]
    fn dedupe_only_writes_on_change() {
        let _dir = temp_data_dir();
        let acc = fresh_account();
        record_friend_dog(&acc, 100, PROTECT_DOG_ID);
        flush_friend_pet_cache_now(&acc);
        assert!(!load_state(&acc).lock().dirty);
        // 同一天狗没变：不再排队落盘（bot pet-cache.ts:128-129）
        record_friend_dog(&acc, 100, PROTECT_DOG_ID);
        assert!(!load_state(&acc).lock().dirty, "unchanged dog must not dirty cache");
        // 结论变化：排队
        record_friend_dog(&acc, 100, 0);
        assert!(load_state(&acc).lock().dirty);
        flush_friend_pet_cache_now(&acc);
        assert_eq!(get_friend_dog_state(&acc, 100), FriendDogState::Other);
    }

    #[test]
    #[serial_test::serial(farm_data_dir)]
    fn persist_and_reload() {
        let _dir = temp_data_dir();
        let acc = fresh_account();
        record_friend_dog(&acc, 100, PROTECT_DOG_ID);
        record_friend_dog(&acc, 200, 12_345);
        mark_full_sync_done(&acc);
        // 清内存后从文件重新加载（文件保留）
        clear_friend_pet_cache(&acc);
        assert_eq!(get_friend_dog_state(&acc, 100), FriendDogState::Protect);
        assert_eq!(get_friend_dog_state(&acc, 200), FriendDogState::Other);
        assert!(is_full_sync_done_today(&acc));
        let stats = get_friend_pet_cache_stats(&acc);
        assert_eq!(stats["known"], 2);
        assert_eq!(stats["protect"], 1);
        assert_eq!(stats["fullSyncDone"], true);
    }

    #[test]
    #[serial_test::serial(farm_data_dir)]
    fn corrupt_file_treated_as_empty() {
        let _dir = temp_data_dir();
        let acc = fresh_account();
        let path = cache_file_path(&acc);
        std::fs::write(&path, b"{not-json").expect("write");
        assert_eq!(get_friend_dog_state(&acc, 100), FriendDogState::Unknown);
        let stats = get_friend_pet_cache_stats(&acc);
        assert_eq!(stats["known"], 0);
    }

    #[test]
    #[serial_test::serial(farm_data_dir)]
    fn stale_entries_dropped_on_date_key_mismatch() {
        let _dir = temp_data_dir();
        let acc = fresh_account();
        let state = load_state(&acc);
        {
            let mut guard = state.lock();
            let today = crate::utils::time::today_system_date_key();
            let yesterday = if today.starts_with("20") { "1999-01-01".to_string() } else { today };
            guard.entries.insert(
                300,
                DogEntry { dog_id: PROTECT_DOG_ID, date: yesterday.clone(), checked_at: 0 },
            );
            guard.last_full_sync_date = yesterday;
        }
        drop_stale_entries(&acc);
        let guard = state.lock();
        assert!(!guard.entries.contains_key(&300), "stale entry must be dropped");
        assert!(guard.last_full_sync_date.is_empty());
    }

    #[test]
    #[serial_test::serial(farm_data_dir)]
    fn stats_shape_camel_case() {
        let _dir = temp_data_dir();
        let acc = fresh_account();
        let stats = get_friend_pet_cache_stats(&acc);
        assert!(stats.get("fullSyncDone").is_some(), "camelCase key expected");
        assert!(stats.get("date").is_some());
    }
}
