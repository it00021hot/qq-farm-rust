//! Login / Heartbeat 请求体构造（逐字节对齐官方客户端抓包）。
//!
//! 1:1 翻译原 `core/src/utils/network.ts` 的 `buildLoginBody()` / `buildHeartbeatBody()`
//! （bot `9709bcb`，2026-09-14 按 2026-09-11 官方抓包逐字节对齐；
//! 微信分支对齐 bot `864caf3` 的 `client-profile.ts` / `network.ts`）。
//!
//! # 为什么手写字节而不是 prost encode
//!
//! proto3 编码会省略所有取默认值的字段，但官方客户端**显式写出**了它们：
//! - `sharer_id = 0` / `share_cfg_id = 0`（varint 0）
//! - `sharer_open_id`（空串）、`extra`（空 bytes）
//! - `report_data` 的 6 个空字符串字段（callback / cd_extend_info / click_id /
//!   clue_token / req_id / trackid）
//! - `Heartbeat.field_3 = 0`
//!
//! prost 产不出这些「显式默认值」，因此按官方抓包逐字节构造
//! （Login QQ 固定 73 字节、Heartbeat 固定 27 字节），并用官方向量做 golden 测试钉死。
//! 改动本文件前先对照 bot `network.ts` 的同名函数与 `ws_00001_SEND.bin` /
//! `ws_00114_SEND.bin` 抓包向量。
//!
//! # 微信平台分支（bot 864caf3，无官方抓包，行为以 bot 测试钉死）
//!
//! 对齐 bot `wechat-connection.test.js` 的断言：
//! - `device_info` 追加 `network`(5) / `memory`(10) / `device_id`(13)，**有值才写**
//!   （memory 须为正整数；protobufjs 按字段号升序写出）；
//! - `scene_id` **整字段省略**（微信启动场景独立、从 Code 拿不到）；
//! - `report_data.minigame_channel = "other"`（QQ 为 `"other-qq"`）。

/// protobuf wire type 0（varint）
const WIRE_VARINT: u8 = 0;
/// protobuf wire type 2（length-delimited）
const WIRE_LEN: u8 = 2;

/// LoginRequest device_info 的微信扩展字段（bot `getLoginDeviceInfo` 只在微信平台取这些）。
#[derive(Debug, Clone, Default)]
pub struct WxDeviceExtras {
    /// device_info.network（field 5），空串不写
    pub network: String,
    /// device_info.device_id（field 13），空串不写
    pub device_id: String,
    /// device_info.memory（field 10），>0 才写（bot：`Number.isSafeInteger && > 0`）
    pub memory: i64,
}

/// 平台判定（对齐 bot `client-profile.ts` 的 `isWechatPlatform`：`['wx','wechat']`）
#[must_use]
pub fn is_wechat_platform(platform: &str) -> bool {
    let normalized = platform.trim().to_ascii_lowercase();
    normalized == "wx" || normalized == "wechat"
}

/// QQ 平台 Login 体（保持既有 golden 向量入口）。
pub fn build_login_body(client_version: &str, sys_software: &str) -> Vec<u8> {
    build_login_body_for_platform(client_version, sys_software, "qq", None)
}

