//! 网络层。
//!
//! ## 模块
//!
//! - [`client`] — WebSocket 客户端（actor 模式）
//! - [`encryptor`] — 加密/解密器（trait + TSDK 实现）
//! - [`error`] — 网络层错误
//! - [`frame`] — GateMessage 帧的 protobuf 编码/解码
//! - [`gateway`] — 网关连接（状态机 + 收发 + sendMsgAsync 机制）
//! - [`notify`] — 服务器推送事件类型
//! - [`request`] — 异步请求/响应管理（clientSeq 关联）
//! - [`user_state`] — 登录后用户运行时状态

pub mod client;
pub mod encryptor;
pub mod error;
pub mod frame;
pub mod gateway;
pub mod notify;
pub mod request;
pub mod user_state;
