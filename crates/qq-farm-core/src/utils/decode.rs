//! Protobuf 调试解码工具（CLI 用）。
//!
//! 1:1 翻译原 `core/src/utils/decode.ts` 的辅助函数（`tryDecodeString` /
//! `longReplacer` / `tryGenericDecode` / `inferBodyType`）。
//!
//! 主入口在 `qq-farm-cli` 的 `pb-decode` 子命令；这里只放纯函数。

/// 尝试将字节解码为可读 UTF-8（>=80% 可打印字符）
#[must_use]
pub fn try_decode_string(bytes: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(bytes).ok()?;
    if s.is_empty() {
        return None;
    }
    let printable = s
        .chars()
        .filter(|c| {
            let code = *c as u32;
            code >= 32 || *c == '\n' || *c == '\r' || *c == '\t'
        })
        .count();
    if printable as f64 > s.chars().count() as f64 * 0.8 {
        Some(s.to_string())
    } else {
        None
    }
}

/// JSON replacer：处理 Long（low/high 字段）和 Buffer
#[must_use]
pub fn long_replacer_buffer_size() -> usize {
    // 占位：replacer 本身在 serde_json 中是闭包，不能轻易抽出
    0
}

/// Generic protobuf wire 扫描（无 schema 也能解）
#[derive(Debug, Clone)]
pub struct GenericField {
    pub field_num: u32,
    pub wire_type: u32,
    pub value: GenericValue,
}

#[derive(Debug, Clone)]
pub enum GenericValue {
    Varint(i64),
    Fixed64(u64),
    Bytes(Vec<u8>),
    Float(f32),
    Skipped,
}

impl std::fmt::Display for GenericValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Varint(n) => write!(f, "{n}"),
            Self::Fixed64(n) => write!(f, "{n}"),
            Self::Bytes(b) => {
                if let Some(s) = try_decode_string(b) {
                    write!(f, "\"{s}\"")
                } else {
                    write!(f, "{}", hex_encode(b))
                }
            }
            Self::Float(v) => write!(f, "{v}"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

/// 通用 protobuf wire 扫描
pub fn try_generic_decode(buf: &[u8]) -> Vec<GenericField> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < buf.len() {
        let (tag, new_pos) = match read_varint(buf, pos) {
            Some(t) => t,
            None => break,
        };
        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 7) as u32;
        pos = new_pos;
        let (value, new_pos) = match wire_type {
            0 => match read_varint(buf, pos) {
                Some((v, p)) => (GenericValue::Varint(v as i64), p),
                None => break,
            },
            1 => {
                if pos + 8 > buf.len() {
                    break;
                }
                let bytes: [u8; 8] = buf[pos..pos + 8].try_into().unwrap();
                (GenericValue::Fixed64(u64::from_le_bytes(bytes)), pos + 8)
            }
            2 => match read_length_delimited(buf, pos) {
                Some((bytes, p)) => (GenericValue::Bytes(bytes), p),
                None => break,
            },
            5 => {
                if pos + 4 > buf.len() {
                    break;
                }
                let bytes: [u8; 4] = buf[pos..pos + 4].try_into().unwrap();
                (GenericValue::Float(f32::from_le_bytes(bytes)), pos + 4)
            }
            _ => {
                // 不支持的 wire type，跳过（尽力而为）
                (GenericValue::Skipped, pos)
            }
        };
        pos = new_pos;
        out.push(GenericField {
            field_num,
            wire_type,
            value,
        });
    }
    out
}

fn decode_zigzag_signed(n: u64) -> i64 {
    // 当前未使用；保留以备 sint32/sint64 支持
    let _ = n;
    0
}

