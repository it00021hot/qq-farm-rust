//! 应用事件总线 — 包装 core RuntimeEvent，并产出与 web Socket 对齐的实时信封。

use qq_farm_core::runtime::runtime_state::{LogEntry, RuntimeEvent};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::broadcast;

use crate::session::AppContext;

/// 应用层事件（当前直接包装 RuntimeEvent）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct AppEvent(pub RuntimeEvent);

impl AppEvent {
    #[must_use]
    pub fn into_inner(self) -> RuntimeEvent {
        self.0
    }

    #[must_use]
    pub fn as_inner(&self) -> &RuntimeEvent {
        &self.0
    }

    /// 转成 web / desktop 共用的实时信封（一条 RuntimeEvent 可能对应多条）。
    #[must_use]
    pub fn to_realtime(&self) -> Vec<PanelRealtimeEvent> {
        PanelRealtimeEvent::from_runtime(&self.0)
    }
}

impl From<RuntimeEvent> for AppEvent {
    fn from(e: RuntimeEvent) -> Self {
        Self(e)
    }
}

impl From<AppEvent> for RuntimeEvent {
    fn from(e: AppEvent) -> Self {
        e.0
    }
}

/// 对齐 web Socket.IO：`{ type, payload, accountId }`。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelRealtimeEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: Value,
    pub account_id: Option<String>,
}

impl PanelRealtimeEvent {
    #[must_use]
    pub fn from_runtime(ev: &RuntimeEvent) -> Vec<Self> {
        match ev {
            RuntimeEvent::Status {
                account_id,
                account_name,
                status,
            } => {
                if account_id.is_empty() {
                    return Vec::new();
                }
                let flattened = flatten_status_body(status, account_id, account_name);
                vec![Self {
                    event_type: "status:update".into(),
                    payload: json!({
                        "accountId": account_id,
                        "status": flattened,
                    }),
                    account_id: Some(account_id.clone()),
                }]
            }
            RuntimeEvent::Log(entry) => {
                let id = entry.account_id.clone().filter(|s| !s.is_empty());
                let payload = log_payload(entry);
                let mut out = vec![Self {
                    event_type: "log:new".into(),
                    payload: payload.clone(),
                    account_id: id.clone(),
                }];
                out.extend(derived_from_log(entry, &payload, id.as_deref()));
                out
            }
            RuntimeEvent::AccountLog(entry) => {
                if entry.account_id.is_empty() {
                    return Vec::new();
                }
                vec![Self {
                    event_type: "account-log:new".into(),
                    payload: serde_json::to_value(entry).unwrap_or(json!({
                        "message": entry.msg,
                        "accountId": entry.account_id,
                    })),
                    account_id: Some(entry.account_id.clone()),
                }]
            }
            RuntimeEvent::WorkerLog {
                entry,
                account_id,
                ..
            } => {
                if account_id.is_empty() {
                    return Vec::new();
                }
                vec![Self {
                    event_type: "log:new".into(),
                    payload: entry.clone(),
                    account_id: Some(account_id.clone()),
                }]
            }
        }
    }
}

fn flatten_status_body(status: &Value, account_id: &str, account_name: &str) -> Value {
    let nested = status.get("status").cloned().unwrap_or(Value::Null);
    let connected = status
        .pointer("/connection/connected")
        .and_then(Value::as_bool)
        .or_else(|| status.get("online").and_then(Value::as_bool))
        .unwrap_or(false);
    let nick = nested
        .get("name")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| status.get("nick").and_then(Value::as_str))
        .filter(|s| !s.is_empty())
        .unwrap_or(account_name);
    let mut out = if let Some(obj) = status.as_object() {
        Value::Object(obj.clone())
    } else {
        json!({})
    };
    if let Some(obj) = out.as_object_mut() {
        obj.insert("accountId".into(), json!(account_id));
        obj.entry("nick".to_string()).or_insert_with(|| json!(nick));
        if !obj.contains_key("level") {
            if let Some(v) = nested.get("level") {
                obj.insert("level".into(), v.clone());
            }
        }
        if !obj.contains_key("exp") {
            if let Some(v) = nested.get("exp") {
                obj.insert("exp".into(), v.clone());
            }
        }
        if !obj.contains_key("gold") {
            if let Some(v) = nested.get("gold") {
                obj.insert("gold".into(), v.clone());
            }
        }
        obj.insert("online".into(), json!(connected));
        obj.entry("runStatus".to_string())
            .or_insert_with(|| json!(if connected { 1 } else { 0 }));
    }
    out
}

fn log_payload(entry: &LogEntry) -> Value {
    let mut payload = serde_json::to_value(entry).unwrap_or_else(|_| {
        json!({
            "tag": entry.tag,
            "msg": entry.msg,
            "accountId": entry.account_id,
        })
    });
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("message".into(), json!(entry.msg));
        if let Some(event) = entry.meta.get("event") {
            obj.entry("event".to_string()).or_insert_with(|| event.clone());
        }
        if let Some(module) = entry.meta.get("module") {
            obj.entry("module".to_string())
                .or_insert_with(|| module.clone());
        }
        if let Some(gid) = entry
            .meta
            .get("friendGid")
            .or_else(|| entry.meta.get("targetGid"))
        {
            obj.entry("friendGid".to_string()).or_insert_with(|| gid.clone());
            obj.entry("targetGid".to_string()).or_insert_with(|| gid.clone());
        }
        if let Some(name) = entry.meta.get("friendName") {
            obj.entry("friendName".to_string())
                .or_insert_with(|| name.clone());
        }
        if let Some(result) = entry.meta.get("result") {
            obj.entry("result".to_string())
                .or_insert_with(|| result.clone());
        }
        if let Some(actions) = entry.meta.get("actions") {
            obj.entry("actions".to_string())
                .or_insert_with(|| actions.clone());
        }
        obj.entry("isWarn".to_string())
            .or_insert_with(|| json!(entry.is_warn));
        obj.entry("tag".to_string())
            .or_insert_with(|| json!(entry.tag));
    }
    payload
}

