//! Runtime 状态 — 全局日志 / 账号日志 / 配置版本号 / 事件总线。
//!
//! 1:1 翻译原 `core/src/runtime/runtime-state.ts`（237 行）。
//!
//! ## 职责
//!
//! - 持有 `workers` 映射（accountId → WorkerInfo）
//! - `globalLogs` 数组（上限 1000，FIFO 淘汰）
//! - `accountLogs` 数组（上限 300，FIFO 淘汰）
//! - `configRevision` 自增计数器（每次配置变更 +1）
//! - 事件总线（log / account_log / status / worker_log）
//! - 默认 status 构造（合并持久化 stats + store.getAutomation + store.getPreferredSeed）
//!
//! ## 与原 TS 的差异
//!
//! - `EventEmitter` 改为 `tokio::sync::broadcast`（支持 async 订阅）
//! - `globalLogs` / `accountLogs` 改为 `parking_lot::Mutex<Vec<_>>`
//! - `operationKeys` 通过 `RuntimeStateOptions` 注入
//! - 持久化 stats 来自 `services::stats::load_persisted_stats`
//! - store 通过抽象 `AccountStoreLike` trait 注入（不绑死 `models::store`）

use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::models::AccountConfigSnapshot;
use crate::services::stats::{get_today_key, load_persisted_stats};

/// 全局日志上限
pub const GLOBAL_LOG_CAP: usize = 1000;
/// 账号日志上限
pub const ACCOUNT_LOG_CAP: usize = 300;

/// 日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub time: String,
    pub tag: String,
    pub msg: String,
    #[serde(default)]
    pub meta: serde_json::Value,
    pub ts: i64,
    /// 搜索文本（msg + tag + meta 小写拼接）
    #[serde(skip)]
    pub search_text: String,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub account_name: Option<String>,
    #[serde(default)]
    pub is_warn: bool,
}

/// 账号日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLogEntry {
    pub time: String,
    pub action: String,
    pub msg: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub account_name: String,
}

/// 日志过滤
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogFilters {
    pub keyword: Option<String>,
    pub tag: Option<String>,
    pub module: Option<String>,
    pub event: Option<String>,
    pub is_warn: Option<bool>,
    pub time_from: Option<String>,
    pub time_to: Option<String>,
}

/// 运行时事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeEvent {
    Log(LogEntry),
    AccountLog(AccountLogEntry),
    Status {
        account_id: String,
        account_name: String,
        status: serde_json::Value,
    },
    WorkerLog {
        entry: serde_json::Value,
        account_id: String,
        account_name: String,
    },
}

/// 抽象 Account Store（用于解耦 models::store）
pub trait AccountStoreLike: Send + Sync {
    fn get_config_snapshot(&self, account_id: &str) -> AccountConfigSnapshot;
    fn get_automation(&self, account_id: &str) -> serde_json::Value;
    fn get_preferred_seed(&self, account_id: &str) -> i64;
}

/// 默认 status
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultStatus {
    pub connection: ConnectionStatus,
    pub status: BasicStatus,
    pub uptime: u64,
    pub operations: std::collections::HashMap<String, i64>,
    pub total_steal: i64,
    pub session_exp_gained: i64,
    pub session_gold_gained: i64,
    pub session_coupon_gained: i64,
    pub last_exp_gain: i64,
    pub last_gold_gain: i64,
    pub limits: serde_json::Value,
    pub ws_error: Option<String>,
    pub automation: serde_json::Value,
    pub preferred_seed: i64,
    #[serde(rename = "levelProgress")]
    pub exp_progress: ExpProgress,
    pub config_revision: u64,
    pub account_id: String,
}

/// 连接状态
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionStatus {
    pub connected: bool,
}

/// 基础状态
#[derive(Debug, Clone, Serialize)]
pub struct BasicStatus {
    pub name: String,
    pub level: i64,
    pub gold: i64,
    pub exp: i64,
    pub platform: String,
}

/// 经验进度
#[derive(Debug, Clone, Serialize)]
pub struct ExpProgress {
    pub current: i64,
    pub needed: i64,
    pub level: i64,
}

