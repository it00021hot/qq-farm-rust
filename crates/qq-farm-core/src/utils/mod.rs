//! 通用工具。
//!
//! 1:1 翻译原 `core/src/utils/` 下 9 个文件中的 5 个：
//!
//! - [`logger`] — `tracing` 初始化（保持）
//! - [`time`] — 时间 / 服务器时间同步 / 时间窗口判断
//! - [`random`] — sleep / random delay / gateway token
//! - [`login_url`] — 登录 URL 解析（提取 code / client hints）
//! - [`qr`] — QR 登录 cookie / hash 工具
//! - [`decode`] — Protobuf 调试解码（CLI 用）

pub mod decode;
pub mod logger;
pub mod login_url;
pub mod qr;
pub mod random;
pub mod time;
