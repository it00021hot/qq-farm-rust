//! 状态栏 — 终端固定位置显示用户状态。
//!
//! 1:1 翻译原 `core/src/services/status.ts`（204 行）。
//!
//! 使用 ANSI 转义码在终端顶部显示固定状态栏（平台 / 昵称 / 等级 / 金币 / 经验）。
//! 主要用于 TTY 环境；非 TTY 时 init 返回 false。
//!
//! 注意：真实终端交互测试受环境限制，本文件以单元测试覆盖纯逻辑部分。

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::config::game_config::global as global_game_config;

// =====================================================================
// 状态数据
// =====================================================================

/// 状态数据
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StatusData {
    pub platform: String,
    pub name: String,
    pub level: i64,
    pub gold: i64,
    pub exp: i64,
}

impl StatusData {
    #[must_use]
    pub fn new() -> Self {
        Self {
            platform: "qq".to_string(),
            ..Default::default()
        }
    }
}

// =====================================================================
// 钩子
// =====================================================================

/// 记录金/经验变化的钩子
pub type RecordGoldExpHook = Arc<dyn Fn(i64, i64) + Send + Sync>;

static RECORD_HOOK: Mutex<Option<RecordGoldExpHook>> = Mutex::new(None);

/// 设置金/经验钩子（admin 未加载时为空）
pub fn set_record_gold_exp_hook(hook: RecordGoldExpHook) {
    *RECORD_HOOK.lock() = Some(hook);
}

// =====================================================================
// ANSI 转义码
// =====================================================================

const ESC: &str = "\x1B";
const SAVE_CURSOR: &str = "\x1B7";
const RESTORE_CURSOR: &str = "\x1B8";
const CLEAR_LINE: &str = "\x1B[2K";
const RESET_SCROLL: &str = "\x1B[r";
const BOLD: &str = "\x1B[1m";
const RESET: &str = "\x1B[0m";
const DIM: &str = "\x1B[2m";
const CYAN: &str = "\x1B[36m";
const YELLOW: &str = "\x1B[33m";
const GREEN: &str = "\x1B[32m";
const MAGENTA: &str = "\x1B[35m";

const STATUS_LINES: usize = 2;

fn move_to(row: usize, col: usize) -> String {
    format!("{ESC}[{row};{col}H")
}

fn scroll_region(top: usize, bottom: usize) -> String {
    format!("{ESC}[{top};{bottom}r")
}

// =====================================================================
// 全局状态
// =====================================================================

static STATUS_DATA: Mutex<StatusData> = Mutex::new(StatusData {
    platform: String::new(),
    name: String::new(),
    level: 0,
    gold: 0,
    exp: 0,
});
static STATUS_BY_ACCOUNT: Mutex<Option<HashMap<String, StatusData>>> = Mutex::new(None);
static STATUS_ENABLED: Mutex<bool> = Mutex::new(false);
static TERM_ROWS: Mutex<usize> = Mutex::new(24);

fn account_map() -> parking_lot::MutexGuard<'static, Option<HashMap<String, StatusData>>> {
    STATUS_BY_ACCOUNT.lock()
}

fn map_slot(account_id: &str) -> StatusData {
    if account_id.is_empty() {
        return STATUS_DATA.lock().clone();
    }
    let mut guard = account_map();
    let map = guard.get_or_insert_with(HashMap::new);
    map.get(account_id).cloned().unwrap_or_default()
}

fn write_slot(account_id: &str, data: StatusData) {
    if account_id.is_empty() {
        *STATUS_DATA.lock() = data;
        return;
    }
    let mut guard = account_map();
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(account_id.to_string(), data);
}

/// 是否启用了状态栏
#[must_use]
pub fn is_enabled() -> bool {
    *STATUS_ENABLED.lock()
}

/// 读取当前状态数据快照（CLI / 单测；多账号请用 [`status_data_for`]）
#[must_use]
pub fn status_data() -> StatusData {
    STATUS_DATA.lock().clone()
}

/// 按账号读取状态（对齐 TS 每 worker 独立 `statusData`）
#[must_use]
pub fn status_data_for(account_id: &str) -> StatusData {
    map_slot(account_id)
}

/// 检测 stdout 是否为 TTY
fn stdout_is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

// =====================================================================
// 初始化 / 清理
// =====================================================================