/// Runtime 状态
pub struct RuntimeState {
    /// worker 映射（accountId → WorkerInfo）
    pub workers: Mutex<std::collections::HashMap<String, WorkerInfo>>,
    /// 全局日志
    pub global_logs: Mutex<Vec<LogEntry>>,
    /// 账号日志
    pub account_logs: Mutex<Vec<AccountLogEntry>>,
    /// 事件总线
    pub events: broadcast::Sender<RuntimeEvent>,
    /// config revision
    config_revision: Mutex<u64>,
    /// 账号 store
    store: Arc<dyn AccountStoreLike>,
    /// 操作 key 列表
    operation_keys: Vec<String>,
}

/// 简化的 worker 信息
#[derive(Debug, Clone, Default, Serialize)]
pub struct WorkerInfo {
    pub account_id: String,
    pub account_name: String,
    /// 上报的 status（任意 JSON）
    pub status: Option<serde_json::Value>,
    /// 最近一次 WS 错误
    pub ws_error: Option<String>,
    /// 正在停止
    pub stopping: bool,
    /// 已 terminal 处理
    pub terminal_handled: bool,
    /// 最近一次判定为断开的时间
    #[serde(default)]
    pub disconnected_since: Option<i64>,
    /// 离线超时自删是否已触发
    #[serde(default)]
    pub auto_delete_triggered: bool,
}

impl RuntimeState {
    /// 创建 runtime state
    #[must_use]
    pub fn new(store: Arc<dyn AccountStoreLike>, operation_keys: Vec<String>) -> Self {
        let (events, _) = broadcast::channel(4096);
        Self {
            workers: Mutex::new(Default::default()),
            global_logs: Mutex::new(Vec::new()),
            account_logs: Mutex::new(Vec::new()),
            events,
            config_revision: Mutex::new(now_ms().max(0) as u64),
            store,
            operation_keys,
        }
    }

    /// 下一个 config revision
    pub fn next_config_revision(&self) -> u64 {
        let mut rev = self.config_revision.lock();
        *rev += 1;
        *rev
    }

    /// 当前 config revision
    #[must_use]
    pub fn config_revision(&self) -> u64 {
        *self.config_revision.lock()
    }

    /// 构造某账号的配置快照（带 __revision）
    #[must_use]
    pub fn build_config_snapshot_for_account(&self, account_id: &str) -> serde_json::Value {
        let mut snapshot = serde_json::to_value(self.store.get_config_snapshot(account_id))
            .unwrap_or(serde_json::Value::Null);
        if let Some(obj) = snapshot.as_object_mut() {
            obj.remove("ui");
            obj.insert("__revision".to_string(), serde_json::json!(self.config_revision()));
        }
        snapshot
    }

    /// 记录全局日志
    pub fn log(&self, tag: &str, msg: &str, extra: Option<serde_json::Value>) {
        let time = format_local_datetime24(None);
        let level = if tag == "错误" { "error" } else { "info" };
        let module_name = if tag == "系统" || tag == "错误" { "system" } else { "" };
        let extra_val = extra.clone();
        let mut entry = LogEntry {
            time,
            tag: tag.to_string(),
            msg: msg.to_string(),
            meta: if module_name.is_empty() {
                extra_val.clone().unwrap_or(serde_json::Value::Null)
            } else {
                let mut meta = extra_val.clone().unwrap_or_else(|| serde_json::json!({}));
                if let Some(obj) = meta.as_object_mut() {
                    obj.entry("module".to_string()).or_insert(serde_json::json!(module_name));
                }
                meta
            },
            ts: now_ms(),
            search_text: String::new(),
            account_id: extra_val
                .as_ref()
                .and_then(|v| v.get("accountId").or_else(|| v.get("account_id")))
                .and_then(|v| v.as_str())
                .map(str::to_string),
            account_name: extra_val
                .as_ref()
                .and_then(|v| v.get("accountName").or_else(|| v.get("account_name")))
                .and_then(|v| v.as_str())
                .map(str::to_string),
            is_warn: tag == "错误"
                || extra_val
                    .as_ref()
                    .and_then(|v| v.get("isWarn").or_else(|| v.get("is_warn")))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
        };
        entry.search_text = format!("{} {} {}", entry.msg, entry.tag, entry.meta).to_lowercase();
        let _ = level;
        tracing::info!(target: "runtime", "{}", msg);

        {
            let mut logs = self.global_logs.lock();
            logs.push(entry.clone());
            if logs.len() > GLOBAL_LOG_CAP {
                let drop = logs.len() - GLOBAL_LOG_CAP;
                logs.drain(0..drop);
            }
        }
        let _ = self.events.send(RuntimeEvent::Log(entry));
    }

