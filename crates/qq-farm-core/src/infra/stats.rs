//! 每日统计 — 收获/种植/施肥/偷菜等操作计数 + 持久化。
//!
//! 1:1 翻译原 `core/src/services/stats.ts`（317 行）。
//!
//! 数据存储：`{data_dir}/stats/{accountId}.json`
//!
//! 跨天自动重置每日统计；金/经验变化计算 session 增量。

use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::config::paths::get_data_dir;

/// 操作计数器
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationsMap {
    pub harvest: i64,
    pub farming: i64,
    pub fertilize: i64,
    pub plant: i64,
    pub steal: i64,
    #[serde(alias = "help_farming")]
    pub help_farming: i64,
    #[serde(alias = "task_claim")]
    pub task_claim: i64,
    pub sell: i64,
    pub upgrade: i64,
    #[serde(alias = "level_up")]
    pub level_up: i64,
}

impl OperationsMap {
    pub fn fields() -> Vec<&'static str> {
        vec![
            "harvest",
            "farming",
            "fertilize",
            "plant",
            "steal",
            "helpFarming",
            "taskClaim",
            "sell",
            "upgrade",
            "levelUp",
        ]
    }

    pub fn reset(&mut self) {
        self.harvest = 0;
        self.farming = 0;
        self.fertilize = 0;
        self.plant = 0;
        self.steal = 0;
        self.help_farming = 0;
        self.task_claim = 0;
        self.sell = 0;
        self.upgrade = 0;
        self.level_up = 0;
    }
}

/// 最后一次状态（金/经验/点券）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LastState {
    pub gold: i64,
    pub exp: i64,
    pub coupon: i64,
}

/// 初始状态（用于计算 session 增量）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InitialState {
    pub gold: Option<i64>,
    pub exp: Option<i64>,
    pub coupon: Option<i64>,
}

/// Session 数据
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionData {
    pub gold_gained: i64,
    pub exp_gained: i64,
    pub coupon_gained: i64,
    pub last_exp_gain: i64,
    pub last_gold_gain: i64,
    pub last_exp_time: Option<i64>,
}

/// 持久化数据结构
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedStats {
    pub date: String,
    #[serde(default)]
    pub operations: OperationsMap,
    #[serde(default)]
    pub initial_state: InitialState,
    /// 累计偷菜数（跨会话；每天也归入 persisted）
    #[serde(default)]
    pub total_steal: i64,
    pub saved_at: i64,
}

// =====================================================================
// 全局状态
// =====================================================================

// =====================================================================
// 全局状态（全部按 account_id 分槽；CLI/单测走 LEGACY_SLOT）
// =====================================================================

/// 无账号上下文时的兼容槽（不再表示「当前登录账号」）
const LEGACY_SLOT: &str = "_";

#[derive(Clone)]
struct StatsSlot {
    operations: OperationsMap,
    last: LastState,
    initial: InitialState,
    session: SessionData,
    date_key: Option<String>,
}

impl Default for StatsSlot {
    fn default() -> Self {
        Self {
            operations: OperationsMap::default(),
            last: LastState { gold: -1, exp: -1, coupon: -1 },
            initial: InitialState::default(),
            session: SessionData::default(),
            date_key: None,
        }
    }
}

static SLOTS: LazyLock<Mutex<std::collections::HashMap<String, StatsSlot>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

fn slot_for(account_id: &str) -> StatsSlot {
    SLOTS.lock().entry(account_id.to_string()).or_default().clone()
}

fn put_slot(account_id: &str, slot: StatsSlot) {
    SLOTS.lock().insert(account_id.to_string(), slot);
}

// =====================================================================
// 文件路径
// =====================================================================

#[must_use]
pub fn stats_file(account_id: &str) -> PathBuf {
    let dir =
        std::env::var("FARM_DATA_DIR").ok().map(PathBuf::from).unwrap_or_else(|| get_data_dir());
    dir.join("stats").join(format!("{account_id}.json"))
}

