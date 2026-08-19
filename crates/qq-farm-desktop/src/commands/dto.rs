//! 桌面 IPC DTO（camelCase；复杂面板载荷可用 `serde_json::Value`）。

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

/// 本机微信快速授权会话。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WxQuickCreateDto {
    pub session_id: String,
    pub app_id: String,
    pub scope: String,
    pub redirect_uri: String,
    pub state: String,
    pub ports: Vec<u16>,
    pub expires_at: i64,
}

/// 本机微信检测结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WxQuickDetectDto {
    pub port: u16,
    pub authorize_uuid: String,
    pub nickname: String,
    pub headimgurl: String,
}

/// 本机微信 authorize 返回的一次性回调。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WxQuickAuthorizeDto {
    pub redirect_url: String,
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
