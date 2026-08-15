//! # qq-farm-core
//!
//! QQ 农场业务核心库。
//!
//! ## 架构分层
//!
//! - [`config`] — 配置层（应用配置 + 游戏配置）
//! - [`models`] — 领域模型（账号、土地、好友等）
//! - [`proto`] — Protobuf 生成的 Rust 类型
//! - [`network`] — 网络层（WebSocket 客户端 + 编解码）
//! - [`crypto`] — 加密（tsdk.wasm 封装）
//! - [`runtime`] — 运行时引擎（多账号调度）
//! - [`services`] — 业务服务（农场、好友、活动等）
//! - [`utils`] — 通用工具（日志、时间、字节处理）
//!
//! ## 设计原则
//!
//! 1. **零 IO 入口** — 本 crate 不直接起进程/服务，作为库被 server/cli/app/desktop 引用
//! 2. **依赖倒置** — 业务 trait 不直接依赖具体网络实现
//! 3. **错误统一** — 所有错误归约到 [`error::Error`]
//! 4. **零 UI** — 禁止依赖 axum / gpui；常量见 [`constants`]

#![doc(html_root_url = "https://docs.rs/qq-farm-core/0.1.0")]

pub mod config;
pub mod constants;
pub mod crypto;
pub mod error;
pub mod infra;
pub mod models;
pub mod network;
pub mod prelude;
pub mod proto;
pub mod runtime;
pub mod services;
pub mod utils;

pub use error::{Error, Result};
