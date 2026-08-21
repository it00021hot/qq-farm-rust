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

/// 通知提供方。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationProvider {
    #[default]
    None,
    QqBot,
    WechatBot,
}

/// 全局 QQ 官方机器人凭据（部署者配置一次，用户不可见）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqBotCredentials {
    pub app_id: String,
    pub client_secret: String,
    #[serde(default)]
    pub bot_invite_url: String,
}

impl QqBotCredentials {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.app_id.trim().is_empty() && !self.client_secret.trim().is_empty()
    }

    #[must_use]
    pub fn invite_url(&self) -> String {
        self.bot_invite_url.trim().to_string()
    }
}

/// 用户扫码绑定结果。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqBotBinding {
    pub user_openid: String,
    #[serde(default)]
    pub bound_at: i64,
    #[serde(default)]
    pub nickname: String,
}

impl QqBotBinding {
    #[must_use]
    pub fn is_bound(&self) -> bool {
        !self.user_openid.trim().is_empty()
    }
}

/// QQ Bot 运行时发送配置（凭据 + 目标用户）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqBotConfig {
    pub app_id: String,
    pub client_secret: String,
    pub user_openid: String,
}

impl QqBotConfig {
    #[must_use]
    pub fn has_credentials(&self) -> bool {
        !self.app_id.trim().is_empty() && !self.client_secret.trim().is_empty()
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.has_credentials() && !self.user_openid.trim().is_empty()
    }

    #[must_use]
    pub fn from_credentials(credentials: &QqBotCredentials) -> Option<Self> {
        if !credentials.is_complete() {
            return None;
        }
        Some(Self {
            app_id: credentials.app_id.trim().to_string(),
            client_secret: credentials.client_secret.trim().to_string(),
            user_openid: String::new(),
        })
    }

    #[must_use]
    pub fn from_parts(credentials: &QqBotCredentials, binding: &QqBotBinding) -> Option<Self> {
        if !credentials.is_complete() || !binding.is_bound() {
            return None;
        }
        Some(Self {
            app_id: credentials.app_id.trim().to_string(),
            client_secret: credentials.client_secret.trim().to_string(),
            user_openid: binding.user_openid.trim().to_string(),
        })
    }
}

/// 微信机器人预留配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WechatBotConfig {}

/// 离线提醒配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineReminder {
    pub provider: NotificationProvider,
    #[serde(default, rename = "qqBotBinding")]
    pub qq_bot_binding: QqBotBinding,
    pub wechat_bot: WechatBotConfig,
    pub title: String,
    pub msg: String,
    pub offline_delete_sec: i64,
}

impl OfflineReminder {
    /// 是否已配置为可发送的机器人。
    #[must_use]
    pub fn is_configured(&self) -> bool {
        self.provider == NotificationProvider::QqBot
            && self.qq_bot_binding.is_bound()
            && effective_qq_bot_credentials().is_complete()
    }

    #[must_use]
    pub fn send_config(&self) -> Option<QqBotConfig> {
        QqBotConfig::from_parts(&effective_qq_bot_credentials(), &self.qq_bot_binding)
    }
}

impl Default for OfflineReminder {
    fn default() -> Self {
        default_offline_reminder()
    }
}

