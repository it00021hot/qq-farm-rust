//! 邀请码处理 — 通过 `ReportArkClick` 模拟点击分享链接。
//!
//! 1:1 翻译原 `core/src/services/invite.ts`（165 行）。
//!
//! ## 背景
//!
//! 1. 首次登录时，游戏会在 `LoginRequest` 中携带 `sharer_id` 和 `sharer_open_id`
//! 2. 已登录状态下点击分享链接，游戏会发送 `ReportArkClickRequest`
//! 3. 服务器收到后自动向分享者发送好友申请
//!
//! 我们用 `ReportArkClickRequest` 模拟已登录状态下的分享链接点击，
//! 把 `share.txt` 里所有未处理的邀请码一次性触发。
//!
//! ## 注意
//!
//! 该功能仅在微信环境（`platform == "wx"`）下有效。

use std::sync::Arc;
use std::time::Duration;

use prost::Message;

use crate::config::paths::get_share_file_path;
use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::userpb::{ReportArkClickReply, ReportArkClickRequest};
use crate::services::json_db::{read_text_file, write_text_file_atomic};

const USER_SERVICE: &str = "gamepb.userpb.UserService";
/// 微信分享场景 ID
const WECHAT_SHARE_SCENE_ID: &str = "1256";
/// 每条 invite 之间的延迟（毫秒）
pub const INVITE_REQUEST_DELAY_MS: u64 = 2000;

/// 解析后的分享链接
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedShareLink {
    pub uid: Option<String>,
    pub openid: Option<String>,
    pub share_source: Option<String>,
    pub doc_id: Option<String>,
}

/// 邀请处理结果
#[derive(Debug, Clone, Default)]
pub struct InviteProcessResult {
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
}

/// 邀请服务
pub struct InviteService {
    gateway: Arc<Gateway>,
}

impl InviteService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway }
    }

    /// 读取 `share.txt` 文件，解析并去重
    pub fn read_share_file() -> Vec<ParsedShareLink> {
        let path = get_share_file_path();
        let content = read_text_file(&path, "");
        let lines: Vec<&str> = content
            .split('\n')
            .map(str::trim)
            .filter(|l| !l.is_empty() && l.contains("openid="))
            .collect();

        let mut invites: Vec<ParsedShareLink> = Vec::new();
        let mut seen_uids: std::collections::HashSet<String> = std::collections::HashSet::new();

        for line in lines {
            let parsed = parse_share_link(line);
            if let (Some(openid), Some(uid)) = (parsed.openid.as_ref(), parsed.uid.as_ref()) {
                if seen_uids.insert(uid.clone()) {
                    // 跳过空 openid 的项（同时需要 uid 也不为空）
                    if !openid.is_empty() && !uid.is_empty() {
                        invites.push(parsed);
                    }
                }
            }
        }

        invites
    }

    /// 发送 `ReportArkClick` 请求
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn send_report_ark_click(
        &self,
        sharer_id: Option<&str>,
        sharer_open_id: Option<&str>,
        share_source: Option<&str>,
    ) -> Result<ReportArkClickReply> {
        let sharer_id_num: i64 = sharer_id.and_then(|s| s.parse().ok()).unwrap_or(0);
        let share_cfg_id_num: i64 = share_source.and_then(|s| s.parse().ok()).unwrap_or(0);
        let req = ReportArkClickRequest {
            sharer_id: sharer_id_num,
            sharer_open_id: sharer_open_id.unwrap_or("").to_string(),
            share_cfg_id: share_cfg_id_num,
            scene_id: WECHAT_SHARE_SCENE_ID.to_string(),
        };
        let body = self
            .gateway
            .request(USER_SERVICE, "ReportArkClick", &req.encode_to_vec(), 10_000)
            .await?;
        Ok(ReportArkClickReply::decode(&body[..])?)
    }

    /// 处理邀请码列表（仅微信环境）
    pub async fn process_invite_codes(&self) -> InviteProcessResult {
        if self.gateway.platform() != "wx" {
            tracing::info!("[邀请] 当前为 QQ 环境，跳过邀请码处理（仅微信支持）");
            return InviteProcessResult::default();
        }

        let invites = Self::read_share_file();
        if invites.is_empty() {
            return InviteProcessResult::default();
        }

        tracing::info!("[邀请] 读取到 {} 个邀请码（已去重），开始逐个处理...", invites.len());

        let mut result = InviteProcessResult { attempted: invites.len(), ..Default::default() };
        for (i, invite) in invites.iter().enumerate() {
            match self
                .send_report_ark_click(
                    invite.uid.as_deref(),
                    invite.openid.as_deref(),
                    invite.share_source.as_deref(),
                )
                .await
            {
                Ok(_) => {
                    result.succeeded += 1;
                    tracing::info!(
                        "[邀请] [{}/{}] 已向 uid={} 发送好友申请",
                        i + 1,
                        invites.len(),
                        invite.uid.as_deref().unwrap_or("?")
                    );
                }
                Err(e) => {
                    result.failed += 1;
                    tracing::warn!(
                        "[邀请] [{}/{}] 向 uid={} 发送申请失败: {}",
                        i + 1,
                        invites.len(),
                        invite.uid.as_deref().unwrap_or("?"),
                        e
                    );
                }
            }

            if i + 1 < invites.len() {
                tokio::time::sleep(Duration::from_millis(INVITE_REQUEST_DELAY_MS)).await;
            }
        }

        tracing::info!("[邀请] 处理完成: 成功 {}, 失败 {}", result.succeeded, result.failed);

        Self::clear_share_file();
        result
    }

    /// 清空 share.txt
    pub fn clear_share_file() {
        let path = get_share_file_path();
        if let Err(e) = write_text_file_atomic(&path, "") {
            tracing::warn!("[邀请] 清空 share.txt 失败: {}", e);
        } else {
            tracing::info!("[邀请] 已清空 share.txt");
        }
    }
}

