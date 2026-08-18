//! 微信扫码登录 — 协议层 + HTTP QR 流程。
//!
//! 子模块：
//! - [`native_protocol`] — MMTLS 编码原语（varint / protobuf / LZ4 / AES-GCM / ECDH）
//! - [`service`] — 微信 QR 会话流程

pub mod native_protocol;
pub mod service;
pub mod wx_auth;

pub use wx_auth::{WxAuthError, WxAuthErrorKind, YybCredentials};
