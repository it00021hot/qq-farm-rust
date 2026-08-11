//! WS 帧编解码。
//!
//! 每个 WS 帧是一个 protobuf `gatepb.Message`，包含：
//! - `meta`: 消息元信息（service/method/type/seq/error...）
//! - `body`: 已加密的业务负载
//! - `token`: 网关 token（用于握手）

use prost::bytes::Bytes;
use prost::Message as _;

use crate::proto::generated::gatepb::{Message, MessageType, Meta};

/// 帧构建器：把业务参数打包成 protobuf Message
#[derive(Debug, Clone)]
pub struct FrameBuilder {
    service_name: String,
    method_name: String,
    message_type: MessageType,
    client_seq: i64,
    server_seq: i64,
    body: Bytes,
    token: String,
}

impl FrameBuilder {
    /// 创建新帧
    #[must_use]
    pub fn new(
        service_name: impl Into<String>,
        method_name: impl Into<String>,
        message_type: MessageType,
    ) -> Self {
        Self {
            service_name: service_name.into(),
            method_name: method_name.into(),
            message_type,
            client_seq: 0,
            server_seq: 0,
            body: Bytes::new(),
            token: String::new(),
        }
    }

    /// 创建一个 Request 帧
    #[must_use]
    pub fn request(service_name: impl Into<String>, method_name: impl Into<String>) -> Self {
        Self::new(service_name, method_name, MessageType::Request)
    }

    /// 创建一个 Notify 帧（无响应）
    #[must_use]
    pub fn notify(service_name: impl Into<String>, method_name: impl Into<String>) -> Self {
        Self::new(service_name, method_name, MessageType::Notify)
    }

    #[must_use]
    pub fn with_client_seq(mut self, seq: i64) -> Self {
        self.client_seq = seq;
        self
    }

    #[must_use]
    pub fn with_server_seq(mut self, seq: i64) -> Self {
        self.server_seq = seq;
        self
    }

    #[must_use]
    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = Bytes::from(body);
        self
    }

    #[must_use]
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = token.into();
        self
    }

    /// 编码为 protobuf bytes
    pub fn encode(&self) -> Result<Vec<u8>, prost::EncodeError> {
        let meta = Meta {
            service_name: self.service_name.clone(),
            method_name: self.method_name.clone(),
            message_type: self.message_type as i32,
            client_seq: self.client_seq,
            server_seq: self.server_seq,
            error_code: 0,
            error_message: String::new(),
            metadata: Default::default(),
        };
        let msg = Message {
            meta: Some(meta),
            body: self.body.clone(),
            token: self.token.clone(),
        };
        Ok(msg.encode_to_vec())
    }
}

/// 帧解析器：从 protobuf bytes 解出已解析的 Message
#[derive(Debug, Clone)]
pub struct FrameParser {
    inner: Message,
}

impl FrameParser {
    /// 解析
    pub fn parse(bytes: &[u8]) -> Result<Self, prost::DecodeError> {
        let inner = Message::decode(bytes)?;
        Ok(Self { inner })
    }

    #[must_use]
    pub fn message(&self) -> &Message {
        &self.inner
    }

    #[must_use]
    pub fn meta(&self) -> Option<&Meta> {
        self.inner.meta.as_ref()
    }

    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.inner.body
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.inner.token
    }

    /// 消息类型（None 表示 meta 缺失或枚举值未知）
    #[must_use]
    pub fn message_type(&self) -> Option<MessageType> {
        self.inner
            .meta
            .as_ref()
            .and_then(|m| MessageType::try_from(m.message_type).ok())
    }

    #[must_use]
    pub fn client_seq(&self) -> i64 {
        self.inner.meta.as_ref().map_or(0, |m| m.client_seq)
    }

    #[must_use]
    pub fn server_seq(&self) -> i64 {
        self.inner.meta.as_ref().map_or(0, |m| m.server_seq)
    }

    #[must_use]
    pub fn service_name(&self) -> &str {
        self.inner
            .meta
            .as_ref()
            .map_or("", |m| m.service_name.as_str())
    }

    #[must_use]
    pub fn method_name(&self) -> &str {
        self.inner
            .meta
            .as_ref()
            .map_or("", |m| m.method_name.as_str())
    }

    #[must_use]
    pub fn error_code(&self) -> i64 {
        self.inner.meta.as_ref().map_or(0, |m| m.error_code)
    }

    #[must_use]
    pub fn error_message(&self) -> &str {
        self.inner
            .meta
            .as_ref()
            .map_or("", |m| m.error_message.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let frame = FrameBuilder::request("gamepb.userpb.UserService", "GetUserSettings")
            .with_client_seq(42)
            .with_server_seq(0)
            .with_body(b"hello".to_vec())
            .with_token("token-xyz");

        let bytes = frame.encode().expect("encode");
        let parsed = FrameParser::parse(&bytes).expect("parse");

        assert_eq!(parsed.service_name(), "gamepb.userpb.UserService");
        assert_eq!(parsed.method_name(), "GetUserSettings");
        assert_eq!(parsed.client_seq(), 42);
        assert_eq!(parsed.body(), b"hello");
        assert_eq!(parsed.token(), "token-xyz");
        assert_eq!(parsed.message_type(), Some(MessageType::Request));
    }

    #[test]
    fn notify_frame() {
        let frame = FrameBuilder::notify("gate.GateService", "PushEvent").with_client_seq(7);
        let bytes = frame.encode().expect("encode");
        let parsed = FrameParser::parse(&bytes).expect("parse");
        assert_eq!(parsed.message_type(), Some(MessageType::Notify));
        assert_eq!(parsed.client_seq(), 7);
    }
}