/// 默认 OfflineReminder
#[must_use]
pub fn default_offline_reminder() -> OfflineReminder {
    OfflineReminder {
        provider: NotificationProvider::None,
        qq_bot_binding: QqBotBinding::default(),
        wechat_bot: WechatBotConfig::default(),
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
    /// QQ 官方机器人全局凭据
    pub qq_bot_credentials: QqBotCredentials,
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
            qq_bot_credentials: QqBotCredentials::default(),
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

/// 当前 Gateway 需要的 QQ Bot 凭据。
#[must_use]
pub fn gateway_qq_bot_config() -> Option<QqBotConfig> {
    QqBotConfig::from_credentials(&effective_qq_bot_credentials())
}

/// 读取持久化的 QQ Bot 凭据。
#[must_use]
pub fn get_qq_bot_credentials() -> QqBotCredentials {
    STATE.read().qq_bot_credentials.clone()
}

/// 保存 QQ Bot 凭据。
pub fn set_qq_bot_credentials(credentials: QqBotCredentials) {
    STATE.write().qq_bot_credentials = credentials;
    let _ = save_global_config();
}

/// 生效中的 QQ Bot 凭据：环境变量优先，其次持久化配置。
#[must_use]
pub fn effective_qq_bot_credentials() -> QqBotCredentials {
    let env_app = std::env::var("QQ_FARM_QQ_BOT_APP_ID").unwrap_or_default();
    let env_secret = std::env::var("QQ_FARM_QQ_BOT_APP_SECRET").unwrap_or_default();
    let env_url = std::env::var("QQ_FARM_QQ_BOT_INVITE_URL").unwrap_or_default();
    if !env_app.trim().is_empty() && !env_secret.trim().is_empty() {
        return QqBotCredentials {
            app_id: env_app,
            client_secret: env_secret,
            bot_invite_url: env_url,
        };
    }
    get_qq_bot_credentials()
}

/// 将绑定结果写入用户离线提醒。
pub fn apply_qq_bot_binding(username: &str, binding: QqBotBinding) {
    let mut reminder = get_user_offline_reminder(username).unwrap_or_else(get_offline_reminder);
    reminder.provider = NotificationProvider::QqBot;
    reminder.qq_bot_binding = binding;
    set_user_offline_reminder(username, reminder);
}

/// 清除用户 QQ Bot 绑定。
pub fn clear_qq_bot_binding(username: &str) {
    let mut reminder = get_user_offline_reminder(username).unwrap_or_else(get_offline_reminder);
    reminder.qq_bot_binding = QqBotBinding::default();
    if reminder.provider == NotificationProvider::QqBot {
        reminder.provider = NotificationProvider::None;
    }
    set_user_offline_reminder(username, reminder);
}

/// 当前所有已启用且配置完整的 QQ Bot。
#[deprecated(note = "use gateway_qq_bot_config instead")]
#[must_use]
pub fn configured_qq_bots() -> Vec<QqBotConfig> {
    gateway_qq_bot_config().into_iter().collect()
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
///
/// **不阻塞调用方**：序列化在调用方（很快），fs::write + rename 跑在 blocking pool。
/// 返回 `Ok(())` 仅表示调度成功；真正的 I/O 错误通过 `tracing::error!` 记录。
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
        "qqBotCredentials": state.qq_bot_credentials,
    });
    let body = serde_json::to_string_pretty(&data).map_err(std::io::Error::other)?;

    let path = store_file();
    let tmp = path.with_extension("json.tmp");
    let _ = crate::infra::spawn_blocking(move || -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        if let Err(e) = fs::write(&tmp, &body) {
            tracing::error!(path = %tmp.display(), error = %e, "写入 store.json.tmp 失败");
            return Err(e);
        }
        if let Err(e) = fs::rename(&tmp, &path) {
            tracing::error!(path = %path.display(), error = %e, "原子替换 store.json 失败");
            return Err(e);
        }
        Ok(())
    });
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
    if let Some(creds) = data.get("qqBotCredentials") {
        if let Ok(parsed) = serde_json::from_value::<QqBotCredentials>(creds.clone()) {
            new_global.qq_bot_credentials = parsed;
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
    use std::fs;

    /// 把会落盘的 store 操作隔离到临时目录，避免覆盖开发/安装包的 `store.json`。
    struct TempFarmData {
        prev: Option<String>,
        dir: PathBuf,
    }

    impl TempFarmData {
        fn enter() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "qq-farm-global-cfg-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ));
            let _ = fs::create_dir_all(&dir);
            let prev = std::env::var("FARM_DATA_DIR").ok();
            std::env::set_var("FARM_DATA_DIR", &dir);
            Self { prev, dir }
        }
    }

    impl Drop for TempFarmData {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
            match &self.prev {
                Some(v) => std::env::set_var("FARM_DATA_DIR", v),
                None => std::env::remove_var("FARM_DATA_DIR"),
            }
        }
    }

    fn reset() -> TempFarmData {
        let guard = TempFarmData::enter();
        set_state(GlobalConfigState::new());
        crate::models::store::account_config::set_state(
            crate::models::store::account_config::AccountConfigState::new(),
        );
        guard
    }

    #[test]
    #[serial(global_config)]
    #[serial(farm_data_dir)]
    fn ui_theme_default() {
        let _dir = reset();
        let ui = get_ui();
        assert_eq!(ui.theme, "light");
    }

    #[test]
    #[serial(global_config)]
    #[serial(farm_data_dir)]
    fn ui_theme_set_valid() {
        let _dir = reset();
        set_ui_theme("dark");
        assert_eq!(get_ui().theme, "dark");
    }

    #[test]
    #[serial(global_config)]
    #[serial(farm_data_dir)]
    fn ui_theme_set_invalid_ignored() {
        let _dir = reset();
        set_ui_theme("invalid");
        assert_eq!(get_ui().theme, "light");
    }

    #[test]
    #[serial(global_config)]
    #[serial(farm_data_dir)]
    fn announcement_show_logic() {
        let _dir = reset();
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
    #[serial(farm_data_dir)]
    fn announcement_show_always_when_not_show_once() {
        let _dir = reset();
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
    #[serial(farm_data_dir)]
    fn user_offline_reminder_crud() {
        let _dir = reset();
        let r = default_offline_reminder();
        set_user_offline_reminder("alice", r.clone());
        assert!(get_user_offline_reminder("alice").is_some());
        assert!(delete_user_offline_reminder("alice"));
        assert!(get_user_offline_reminder("alice").is_none());
    }

    #[test]
    #[serial(global_config)]
    #[serial(farm_data_dir)]
    fn qq_bot_offline_reminder_requires_binding_and_credentials() {
        let _dir = reset();
        assert!(!default_offline_reminder().is_configured());
        assert!(!OfflineReminder::default().is_configured());
        let mut reminder = OfflineReminder {
            provider: NotificationProvider::QqBot,
            qq_bot_binding: QqBotBinding {
                user_openid: "openid".into(),
                bound_at: 1,
                nickname: String::new(),
            },
            ..Default::default()
        };
        assert!(!reminder.is_configured());
        set_qq_bot_credentials(QqBotCredentials {
            app_id: "app".into(),
            client_secret: "secret".into(),
            bot_invite_url: String::new(),
        });
        assert!(reminder.is_configured());
        assert!(reminder.send_config().is_some());
        reminder.provider = NotificationProvider::WechatBot;
        assert!(!reminder.is_configured());
    }

    #[test]
    fn qq_bot_credentials_invite_url_is_explicit_only() {
        let creds = QqBotCredentials {
            app_id: "123".into(),
            client_secret: "sec".into(),
            bot_invite_url: String::new(),
        };
        assert!(creds.invite_url().is_empty());
        let with_url = QqBotCredentials {
            bot_invite_url: "https://example.com/bot".into(),
            ..creds
        };
        assert_eq!(with_url.invite_url(), "https://example.com/bot");
    }

    #[test]
    fn legacy_notification_shape_is_rejected() {
        let legacy = serde_json::json!({
            "channel": "webhook",
            "endpoint": "https://example.com",
            "token": "legacy",
            "title": "old",
            "msg": "old",
            "offlineDeleteSec": 0
        });
        assert!(serde_json::from_value::<OfflineReminder>(legacy).is_err());
    }

    #[test]
    #[serial(global_config)]
    #[serial(farm_data_dir)]
    fn admin_password_hash_set_get() {
        let _dir = reset();
        set_admin_password_hash("hash_abc".to_string());
        assert_eq!(get_admin_password_hash(), "hash_abc");
    }

    #[test]
    #[serial(global_config)]
    #[serial(farm_data_dir)]
    fn system_config_roundtrip() {
        let _dir = reset();
        let mut sys = crate::config::system_config::SystemConfig::default_system();
        sys.server_url = "wss://test.com".to_string();
        set_system_config(sys.clone());
        let got = get_system_config().expect("get");
        assert_eq!(got.server_url, "wss://test.com");
    }
}
