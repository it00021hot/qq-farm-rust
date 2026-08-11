//! 时间工具。

use chrono::{DateTime, Local};

/// 当前本地时间（格式化字符串，yyyy-MM-dd HH:mm:ss）
#[must_use]
pub fn now_local_str() -> String {
    let now: DateTime<Local> = Local::now();
    now.format("%Y-%m-%d %H:%M:%S").to_string()
}
