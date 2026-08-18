//! 桌面 IPC DTO（camelCase；复杂面板载荷可用 `serde_json::Value`）。

use qq_farm_core::models::types::PlantingStrategy;
use serde::{Deserialize, Serialize};

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
    /// 是否已持久化应用宝授权（可换码重连）
    pub wx_authorized: bool,
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

/// 微信扫码创建结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WxLoginCreateDto {
    pub task_id: String,
    pub app_id: String,
    pub status: String,
    pub expires_at: i64,
    /// JPEG 二维码（base64）。
    pub qr_jpeg_base64: String,
}

/// 微信扫码状态。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WxLoginStatusDto {
    pub task_id: String,
    pub app_id: String,
    pub status: String,
    pub expires_at: i64,
}

/// 背包出售单项。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BagSellItem {
    pub item_id: i64,
    pub count: i64,
    #[serde(default)]
    pub uid: i64,
}
