//! 服务器推送事件（Notify）类型。
//!
//! 原 network.ts 里 handleNotify 把所有 EventMessage 拆出来判断。
//! 这里只做"类型 + 路由分发"的最小集合，业务级 handler 留到阶段 1E+。

use crate::proto::generated::gamepb::plantpb::LandInfo;
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
    /// 土地状态变化（自己的田或好友气泡）
    LandsChanged {
        /// 事件类型
        event_type: String,
        /// 农场主人 GID
        host_gid: i64,
        /// 变更的土地数
        changed_count: usize,
        /// 推送里带的土地（好友气泡只刷新这些，不全量 GetAll）
        lands: Vec<LandInfo>,
    },
    /// 物品变化
    ItemChanged { event_type: String, items: Vec<ItemChgLite> },
    /// 基本信息变化（升级 / 金币 / 经验）
    BasicChanged { event_type: String, level: Option<i64>, gold: Option<i64>, exp: Option<i64> },
    /// 未知 / 未处理的事件类型
    Unknown { event_type: String },
    /// 好友申请
    FriendApplications { applications: Vec<(i64, String)> },
}

/// ItemNotify 里一条物品变化（对齐 network.ts handleNotify）
#[derive(Debug, Clone)]
pub struct ItemChgLite {
    pub id: i64,
    pub count: i64,
    pub delta: i64,
}

/// 解析 EventMessage
pub fn parse_event(event: &EventMessage) -> NotifyEvent {
    let event_type = event.message_type.clone();
    let body = event.body.clone();

    if event_type.contains("Kickout") {
        match crate::proto::generated::gatepb::KickoutNotify::decode(body) {
            Ok(notify) => NotifyEvent::Kickout { event_type, reason: notify.reason_message },
            Err(_) => NotifyEvent::Kickout { event_type, reason: String::from("未知") },
        }
    } else if event_type.contains("LandsNotify") {
        match crate::proto::generated::gamepb::plantpb::LandsNotify::decode(body) {
            Ok(notify) => NotifyEvent::LandsChanged {
                event_type,
                host_gid: notify.host_gid,
                changed_count: notify.lands.len(),
                lands: notify.lands,
            },
            Err(_) => NotifyEvent::LandsChanged {
                event_type,
                host_gid: 0,
                changed_count: 0,
                lands: Vec::new(),
            },
        }
    } else if event_type.contains("ItemNotify") {
        match crate::proto::generated::gamepb::itempb::ItemNotify::decode(body) {
            Ok(notify) => NotifyEvent::ItemChanged {
                event_type,
                items: notify
                    .items
                    .into_iter()
                    .filter_map(|chg| {
                        let item = chg.item?;
                        Some(ItemChgLite { id: item.id, count: item.count, delta: chg.delta })
                    })
                    .collect(),
            },
            Err(_) => NotifyEvent::ItemChanged { event_type, items: Vec::new() },
        }
    } else if event_type.contains("BasicNotify") {
        // proto3 缺省就是 0。必须按 wire tag 判断字段是否真的在包里，
        // 对齐原 network.ts 的 hasOwn(notify.basic, 'gold'|'exp'|'level')。
        let body_bytes: &[u8] = body.as_ref();
        let has_level = nested_field_present(body_bytes, 1, 3);
        let has_exp = nested_field_present(body_bytes, 1, 4);
        let has_gold = nested_field_present(body_bytes, 1, 5);
        match crate::proto::generated::gamepb::userpb::BasicNotify::decode(body) {
            Ok(notify) => {
                let basic = notify.basic;
                NotifyEvent::BasicChanged {
                    event_type,
                    level: basic
                        .as_ref()
                        .and_then(|b| (has_level && b.level > 0).then_some(b.level)),
                    gold: basic.as_ref().and_then(|b| has_gold.then_some(b.gold)),
                    exp: basic.as_ref().and_then(|b| has_exp.then_some(b.exp)),
                }
            }
            Err(_) => NotifyEvent::BasicChanged { event_type, level: None, gold: None, exp: None },
        }
    } else if event_type.contains("FriendApplicationReceivedNotify") {
        match crate::proto::generated::gamepb::friendpb::FriendApplicationReceivedNotify::decode(
            body,
        ) {
            Ok(notify) => NotifyEvent::FriendApplications {
                applications: notify
                    .applications
                    .into_iter()
                    .filter(|a| a.gid > 0)
                    .map(|a| {
                        let name =
                            if a.name.is_empty() { format!("GID:{}", a.gid) } else { a.name };
                        (a.gid, name)
                    })
                    .collect(),
            },
            Err(_) => NotifyEvent::FriendApplications { applications: Vec::new() },
        }
    } else {
        NotifyEvent::Unknown { event_type }
    }
}

// 引入 decode trait
use prost::Message as _;

