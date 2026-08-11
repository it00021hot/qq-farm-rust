//! 服务器推送事件（Notify）类型。
//!
//! 原 network.ts 里 handleNotify 把所有 EventMessage 拆出来判断。
//! 这里只做"类型 + 路由分发"的最小集合，业务级 handler 留到阶段 1E+。

use crate::proto::generated::gatepb::EventMessage;

/// 服务器推送的统一事件枚举
#[derive(Debug, Clone)]
pub enum NotifyEvent {
    /// 被踢下线
    Kickout {
        /// 事件类型字符串
        event_type: String,
        /// 原因描述
        reason: String,
    },
    /// 土地状态变化
    LandsChanged {
        /// 事件类型
        event_type: String,
        /// 农场主人 GID
        host_gid: i64,
        /// 变更的土地数（具体解码留到阶段 1E+）
        changed_count: usize,
    },
    /// 物品变化
    ItemChanged {
        event_type: String,
        /// 变更的物品数
        changed_count: usize,
    },
    /// 未知 / 未处理的事件类型
    Unknown {
        event_type: String,
    },
}

/// 解析 EventMessage
pub fn parse_event(event: &EventMessage) -> NotifyEvent {
    let event_type = event.message_type.clone();
    let body = event.body.clone();

    if event_type.contains("Kickout") {
        match crate::proto::generated::gatepb::KickoutNotify::decode(body) {
            Ok(notify) => NotifyEvent::Kickout {
                event_type,
                reason: notify.reason_message,
            },
            Err(_) => NotifyEvent::Kickout {
                event_type,
                reason: String::from("未知"),
            },
        }
    } else if event_type.contains("LandsNotify") {
        match crate::proto::generated::gamepb::plantpb::LandsNotify::decode(body) {
            Ok(notify) => NotifyEvent::LandsChanged {
                event_type,
                host_gid: notify.host_gid,
                changed_count: notify.lands.len(),
            },
            Err(_) => NotifyEvent::LandsChanged {
                event_type,
                host_gid: 0,
                changed_count: 0,
            },
        }
    } else if event_type.contains("ItemNotify") {
        match crate::proto::generated::gamepb::itempb::ItemNotify::decode(body) {
            Ok(notify) => NotifyEvent::ItemChanged {
                event_type,
                changed_count: notify.items.len(),
            },
            Err(_) => NotifyEvent::ItemChanged {
                event_type,
                changed_count: 0,
            },
        }
    } else {
        NotifyEvent::Unknown { event_type }
    }
}

// 引入 decode trait
use prost::Message as _;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_event() {
        let ev = EventMessage {
            message_type: "FooNotify".to_string(),
            body: b"".to_vec().into(),
        };
        match parse_event(&ev) {
            NotifyEvent::Unknown { event_type } => assert_eq!(event_type, "FooNotify"),
            _ => panic!("expected Unknown"),
        }
    }

    #[test]
    fn kickout_event() {
        let kickout = crate::proto::generated::gatepb::KickoutNotify {
            reason: 1,
            reason_message: "test reason".to_string(),
        };
        let ev = EventMessage {
            message_type: "GateUserKickoutNotify".to_string(),
            body: kickout.encode_to_vec().into(),
        };
        match parse_event(&ev) {
            NotifyEvent::Kickout { event_type, reason } => {
                assert_eq!(event_type, "GateUserKickoutNotify");
                assert_eq!(reason, "test reason");
            }
            _ => panic!("expected Kickout"),
        }
    }
}
