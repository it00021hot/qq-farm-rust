//! 统一错误类型。
//!
//! 所有 crate 内部错误归约到 [`Error`]，向上层（`server` / `cli`）暴露 [`Result<T>`]。

use thiserror::Error;

/// 业务核心库统一错误。
#[derive(Debug, Error)]
pub enum Error {
    /// 配置错误（缺失、解析失败、值非法）
    #[error("config error: {0}")]
    Config(String),

    /// Protobuf 编解码错误
    #[error("protobuf error: {0}")]
    Protobuf(#[from] prost::DecodeError),

    /// Protobuf 编码错误
    #[error("protobuf encode error: {0}")]
    ProtobufEncode(#[from] prost::EncodeError),

    /// 网络错误（WebSocket / HTTP）
    #[error("network error: {0}")]
    Network(String),

    /// WASM 运行时错误
    #[error("wasm error: {0}")]
    Wasm(#[from] wasmtime::Error),

    /// 加密/解密错误（业务层语义）
    #[error("crypto error: {0}")]
    Crypto(String),

    /// 协议错误（游戏服务端返回不符合预期）
    #[error("protocol error: {0}")]
    Protocol(String),

    /// 账号相关错误
    #[error("account error: {context} (account_id={account_id})")]
    Account {
        account_id: String,
        context: String,
    },

    /// 业务逻辑错误（如：作物等级不足、化肥不足）
    #[error("business error: {0}")]
    Business(String),

    /// 资源未找到
    #[error("not found: {0}")]
    NotFound(String),

    /// 序列化错误
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// 通用 IO 错误
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// 通用错误（兜底用，应当尽量收敛到具体变体）
    #[error("internal error: {0}")]
    Internal(String),
}

impl Error {
    /// 便捷构造：账号错误
    #[must_use]
    pub fn account<S1: Into<String>, S2: Into<String>>(account_id: S1, context: S2) -> Self {
        Self::Account {
            account_id: account_id.into(),
            context: context.into(),
        }
    }

    /// 便捷构造：内部错误
    #[must_use]
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        Self::Internal(msg.into())
    }
}

/// 业务核心库结果别名。
pub type Result<T> = std::result::Result<T, Error>;
