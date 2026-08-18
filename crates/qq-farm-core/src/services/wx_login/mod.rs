//! 微信扫码登录 — 协议层 + HTTP QR 流程。
//!
//! 子模块：
//! - [`native_protocol`] — MMTLS 编码原语（varint / protobuf / LZ4 / AES-GCM / ECDH）
//! - [`service`] — 微信 QR 会话流程
//! - [`local_wechat`] — 本机微信 HTTPS（桌面进程代理，不走 WebView）

pub mod local_wechat;
pub mod native_protocol;
pub mod service;
pub mod wx_auth;

pub use local_wechat::{
    LocalWechatAuthorizeResult, LocalWechatClient, LocalWechatOAuth, LocalWechatPayload,
    LocalWechatPosition, LocalWechatProfile,
};
pub use wx_auth::{WxAuthError, WxAuthErrorKind, YybCredentials};
