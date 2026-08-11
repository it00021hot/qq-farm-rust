//! 加密模块。
//!
//! - [`tsdk`] — 微信 TSDK (`tsdk.wasm`) 封装：用于游戏会话的加解密、握手、心跳等。
//!
//! 阶段 0：仅 [`tsdk::TsdkRuntime`] 的最小可运行实现（加密 + 解密往返）。
//! 阶段 1+：补全 `bind_user` / `heartbeat_tick` 等完整接口。

pub mod tsdk;
