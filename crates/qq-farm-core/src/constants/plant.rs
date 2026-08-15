//! 种植 / 生长阶段常量（唯一源；勿在业务文件再定义一份）。

/// 阶段中文名（下标 = [`crate::config::PlantPhase`] as usize）
pub const PHASE_NAMES: [&str; 8] = ["未知", "种子", "发芽", "小叶", "大叶", "开花", "成熟", "枯死"];

pub const PHASE_UNKNOWN: i32 = 0;
pub const PHASE_MATURE: i32 = 6;
pub const PHASE_DEAD: i32 = 7;