// =====================================================================
// 持久化
// =====================================================================

pub fn load_persisted_stats(account_id: &str) -> Option<PersistedStats> {
    let path = stats_file(account_id);
    let raw = fs::read_to_string(&path).ok()?;
    if raw.trim().is_empty() {
        return None;
    }
    serde_json::from_str(&raw).ok()
}

pub fn save_persisted_stats(account_id: &str, data: &PersistedStats) {
    let path = stats_file(account_id);
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }
    if let Ok(body) = serde_json::to_string_pretty(data) {
        let pid = std::process::id();
        let ts = crate::utils::time::now_ms();
        let tmp = path.with_file_name(format!(
            "{}.{pid}.{ts}.tmp",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("stats.json")
        ));
        let _ = crate::infra::spawn_blocking(move || {
            let _ = fs::write(&tmp, &body);
            let _ = fs::rename(&tmp, &path);
        });
    }
}

// =====================================================================
// 工具
// =====================================================================

/// 获取今日日期 key（YYYY-MM-DD）
#[must_use]
pub fn get_today_key() -> String {
    use chrono::Datelike;
    use chrono::Local;
    let now = Local::now();
    format!("{}-{:02}-{:02}", now.year(), now.month(), now.day())
}

/// 检查跨天并重置
pub fn check_and_reset_daily_stats() {
    let today = get_today_key();
    let mut slots = SLOTS.lock();
    for slot in slots.values_mut() {
        if let Some(prev) = slot.date_key.as_ref() {
            if prev != &today {
                slot.operations.reset();
                slot.date_key = Some(today.clone());
            }
        }
    }
}

// =====================================================================
// 公共 API
// =====================================================================

/// 记录一次操作（CLI / 单测兼容槽）
pub fn record_operation(op_type: &str, count: i64) {
    record_operation_for(LEGACY_SLOT, op_type, count);
}

/// 按账号记录操作（多 worker 隔离）
pub fn record_operation_for(account_id: &str, op_type: &str, count: i64) {
    if account_id.is_empty() {
        return;
    }
    let mut slot = slot_for(account_id);
    let today = get_today_key();
    if let Some(prev) = slot.date_key.as_ref() {
        if prev != &today {
            slot.operations.reset();
        }
    }
    slot.date_key = Some(today);
    match op_type {
        "harvest" => slot.operations.harvest += count,
        "farming" => slot.operations.farming += count,
        "fertilize" => slot.operations.fertilize += count,
        "plant" => slot.operations.plant += count,
        "steal" => slot.operations.steal += count,
        "helpFarming" => slot.operations.help_farming += count,
        "taskClaim" => slot.operations.task_claim += count,
        "sell" => slot.operations.sell += count,
        "upgrade" => slot.operations.upgrade += count,
        "levelUp" => slot.operations.level_up += count,
        _ => {
            put_slot(account_id, slot);
            return;
        }
    }
    put_slot(account_id, slot);
    schedule_save(account_id.to_string());
}

/// 初始化（不持久化；写入兼容槽）
pub fn init_stats(gold: i64, exp: i64, coupon: i64) {
    let mut slot = slot_for(LEGACY_SLOT);
    slot.last.gold = gold;
    slot.last.exp = exp;
    slot.last.coupon = coupon;
    slot.initial.gold = Some(gold);
    slot.initial.exp = Some(exp);
    slot.initial.coupon = Some(coupon);
    put_slot(LEGACY_SLOT, slot);
}

