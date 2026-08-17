//! 面板运行日志入口，对齐原 `setLogHook` → `GLOBAL_LOGS.push` + `log:new`。
//!
//! 多账号 in-process：按 account_id 注册 hook。Worker 日志直接写入 RuntimeState，
//! 不经过 WorkerEvent broadcast，避免状态洪峰把日志挤掉。

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use parking_lot::RwLock;
use serde_json::Value;
use tokio::sync::broadcast;

use crate::constants::PanelEvent;
use crate::runtime::events::WorkerEvent;
use crate::runtime::runtime_state::RuntimeState;

struct PanelLogHook {
    account_id: String,
    account_name: String,
    tx: Option<broadcast::Sender<WorkerEvent>>,
    state: Option<Arc<RuntimeState>>,
}

fn hooks() -> &'static RwLock<HashMap<String, Arc<PanelLogHook>>> {
    static HOOKS: OnceLock<RwLock<HashMap<String, Arc<PanelLogHook>>>> = OnceLock::new();
    HOOKS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Worker 启动时挂上（仅 WorkerEvent，给单测 / CLI 用）。
pub fn register(account_id: &str, account_name: &str, tx: broadcast::Sender<WorkerEvent>) {
    register_inner(account_id, account_name, Some(tx), None);
}

/// Worker 启动时挂上，日志直接进 RuntimeState（对齐 TS `GLOBAL_LOGS.push`）。
pub fn register_with_runtime(
    account_id: &str,
    account_name: &str,
    state: Arc<RuntimeState>,
) {
    register_inner(account_id, account_name, None, Some(state));
}

fn register_inner(
    account_id: &str,
    account_name: &str,
    tx: Option<broadcast::Sender<WorkerEvent>>,
    state: Option<Arc<RuntimeState>>,
) {
    if account_id.is_empty() {
        return;
    }
    hooks().write().insert(
        account_id.to_string(),
        Arc::new(PanelLogHook {
            account_id: account_id.to_string(),
            account_name: account_name.to_string(),
            tx,
            state,
        }),
    );
}

/// Worker 停止时摘掉，避免往已关闭通道写。
pub fn unregister(account_id: &str) {
    if account_id.is_empty() {
        return;
    }
    hooks().write().remove(account_id);
}

/// 对齐原 `log(tag, msg, meta)`。`event` 必须是 [`PanelEvent`]（存储英文 snake_case）。
pub fn log(
    account_id: &str,
    tag: &str,
    msg: impl AsRef<str>,
    event: PanelEvent,
    extra: Option<Value>,
) {
    emit(account_id, tag, msg.as_ref(), false, event, extra);
}

/// 对齐原 `logWarn(tag, msg, meta)`。
pub fn log_warn(
    account_id: &str,
    tag: &str,
    msg: impl AsRef<str>,
    event: PanelEvent,
    extra: Option<Value>,
) {
    emit(account_id, tag, msg.as_ref(), true, event, extra);
}

fn emit(
    account_id: &str,
    tag: &str,
    msg: &str,
    is_warn: bool,
    event: PanelEvent,
    extra: Option<Value>,
) {
    if account_id.is_empty() {
        return;
    }
    let Some(hook) = hooks().read().get(account_id).cloned() else {
        return;
    };
    let module = extra
        .as_ref()
        .and_then(|v| v.get("module"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| event.module().to_string());
    let mut meta = extra.unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = meta.as_object_mut() {
        obj.entry("accountId".to_string())
            .or_insert_with(|| serde_json::json!(hook.account_id));
        obj.entry("accountName".to_string())
            .or_insert_with(|| serde_json::json!(hook.account_name));
        obj.insert("module".to_string(), serde_json::json!(module));
        obj.insert("event".to_string(), serde_json::json!(event.as_str()));
        obj.insert("isWarn".to_string(), serde_json::json!(is_warn));
    }
    if let Some(state) = &hook.state {
        state.log(tag, msg, Some(meta));
        return;
    }
    if let Some(tx) = &hook.tx {
        let _ = tx.send(WorkerEvent::Log {
            account_id: hook.account_id.clone(),
            account_name: hook.account_name.clone(),
            level: if is_warn {
                "warn".to_string()
            } else {
                "info".to_string()
            },
            module,
            message: msg.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_emit_unregister() {
        let (tx, mut rx) = broadcast::channel(8);
        register("acc-log", "测试号", tx);
        log(
            "acc-log",
            "农场",
            "收获完成 2 块土地",
            PanelEvent::HarvestCrop,
            Some(serde_json::json!({ "module": "farm" })),
        );
        let ev = rx.try_recv().expect("log event");
        match ev {
            WorkerEvent::Log {
                account_id,
                module,
                message,
                ..
            } => {
                assert_eq!(account_id, "acc-log");
                assert_eq!(module, "farm");
                assert!(message.contains("收获完成"));
            }
            other => panic!("expected Log, got {other:?}"),
        }
        unregister("acc-log");
        log("acc-log", "农场", "should drop", PanelEvent::FarmCycle, None);
        assert!(rx.try_recv().is_err());
    }
}