/// 初始化状态栏
pub fn init_status_bar(stdout_is_tty_override: Option<bool>) -> bool {
    let is_tty = stdout_is_tty_override.unwrap_or_else(stdout_is_tty);
    if !is_tty {
        return false;
    }

    let rows = 24; // 不主动获取 stdout().rows()，避免引入 crossterm 依赖
    *TERM_ROWS.lock() = rows;
    *STATUS_ENABLED.lock() = true;

    let _ = write_all_stdout(&scroll_region(STATUS_LINES + 1, rows));
    let _ = write_all_stdout(&move_to(STATUS_LINES + 1, 1));
    render_status_bar();
    true
}

/// 清理状态栏
pub fn cleanup_status_bar() {
    if !is_enabled() {
        return;
    }
    *STATUS_ENABLED.lock() = false;
    let _ = write_all_stdout(RESET_SCROLL);
    let _ = write_all_stdout(&format!(
        "{}{}{}{}{}",
        move_to(1, 1),
        CLEAR_LINE,
        move_to(2, 1),
        CLEAR_LINE,
        ""
    ));
}

fn write_all_stdout(s: &str) -> io::Result<()> {
    let mut out = io::stdout().lock();
    out.write_all(s.as_bytes())?;
    out.flush()
}

// =====================================================================
// 渲染
// =====================================================================

/// 渲染状态栏
pub fn render_status_bar() {
    if !is_enabled() {
        return;
    }
    let data = STATUS_DATA.lock().clone();
    let line1 = build_line1(&data);
    let width = 80;
    let line2 = format!("{DIM}{}{RESET}", "─".repeat(width.min(80)));

    let _ = write_all_stdout(SAVE_CURSOR);
    let _ = write_all_stdout(&format!(
        "{}{}{}",
        move_to(1, 1),
        CLEAR_LINE,
        line1
    ));
    let _ = write_all_stdout(&format!(
        "{}{}{}",
        move_to(2, 1),
        CLEAR_LINE,
        line2
    ));
    let _ = write_all_stdout(RESTORE_CURSOR);
}

fn build_line1(data: &StatusData) -> String {
    let platform_str = if data.platform == "wx" {
        format!("{MAGENTA}微信{RESET}")
    } else {
        format!("{CYAN}QQ{RESET}")
    };
    let name_str = if !data.name.is_empty() {
        format!("{BOLD}{}{RESET}", data.name)
    } else {
        "未登录".to_string()
    };
    let level_str = format!("{GREEN}Lv{}{RESET}", data.level);
    let gold_str = format!("{YELLOW}金币:{}{RESET}", data.gold);

    let exp_str = if data.level > 0 && data.exp >= 0 {
        let gc = global_game_config();
        let table = gc.get_level_exp_table();
        if !table.is_empty() {
            let (cur, need) = gc.get_level_exp_progress(data.level, data.exp);
            format!("{DIM}经验:{cur}/{need}{RESET}")
        } else {
            format!("{DIM}经验:{}{RESET}", data.exp)
        }
    } else {
        String::new()
    };

    if exp_str.is_empty() {
        format!("{platform_str} | {name_str} | {level_str} | {gold_str}")
    } else {
        format!("{platform_str} | {name_str} | {level_str} | {gold_str} | {exp_str}")
    }
}

// =====================================================================
// 更新 API
// =====================================================================

/// 更新状态（部分字段）
pub fn update_status(data: &StatusData) {
    let mut current = STATUS_DATA.lock();
    let mut changed = false;
    let mut gold_or_exp_changed = false;
    if current.platform != data.platform {
        current.platform = data.platform.clone();
        changed = true;
    }
    if current.name != data.name {
        current.name = data.name.clone();
        changed = true;
    }
    if current.level != data.level {
        current.level = data.level;
        changed = true;
    }
    if current.gold != data.gold {
        current.gold = data.gold;
        changed = true;
        gold_or_exp_changed = true;
    }
    if current.exp != data.exp {
        current.exp = data.exp;
        changed = true;
        gold_or_exp_changed = true;
    }
    drop(current);

    if changed {
        if is_enabled() {
            render_status_bar();
        }
        // 钩子在金币/经验实际变化时触发（对齐 TS：字段被提供且变化）
        if gold_or_exp_changed {
            let hook = RECORD_HOOK.lock().clone();
            if let Some(h) = hook {
                let snapshot = STATUS_DATA.lock().clone();
                h(snapshot.gold, snapshot.exp);
            }
        }
    }
}

