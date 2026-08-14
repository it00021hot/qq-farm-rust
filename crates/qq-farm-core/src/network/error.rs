//! 网络层错误类型。

use thiserror::Error;

/// 网络层错误
#[derive(Debug, Error)]
pub enum NetworkError {
    /// WebSocket 连接错误
    #[error("websocket error: {0}")]
    WebSocket(String),

    /// WebSocket 已关闭
    #[error("websocket closed (code={code}, reason={reason})")]
    Closed { code: u16, reason: String },

    /// 帧编解码错误
    #[error("frame codec error: {0}")]
    Frame(String),

    /// 加密/解密错误
    #[error("encrypt error: {0}")]
    Encrypt(String),

    /// 解密错误
    #[error("decrypt error: {0}")]
    Decrypt(String),

    /// 网关错误（业务级 error_code != 0）
    #[error("gateway error: {service_name}.{method_name} code={code} {error_message}")]
    Gateway {
        code: i64,
        service_name: String,
        method_name: String,
        error_message: String,
        client_seq: i64,
    },

    /// 连接阶段错误（如未登录就发请求）
    #[error("connection phase error: {0}")]
    Phase(String),

    /// 请求队列已满
    #[error("request queue full (pending={pending})")]
    QueueFull { pending: usize },

    /// 请求超时（对齐 TS `请求超时: ${methodName} (seq=${seq}, pending=${pending})`）
    #[error("请求超时: {method_name} (seq={client_seq}, pending={pending})")]
    Timeout {
        client_seq: i64,
        service_name: String,
        method_name: String,
        pending: usize,
    },

    /// 主动关闭
    #[error("intentional close: {0}")]
    IntentionalClose(String),

    /// IO 错误
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// 网络层结果别名
pub type Result<T> = std::result::Result<T, NetworkError>;
