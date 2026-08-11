//! 每日统计 — 收获/种植/施肥/偷菜等操作计数 + 持久化。
//!
//! 1:1 翻译原 `core/src/services/stats.ts`（317 行）。
//!
//! 数据存储：`{data_dir}/stats/{accountId}.json`
//!
//! 跨天自动重置每日统计；金/经验变化计算 session 增量。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::config::paths::get_data_dir;

/// 操作计数器
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OperationsMap {
    pub harvest: i64,
    pub farming: i64,
    pub fertilize: i64,
    pub plant: i64,
    pub steal: i64,
    pub help_farming: i64,
    pub task_claim: i64,
    pub sell: i64,
    pub upgrade: i64,
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
    pub saved_at: i64,
}

// =====================================================================
// 全局状态
// =====================================================================

static OPERATIONS: Mutex<OperationsMap> = Mutex::new(OperationsMap {
    harvest: 0,
    farming: 0,
    fertilize: 0,
    plant: 0,
    steal: 0,
    help_farming: 0,
    task_claim: 0,
    sell: 0,
    upgrade: 0,
    level_up: 0,
});
static LAST_STATE: Mutex<LastState> = Mutex::new(LastState {
    gold: -1,
    exp: -1,
    coupon: -1,
});
static INITIAL_STATE: Mutex<InitialState> = Mutex::new(InitialState {
    gold: None,
    exp: None,
    coupon: None,
});
static SESSION: Mutex<SessionData> = Mutex::new(SessionData {
    gold_gained: 0,
    exp_gained: 0,
    coupon_gained: 0,
    last_exp_gain: 0,
    last_gold_gain: 0,
    last_exp_time: None,
});
static CURRENT_DATE_KEY: Mutex<Option<String>> = Mutex::new(None);
static CURRENT_ACCOUNT_ID: Mutex<Option<String>> = Mutex::new(None);

// =====================================================================
// 文件路径
// =====================================================================

#[must_use]
pub fn stats_file(account_id: &str) -> PathBuf {
    let dir = std::env::var("FARM_DATA_DIR")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| get_data_dir());
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
        let _ = fs::write(&tmp, body);
        let _ = fs::rename(&tmp, &path);
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
    let account = CURRENT_ACCOUNT_ID.lock().clone();
    if account.is_none() {
        return;
    }
    let today = get_today_key();
    let mut current = CURRENT_DATE_KEY.lock();
    if let Some(prev) = current.as_ref() {
        if prev != &today {
            tracing::warn!("[统计] 检测到跨天，重置每日统计 ({prev} -> {today})");
            OPERATIONS.lock().reset();
        }
    }
    *current = Some(today);
}

// =====================================================================
// 公共 API
// =====================================================================

/// 记录一次操作
pub fn record_operation(op_type: &str, count: i64) {
    check_and_reset_daily_stats();
    let mut ops = OPERATIONS.lock();
    match op_type {
        "harvest" => ops.harvest += count,
        "farming" => ops.farming += count,
        "fertilize" => ops.fertilize += count,
        "plant" => ops.plant += count,
        "steal" => ops.steal += count,
        "helpFarming" => ops.help_farming += count,
        "taskClaim" => ops.task_claim += count,
        "sell" => ops.sell += count,
        "upgrade" => ops.upgrade += count,
        "levelUp" => ops.level_up += count,
        _ => return,
    }
    drop(ops);
    schedule_save();
}

/// 初始化（不持久化）
pub fn init_stats(gold: i64, exp: i64, coupon: i64) {
    let g = gold; // i64 always finite
    let e = exp;
    let c = coupon;
    let mut last = LAST_STATE.lock();
    last.gold = g;
    last.exp = e;
    last.coupon = c;
    drop(last);
    let mut init = INITIAL_STATE.lock();
    init.gold = Some(g);
    init.exp = Some(e);
    init.coupon = Some(c);
}