/// 设置平台
pub fn set_status_platform(platform: &str) {
    let mut s = STATUS_DATA.lock().clone();
    s.platform = platform.to_string();
    update_status(&s);
}

/// 按账号设置平台（对齐 TS `setStatusPlatform(CONFIG.platform)`）
pub fn set_status_platform_for(account_id: &str, platform: &str) {
    let mut s = map_slot(account_id);
    s.platform = platform.to_string();
    write_slot(account_id, s.clone());
    if account_id.is_empty() {
        update_status(&s);
    }
}

fn apply_login_fields(s: &mut StatusData, basic: &serde_json::Value) {
    let Some(obj) = basic.as_object() else {
        return;
    };
    if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
        s.name = name.to_string();
    }
    if let Some(level) = obj.get("level").and_then(|v| v.as_i64()) {
        s.level = level;
    }
    if let Some(gold) = obj.get("gold").and_then(|v| v.as_i64()) {
        s.gold = gold;
    }
    if let Some(exp) = obj.get("exp").and_then(|v| v.as_i64()) {
        s.exp = exp;
    }
}

/// 从登录数据更新状态
pub fn update_status_from_login(basic: &serde_json::Value) {
    let mut s = STATUS_DATA.lock().clone();
    apply_login_fields(&mut s, basic);
    update_status(&s);
}

/// 按账号从登录数据更新状态
pub fn update_status_from_login_for(account_id: &str, basic: &serde_json::Value) {
    let mut s = map_slot(account_id);
    apply_login_fields(&mut s, basic);
    write_slot(account_id, s.clone());
    if account_id.is_empty() {
        update_status(&s);
    }
}

/// 更新金币
pub fn update_status_gold(gold: i64) {
    let mut s = STATUS_DATA.lock().clone();
    s.gold = gold;
    update_status(&s);
}

/// 按账号更新金币
pub fn update_status_gold_for(account_id: &str, gold: i64) {
    let mut s = map_slot(account_id);
    s.gold = gold;
    write_slot(account_id, s.clone());
    if account_id.is_empty() {
        update_status(&s);
    }
}

/// 务农/收获回包里的奖励按增量记入金币/经验（对齐 notify 缺失时的面板效率）。
///
/// `FarmingResult.reward` / `HarvestReply.items` 的 count 是本次获得值，不是背包绝对值。
pub fn apply_reward_deltas_for<'a>(
    account_id: &str,
    items: impl IntoIterator<Item = &'a crate::proto::generated::corepb::Item>,
) {
    if account_id.is_empty() {
        return;
    }
    let mut gold_delta: i64 = 0;
    let mut exp_delta: i64 = 0;
    for item in items {
        if item.count <= 0 {
            continue;
        }
        match item.id {
            1101 => exp_delta += item.count,
            1 | 1001 => gold_delta += item.count,
            _ => {}
        }
    }
    if gold_delta == 0 && exp_delta == 0 {
        return;
    }
    let mut s = map_slot(account_id);
    if gold_delta != 0 {
        s.gold = s.gold.saturating_add(gold_delta);
    }
    if exp_delta != 0 {
        s.exp = s.exp.saturating_add(exp_delta);
    }
    write_slot(account_id, s);
}

/// 更新等级和经验
pub fn update_status_level(level: i64, exp: Option<i64>) {
    let mut s = STATUS_DATA.lock().clone();
    s.level = level;
    if let Some(e) = exp {
        s.exp = e;
    }
    update_status(&s);
}