/// 按平台构造 LoginRequest 体。
///
/// - QQ（默认，含空/未知平台兜底）：与官方 73 字节抓包逐字节一致；
/// - 微信（`wx`/`wechat`）：device_info 追加有值的 network/memory/device_id、
///   scene_id 省略、minigame_channel 用 `"other"`。
pub fn build_login_body_for_platform(
    client_version: &str,
    sys_software: &str,
    platform: &str,
    wx_extras: Option<&WxDeviceExtras>,
) -> Vec<u8> {
    let wechat = is_wechat_platform(platform);
    let mut b = Vec::with_capacity(73);
    // field 3 sharer_id = 0（显式 varint 0）
    push_tag(&mut b, 3, WIRE_VARINT);
    push_varint(&mut b, 0);
    // field 4 sharer_open_id = ""（显式空串）
    push_len_delim(&mut b, 4, &[]);
    // field 5 device_info（protobufjs 按字段号升序写出：1/2 + 微信 5/10/13）
    let mut di = Vec::with_capacity(client_version.len() + sys_software.len() + 4);
    push_len_delim(&mut di, 1, client_version.as_bytes());
    push_len_delim(&mut di, 2, sys_software.as_bytes());
    if wechat {
        if let Some(extras) = wx_extras {
            if !extras.network.is_empty() {
                push_len_delim(&mut di, 5, extras.network.as_bytes());
            }
            if extras.memory > 0 {
                push_tag(&mut di, 10, WIRE_VARINT);
                push_varint(&mut di, extras.memory as u64);
            }
            if !extras.device_id.is_empty() {
                push_len_delim(&mut di, 13, extras.device_id.as_bytes());
            }
        }
    }
    push_len_delim(&mut b, 5, &di);
    // field 6 share_cfg_id = 0（显式 varint 0）
    push_tag(&mut b, 6, WIRE_VARINT);
    push_varint(&mut b, 0);
    // field 7 scene_id：微信省略整字段（bot：启动场景独立、拿不到）
    if !wechat {
        push_len_delim(&mut b, 7, b"1234567");
    }
    // field 8 report_data { 1..4 空串, 5 channel, 6 = 2, 7..8 空串 }
    let mut rd = Vec::with_capacity(24);
    for field in [1, 2, 3, 4] {
        push_len_delim(&mut rd, field, &[]);
    }
    let channel: &[u8] = if wechat { b"other" } else { b"other-qq" };
    push_len_delim(&mut rd, 5, channel);
    push_tag(&mut rd, 6, WIRE_VARINT);
    push_varint(&mut rd, 2);
    push_len_delim(&mut rd, 7, &[]);
    push_len_delim(&mut rd, 8, &[]);
    push_len_delim(&mut b, 8, &rd);
    // field 9 extra = ""（显式空 bytes）
    push_len_delim(&mut b, 9, &[]);
    b
}

/// HeartbeatRequest 固定 27 字节（field_3 显式写 0）。
pub fn build_heartbeat_body(gid: i64, client_version: &str) -> Vec<u8> {
    let mut b = Vec::with_capacity(27);
    push_tag(&mut b, 1, WIRE_VARINT);
    push_varint(&mut b, gid as u64);
    push_len_delim(&mut b, 2, client_version.as_bytes());
    push_tag(&mut b, 3, WIRE_VARINT);
    push_varint(&mut b, 0);
    b
}

fn push_tag(buf: &mut Vec<u8>, field: u32, wire_type: u8) {
    push_varint(buf, ((field << 3) | wire_type as u32) as u64);
}

fn push_varint(buf: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.push(byte);
            break;
        }
        buf.push(byte | 0x80);
    }
}

