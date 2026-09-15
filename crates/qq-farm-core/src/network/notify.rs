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
    /// 基本信息变化（升级 / 金币 / 经验 / 昵称 / 头像）
    BasicChanged {
        event_type: String,
        level: Option<i64>,
        gold: Option<i64>,
        exp: Option<i64>,
        /// 最新昵称（proto3 空串 = 未携带；只在非空时给出，对齐 go applyBasicNotify）
        nick: Option<String>,
        /// 最新头像 URL（同样只在非空时给出）
        avatar: Option<String>,
    },
    /// 未知 / 未处理的事件类型
    Unknown { event_type: String },
    /// 好友申请（gid / 名称 / 等级）
    FriendApplications { applications: Vec<(i64, String, i64)> },
    /// 宠物"同气连枝"礼包待拾取（PendingGiftCountNotify）
    DogSkillGiftPending { count: i64 },
    /// 宠物守护记录更新（NewProtectLogNotify，空事件）
    DogProtectLogChanged,
    /// 活动列表变化（ActiviesChangeNotify，用于失效活动时间窗缓存）
    ActivitiesChanged,
    /// 通行证（游记）变化（BattlePassChangeNotify，用于刷新赛季缓存）
    BattlePassChanged { pass: Option<crate::proto::generated::gamepb::seasonpb::SeasonPass> },
    /// 任务信息推送
    TaskInfoNotify { task_info: Option<crate::proto::generated::gamepb::taskpb::TaskInfo> },
    /// 天气变化（自己或好友农场；WeatherChangeNotify）
    WeatherChanged { event_type: String, host_gid: i64 },
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
                // 昵称 / 头像只在非空时携带（proto3 空串等价于未携带；
                // go applyBasicNotify 同样对空串直接跳过，避免清空已有值）
                let nick = basic.as_ref().filter(|b| !b.name.is_empty()).map(|b| b.name.clone());
                let avatar = basic
                    .as_ref()
                    .filter(|b| !b.avatar_url.is_empty())
                    .map(|b| b.avatar_url.clone());
                NotifyEvent::BasicChanged {
                    event_type,
                    level: basic
                        .as_ref()
                        .and_then(|b| (has_level && b.level > 0).then_some(b.level)),
                    gold: basic.as_ref().and_then(|b| has_gold.then_some(b.gold)),
                    exp: basic.as_ref().and_then(|b| has_exp.then_some(b.exp)),
                    nick,
                    avatar,
                }
            }
            Err(_) => NotifyEvent::BasicChanged {
                event_type,
                level: None,
                gold: None,
                exp: None,
                nick: None,
                avatar: None,
            },
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
                        (a.gid, name, a.level)
                    })
                    .collect(),
            },
            Err(_) => NotifyEvent::FriendApplications { applications: Vec::new() },
        }
    } else if event_type.contains("PendingGiftCountNotify") {
        match crate::proto::generated::gamepb::dogpb::PendingGiftCountNotify::decode(body) {
            Ok(notify) => NotifyEvent::DogSkillGiftPending { count: notify.count.max(0) },
            Err(_) => NotifyEvent::DogSkillGiftPending { count: 0 },
        }
    } else if event_type.contains("NewProtectLogNotify") {
        NotifyEvent::DogProtectLogChanged
    } else if event_type.contains("BattlePassChangeNotify") {
        // 通行证（游记）进度变化推送，对齐 go manager.go 的 applyBattlePassNotify
        match crate::proto::generated::gamepb::seasonpb::BattlePassChangeNotify::decode(body) {
            Ok(notify) => NotifyEvent::BattlePassChanged { pass: notify.pass },
            Err(_) => NotifyEvent::BattlePassChanged { pass: None },
        }
    } else if event_type.contains("ActiviesChangeNotify")
        || event_type.contains("ActivityChangeNotify")
    {
        // proto 真实消息名是 ActiviesChangeNotify（proto/activitypb.proto，服务端
        // 就少拼一个 i），此前误写成 ActivitiesChangedNotify / ActivitiesNotify，
        // 永远匹配不上导致活动时间窗缓存失效链路不通。匹配集对齐 go manager.go：
        // 同时兼容 ActivityChangeNotify。
        // go 还兜底匹配 activity 服务的 method 名（service == "gamepb.activitypb.ActivityService"
        // && method 含 "Activity"），但 rust 的 EventMessage 只有 message_type / body
        // 两个字段（proto/game.proto），网关推送帧拿不到 service/method，且 message_type
        // 本身就是 proto 消息名，上面两条子串匹配已完整覆盖真实推送。
        NotifyEvent::ActivitiesChanged
    } else if event_type.contains("TaskInfoNotify") {
        match crate::proto::generated::gamepb::taskpb::TaskInfoNotify::decode(body) {
            Ok(notify) => NotifyEvent::TaskInfoNotify { task_info: notify.task_info },
            Err(_) => NotifyEvent::TaskInfoNotify { task_info: None },
        }
    } else if event_type.contains("WeatherChangeNotify") {
        match crate::proto::generated::gamepb::weatherpb::WeatherChangeNotify::decode(body) {
            Ok(notify) => NotifyEvent::WeatherChanged { event_type, host_gid: notify.host_gid },
            Err(_) => NotifyEvent::WeatherChanged { event_type, host_gid: 0 },
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

fn find_length_delimited_field(buf: &[u8], field: u32) -> Option<&[u8]> {
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
        // prost 会省略默认 0，所以手工编一个带 gold=0 / exp=42 的 BasicInfo。
        // BasicInfo: field 4 exp=42 (0x20, 42), field 5 gold=0 (0x28, 0)
        // BasicNotify: field 1 length-delimited
        let inner = [0x20, 42, 0x28, 0];
        let mut body = vec![0x0a, inner.len() as u8];
        body.extend_from_slice(&inner);
        let ev = EventMessage { message_type: "BasicNotify".to_string(), body: body.into() };
        match parse_event(&ev) {
            NotifyEvent::BasicChanged { level, gold, exp, nick, avatar, .. } => {
                assert_eq!(level, None);
                assert_eq!(gold, Some(0));
                assert_eq!(exp, Some(42));
                assert_eq!(nick, None, "缺昵称的包不得带空串");
                assert_eq!(avatar, None, "缺头像的包不得带空串");
            }
            other => panic!("expected BasicChanged, got {other:?}"),
        }
    }

    #[test]
    fn basic_notify_extracts_nick_and_avatar() {
        let basic = crate::proto::generated::gamepb::userpb::BasicInfo {
            name: "新昵称".to_string(),
            avatar_url: "https://example.com/a.png".to_string(),
            ..Default::default()
        };
        let notify = crate::proto::generated::gamepb::userpb::BasicNotify { basic: Some(basic) };
        let ev = EventMessage {
            message_type: "BasicNotify".to_string(),
            body: notify.encode_to_vec().into(),
        };
        match parse_event(&ev) {
            NotifyEvent::BasicChanged { nick, avatar, .. } => {
                assert_eq!(nick.as_deref(), Some("新昵称"));
                assert_eq!(avatar.as_deref(), Some("https://example.com/a.png"));
            }
            other => panic!("expected BasicChanged, got {other:?}"),
        }
    }

    #[test]
    fn activies_change_notify_matches_real_proto_name() {
        // proto 里只有 ActiviesChangeNotify（服务端少拼一个 i）；
        // 此前匹配集写错导致推送被判为 Unknown，活动缓存失效链路不通
        for name in ["ActiviesChangeNotify", "ActivityChangeNotify"] {
            let ev = EventMessage { message_type: name.to_string(), body: b"".to_vec().into() };
            match parse_event(&ev) {
                NotifyEvent::ActivitiesChanged => {}
                other => panic!("expected ActivitiesChanged for {name}, got {other:?}"),
            }
        }
    }

    #[test]
    fn battle_pass_change_notify_extracts_pass() {
        let pass = crate::proto::generated::gamepb::seasonpb::SeasonPass {
            activity_id: 100,
            current_level: 3,
            current_progress: 1,
            progress_target: 10,
            ..Default::default()
        };
        let notify =
            crate::proto::generated::gamepb::seasonpb::BattlePassChangeNotify { pass: Some(pass) };
        let ev = EventMessage {
            message_type: "BattlePassChangeNotify".to_string(),
            body: notify.encode_to_vec().into(),
        };
        match parse_event(&ev) {
            NotifyEvent::BattlePassChanged { pass } => {
                let pass = pass.expect("应解出通行证");
                assert_eq!(pass.activity_id, 100);
                assert_eq!(pass.current_level, 3);
                assert_eq!(pass.current_progress, 1);
            }
            other => panic!("expected BattlePassChanged, got {other:?}"),
        }
    }
}