    /// 记录账号日志
    pub fn add_account_log(
        &self,
        action: &str,
        msg: &str,
        account_id: Option<&str>,
        account_name: Option<&str>,
        extra: Option<serde_json::Value>,
    ) {
        let entry = AccountLogEntry {
            time: format_local_datetime24(None),
            action: action.to_string(),
            msg: msg.to_string(),
            account_id: account_id.unwrap_or("").to_string(),
            account_name: account_name.unwrap_or("").to_string(),
        };
        let _ = extra;
        {
            let mut logs = self.account_logs.lock();
            logs.push(entry.clone());
            if logs.len() > ACCOUNT_LOG_CAP {
                let drop = logs.len() - ACCOUNT_LOG_CAP;
                logs.drain(0..drop);
            }
        }
        let _ = self.events.send(RuntimeEvent::AccountLog(entry));
    }

    /// 把 status 归一化为面板格式
    #[must_use]
    pub fn normalize_status_for_panel(
        &self,
        data: Option<&serde_json::Value>,
        account_id: &str,
        account_name: &str,
    ) -> serde_json::Value {
        let src = data.cloned().unwrap_or(serde_json::Value::Null);
        let mut ops = if let Some(o) = src.get("operations").and_then(|v| v.as_object()) {
            o.clone()
        } else {
            serde_json::Map::new()
        };
        for k in &self.operation_keys {
            let v = ops.get(k);
            let n = v.and_then(|x| x.as_i64()).unwrap_or(0);
            ops.insert(k.clone(), serde_json::json!(n));
        }
        let mut result = src.as_object().cloned().unwrap_or_default();
        result.insert("accountId".to_string(), serde_json::json!(account_id));
        result.insert("accountName".to_string(), serde_json::json!(account_name));
        result.insert("operations".to_string(), serde_json::Value::Object(ops));
        serde_json::Value::Object(result)
    }

    /// 构造默认 status（合并持久化 stats）
    #[must_use]
    pub fn build_default_status(&self, account_id: &str) -> DefaultStatus {
        let id = account_id.to_string();
        let mut operations = std::collections::HashMap::new();
        for k in &self.operation_keys {
            operations.insert(k.clone(), 0);
        }
        let mut total_steal: i64 = 0;

        if !id.is_empty() {
            if let Some(saved) = load_persisted_stats(&id) {
                let today_key = get_today_key();
                if saved.date == today_key {
                    // 从 typed OperationsMap 取值（与 operation_keys 1:1 对应）
                    let ops_json = serde_json::to_value(&saved.operations)
                        .unwrap_or(serde_json::Value::Null);
                    if let Some(obj) = ops_json.as_object() {
                        for k in &self.operation_keys {
                            if let Some(v) = obj.get(k).and_then(|x| x.as_i64()) {
                                operations.insert(k.clone(), v);
                            }
                        }
                    }
                }
                total_steal = saved.total_steal;
            }
        }

        DefaultStatus {
            connection: ConnectionStatus { connected: false },
            status: BasicStatus {
                name: String::new(),
                level: 0,
                gold: 0,
                exp: 0,
                platform: "qq".to_string(),
            },
            uptime: 0,
            operations,
            total_steal,
            session_exp_gained: 0,
            session_gold_gained: 0,
            session_coupon_gained: 0,
            last_exp_gain: 0,
            last_gold_gain: 0,
            limits: serde_json::Value::Null,
            ws_error: None,
            automation: self.store.get_automation(account_id),
            preferred_seed: self.store.get_preferred_seed(account_id),
            exp_progress: ExpProgress { current: 0, needed: 0, level: 0 },
            config_revision: self.config_revision(),
            account_id: id,
        }
    }