/// 初始化 + 加载持久化数据
pub fn init_stats_with_persistence(account_id: &str, gold: i64, exp: i64, coupon: i64) {
    *CURRENT_ACCOUNT_ID.lock() = Some(account_id.to_string());
    let today = get_today_key();
    *CURRENT_DATE_KEY.lock() = Some(today.clone());

    if let Some(saved) = load_persisted_stats(account_id) {
        if saved.date == today {
            // 恢复
            let mut ops = OPERATIONS.lock();
            ops.harvest = saved.operations.harvest;
            ops.farming = saved.operations.farming;
            ops.fertilize = saved.operations.fertilize;
            ops.plant = saved.operations.plant;
            ops.steal = saved.operations.steal;
            ops.help_farming = saved.operations.help_farming;
            ops.task_claim = saved.operations.task_claim;
            ops.sell = saved.operations.sell;
            ops.upgrade = saved.operations.upgrade;
            ops.level_up = saved.operations.level_up;
            drop(ops);
            tracing::warn!(
                "[统计] 已恢复今日统计数据: {}",
                serde_json::to_string(&saved.operations).unwrap_or_default()
            );
        } else {
            OPERATIONS.lock().reset();
            tracing::warn!("[统计] 日期已变更，重置统计 ({} -> {today})", saved.date);
        }
    } else {
        OPERATIONS.lock().reset();
    }

    init_stats(gold, exp, coupon);
}

/// 更新最后状态（用于 session 计算）
pub fn update_stats(current_gold: i64, current_exp: i64) {
    let mut last = LAST_STATE.lock();
    if last.gold == -1 {
        last.gold = current_gold;
    }
    if last.exp == -1 {
        last.exp = current_exp;
    }

    if current_gold > last.gold {
        let delta = current_gold - last.gold;
        SESSION.lock().last_gold_gain = delta;
    } else if current_gold < last.gold {
        SESSION.lock().last_gold_gain = 0;
    }
    last.gold = current_gold;

    if current_exp > last.exp {
        let delta = current_exp - last.exp;
        let now = crate::utils::time::now_ms();
        let session = SESSION.lock();
        if delta == session.last_exp_gain
            && session
                .last_exp_time
                .map_or(false, |t| now - t < 1000)
        {
            // 忽略重复经验增量
        } else {
            drop(session);
            let mut s = SESSION.lock();
            s.last_exp_gain = delta;
            s.last_exp_time = Some(now);
        }
    } else {
        SESSION.lock().last_exp_gain = 0;
    }
    last.exp = current_exp;
}

/// 记录金/经验
pub fn record_gold_exp(gold: i64, exp: i64) {
    update_stats(gold, exp);
}

/// 重置 session 增量
pub fn reset_session_gains() {
    let mut s = SESSION.lock();
    s.gold_gained = 0;
    s.exp_gained = 0;
    s.coupon_gained = 0;
    s.last_gold_gain = 0;
    s.last_exp_gain = 0;
    s.last_exp_time = None;
}

/// 重算 session 增量
pub fn recompute_session_totals(current_gold: i64, current_exp: i64, current_coupon: i64) {
    let mut init = INITIAL_STATE.lock();
    if init.gold.is_none() || init.exp.is_none() || init.coupon.is_none() {
        init.gold = Some(current_gold);
        init.exp = Some(current_exp);
        init.coupon = Some(current_coupon);
    }
    let init_gold = init.gold.unwrap_or(0);
    let init_exp = init.exp.unwrap_or(0);
    let init_coupon = init.coupon.unwrap_or(0);
    drop(init);

    let mut s = SESSION.lock();
    s.gold_gained = current_gold - init_gold;
    s.exp_gained = current_exp - init_exp;
    s.coupon_gained = current_coupon - init_coupon;
}