fn derived_from_log(entry: &LogEntry, payload: &Value, account_id: Option<&str>) -> Vec<PanelRealtimeEvent> {
    let module = entry
        .meta
        .get("module")
        .and_then(Value::as_str)
        .unwrap_or("");
    let event = entry
        .meta
        .get("event")
        .and_then(Value::as_str)
        .unwrap_or("");
    let id = account_id.map(str::to_string);
    let mut out = Vec::new();

    if module == "friend"
        || event == "visit_friend"
        || event == "friend_cycle"
        || event == "care_friend"
        || event == "enter_farm"
    {
        let action = infer_friend_action(payload, event);
        let mut body = payload.clone();
        if let Some(obj) = body.as_object_mut() {
            obj.entry("action".to_string()).or_insert_with(|| json!(action));
            obj.entry("result".to_string()).or_insert_with(|| {
                entry
                    .meta
                    .get("result")
                    .cloned()
                    .unwrap_or_else(|| json!("ok"))
            });
        }
        out.push(PanelRealtimeEvent {
            event_type: "friend_interact".into(),
            payload: body,
            account_id: id.clone(),
        });
    }

    if module == "farm"
        || event == "farm_cycle"
        || event == "harvest_crop"
        || event == "plant_seed"
        || event == "fertilize"
    {
        out.push(PanelRealtimeEvent {
            event_type: "farm_operation".into(),
            payload: payload.clone(),
            account_id: id,
        });
    }

    out
}

fn infer_friend_action(payload: &Value, event: &str) -> &'static str {
    let msg = payload
        .get("message")
        .or_else(|| payload.get("msg"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let actions = payload.get("actions").and_then(Value::as_array);
    let joined = actions
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default();
    let blob = format!("{msg} {joined} {event}");
    if blob.contains('偷') || blob.contains("steal") {
        "steal"
    } else if blob.contains("帮助") || blob.contains("help") || blob.contains("照顾") {
        "help"
    } else if blob.contains("放虫") || blob.contains("bad") {
        "bad"
    } else {
        "visit"
    }
}

impl AppContext {
    /// 订阅运行时事件。
    ///
    /// desktop / 其它非 HTTP 客户端应通过此方法订阅，并将收到的 [`RuntimeEvent`] 包装为 [`AppEvent`]。
    /// HTTP server 的 Socket.IO 转发仍可直接使用 `RuntimeEngine::runtime_state().subscribe()`。
    pub fn subscribe_events(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.engine.subscribe_runtime_events()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qq_farm_core::runtime::runtime_state::LogEntry;

    fn log_entry(event: &str, module: &str, msg: &str) -> LogEntry {
        LogEntry {
            time: "12:00:00".into(),
            tag: "好友".into(),
            msg: msg.into(),
            meta: json!({
                "module": module,
                "event": event,
                "friendGid": 42,
                "friendName": "bob",
                "result": "ok",
                "actions": ["偷2"],
            }),
            ts: 1,
            search_text: String::new(),
            account_id: Some("7".into()),
            account_name: Some("acc".into()),
            is_warn: false,
        }
    }

    #[test]
    fn status_event_uses_web_envelope() {
        let ev = RuntimeEvent::Status {
            account_id: "1".into(),
            account_name: "nick".into(),
            status: json!({
                "connection": { "connected": true },
                "status": { "name": "农夫", "level": 12, "gold": 99, "exp": 3 }
            }),
        };
        let out = AppEvent::from(ev).to_realtime();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].event_type, "status:update");
        assert_eq!(out[0].account_id.as_deref(), Some("1"));
        assert_eq!(out[0].payload["status"]["level"], 12);
        assert_eq!(out[0].payload["status"]["nick"], "农夫");
        assert_eq!(out[0].payload["status"]["online"], true);
    }

    #[test]
    fn visit_friend_log_emits_friend_interact() {
        let ev = RuntimeEvent::Log(log_entry("visit_friend", "friend", "bob: 偷2"));
        let types: Vec<_> = AppEvent::from(ev)
            .to_realtime()
            .into_iter()
            .map(|e| e.event_type)
            .collect();
        assert!(types.contains(&"log:new".to_string()));
        assert!(types.contains(&"friend_interact".to_string()));
        let interact = AppEvent::from(RuntimeEvent::Log(log_entry(
            "visit_friend",
            "friend",
            "bob: 偷2",
        )))
        .to_realtime()
        .into_iter()
        .find(|e| e.event_type == "friend_interact")
        .expect("friend_interact");
        assert_eq!(interact.payload["action"], "steal");
        assert_eq!(interact.payload["targetGid"], 42);
    }

    #[test]
    fn farm_cycle_log_emits_farm_operation() {
        let mut entry = log_entry("farm_cycle", "farm", "巡查完成");
        entry.tag = "农场".into();
        let types: Vec<_> = AppEvent::from(RuntimeEvent::Log(entry))
            .to_realtime()
            .into_iter()
            .map(|e| e.event_type)
            .collect();
        assert!(types.contains(&"farm_operation".to_string()));
    }
}