/// 扫描 protobuf 二进制，判断 `outer_field` 子消息里是否带了 `inner_field`。
///
/// BasicNotify.basic = field 1（length-delimited）；BasicInfo 里 3=level, 4=exp, 5=gold。
fn nested_field_present(buf: &[u8], outer_field: u32, inner_field: u32) -> bool {
    let Some(nested) = find_length_delimited_field(buf, outer_field) else {
        return false;
    };
    field_present(nested, inner_field)
}

fn field_present(buf: &[u8], field: u32) -> bool {
    let mut i = 0usize;
    while i < buf.len() {
        let Some((num, wire)) = read_key(buf, &mut i) else {
            return false;
        };
        if num == field {
            return true;
        }
        if !skip_value(buf, &mut i, wire) {
            return false;
        }
    }
    false
}

fn find_length_delimited_field<'a>(buf: &'a [u8], field: u32) -> Option<&'a [u8]> {
    let mut i = 0usize;
    while i < buf.len() {
        let (num, wire) = read_key(buf, &mut i)?;
        if num == field && wire == 2 {
            let len = read_varint(buf, &mut i)? as usize;
            let end = i.checked_add(len)?;
            if end > buf.len() {
                return None;
            }
            return Some(&buf[i..end]);
        }
        if !skip_value(buf, &mut i, wire) {
            return None;
        }
    }
    None
}

fn read_key(buf: &[u8], i: &mut usize) -> Option<(u32, u32)> {
    let tag = read_varint(buf, i)?;
    Some(((tag >> 3) as u32, (tag & 7) as u32))
}

fn read_varint(buf: &[u8], i: &mut usize) -> Option<u64> {
    let mut result = 0u64;
    let mut shift = 0u32;
    loop {
        if *i >= buf.len() || shift >= 64 {
            return None;
        }
        let byte = buf[*i];
        *i += 1;
        result |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
    }
}

fn skip_value(buf: &[u8], i: &mut usize, wire: u32) -> bool {
    match wire {
        0 => read_varint(buf, i).is_some(),
        1 => {
            *i = i.saturating_add(8);
            *i <= buf.len()
        }
        2 => {
            let Some(len) = read_varint(buf, i) else {
                return false;
            };
            *i = i.saturating_add(len as usize);
            *i <= buf.len()
        }
        5 => {
            *i = i.saturating_add(4);
            *i <= buf.len()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_event() {
        let ev = EventMessage { message_type: "FooNotify".to_string(), body: b"".to_vec().into() };
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

    #[test]
    fn item_notify_extracts_changes() {
        let item =
            crate::proto::generated::corepb::Item { id: 1001, count: 50, ..Default::default() };
        let chg = crate::proto::generated::corepb::ItemChg { item: Some(item), delta: 10 };
        let notify = crate::proto::generated::gamepb::itempb::ItemNotify { items: vec![chg] };
        let ev = EventMessage {
            message_type: "ItemNotify".to_string(),
            body: notify.encode_to_vec().into(),
        };
        match parse_event(&ev) {
            NotifyEvent::ItemChanged { items, .. } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].id, 1001);
                assert_eq!(items[0].count, 50);
                assert_eq!(items[0].delta, 10);
            }
            _ => panic!("expected ItemChanged"),
        }
    }

    #[test]
    fn basic_notify_level_only_does_not_zero_gold_or_exp() {
        let basic =
            crate::proto::generated::gamepb::userpb::BasicInfo { level: 12, ..Default::default() };
        let notify = crate::proto::generated::gamepb::userpb::BasicNotify { basic: Some(basic) };
        let ev = EventMessage {
            message_type: "BasicNotify".to_string(),
            body: notify.encode_to_vec().into(),
        };
        match parse_event(&ev) {
            NotifyEvent::BasicChanged { level, gold, exp, .. } => {
                assert_eq!(level, Some(12));
                assert_eq!(gold, None, "缺 gold 的包不得写成 Some(0)");
                assert_eq!(exp, None, "缺 exp 的包不得写成 Some(0)");
            }
            other => panic!("expected BasicChanged, got {other:?}"),
        }
    }

    #[test]
    fn basic_notify_gold_on_wire_is_applied_even_if_zero() {
        let basic = crate::proto::generated::gamepb::userpb::BasicInfo {
            gold: 0,
            exp: 42,
            ..Default::default()
        };
        // prost 会省略默认 0，所以手工编一个带 gold=0 / exp=42 的 BasicInfo。
        // BasicInfo: field 4 exp=42 (0x20, 42), field 5 gold=0 (0x28, 0)
        // BasicNotify: field 1 length-delimited
        let inner = [0x20, 42, 0x28, 0];
        let mut body = vec![0x0a, inner.len() as u8];
        body.extend_from_slice(&inner);
        let ev = EventMessage { message_type: "BasicNotify".to_string(), body: body.into() };
        match parse_event(&ev) {
            NotifyEvent::BasicChanged { level, gold, exp, .. } => {
                assert_eq!(level, None);
                assert_eq!(gold, Some(0));
                assert_eq!(exp, Some(42));
            }
            other => panic!("expected BasicChanged, got {other:?}"),
        }
    }
}