// =====================================================================
// 纯函数
// =====================================================================

/// 解析分享链接字符串，提取 uid / openid / share_source / doc_id
///
/// 接受完整 URL（带或不带 `?` 前缀）、query string、或 `key=val&key=val` 格式
#[must_use]
pub fn parse_share_link(link: &str) -> ParsedShareLink {
    let mut result = ParsedShareLink::default();
    let query_str = link.strip_prefix('?').unwrap_or(link);

    for pair in query_str.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.split_once('=') {
            Some(kv) => kv,
            None => continue,
        };
        let value = url_decode(v);
        match k {
            "uid" => result.uid = Some(value),
            "openid" => result.openid = Some(value),
            "share_source" => result.share_source = Some(value),
            "doc_id" => result.doc_id = Some(value),
            _ => {}
        }
    }

    result
}

/// 简易 URL 解码（处理 `%XX` 和 `+`）
fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_value(bytes[i + 1]);
                let lo = hex_value(bytes[i + 2]);
                match (hi, lo) {
                    (Some(h), Some(l)) => {
                        out.push((h << 4) | l);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_constant_matches_ts() {
        assert_eq!(USER_SERVICE, "gamepb.userpb.UserService");
    }

    #[test]
    fn wechat_scene_id_constant() {
        assert_eq!(WECHAT_SHARE_SCENE_ID, "1256");
    }

    #[test]
    fn parse_share_link_basic() {
        let p = parse_share_link("?uid=123&openid=abc&share_source=42&doc_id=doc1");
        assert_eq!(p.uid.as_deref(), Some("123"));
        assert_eq!(p.openid.as_deref(), Some("abc"));
        assert_eq!(p.share_source.as_deref(), Some("42"));
        assert_eq!(p.doc_id.as_deref(), Some("doc1"));
    }

    #[test]
    fn parse_share_link_no_question_mark() {
        let p = parse_share_link("uid=123&openid=abc");
        assert_eq!(p.uid.as_deref(), Some("123"));
        assert_eq!(p.openid.as_deref(), Some("abc"));
    }

    #[test]
    fn parse_share_link_missing_fields() {
        let p = parse_share_link("?uid=123");
        assert_eq!(p.uid.as_deref(), Some("123"));
        assert_eq!(p.openid, None);
        assert_eq!(p.share_source, None);
        assert_eq!(p.doc_id, None);
    }

    #[test]
    fn parse_share_link_empty() {
        let p = parse_share_link("");
        assert_eq!(p, ParsedShareLink::default());
    }

    #[test]
    fn parse_share_link_url_encoded() {
        let p = parse_share_link("?uid=user%20123&openid=abc%2Bdef");
        assert_eq!(p.uid.as_deref(), Some("user 123"));
        assert_eq!(p.openid.as_deref(), Some("abc+def"));
    }

    #[test]
    fn parse_share_link_plus_to_space() {
        let p = parse_share_link("uid=user+123&openid=abc");
        assert_eq!(p.uid.as_deref(), Some("user 123"));
    }

    #[test]
    fn parse_share_link_skips_malformed() {
        let p = parse_share_link("?uid=123&malformed&openid=abc");
        assert_eq!(p.uid.as_deref(), Some("123"));
        assert_eq!(p.openid.as_deref(), Some("abc"));
    }

    #[test]
    fn url_decode_handles_unicode() {
        // %E4%B8%AD = 中 (UTF-8)
        let s = url_decode("%E4%B8%AD%E6%96%87");
        assert_eq!(s, "中文");
    }

    #[test]
    fn url_decode_passthrough() {
        let s = url_decode("hello-world_123");
        assert_eq!(s, "hello-world_123");
    }

    #[test]
    fn url_decode_invalid_pct_kept() {
        let s = url_decode("a%ZZb");
        // %ZZ 无法解析：保留字面
        assert_eq!(s, "a%ZZb");
    }

    #[test]
    fn hex_value_basic() {
        assert_eq!(hex_value(b'0'), Some(0));
        assert_eq!(hex_value(b'9'), Some(9));
        assert_eq!(hex_value(b'a'), Some(10));
        assert_eq!(hex_value(b'F'), Some(15));
        assert_eq!(hex_value(b'g'), None);
        assert_eq!(hex_value(b'!'), None);
    }

    #[test]
    fn parsed_share_link_default() {
        let p = ParsedShareLink::default();
        assert_eq!(p.uid, None);
        assert_eq!(p.openid, None);
    }

    #[test]
    fn invite_process_result_default() {
        let r = InviteProcessResult::default();
        assert_eq!(r.attempted, 0);
        assert_eq!(r.succeeded, 0);
        assert_eq!(r.failed, 0);
    }
}
