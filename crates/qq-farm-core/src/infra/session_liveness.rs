//! 会话存活持久化：记录每个账号"最后确认在线"的时刻。
//!
//! 用途：进程重启后的自动重连必须避开服务端旧 session 释放窗口
//! （重启前会话还活着、杀进程后 TCP 未优雅登出，立刻重登会被判
//! "已在其他终端登录"连环踢——同 timing.rs 里踢线等 3 分钟的教训）。
//! 2026-09-11 实测：重启后 0~2 分钟内自动登录的三个账号全部登录后
//! ~1 秒入站冻结；15 分钟后的重连全部健康。

use std::collections::HashMap;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// 确认在线的落盘节流：期间只更新内存
const PERSIST_THROTTLE_MS: i64 = 30_000;

#[derive(Debug, Default, Serialize, Deserialize)]
struct LivenessFile {
    /// account_id -> 最后确认在线的 epoch 毫秒
    #[serde(default)]
    last_seen: HashMap<String, i64>,
}

struct State {
    data: LivenessFile,
    /// 每个 account 上次落盘时间，用于节流
    last_persist: HashMap<String, i64>,
    loaded: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn data_dir() -> std::path::PathBuf {
    crate::config::paths::get_data_dir()
}

fn liveness_path() -> std::path::PathBuf {
    data_dir().join("session-liveness.json")
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock();
    let state = guard.get_or_insert_with(|| State {
        data: load_from_disk(),
        last_persist: HashMap::new(),
        loaded: true,
    });
    if !state.loaded {
        state.data = load_from_disk();
        state.loaded = true;
    }
    f(state)
}

fn load_from_disk() -> LivenessFile {
    std::fs::read_to_string(liveness_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn persist(data: &LivenessFile) {
    let path = liveness_path();
    if let Ok(json) = serde_json::to_string(data) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // 极小文件 + 30s 节流，直接同步写
        let _ = std::fs::write(&path, json);
    }
}

fn now_ms() -> i64 {
    crate::utils::time::now_ms()
}

/// 账号确认在线（状态广播时调用，内部节流落盘）
pub fn note_online(account_id: &str) {
    let now = now_ms();
    with_state(|s| {
        s.data.last_seen.insert(account_id.to_string(), now);
        let last = *s.last_persist.get(account_id).unwrap_or(&0);
        if now - last >= PERSIST_THROTTLE_MS {
            s.last_persist.insert(account_id.to_string(), now);
            persist(&s.data);
        }
    });
}

/// 账号确认离线（worker 停止时调用，立即落盘）
pub fn note_offline(account_id: &str) {
    with_state(|s| {
        s.data.last_seen.remove(account_id);
        s.last_persist.remove(account_id);
        persist(&s.data);
    });
}

/// 进程启动后自动登录前应额外等待的毫秒数（0 = 可立即登录）。
/// 上次进程的会话若在释放窗口内（见 WX_KICKOUT_RECONNECT_DELAY_MS），
/// 等待剩余时间再登，避免撞上服务端旧 session 释放。
#[must_use]
pub fn boot_delay_ms(account_id: &str) -> u64 {
    let window = crate::constants::WX_KICKOUT_RECONNECT_DELAY_MS as i64;
    let now = now_ms();
    with_state(|s| match s.data.last_seen.get(account_id) {
        Some(seen) => (seen + window - now).max(0) as u64,
        None => 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_delay_respects_window() {
        let mut data = LivenessFile::default();
        let now = now_ms();
        data.last_seen.insert("a1".to_string(), now - 1_000);
        let seen = now - 1_000;
        let window = crate::constants::WX_KICKOUT_RECONNECT_DELAY_MS as i64;
        let delay = (seen + window - now).max(0) as u64;
        assert!(delay > 0 && delay <= window as u64);
    }
}