fn push_len_delim(buf: &mut Vec<u8>, field: u32, payload: &[u8]) {
    push_tag(buf, field, WIRE_LEN);
    push_varint(buf, payload.len() as u64);
    buf.extend_from_slice(payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERSION: &str = "1.14.0.4_20260911";

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// 官方向量：2026-09-11 QQ 小程序抓包 `ws_00001_SEND.bin` 解密后的 Login 体，
    /// 钉死在 bot `network-keepalive.test.js`。
    #[test]
    fn login_body_matches_official_capture_byte_for_byte() {
        let body = build_login_body(VERSION, "Windows");
        assert_eq!(body.len(), 73);
        assert_eq!(
            hex(&body),
            concat!(
                "180022002a1c0a11312e31342e302e345f3230323630393131120757696e646f7773",
                "30003a073132333435363742180a0012001a0022002a086f746865722d717130023a0042004a00"
            )
        );
    }

    /// 官方向量：`ws_00114_SEND.bin`，gid = 1220537209。
    #[test]
    fn heartbeat_body_matches_official_capture_byte_for_byte() {
        let body = build_heartbeat_body(1_220_537_209, VERSION);
        assert_eq!(body.len(), 27);
        assert_eq!(hex(&body), "08f9d6ffc5041211312e31342e302e345f32303236303931311800");
    }

    /// prost 对照：确认 prost 会省略显式默认值字段（产不出官方向量），
    /// 防止将来有人“简化”回 prost encode 而不破坏字节对齐时能被测试拦住。
    #[test]
    fn prost_encode_cannot_reproduce_official_login_body() {
        use crate::proto::generated::gamepb::userpb::{DeviceInfo, LoginRequest, ReportData};
        let req = LoginRequest {
            sharer_id: 0,
            sharer_open_id: String::new(),
            device_info: Some(DeviceInfo {
                client_version: VERSION.to_string(),
                sys_software: "Windows".to_string(),
                ..Default::default()
            }),
            share_cfg_id: 0,
            scene_id: "1234567".to_string(),
            report_data: Some(ReportData {
                minigame_channel: "other-qq".to_string(),
                minigame_platid: 2,
                ..Default::default()
            }),
            extra: Default::default(),
        };
        let prost_body = prost::Message::encode_to_vec(&req);
        assert_ne!(prost_body.len(), 73, "prost proto3 会省略默认值字段");
        assert_ne!(hex(&prost_body), hex(&build_login_body(VERSION, "Windows")));
    }

    /// 非默认入参（超长版本号 / 空系统名）仍须产出可解码的同构消息。
    #[test]
    fn builders_scale_with_arguments() {
        use prost::Message;

        let long_version = "1.14.0.5_20270101";
        let login = build_login_body(long_version, "Windows Unknown x64");
        let decoded =
            crate::proto::generated::gamepb::userpb::LoginRequest::decode(login.as_slice())
                .expect("decode");
        let di = decoded.device_info.expect("device_info");
        assert_eq!(di.client_version, long_version);
        assert_eq!(di.sys_software, "Windows Unknown x64");
        assert_eq!(decoded.scene_id, "1234567");
        let rd = decoded.report_data.expect("report_data");
        assert_eq!(rd.minigame_channel, "other-qq");
        assert_eq!(rd.minigame_platid, 2);

        let hb = build_heartbeat_body(1, long_version);
        let decoded =
            crate::proto::generated::gamepb::userpb::HeartbeatRequest::decode(hb.as_slice())
                .expect("decode");
        assert_eq!(decoded.gid, 1);
        assert_eq!(decoded.client_version, long_version);
        assert_eq!(decoded.field_3, 0);
    }

    // ===== 微信分支（bot 864caf3，无官方抓包；断言对齐 bot wechat-connection.test.js）=====

    fn read_varint(buf: &[u8], pos: &mut usize) -> u64 {
        let mut result = 0u64;
        let mut shift = 0;
        loop {
            let byte = buf[*pos];
            *pos += 1;
            result |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        result
    }

    /// 遍历一段 protobuf 消息的顶层字段（prost 表达不了 proto3 字段 presence，
    /// bot 用 `Object.hasOwn` 断言的「字段缺席」只能在 wire 层验证）。
    fn wire_fields(buf: &[u8]) -> Vec<(u32, u8)> {
        let mut out = Vec::new();
        let mut pos = 0;
        while pos < buf.len() {
            let tag = read_varint(buf, &mut pos);
            let field = (tag >> 3) as u32;
            let wire = (tag & 0x7) as u8;
            match wire {
                0 => {
                    read_varint(buf, &mut pos);
                }
                2 => {
                    let len = read_varint(buf, &mut pos) as usize;
                    pos += len;
                }
                _ => panic!("unexpected wire type {wire}"),
            }
            out.push((field, wire));
        }
        out
    }

    /// bot「decoded WeChat login uses configured device fields and does not invent
    /// a launch scene」对应：device_info 携带配置的 network/memory/device_id、
    /// channel=other、**scene_id 整字段缺席**；memory 非法（≤0）时不写。
    #[test]
    fn wechat_login_body_uses_device_fields_and_omits_scene_id() {
        use prost::Message;

        let extras = WxDeviceExtras {
            network: "wifi".into(),
            device_id: "test-device".into(),
            memory: 8_192,
        };
        let body = build_login_body_for_platform(VERSION, "Windows test", "wx", Some(&extras));

        let decoded =
            crate::proto::generated::gamepb::userpb::LoginRequest::decode(body.as_slice())
                .expect("decode");
        let di = decoded.device_info.expect("device_info");
        assert_eq!(di.client_version, VERSION);
        assert_eq!(di.sys_software, "Windows test");
        assert_eq!(di.network, "wifi");
        assert_eq!(di.memory, 8_192);
        assert_eq!(di.device_id, "test-device");
        let rd = decoded.report_data.expect("report_data");
        assert_eq!(rd.minigame_channel, "other");
        assert_eq!(rd.minigame_platid, 2);

        // wire 层：顶层无 field 7（scene_id）；device_info 子字段 = {1,2,5,10,13} 升序
        let top = wire_fields(&body);
        assert!(!top.iter().any(|(f, _)| *f == 7), "scene_id (field 7) 必须整字段缺席");
        let (_, _, di_payload) = split_len_field(&body, 5).expect("device_info payload");
        assert_eq!(
            wire_fields(di_payload),
            vec![(1, WIRE_LEN), (2, WIRE_LEN), (5, WIRE_LEN), (10, WIRE_VARINT), (13, WIRE_LEN)]
        );

        // memory 非法（bot 传 'invalid'）→ 0 → 不写 field 10
        let bad =
            WxDeviceExtras { network: "wifi".into(), device_id: "test-device".into(), memory: 0 };
        let body2 = build_login_body_for_platform(VERSION, "Windows test", "wechat", Some(&bad));
        let (_, _, di2) = split_len_field(&body2, 5).expect("device_info payload");
        assert!(!wire_fields(di2).iter().any(|(f, _)| *f == 10), "memory <= 0 不写");
    }

    /// device_info 内空 network / device_id 不写（bot：有值才写）。
    #[test]
    fn wechat_login_body_skips_empty_extras() {
        let extras = WxDeviceExtras::default();
        let body = build_login_body_for_platform(VERSION, "Windows", "wx", Some(&extras));
        let (_, _, di) = split_len_field(&body, 5).expect("device_info payload");
        assert_eq!(wire_fields(di), vec![(1, WIRE_LEN), (2, WIRE_LEN)]);
    }

    /// QQ 平台忽略微信扩展字段：device_info 只有 {1,2}，scene_id 在，channel=other-qq。
    #[test]
    fn qq_login_body_ignores_wx_extras() {
        let extras = WxDeviceExtras {
            network: "wifi".into(),
            device_id: "test-device".into(),
            memory: 8_192,
        };
        let body = build_login_body_for_platform(VERSION, "Windows", "qq", Some(&extras));
        assert_eq!(hex(&body), hex(&build_login_body(VERSION, "Windows")), "QQ 体与 golden 一致");
        let (_, _, di) = split_len_field(&body, 5).expect("device_info payload");
        assert_eq!(wire_fields(di), vec![(1, WIRE_LEN), (2, WIRE_LEN)]);
        let top = wire_fields(&body);
        assert!(top.iter().any(|(f, _)| *f == 7), "QQ 保留 scene_id");
    }

    /// 从顶层消息中取出指定 LEN 字段的 payload（测试辅助）。
    fn split_len_field<'a>(buf: &'a [u8], field: u32) -> Option<(usize, u8, &'a [u8])> {
        let mut pos = 0;
        while pos < buf.len() {
            let tag = read_varint(buf, &mut pos);
            let f = (tag >> 3) as u32;
            let wire = (tag & 0x7) as u8;
            match wire {
                0 => {
                    read_varint(buf, &mut pos);
                }
                2 => {
                    let len = read_varint(buf, &mut pos) as usize;
                    let payload = &buf[pos..pos + len];
                    if f == field {
                        return Some((pos, wire, payload));
                    }
                    pos += len;
                }
                _ => panic!("unexpected wire type {wire}"),
            }
        }
        None
    }

    /// 平台判定对齐 bot `isWechatPlatform`（trim + lowercase + ['wx','wechat']）。
    #[test]
    fn is_wechat_platform_matches_bot() {
        assert!(is_wechat_platform("wx"));
        assert!(is_wechat_platform("wechat"));
        assert!(is_wechat_platform(" WX "));
        assert!(is_wechat_platform("WeChat"));
        assert!(!is_wechat_platform("qq"));
        assert!(!is_wechat_platform(""));
        assert!(!is_wechat_platform("unknown"));
    }
}
