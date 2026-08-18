//! 全局 UI / 公告 / 管理员密码 / 系统配置管理。
//!
//! 1:1 翻译原 `core/src/models/store/global-config.ts`（281 行）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::config::paths::get_data_file;
use crate::config::system_config::SystemConfig;

const UI_THEMES: &[&str] = &["light", "dark"];

/// UI 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIConfig {
    pub theme: String,
}

impl Default for UIConfig {
    fn default() -> Self {
        Self { theme: "light".to_string() }
    }
}

/// 离线提醒配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineReminder {
    pub channel: String,
    #[serde(alias = "relogin_url_mode")]
    pub relogin_url_mode: String,
    pub endpoint: String,
    pub token: String,
    pub title: String,
    pub msg: String,
    #[serde(alias = "offline_delete_sec")]
    pub offline_delete_sec: i64,
}

impl OfflineReminder {
    /// 是否已填到可尝试推送。
    ///
    /// 出厂默认 `channel=webhook` 但地址/token 都空，不算已配置，避免刷运行日志。
    #[must_use]
    pub fn is_configured(&self) -> bool {
        let channel = self.channel.trim().to_ascii_lowercase();
        if channel.is_empty() || channel == "none" {
            return false;
        }
        !self.endpoint.trim().is_empty() || !self.token.trim().is_empty()
    }
}

/// 默认 OfflineReminder
#[must_use]
pub fn default_offline_reminder() -> OfflineReminder {
    OfflineReminder {
        channel: "webhook".to_string(),
        relogin_url_mode: "none".to_string(),
        endpoint: String::new(),
        token: String::new(),
        title: "账号下线提醒".to_string(),
        msg: "账号下线".to_string(),
        offline_delete_sec: 0,
    }
}

/// 公告
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    pub content: String,
    #[serde(alias = "show_once")]
    pub show_once: bool,
    #[serde(alias = "updated_at")]
    pub updated_at: i64,
}

impl Announcement {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

/// 全局配置状态
#[derive(Clone)]
pub struct GlobalConfigState {
    /// UI 主题
    pub ui: UIConfig,
    /// 全局默认离线提醒
    pub offline_reminder: OfflineReminder,
    /// 按用户覆盖的离线提醒
    pub user_offline_reminders: HashMap<String, OfflineReminder>,
    /// 管理员密码（hash）
    pub admin_password_hash: String,
    /// 公告
    pub announcement: Announcement,
    /// 公告已读记录（username -> timestamp）
    pub announcement_read_records: HashMap<String, i64>,
    /// 系统配置（设备 / serverUrl / clientVersion 等）
    pub system_config: Option<SystemConfig>,
}

impl GlobalConfigState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ui: UIConfig::default(),
            offline_reminder: default_offline_reminder(),
            user_offline_reminders: HashMap::new(),
            admin_password_hash: String::new(),
            announcement: Announcement::default(),
            announcement_read_records: HashMap::new(),
            system_config: None,
        }
    }
}

impl Default for GlobalConfigState {
    fn default() -> Self {
        Self::new()
    }
}

static STATE: once_cell::sync::Lazy<Arc<RwLock<GlobalConfigState>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(GlobalConfigState::new())));

/// 获取全局状态
#[must_use]
pub fn state() -> Arc<RwLock<GlobalConfigState>> {
    Arc::clone(&STATE)
}

/// 替换全局状态
pub fn set_state(new: GlobalConfigState) {
    *STATE.write() = new;
}

// =====================================================================
// UI
// =====================================================================

/// 获取 UI 配置
#[must_use]
pub fn get_ui() -> UIConfig {
    STATE.read().ui.clone()
}

/// 设置主题
pub fn set_ui_theme(theme: &str) {
    let normalized = theme.to_ascii_lowercase();
    if UI_THEMES.contains(&normalized.as_str()) {
        STATE.write().ui.theme = normalized;
        let _ = save_global_config();
    }
}

// =====================================================================
// 离线提醒
// =====================================================================

/// 获取全局离线提醒
#[must_use]
pub fn get_offline_reminder() -> OfflineReminder {
    STATE.read().offline_reminder.clone()
}

/// 设置全局离线提醒
pub fn set_offline_reminder(reminder: OfflineReminder) {
    STATE.write().offline_reminder = reminder;
    let _ = save_global_config();
}

/// 获取某用户离线提醒
#[must_use]
pub fn get_user_offline_reminder(username: &str) -> Option<OfflineReminder> {
    STATE.read().user_offline_reminders.get(username).cloned()
}

/// 设置某用户离线提醒
pub fn set_user_offline_reminder(username: &str, reminder: OfflineReminder) {
    STATE.write().user_offline_reminders.insert(username.to_string(), reminder);
    let _ = save_global_config();
}