/// 初始化 + 加载持久化数据
pub fn init_stats_with_persistence(account_id: &str, gold: i64, exp: i64, coupon: i64) {
    if account_id.is_empty() {
        return;
    }
    let today = get_today_key();
    let mut slot = slot_for(account_id);

    if let Some(saved) = load_persisted_stats(account_id) {
        if saved.date == today {
            slot.operations = saved.operations.clone();
            tracing::warn!(
                "[统计] 已恢复今日统计数据: {}",
                serde_json::to_string(&saved.operations).unwrap_or_default()
            );
        } else {
            slot.operations.reset();
            tracing::warn!("[统计] 日期已变更，重置统计 ({} -> {today})", saved.date);
        }
    } else {
        slot.operations.reset();
    }

    slot.last.gold = gold;
    slot.last.exp = exp;
    slot.last.coupon = coupon;
    slot.initial.gold = Some(gold);
    slot.initial.exp = Some(exp);
    slot.initial.coupon = Some(coupon);
    slot.date_key = Some(today);
    put_slot(account_id, slot);
}

fn apply_update_stats(slot: &mut StatsSlot, current_gold: i64, current_exp: i64) {
    if slot.last.gold == -1 {
        slot.last.gold = current_gold;
    }
    if slot.last.exp == -1 {
        slot.last.exp = current_exp;
    }

    if current_gold > slot.last.gold {
        slot.session.last_gold_gain = current_gold - slot.last.gold;
    } else if current_gold < slot.last.gold {
        slot.session.last_gold_gain = 0;
    }
    slot.last.gold = current_gold;

    if current_exp > slot.last.exp {
        let delta = current_exp - slot.last.exp;
        let now = crate::utils::time::now_ms();
        if delta == slot.session.last_exp_gain
            && slot.session.last_exp_time.is_some_and(|t| now - t < 1000)
        {
            // 忽略重复经验增量
        } else {
            slot.session.last_exp_gain = delta;
            slot.session.last_exp_time = Some(now);
        }
    } else {
        slot.session.last_exp_gain = 0;
    }
    slot.last.exp = current_exp;
}

/// 更新最后状态（用于 session 计算）
pub fn update_stats(current_gold: i64, current_exp: i64) {
    let mut slot = slot_for(LEGACY_SLOT);
    apply_update_stats(&mut slot, current_gold, current_exp);
    put_slot(LEGACY_SLOT, slot);
}

/// 记录金/经验
pub fn record_gold_exp(gold: i64, exp: i64) {
    update_stats(gold, exp);
}

/// 重置 session 增量
pub fn reset_session_gains() {
    reset_session_gains_for(LEGACY_SLOT);
}

/// 按账号重置 session 增量（登录基线之后调用，对齐 TS `resetSessionGains`）
pub fn reset_session_gains_for(account_id: &str) {
    if account_id.is_empty() {
        return;
    }
    let mut slot = slot_for(account_id);
    slot.session.gold_gained = 0;
    slot.session.exp_gained = 0;
    slot.session.coupon_gained = 0;
    slot.session.last_gold_gain = 0;
    slot.session.last_exp_gain = 0;
    slot.session.last_exp_time = None;
    put_slot(account_id, slot);
}

fn json_i64(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value?;
    value
        .as_i64()
        .or_else(|| value.as_u64().map(|n| n as i64))
        .or_else(|| value.as_f64().map(|n| n as i64))
}

/// 重算 session 增量
pub fn recompute_session_totals(current_gold: i64, current_exp: i64, current_coupon: i64) {
    let mut slot = slot_for(LEGACY_SLOT);
    if slot.initial.gold.is_none() || slot.initial.exp.is_none() || slot.initial.coupon.is_none() {
        slot.initial.gold = Some(current_gold);
        slot.initial.exp = Some(current_exp);
        slot.initial.coupon = Some(current_coupon);
    }
    let init_gold = slot.initial.gold.unwrap_or(0);
    let init_exp = slot.initial.exp.unwrap_or(0);
    let init_coupon = slot.initial.coupon.unwrap_or(0);
    slot.session.gold_gained = current_gold - init_gold;
    slot.session.exp_gained = current_exp - init_exp;
    slot.session.coupon_gained = current_coupon - init_coupon;
    put_slot(LEGACY_SLOT, slot);
}

