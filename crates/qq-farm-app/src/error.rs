//! 应用层错误 — 从 core::Error 映射，不含 HTTP 语义。

use thiserror::Error;

/// 应用门面统一错误。
#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("{0}")]
    Forbidden(String),

    #[error("internal: {0}")]
    Internal(String),

    /// Worker 未运行（server 映射为 HTTP 200 + `{ok:false}`）
    #[error("账号未运行")]
    AccountNotRunning,

    #[error(transparent)]
    Core(#[from] qq_farm_core::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    #[must_use]
    pub fn from_core(e: qq_farm_core::Error) -> Self {
        match e {
            qq_farm_core::Error::NotFound(m) => Self::NotFound(m),
            qq_farm_core::Error::Business(m) => Self::BadRequest(m),
            other => Self::Core(other),
        }
    }
}