    /// 过滤日志
    #[must_use]
    pub fn filter_logs(&self, list: &[LogEntry], filters: &LogFilters) -> Vec<LogEntry> {
        let keyword = filters
            .keyword
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_lowercase();
        let keyword_terms: Vec<String> = if keyword.is_empty() {
            vec![]
        } else {
            keyword.split_whitespace().map(str::to_string).collect()
        };
        let tag = filters.tag.as_deref().unwrap_or("").trim();
        let module = filters.module.as_deref().unwrap_or("").trim();
        let event_name = filters.event.as_deref().unwrap_or("").trim();
        let is_warn = filters.is_warn;
        let time_from_ms = filters
            .time_from
            .as_deref()
            .and_then(|s| chrono_parse(s))
            .unwrap_or(i64::MIN);
        let time_to_ms = filters
            .time_to
            .as_deref()
            .and_then(|s| chrono_parse(s))
            .unwrap_or(i64::MAX);

        list.iter()
            .filter(|l| {
                let log_ms = if l.ts > 0 { l.ts } else { 0 };
                if time_from_ms > i64::MIN && log_ms < time_from_ms {
                    return false;
                }
                if time_to_ms < i64::MAX && log_ms > time_to_ms {
                    return false;
                }
                if !tag.is_empty() && l.tag != tag {
                    return false;
                }
                if !module.is_empty() {
                    let log_module = l.meta.get("module").and_then(|v| v.as_str()).unwrap_or("");
                    if module == "system" {
                        let is_system_tag = l.tag == "系统" || l.tag == "错误";
                        if log_module != "system" && !is_system_tag {
                            return false;
                        }
                    } else if log_module != module {
                        return false;
                    }
                }
                if !event_name.is_empty() {
                    let log_event = l.meta.get("event").and_then(|v| v.as_str()).unwrap_or("");
                    if log_event != event_name {
                        return false;
                    }
                }
                if let Some(expected) = is_warn {
                    if l.is_warn != expected {
                        return false;
                    }
                }
                if !keyword_terms.is_empty() {
                    let text = l.search_text.to_lowercase();
                    for term in &keyword_terms {
                        if !text.contains(term) {
                            return false;
                        }
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    /// 订阅 runtime 事件
    pub fn subscribe(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.events.subscribe()
    }
}

// =====================================================================
// 纯函数
// =====================================================================

fn format_local_datetime24(date: Option<i64>) -> String {
    use chrono::{Local, TimeZone};
    let d = date.map_or_else(Local::now, |ms| {
        let secs = ms / 1000;
        let nsecs = ((ms % 1000).max(0) as u32) * 1_000_000;
        Local
            .timestamp_opt(secs, nsecs)
            .single()
            .unwrap_or_else(Local::now)
    });
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        d.format("%Y").to_string().parse::<i32>().unwrap_or(0),
        d.format("%m").to_string().parse::<u32>().unwrap_or(0),
        d.format("%d").to_string().parse::<u32>().unwrap_or(0),
        d.format("%H").to_string().parse::<u32>().unwrap_or(0),
        d.format("%M").to_string().parse::<u32>().unwrap_or(0),
        d.format("%S").to_string().parse::<u32>().unwrap_or(0),
    )
}

fn chrono_parse(s: &str) -> Option<i64> {
    use chrono::DateTime;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp_millis())
        .or_else(|| {
            // 简化：尝试 yyyy-MM-dd HH:mm:ss
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|dt| dt.and_utc().timestamp_millis())
        })
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Mock store
    struct MockStore;
    impl AccountStoreLike for MockStore {
        fn get_config_snapshot(&self, _id: &str) -> AccountConfigSnapshot {
            AccountConfigSnapshot::default()
        }
        fn get_automation(&self, _id: &str) -> serde_json::Value {
            serde_json::json!({})
        }
        fn get_preferred_seed(&self, _id: &str) -> i64 {
            0
        }
    }

    fn make_state() -> RuntimeState {
        RuntimeState::new(
            Arc::new(MockStore),
            vec![
                "harvest".to_string(),
                "farming".to_string(),
                "fertilize".to_string(),
                "plant".to_string(),
                "steal".to_string(),
                "helpFarming".to_string(),
                "taskClaim".to_string(),
                "sell".to_string(),
                "upgrade".to_string(),
            ],
        )
    }

    #[test]
    fn log_basic() {
        let s = make_state();
        s.log("系统", "hello", None);
        s.log("错误", "boom", None);
        let logs = s.global_logs.lock();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].tag, "系统");
        assert_eq!(logs[1].tag, "错误");
        assert!(logs[1].is_warn);
    }

    #[test]
    fn log_cap_1000() {
        let s = make_state();
        for i in 0..1500 {
            s.log("系统", &format!("msg {i}"), None);
        }
        let logs = s.global_logs.lock();
        assert_eq!(logs.len(), GLOBAL_LOG_CAP);
        // 最新在末尾
        assert!(logs.last().unwrap().msg.contains("1499"));
    }