/// 获取完整状态快照
#[must_use]
pub fn get_stats(
    status_data: Option<&serde_json::Value>,
    user_state: Option<&serde_json::Value>,
    connected: bool,
    limits: serde_json::Value,
) -> serde_json::Value {
    get_stats_for(LEGACY_SLOT, status_data, user_state, connected, limits)
}

/// 按账号取统计快照
#[must_use]
pub fn get_stats_for(
    account_id: &str,
    status_data: Option<&serde_json::Value>,
    user_state: Option<&serde_json::Value>,
    connected: bool,
    limits: serde_json::Value,
) -> serde_json::Value {
    let account_id = if account_id.is_empty() { LEGACY_SLOT } else { account_id };
    let mut slot = slot_for(account_id);
    let today = get_today_key();
    if let Some(prev) = slot.date_key.as_ref() {
        if prev != &today {
            slot.operations.reset();
        }
    }
    slot.date_key = Some(today);

    let status_obj = status_data.and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let user_obj = user_state.and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let current_gold =
        json_i64(user_obj.get("gold")).or_else(|| json_i64(status_obj.get("gold"))).unwrap_or(0);
    let current_exp =
        json_i64(user_obj.get("exp")).or_else(|| json_i64(status_obj.get("exp"))).unwrap_or(0);
    let current_coupon = json_i64(user_obj.get("coupon"))
        .or_else(|| json_i64(status_obj.get("coupon")))
        .unwrap_or(0);
    let current_gold_bean = json_i64(user_obj.get("goldBean"))
        .or_else(|| json_i64(status_obj.get("goldBean")))
        .unwrap_or(0);

    if connected {
        if slot.last.gold == -1 {
            slot.last.gold = current_gold;
        }
        if slot.last.exp == -1 {
            slot.last.exp = current_exp;
        }
        if current_gold > slot.last.gold {
            slot.session.last_gold_gain = current_gold - slot.last.gold;
        } else if current_gold < slot.last.gold {
            slot.session.last_gold_gain = 0;
        }
        slot.last.gold = current_gold;
        if current_exp > slot.last.exp {
            slot.session.last_exp_gain = current_exp - slot.last.exp;
        } else {
            slot.session.last_exp_gain = 0;
        }
        slot.last.exp = current_exp;
        if slot.initial.gold.is_none() {
            slot.initial.gold = Some(current_gold);
            slot.initial.exp = Some(current_exp);
            slot.initial.coupon = Some(current_coupon);
        }
        slot.session.gold_gained = current_gold - slot.initial.gold.unwrap_or(0);
        slot.session.exp_gained = current_exp - slot.initial.exp.unwrap_or(0);
        slot.session.coupon_gained = current_coupon - slot.initial.coupon.unwrap_or(0);
    }

    let name = user_obj
        .get("name")
        .and_then(|v| v.as_str())
        .or_else(|| status_obj.get("name").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    let avatar = user_obj
        .get("avatar")
        .and_then(|v| v.as_str())
        .or_else(|| user_obj.get("avatarUrl").and_then(|v| v.as_str()))
        .or_else(|| status_obj.get("avatar").and_then(|v| v.as_str()))
        .or_else(|| status_obj.get("avatarUrl").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    let level = status_obj
        .get("level")
        .and_then(|v| v.as_i64())
        .or_else(|| user_obj.get("level").and_then(|v| v.as_i64()))
        .unwrap_or(0);
    let platform = status_obj
        .get("platform")
        .and_then(|v| v.as_str())
        .or_else(|| user_obj.get("platform").and_then(|v| v.as_str()))
        .unwrap_or("qq")
        .to_string();
    let ops = slot.operations.clone();
    let session = slot.session.clone();
    put_slot(account_id, slot);

    serde_json::json!({
        "connection": { "connected": connected },
        "status": {
            "name": name,
            "avatar": avatar,
            "level": level,
            "gold": current_gold,
            "coupon": current_coupon,
            "goldBean": current_gold_bean,
            "exp": current_exp,
            "platform": platform,
            "travelPass": null,
        },
        "uptime": 0,
        "operations": ops,
        "sessionExpGained": session.exp_gained,
        "sessionGoldGained": session.gold_gained,
        "sessionCouponGained": session.coupon_gained,
        "lastExpGain": session.last_exp_gain,
        "lastGoldGain": session.last_gold_gain,
        "limits": limits,
    })
}

/// 立即保存指定账号
pub fn save_stats_for(account_id: &str) {
    if account_id.is_empty() {
        return;
    }
    let slot = slot_for(account_id);
    let today = get_today_key();
    let data = PersistedStats {
        date: today,
        operations: slot.operations,
        initial_state: slot.initial,
        total_steal: load_persisted_stats(account_id).map(|p| p.total_steal).unwrap_or(0),
        saved_at: crate::utils::time::now_ms(),
    };
    save_persisted_stats(account_id, &data);
}

/// 立即保存（兼容旧测试：保存所有 slot）
pub fn save_stats() {
    let ids: Vec<String> = SLOTS.lock().keys().cloned().collect();
    for id in ids {
        save_stats_for(&id);
    }
}

use std::sync::OnceLock;
use tokio::sync::Notify;

static SAVE_NOTIFY: OnceLock<Notify> = OnceLock::new();

fn schedule_save(account_id: String) {
    let _ = SAVE_NOTIFY.get_or_init(Notify::new);
    if account_id.is_empty() {
        return;
    }
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        save_stats_for(&account_id);
    });
}