/// 删除某用户离线提醒
pub fn delete_user_offline_reminder(username: &str) -> bool {
    let removed = STATE.write().user_offline_reminders.remove(username).is_some();
    if removed {
        let _ = save_global_config();
    }
    removed
}

// =====================================================================
// 管理员密码 hash
// =====================================================================

/// 获取管理员密码 hash
#[must_use]
pub fn get_admin_password_hash() -> String {
    STATE.read().admin_password_hash.clone()
}

/// 设置管理员密码 hash
pub fn set_admin_password_hash(hash: String) {
    STATE.write().admin_password_hash = hash;
    let _ = save_global_config();
}

// =====================================================================
// 公告
// =====================================================================

/// 获取公告
#[must_use]
pub fn get_announcement() -> Announcement {
    STATE.read().announcement.clone()
}

/// 设置公告
pub fn set_announcement(ann: Announcement) {
    STATE.write().announcement = ann;
    let _ = save_global_config();
}

/// 获取用户公告已读时间
#[must_use]
pub fn get_announcement_read_record(username: &str) -> i64 {
    STATE.read().announcement_read_records.get(username).copied().unwrap_or(0)
}

/// 标记公告已读
pub fn mark_announcement_read(username: &str) {
    STATE
        .write()
        .announcement_read_records
        .insert(username.to_string(), crate::utils::time::now_secs());
    let _ = save_global_config();
}

/// 是否应显示公告
#[must_use]
pub fn should_show_announcement(username: &str) -> bool {
    let ann = STATE.read().announcement.clone();
    if ann.is_empty() {
        return false;
    }
    if ann.show_once {
        return get_announcement_read_record(username) < ann.updated_at;
    }
    true
}

// =====================================================================
// 系统配置
// =====================================================================

/// 获取系统配置
#[must_use]
pub fn get_system_config() -> Option<SystemConfig> {
    STATE.read().system_config.clone()
}

/// 设置系统配置
/// 重置系统配置（清空）
pub fn reset_system_config() {
    STATE.write().system_config = None;
    let _ = save_global_config();
}

pub fn set_system_config(cfg: SystemConfig) {
    STATE.write().system_config = Some(cfg);
    let _ = save_global_config();
}

// =====================================================================
// 文件持久化
// =====================================================================

/// 全局 store 文件路径
#[must_use]
pub fn store_file() -> PathBuf {
    get_data_file("store.json")
}