    #[test]
    fn add_account_log_basic() {
        let s = make_state();
        s.add_account_log("login", "用户登录成功", Some("acc1"), Some("测试"), None);
        let logs = s.account_logs.lock();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action, "login");
        assert_eq!(logs[0].account_id, "acc1");
    }

    #[test]
    fn add_account_log_cap_300() {
        let s = make_state();
        for i in 0..400 {
            s.add_account_log("op", &format!("{i}"), Some("a"), Some("n"), None);
        }
        let logs = s.account_logs.lock();
        assert_eq!(logs.len(), ACCOUNT_LOG_CAP);
    }

    #[test]
    fn next_config_revision_increments() {
        let s = make_state();
        let r1 = s.next_config_revision();
        let r2 = s.next_config_revision();
        let r3 = s.next_config_revision();
        assert!(r2 > r1);
        assert!(r3 > r2);
    }

    #[test]
    fn build_default_status_empty_id() {
        let s = make_state();
        let st = s.build_default_status("");
        assert_eq!(st.account_id, "");
        assert!(!st.connection.connected);
        assert_eq!(st.status.platform, "qq");
    }

    #[test]
    fn build_default_status_has_all_operation_keys() {
        let s = make_state();
        let st = s.build_default_status("acc1");
        let keys: HashSet<&str> = st.operations.keys().map(String::as_str).collect();
        assert!(keys.contains("harvest"));
        assert!(keys.contains("fertilize"));
    }

    #[test]
    fn normalize_status_fills_missing_ops() {
        let s = make_state();
        let src = serde_json::json!({
            "operations": { "harvest": 5, "extra": 99 }
        });
        let result = s.normalize_status_for_panel(Some(&src), "acc1", "test");
        let ops = result.get("operations").unwrap().as_object().unwrap();
        assert_eq!(ops.get("harvest").unwrap().as_i64().unwrap(), 5);
        assert_eq!(ops.get("farming").unwrap().as_i64().unwrap(), 0);
        assert_eq!(ops.get("extra").unwrap().as_i64().unwrap(), 99);
        assert_eq!(result.get("accountId").unwrap().as_str().unwrap(), "acc1");
    }

    #[test]
    fn normalize_status_null_data() {
        let s = make_state();
        let result = s.normalize_status_for_panel(None, "acc1", "test");
        assert_eq!(result.get("accountId").unwrap().as_str().unwrap(), "acc1");
    }

    #[test]
    fn filter_logs_keyword() {
        let s = make_state();
        s.log("系统", "alpha", None);
        s.log("系统", "beta", None);
        s.log("错误", "gamma", None);
        let logs = s.global_logs.lock().clone();
        let filters = LogFilters {
            keyword: Some("alpha".to_string()),
            ..Default::default()
        };
        let filtered = s.filter_logs(&logs, &filters);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].msg.contains("alpha"));
    }

    #[test]
    fn filter_logs_multi_term() {
        let s = make_state();
        s.log("系统", "alpha beta gamma", None);
        s.log("系统", "alpha only", None);
        let logs = s.global_logs.lock().clone();
        let filters = LogFilters {
            keyword: Some("alpha beta".to_string()),
            ..Default::default()
        };
        let filtered = s.filter_logs(&logs, &filters);
        // AND 语义
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].msg.contains("gamma"));
    }

    #[test]
    fn filter_logs_tag() {
        let s = make_state();
        s.log("系统", "info", None);
        s.log("错误", "err", None);
        let logs = s.global_logs.lock().clone();
        let filters = LogFilters {
            tag: Some("错误".to_string()),
            ..Default::default()
        };
        let filtered = s.filter_logs(&logs, &filters);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].tag, "错误");
    }

    #[test]
    fn filter_logs_module_system() {
        let s = make_state();
        s.log("系统", "sys1", None);
        s.log("错误", "err1", None);
        s.log("其他", "other1", None);
        let logs = s.global_logs.lock().clone();
        let filters = LogFilters {
            module: Some("system".to_string()),
            ..Default::default()
        };
        let filtered = s.filter_logs(&logs, &filters);
        // 系统和错误 tag 都被 system module 接受
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_logs_is_warn() {
        let s = make_state();
        s.log("系统", "info", None);
        s.log("错误", "err", None);
        let logs = s.global_logs.lock().clone();
        let filters = LogFilters {
            is_warn: Some(true),
            ..Default::default()
        };
        let filtered = s.filter_logs(&logs, &filters);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].is_warn);
    }

    #[test]
    fn filter_logs_time_range() {
        let s = make_state();
        s.log("系统", "old", None);
        let logs = s.global_logs.lock().clone();
        let now = now_ms();
        // time_from = now + 1000（应过滤掉）
        let filters = LogFilters {
            time_from: Some(format_iso(now + 1000)),
            ..Default::default()
        };
        let filtered = s.filter_logs(&logs, &filters);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn build_config_snapshot_has_revision() {
        let s = make_state();
        let snap = s.build_config_snapshot_for_account("acc1");
        let rev = snap.get("__revision").and_then(|v| v.as_u64());
        assert!(rev.is_some());
    }

    #[test]
    fn build_config_snapshot_no_ui() {
        let s = make_state();
        let snap = s.build_config_snapshot_for_account("acc1");
        assert!(snap.get("ui").is_none() || snap.get("ui").unwrap().is_null());
    }

    #[test]
    fn format_local_datetime24_format() {
        let s = format_local_datetime24(None);
        // 2024-01-01 00:00:00 之类
        assert_eq!(s.len(), 19);
        assert_eq!(s.chars().nth(4), Some('-'));
        assert_eq!(s.chars().nth(10), Some(' '));
        assert_eq!(s.chars().nth(13), Some(':'));
        assert_eq!(s.chars().nth(16), Some(':'));
    }

    #[test]
    fn format_local_datetime24_specific() {
        // 2024-01-15 10:30:45 (北京时间)
        let ms = chrono::DateTime::parse_from_rfc3339("2024-01-15T02:30:45Z")
            .unwrap()
            .timestamp_millis();
        let s = format_local_datetime24(Some(ms));
        // 应该是 YYYY-MM-DD HH:MM:SS 格式
        assert!(s.starts_with("2024-01-15") || s.starts_with("2024-01-14"));
    }

    #[test]
    fn chrono_parse_rfc3339() {
        let n = chrono_parse("2024-01-15T10:30:45Z");
        assert!(n.is_some());
    }

    #[test]
    fn chrono_parse_custom() {
        let n = chrono_parse("2024-01-15 10:30:45");
        assert!(n.is_some());
    }

    #[test]
    fn chrono_parse_invalid() {
        assert!(chrono_parse("not a date").is_none());
    }

    #[test]
    fn filter_logs_event() {
        let s = make_state();
        // 通过 log 注入带 event 的 meta
        let mut entry = LogEntry {
            time: "2024-01-01 00:00:00".to_string(),
            tag: "tag".to_string(),
            msg: "msg".to_string(),
            meta: serde_json::json!({"event": "login"}),
            ts: now_ms(),
            search_text: String::new(),
            account_id: None,
            account_name: None,
            is_warn: false,
        };
        entry.search_text = "msg tag".to_string();
        {
            let mut logs = s.global_logs.lock();
            logs.push(entry);
        }
        let logs = s.global_logs.lock().clone();
        let filters = LogFilters {
            event: Some("login".to_string()),
            ..Default::default()
        };
        let filtered = s.filter_logs(&logs, &filters);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn worker_info_default() {
        let w = WorkerInfo::default();
        assert!(w.account_id.is_empty());
        assert!(!w.stopping);
        assert!(!w.terminal_handled);
    }

    #[test]
    fn events_subscription_works() {
        let s = make_state();
        let mut rx = s.subscribe();
        s.log("系统", "test", None);
        // 异步检查，可能需要 yield
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let result =
                tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await;
            assert!(result.is_ok());
        });
    }

    #[test]
    fn events_subscription_no_blocking() {
        let s = make_state();
        let _rx = s.subscribe();
        s.log("系统", "test", None);
    }

    fn format_iso(ms: i64) -> String {
        chrono::DateTime::from_timestamp(ms / 1000, ((ms % 1000) * 1_000_000) as u32)
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_default()
    }

    #[test]
    fn default_status_operations_count() {
        let s = make_state();
        let st = s.build_default_status("acc1");
        // 9 个 OPERATION_KEYS
        assert_eq!(st.operations.len(), 9);
    }
}