/// 按账号更新等级和经验
pub fn update_status_level_for(account_id: &str, level: i64, exp: Option<i64>) {
    let mut s = map_slot(account_id);
    s.level = level;
    if let Some(e) = exp {
        s.exp = e;
    }
    write_slot(account_id, s.clone());
    if account_id.is_empty() {
        update_status(&s);
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        *STATUS_DATA.lock() = StatusData {
            platform: String::new(),
            name: String::new(),
            level: 0,
            gold: 0,
            exp: 0,
        };
        *STATUS_BY_ACCOUNT.lock() = None;
        *STATUS_ENABLED.lock() = false;
        *RECORD_HOOK.lock() = None;
    }

    #[test]
    fn status_data_default() {
        let s = StatusData::default();
        assert_eq!(s.platform, "");
        assert_eq!(s.level, 0);
        assert_eq!(s.gold, 0);
    }

    #[test]
    fn init_status_bar_non_tty_returns_false() {
        reset();
        let ok = init_status_bar(Some(false));
        assert!(!ok);
        assert!(!is_enabled());
    }

    #[test]
    fn init_status_bar_tty_enables() {
        reset();
        let ok = init_status_bar(Some(true));
        assert!(ok);
        assert!(is_enabled());
        cleanup_status_bar();
    }

    #[test]
    fn update_status_changes_only_when_diff() {
        reset();
        let mut s = StatusData::default();
        s.name = "alice".into();
        update_status(&s);
        assert_eq!(status_data().name, "alice");

        // 再次相同 update 不该算变化（这里无 flag 直接 snapshot 验证）
        update_status(&s);
        assert_eq!(status_data().name, "alice");

        // 修改 gold
        s.gold = 100;
        update_status(&s);
        assert_eq!(status_data().gold, 100);
    }

    #[test]
    fn set_status_platform_changes_platform() {
        reset();
        set_status_platform("wx");
        assert_eq!(status_data().platform, "wx");
        set_status_platform("qq");
        assert_eq!(status_data().platform, "qq");
    }

    #[test]
    fn set_status_platform_for_isolates_accounts() {
        reset();
        set_status_platform_for("acc-wx", "wx");
        set_status_platform_for("acc-qq", "qq");
        assert_eq!(status_data_for("acc-wx").platform, "wx");
        assert_eq!(status_data_for("acc-qq").platform, "qq");
    }

    #[test]
    fn update_status_from_login_extracts() {
        reset();
        let basic = serde_json::json!({
            "name": "bob",
            "level": 10,
            "gold": 500,
            "exp": 1000,
        });
        update_status_from_login(&basic);
        let s = status_data();
        assert_eq!(s.name, "bob");
        assert_eq!(s.level, 10);
        assert_eq!(s.gold, 500);
        assert_eq!(s.exp, 1000);
    }

    #[test]
    fn update_status_from_login_keeps_existing_on_missing() {
        reset();
        let mut s = StatusData::default();
        s.name = "keep".into();
        s.level = 5;
        update_status(&s);
        // 只传 level
        let basic = serde_json::json!({"level": 7});
        update_status_from_login(&basic);
        let after = status_data();
        assert_eq!(after.name, "keep");
        assert_eq!(after.level, 7);
    }

    #[test]
    fn test_update_status_gold() {
        reset();
        super::update_status_gold(999);
        assert_eq!(status_data().gold, 999);
    }

    #[test]
    fn update_status_level_with_exp() {
        reset();
        update_status_level(20, Some(5000));
        let s = status_data();
        assert_eq!(s.level, 20);
        assert_eq!(s.exp, 5000);
    }

    #[test]
    fn update_status_level_without_exp() {
        reset();
        update_status_level(15, None);
        let s = status_data();
        assert_eq!(s.level, 15);
        assert_eq!(s.exp, 0);
    }

    #[test]
    fn build_line1_includes_all_fields() {
        let _ = global_game_config();
        let mut s = StatusData::default();
        s.name = "test".into();
        s.level = 5;
        s.gold = 100;
        s.exp = 200;
        s.platform = "qq".into();
        let line = build_line1(&s);
        assert!(line.contains("test"));
        assert!(line.contains("Lv5"));
        assert!(line.contains("100"));
    }

    #[test]
    fn build_line1_handles_wx_platform() {
        let _ = global_game_config();
        let mut s = StatusData::default();
        s.platform = "wx".into();
        let line = build_line1(&s);
        assert!(line.contains("微信"));
    }

    #[test]
    fn build_line1_no_exp_when_level_zero() {
        let _ = global_game_config();
        let s = StatusData::default();
        let line = build_line1(&s);
        assert!(!line.contains("经验"));
    }

    #[test]
    fn cleanup_when_disabled_noop() {
        reset();
        cleanup_status_bar(); // 不应 panic
    }

    #[test]
    fn apply_reward_deltas_adds_farming_exp() {
        reset();
        update_status_from_login_for(
            "acc-reward",
            &serde_json::json!({ "level": 80, "gold": 1000, "exp": 5000 }),
        );
        let items = vec![crate::proto::generated::corepb::Item {
            id: 1101,
            count: 12,
            ..Default::default()
        }];
        apply_reward_deltas_for("acc-reward", &items);
        let s = status_data_for("acc-reward");
        assert_eq!(s.exp, 5012);
        assert_eq!(s.gold, 1000);
    }
}