/// 重置所有状态（测试用）
pub fn reset_for_test() {
    SLOTS.lock().clear();
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial(stats)]
    fn operations_map_reset() {
        let mut m = OperationsMap::default();
        m.harvest = 5;
        m.fertilize = 3;
        m.reset();
        assert_eq!(m.harvest, 0);
        assert_eq!(m.fertilize, 0);
    }

    #[test]
    #[serial(stats)]
    fn today_key_format() {
        let k = get_today_key();
        assert_eq!(k.len(), 10);
        assert!(k.contains('-'));
    }

    #[test]
    #[serial(stats)]
    fn record_operation_increments() {
        reset_for_test();
        record_operation_for("t", "harvest", 1);
        record_operation_for("t", "harvest", 2);
        record_operation_for("t", "fertilize", 5);
        let ops = slot_for("t").operations;
        assert_eq!(ops.harvest, 3);
        assert_eq!(ops.fertilize, 5);
        assert_eq!(ops.plant, 0);
    }

    #[test]
    #[serial(stats)]
    fn record_operation_unknown_ignored() {
        reset_for_test();
        record_operation_for("t", "unknown_op", 1);
        // 不应 panic，状态不变
        let ops = slot_for("t").operations;
        assert_eq!(ops.harvest, 0);
    }

    #[test]
    #[serial(stats)]
    fn init_stats_normalizes() {
        reset_for_test();
        init_stats(100, 50, 5);
        let last = slot_for(LEGACY_SLOT).last;
        assert_eq!(last.gold, 100);
        assert_eq!(last.exp, 50);
        assert_eq!(last.coupon, 5);
        let init = slot_for(LEGACY_SLOT).initial;
        assert_eq!(init.gold, Some(100));
    }

    #[test]
    #[serial(stats)]
    fn update_stats_detects_gain() {
        reset_for_test();
        init_stats(100, 50, 0);
        update_stats(150, 80);
        let s = slot_for(LEGACY_SLOT).session;
        assert_eq!(s.last_gold_gain, 50);
        assert_eq!(s.last_exp_gain, 30);
    }

    #[test]
    #[serial(stats)]
    fn update_stats_loss_resets() {
        reset_for_test();
        init_stats(100, 50, 0);
        update_stats(100, 50);
        update_stats(80, 40); // loss
        let s = slot_for(LEGACY_SLOT).session;
        assert_eq!(s.last_gold_gain, 0);
        assert_eq!(s.last_exp_gain, 0);
    }

    #[test]
    #[serial(stats)]
    fn recompute_session_totals_first_call() {
        reset_for_test();
        init_stats(100, 50, 5);
        recompute_session_totals(150, 80, 10);
        let s = slot_for(LEGACY_SLOT).session;
        assert_eq!(s.gold_gained, 50);
        assert_eq!(s.exp_gained, 30);
        assert_eq!(s.coupon_gained, 5);
    }

    #[test]
    #[serial(stats)]
    fn recompute_session_totals_uses_initial() {
        reset_for_test();
        init_stats(100, 50, 5);
        recompute_session_totals(150, 80, 10);
        // 再次调用，应基于 INITIAL_STATE (100/50/5)
        recompute_session_totals(200, 100, 20);
        let s = slot_for(LEGACY_SLOT).session;
        assert_eq!(s.gold_gained, 100);
        assert_eq!(s.exp_gained, 50);
        assert_eq!(s.coupon_gained, 15);
    }

    #[test]
    #[serial(stats)]
    fn test_reset_session_gains() {
        reset_for_test();
        init_stats(100, 50, 0);
        update_stats(150, 80);
        super::reset_session_gains();
        let s = slot_for(LEGACY_SLOT).session;
        assert_eq!(s.gold_gained, 0);
        assert_eq!(s.exp_gained, 0);
    }

    #[test]
    #[serial(stats)]
    fn get_stats_returns_expected_shape() {
        reset_for_test();
        init_stats(100, 50, 0);
        let status = serde_json::json!({"name": "test", "level": 5, "platform": "qq"});
        let user = serde_json::json!({
            "gold": 150,
            "exp": 80,
            "avatar": "https://cdn.example/a.png"
        });
        let s = get_stats(Some(&status), Some(&user), true, serde_json::json!({}));
        let obj = s.as_object().expect("object");
        assert!(obj.contains_key("connection"));
        assert!(obj.contains_key("status"));
        assert!(obj.contains_key("operations"));
        assert!(obj.contains_key("sessionExpGained"));
        assert_eq!(s["status"]["avatar"], "https://cdn.example/a.png");
    }

    #[test]
    #[serial(stats)]
    fn get_stats_for_session_exp_from_float_json() {
        reset_for_test();
        let account = "acc-eff";
        init_stats_with_persistence(account, 100, 50, 0);
        reset_session_gains_for(account);
        let status = serde_json::json!({"name": "t", "level": 5, "exp": 80.0, "gold": 150.0});
        let user = serde_json::json!({"gold": 150.0, "exp": 80.0});
        let s = get_stats_for(account, Some(&status), Some(&user), true, serde_json::json!({}));
        assert_eq!(s["sessionExpGained"], 30);
        assert_eq!(s["sessionGoldGained"], 50);
    }

    #[test]
    #[serial(farm_data_dir)]
    fn save_and_load_persisted_roundtrip() {
        // 用临时目录避免污染 data/stats
        let temp_dir = std::env::temp_dir().join("qq-farm-stats-test");
        let prev = std::env::var("FARM_DATA_DIR").ok();
        let _ = std::env::set_var("FARM_DATA_DIR", &temp_dir);
        let acc = "test-acc-save";
        let data = PersistedStats {
            date: get_today_key(),
            operations: OperationsMap { harvest: 10, fertilize: 5, ..Default::default() },
            initial_state: InitialState { gold: Some(100), exp: Some(50), coupon: Some(0) },
            total_steal: 0,
            saved_at: crate::utils::time::now_ms(),
        };
        save_persisted_stats(acc, &data);
        let loaded = load_persisted_stats(acc).expect("load");
        assert_eq!(loaded.operations.harvest, 10);
        assert_eq!(loaded.operations.fertilize, 5);
        // 清理
        let _ = std::fs::remove_file(stats_file(acc));
        match prev {
            Some(v) => std::env::set_var("FARM_DATA_DIR", v),
            None => std::env::remove_var("FARM_DATA_DIR"),
        }
    }
}
