use thiserror::Error;

#[derive(Debug, Error)]
pub enum QqBotError {
    #[error("QQ Bot 配置不完整")]
    IncompleteConfig,
    #[error("微信机器人暂未实现")]
    WechatNotImplemented,
    #[error("QQ Bot 请求失败: {0}")]
    Network(String),
    #[error("QQ Bot 响应无效: {0}")]
    InvalidResponse(String),
    #[error("QQ Bot Gateway 未就绪: {0}")]
    Gateway(String),
}

pub type Result<T> = std::result::Result<T, QqBotError>;
