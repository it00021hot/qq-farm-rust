//! 桌面 IPC DTO（camelCase，禁止以 Value 为主返回类型）。

use qq_farm_core::models::types::PlantingStrategy;
use serde::Serialize;

/// 账号摘要。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub id: String,
    pub name: String,
    pub nick: String,
    pub platform: String,
    pub qq: String,
    pub avatar: String,
    pub running: bool,
}

/// 概览快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSnapshot {
    pub ready: bool,
    pub worker_count: usize,
    pub account_count: usize,
    pub accounts: Vec<AccountSummary>,
}

/// 设置只读摘要。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSummary {
    pub account_id: String,
    pub strategy: PlantingStrategy,
    pub preferred_seed: i64,
    pub farm_interval_sec: i64,
    pub farm_min_sec: i64,
    pub farm_max_sec: i64,
    pub automation_farm: bool,
    pub automation_friend: bool,
    pub automation_task: bool,
    pub automation_sell: bool,
}