/// 获取完整状态快照
#[must_use]
pub fn get_stats(
    status_data: Option<&serde_json::Value>,
    user_state: Option<&serde_json::Value>,
    connected: bool,
    limits: serde_json::Value,
) -> serde_json::Value {
    check_and_reset_daily_stats();
    let status_obj = status_data
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let user_obj = user_state
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let raw_gold = user_obj
        .get("gold")
        .and_then(|v| if v.is_null() { None } else { Some(v) })
        .or_else(|| status_obj.get("gold"))
        .cloned()
        .unwrap_or(serde_json::json!(0));
    let raw_exp = user_obj
        .get("exp")
        .and_then(|v| if v.is_null() { None } else { Some(v) })
        .or_else(|| status_obj.get("exp"))
        .cloned()
        .unwrap_or(serde_json::json!(0));
    let raw_coupon = user_obj
        .get("coupon")
        .and_then(|v| if v.is_null() { None } else { Some(v) })
        .or_else(|| status_obj.get("coupon"))
        .cloned()
        .unwrap_or(serde_json::json!(0));
    let raw_gold_bean = user_obj
        .get("goldBean")
        .and_then(|v| if v.is_null() { None } else { Some(v) })
        .or_else(|| status_obj.get("goldBean"))
        .cloned()
        .unwrap_or(serde_json::json!(0));

    let current_gold = raw_gold.as_f64().unwrap_or(0.0) as i64;
    let current_exp = raw_exp.as_f64().unwrap_or(0.0) as i64;
    let current_coupon = raw_coupon.as_f64().unwrap_or(0.0) as i64;
    let current_gold_bean = raw_gold_bean.as_f64().unwrap_or(0.0) as i64;

    if connected {
        update_stats(current_gold, current_exp);
        recompute_session_totals(current_gold, current_exp, current_coupon);
    }

    let operations_snapshot = OPERATIONS.lock().clone();
    let session = SESSION.lock().clone();
    let user_coupon = user_obj
        .get("coupon")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as i64;
    let name = user_obj
        .get("name")
        .and_then(|v| v.as_str())
        .or_else(|| status_obj.get("name").and_then(|v| v.as_str()))
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

    serde_json::json!({
        "connection": { "connected": connected },
        "status": {
            "name": name,
            "level": level,
            "gold": current_gold,
            "coupon": user_coupon,
            "goldBean": current_gold_bean,
            "exp": current_exp,
            "platform": platform,
            "travelPass": null,
        },
        "uptime": 0,  // 上层注入
        "operations": operations_snapshot,
        "sessionExpGained": session.exp_gained,
        "sessionGoldGained": session.gold_gained,
        "sessionCouponGained": session.coupon_gained,
        "lastExpGain": session.last_exp_gain,
        "lastGoldGain": session.last_gold_gain,
        "limits": limits,
    })
}

/// 立即保存
pub fn save_stats() {
    let account = CURRENT_ACCOUNT_ID.lock().clone();
    let Some(account) = account else { return };
    let today = get_today_key();
    let ops = OPERATIONS.lock().clone();
    let init = INITIAL_STATE.lock().clone();
    let data = PersistedStats {
        date: today,
        operations: ops,
        initial_state: init,
        saved_at: crate::utils::time::now_ms(),
    };
    save_persisted_stats(&account, &data);
}

// 简单的延迟保存（debounce 2 秒）
use std::sync::OnceLock;
use tokio::sync::Notify;

static SAVE_NOTIFY: OnceLock<Notify> = OnceLock::new();

fn schedule_save() {
    // 简化：直接 spawn 一个 task 延迟 2 秒保存
    let _ = SAVE_NOTIFY.get_or_init(Notify::new);
    let account = CURRENT_ACCOUNT_ID.lock().clone();
    if account.is_none() {
        return;
    }
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        save_stats();
    });
}

/// 获取当前账号 ID
#[must_use]
pub fn current_account_id() -> Option<String> {
    CURRENT_ACCOUNT_ID.lock().clone()
}