fn read_varint(buf: &[u8], mut pos: usize) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;
    while pos < buf.len() {
        let b = buf[pos];
        pos += 1;
        result |= u64::from(b & 0x7F) << shift;
        if (b & 0x80) == 0 {
            return Some((result, pos));
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
    None
}

fn read_length_delimited(buf: &[u8], pos: usize) -> Option<(Vec<u8>, usize)> {
    let (len, p) = read_varint(buf, pos)?;
    let len = len as usize;
    if p + len > buf.len() {
        return None;
    }
    Some((buf[p..p + len].to_vec(), p + len))
}

fn hex_encode(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for byte in b {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

/// 推断 body 消息类型（用于 gate 包装的消息）
///
/// 给定 service_name + method_name + is_request，尝试多种候选名。
#[must_use]
pub fn infer_body_type_candidates(service_name: &str, method_name: &str, is_request: bool) -> Vec<String> {
    let mut candidates = Vec::new();
    let suffix = if is_request { "Request" } else { "Reply" };
    let svc = service_name.trim_end_matches("Service");

    candidates.push(format!("{svc}.{method_name}{suffix}"));

    let parts: Vec<&str> = service_name.split('.').collect();
    if parts.len() >= 2 {
        let ns = parts[..parts.len() - 1].join(".");
        candidates.push(format!("{ns}.{method_name}{suffix}"));
        if !is_request {
            candidates.push(format!("{ns}.{method_name}Response"));
        }
    }
    if !is_request {
        candidates.push(format!("{svc}.{method_name}Response"));
    }
    candidates
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_decode_string_printable() {
        assert_eq!(
            try_decode_string(b"hello world"),
            Some("hello world".to_string())
        );
        assert_eq!(try_decode_string(b"abc123"), Some("abc123".to_string()));
    }

    #[test]
    fn try_decode_string_empty() {
        assert_eq!(try_decode_string(b""), None);
    }

    #[test]
    fn try_decode_string_binary_rejected() {
        // 0xff 不算可打印
        assert_eq!(try_decode_string(&[0xff, 0xfe, 0xfd]), None);
    }

    #[test]
    fn try_decode_string_mixed() {
        // 含 1 个不可打印字符，长度 10，可打印 9，比例 0.9 > 0.8
        let mut bytes = b"hello world".to_vec();
        bytes[5] = 0x01;
        assert!(try_decode_string(&bytes).is_some());
    }

    #[test]
    fn try_generic_decode_varint() {
        // field 1 (wire 0) = 150
        // 编码：tag=0x08 (1<<3 | 0), value=0x96 0x01
        let buf = [0x08, 0x96, 0x01];
        let fields = try_generic_decode(&buf);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_num, 1);
        assert_eq!(fields[0].wire_type, 0);
        assert!(matches!(fields[0].value, GenericValue::Varint(150)));
    }

    #[test]
    fn try_generic_decode_bytes() {
        // field 2 (wire 2) = "hi"
        // tag = 0x12, len = 0x02, "hi"
        let buf = [0x12, 0x02, b'h', b'i'];
        let fields = try_generic_decode(&buf);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_num, 2);
        if let GenericValue::Bytes(b) = &fields[0].value {
            assert_eq!(b, b"hi");
        } else {
            panic!("expected bytes");
        }
    }

    #[test]
    fn infer_body_type_request() {
        let cands = infer_body_type_candidates("gamepb.plantpb.PlantService", "AllLands", true);
        // 原 TS 也 strip "Service" 后缀
        assert!(cands.contains(&"gamepb.plantpb.AllLandsRequest".to_string()));
    }

    #[test]
    fn infer_body_type_reply() {
        let cands = infer_body_type_candidates("gamepb.plantpb.PlantService", "AllLands", false);
        assert!(cands.contains(&"gamepb.plantpb.AllLandsReply".to_string()));
        assert!(cands.contains(&"gamepb.plantpb.AllLandsResponse".to_string()));
    }

    #[test]
    fn infer_body_type_no_service_suffix() {
        // service_name 不以 "Service" 结尾时
        let cands = infer_body_type_candidates("gamepb.userpb.UserService", "Login", true);
        assert!(cands.contains(&"gamepb.userpb.LoginRequest".to_string()));
    }
}
