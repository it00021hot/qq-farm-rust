//! IPC 错误：`AppError` → 可序列化结构。

use qq_farm_app::error::AppError;
use serde::Serialize;

/// 前端可解析的统一错误体。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: String,
    pub message: String,
}

impl From<AppError> for IpcError {
    fn from(err: AppError) -> Self {
        let code = match &err {
            AppError::NotFound(_) => "not_found",
            AppError::BadRequest(_) => "bad_request",
            AppError::Forbidden(_) => "forbidden",
            AppError::Internal(_) | AppError::Core(_) => "internal",
            AppError::AccountNotRunning => "account_not_running",
        };
        Self { code: code.to_string(), message: err.to_string() }
    }
}

pub type IpcResult<T> = Result<T, IpcError>;