/// 重置所有状态（测试用）
pub fn reset_for_test() {
    OPERATIONS.lock().reset();
    let mut last = LAST_STATE.lock();
    last.gold = -1;
    last.exp = -1;
    last.coupon = -1;
    drop(last);
    *INITIAL_STATE.lock() = InitialState::default();
    *SESSION.lock() = SessionData::default();
    *CURRENT_DATE_KEY.lock() = None;
    *CURRENT_ACCOUNT_ID.lock() = None;
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
        record_operation("harvest", 1);
        record_operation("harvest", 2);
        record_operation("fertilize", 5);
        let ops = OPERATIONS.lock();
        assert_eq!(ops.harvest, 3);
        assert_eq!(ops.fertilize, 5);
        assert_eq!(ops.plant, 0);
    }

    #[test]
    #[serial(stats)]
    fn record_operation_unknown_ignored() {
        reset_for_test();
        record_operation("unknown_op", 1);
        // 不应 panic，状态不变
        let ops = OPERATIONS.lock();
        assert_eq!(ops.harvest, 0);
    }

    #[test]
    #[serial(stats)]
    fn init_stats_normalizes() {
        reset_for_test();
        init_stats(100, 50, 5);
        let last = LAST_STATE.lock();
        assert_eq!(last.gold, 100);
        assert_eq!(last.exp, 50);
        assert_eq!(last.coupon, 5);
        drop(last);
        let init = INITIAL_STATE.lock();
        assert_eq!(init.gold, Some(100));
    }

    #[test]
    #[serial(stats)]
    fn update_stats_detects_gain() {
        reset_for_test();
        init_stats(100, 50, 0);
        update_stats(150, 80);
        let s = SESSION.lock();
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
        let s = SESSION.lock();
        assert_eq!(s.last_gold_gain, 0);
        assert_eq!(s.last_exp_gain, 0);
    }

    #[test]
    #[serial(stats)]
    fn recompute_session_totals_first_call() {
        reset_for_test();
        init_stats(100, 50, 5);
        recompute_session_totals(150, 80, 10);
        let s = SESSION.lock();
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
        let s = SESSION.lock();
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
        let s = SESSION.lock();
        assert_eq!(s.gold_gained, 0);
        assert_eq!(s.exp_gained, 0);
    }

    #[test]
    #[serial(stats)]
    fn get_stats_returns_expected_shape() {
        reset_for_test();
        init_stats(100, 50, 0);
        let status = serde_json::json!({"name": "test", "level": 5, "platform": "qq"});
        let user = serde_json::json!({"gold": 150, "exp": 80});
        let s = get_stats(Some(&status), Some(&user), true, serde_json::json!({}));
        let obj = s.as_object().expect("object");
        assert!(obj.contains_key("connection"));
        assert!(obj.contains_key("status"));
        assert!(obj.contains_key("operations"));
        assert!(obj.contains_key("sessionExpGained"));
    }

    #[test]
    #[serial(stats)]
    fn save_and_load_persisted_roundtrip() {
        // 用临时目录避免污染 data/stats
        let temp_dir = std::env::temp_dir().join("qq-farm-stats-test");
        let _ = std::env::set_var("FARM_DATA_DIR", &temp_dir);
        let acc = "test-acc-save";
        let data = PersistedStats {
            date: get_today_key(),
            operations: OperationsMap {
                harvest: 10,
                fertilize: 5,
                ..Default::default()
            },
            initial_state: InitialState {
                gold: Some(100),
                exp: Some(50),
                coupon: Some(0),
            },
            saved_at: crate::utils::time::now_ms(),
        };
        save_persisted_stats(acc, &data);
        let loaded = load_persisted_stats(acc).expect("load");
        assert_eq!(loaded.operations.harvest, 10);
        assert_eq!(loaded.operations.fertilize, 5);
        // 清理
        let _ = std::fs::remove_file(stats_file(acc));
    }
}