/// 保存全局配置到文件（原子写）
pub fn save_global_config() -> std::io::Result<()> {
    use std::fs;
    let state = STATE.read().clone();
    let account_state = crate::models::store::account_config::state();
    let account_state = account_state.read().clone();

    let data = serde_json::json!({
        "accountConfigs": account_state.account_configs,
        "defaultAccountConfig": account_state.default_account_config,
        "ui": state.ui,
        "offlineReminder": state.offline_reminder,
        "userOfflineReminders": state.user_offline_reminders,
        "adminPasswordHash": state.admin_password_hash,
        "announcement": state.announcement,
        "announcementReadRecords": state.announcement_read_records,
        "systemConfig": state.system_config,
    });
    let body = serde_json::to_string_pretty(&data).map_err(std::io::Error::other)?;

    let path = store_file();
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, body)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// 从文件加载全局配置
pub fn load_global_config() -> std::io::Result<()> {
    use std::fs;
    let path = store_file();
    if !path.exists() {
        return Ok(());
    }
    let raw = fs::read_to_string(&path)?;
    let data: serde_json::Value = serde_json::from_str(&raw).map_err(std::io::Error::other)?;

    let mut new_global = GlobalConfigState::new();
    if let Some(ui) = data.get("ui") {
        if let Ok(parsed) = serde_json::from_value::<UIConfig>(ui.clone()) {
            new_global.ui = parsed;
            let theme = new_global.ui.theme.to_ascii_lowercase();
            new_global.ui.theme =
                if UI_THEMES.contains(&theme.as_str()) { theme } else { "dark".to_string() };
        }
    }
    if let Some(rem) = data.get("offlineReminder") {
        if let Ok(parsed) = serde_json::from_value::<OfflineReminder>(rem.clone()) {
            new_global.offline_reminder = parsed;
        }
    }
    if let Some(map) = data.get("userOfflineReminders").and_then(|v| v.as_object()) {
        for (k, v) in map {
            if let Ok(parsed) = serde_json::from_value::<OfflineReminder>(v.clone()) {
                new_global.user_offline_reminders.insert(k.clone(), parsed);
            }
        }
    }
    if let Some(h) = data.get("adminPasswordHash").and_then(|v| v.as_str()) {
        new_global.admin_password_hash = h.to_string();
    }
    if let Some(ann) = data.get("announcement") {
        if let Ok(parsed) = serde_json::from_value::<Announcement>(ann.clone()) {
            new_global.announcement = parsed;
        }
    }
    if let Some(map) = data.get("announcementReadRecords").and_then(|v| v.as_object()) {
        for (k, v) in map {
            if let Some(t) = v.as_i64() {
                new_global.announcement_read_records.insert(k.clone(), t);
            }
        }
    }
    if let Some(sys) = data.get("systemConfig") {
        if let Ok(parsed) = serde_json::from_value::<SystemConfig>(sys.clone()) {
            new_global.system_config = Some(parsed);
        }
    }
    set_state(new_global);

    // 加载 account configs。默认/回退始终用代码里的 DefaultAccountConfig，
    // 避免 store.json 里旧的 defaultAccountConfig 把帮忙/捣乱又打开。
    if let Some(map) = data.get("accountConfigs").and_then(|v| v.as_object()) {
        let mut new_acc = crate::models::store::account_config::AccountConfigState::new();
        let mut migrated = false;
        for (k, v) in map {
            if let Ok(mut parsed) =
                serde_json::from_value::<crate::models::types::AccountConfig>(v.clone())
            {
                if crate::models::store::normalize::migrate_legacy_bot_automation_defaults(
                    &mut parsed.automation,
                ) {
                    migrated = true;
                }
                new_acc.account_configs.insert(k.clone(), parsed);
            } else {
                tracing::warn!(account_id = %k, "账号配置无法解析，已跳过");
            }
        }
        crate::models::store::account_config::set_state(new_acc);
        if migrated {
            let _ = save_global_config();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn reset() {
        set_state(GlobalConfigState::new());
    }

    #[test]
    #[serial(global_config)]
    fn ui_theme_default() {
        reset();
        let ui = get_ui();
        assert_eq!(ui.theme, "light");
    }

    #[test]
    #[serial(global_config)]
    fn ui_theme_set_valid() {
        reset();
        set_ui_theme("dark");
        assert_eq!(get_ui().theme, "dark");
    }

    #[test]
    #[serial(global_config)]
    fn ui_theme_set_invalid_ignored() {
        reset();
        set_ui_theme("invalid");
        assert_eq!(get_ui().theme, "light");
    }

    #[test]
    #[serial(global_config)]
    fn announcement_show_logic() {
        reset();
        // 空公告不显示
        assert!(!should_show_announcement("user1"));
        // 设置公告
        set_announcement(Announcement {
            content: "Hello".to_string(),
            show_once: true,
            updated_at: 1000,
        });
        // 未读 -> 显示
        assert!(should_show_announcement("user1"));
        // 标记已读
        mark_announcement_read("user1");
        // 已读 -> 不显示
        assert!(!should_show_announcement("user1"));
    }

    #[test]
    #[serial(global_config)]
    fn announcement_show_always_when_not_show_once() {
        reset();
        set_announcement(Announcement {
            content: "Always show".to_string(),
            show_once: false,
            updated_at: 1000,
        });
        assert!(should_show_announcement("u1"));
        mark_announcement_read("u1");
        assert!(should_show_announcement("u1"));
    }

    #[test]
    #[serial(global_config)]
    fn user_offline_reminder_crud() {
        reset();
        let r = default_offline_reminder();
        set_user_offline_reminder("alice", r.clone());
        assert!(get_user_offline_reminder("alice").is_some());
        assert!(delete_user_offline_reminder("alice"));
        assert!(get_user_offline_reminder("alice").is_none());
    }

    #[test]
    fn factory_offline_reminder_is_not_configured() {
        assert!(!default_offline_reminder().is_configured());
        assert!(!OfflineReminder::default().is_configured());
        assert!(OfflineReminder {
            channel: "webhook".into(),
            endpoint: "https://example.com/hook".into(),
            ..Default::default()
        }
        .is_configured());
        assert!(OfflineReminder {
            channel: "bark".into(),
            token: "key".into(),
            ..Default::default()
        }
        .is_configured());
        assert!(!OfflineReminder {
            channel: "none".into(),
            token: "key".into(),
            ..Default::default()
        }
        .is_configured());
    }

    #[test]
    #[serial(global_config)]
    fn admin_password_hash_set_get() {
        reset();
        set_admin_password_hash("hash_abc".to_string());
        assert_eq!(get_admin_password_hash(), "hash_abc");
    }

    #[test]
    #[serial(global_config)]
    fn system_config_roundtrip() {
        reset();
        let mut sys = crate::config::system_config::SystemConfig::default_system();
        sys.server_url = "wss://test.com".to_string();
        set_system_config(sys.clone());
        let got = get_system_config().expect("get");
        assert_eq!(got.server_url, "wss://test.com");
    }
}
