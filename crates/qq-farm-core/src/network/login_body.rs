//! Login / Heartbeat 请求体构造（逐字节对齐官方客户端抓包）。
//!
//! 1:1 翻译原 `core/src/utils/network.ts` 的 `buildLoginBody()` / `buildHeartbeatBody()`
//! （bot `9709bcb`，2026-09-14 按 2026-09-11 官方抓包逐字节对齐）。
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
//! （Login 固定 73 字节、Heartbeat 固定 27 字节），并用官方向量做 golden 测试钉死。
//! 改动本文件前先对照 bot `network.ts` 的同名函数与 `ws_00001_SEND.bin` /
//! `ws_00114_SEND.bin` 抓包向量。

/// protobuf wire type 0（varint）
const WIRE_VARINT: u8 = 0;
/// protobuf wire type 2（length-delimited）
const WIRE_LEN: u8 = 2;

/// LoginRequest 固定 73 字节（client_version="1.14.0.4_20260911"、
/// sys_software="Windows" 时的官方向量长度；其余入参按实际长度伸缩）。
pub fn build_login_body(client_version: &str, sys_software: &str) -> Vec<u8> {
    let mut b = Vec::with_capacity(73);
    // field 3 sharer_id = 0（显式 varint 0）
    push_tag(&mut b, 3, WIRE_VARINT);
    push_varint(&mut b, 0);
    // field 4 sharer_open_id = ""（显式空串）
    push_len_delim(&mut b, 4, &[]);
    // field 5 device_info { 1: client_version, 2: sys_software }
    let mut di = Vec::with_capacity(client_version.len() + sys_software.len() + 4);
    push_len_delim(&mut di, 1, client_version.as_bytes());
    push_len_delim(&mut di, 2, sys_software.as_bytes());
    push_len_delim(&mut b, 5, &di);
    // field 6 share_cfg_id = 0（显式 varint 0）
    push_tag(&mut b, 6, WIRE_VARINT);
    push_varint(&mut b, 0);
    // field 7 scene_id = "1234567"
    push_len_delim(&mut b, 7, b"1234567");
    // field 8 report_data { 1..4 空串, 5 "other-qq", 6 = 2, 7..8 空串 }
    let mut rd = Vec::with_capacity(24);
    for field in [1, 2, 3, 4] {
        push_len_delim(&mut rd, field, &[]);
    }
    push_len_delim(&mut rd, 5, b"other-qq");
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
}
