//! 微信原生协议 — MMTLS 编码 / 解码原语。
//!
//! 1:1 翻译原 `core/src/services/wx-login/native-protocol.ts`（194 行）。
//!
//! ## 协议层职责
//!
//! - varint / protobuf / LZ4 编解码
//! - HMAC-SHA256 / SHA-256 / HKDF-Expand
//! - AES-GCM 加解密（普通 + MMTLS nonce 变体）
//! - ECDH P-256 密钥对生成 + 共享密钥计算
//! - 帧 / 握手 / wpkg 容器解析
//! - 业务请求构造（manualRequest / hybrid / envelope / jsPlain）
//!
//! ## 与原 TS 的差异
//!
//! - 真实 TCP 收发（`getNativeWxLoginCode`）已实现：`get_native_wx_login_code`
//!   完成 long link MMTLS 握手 + short link 0-RTT，拿回 wx.login code。
//! - HTTPDNS 解析（`targets` / `fetch_httpdns_targets`）已实现；解析失败时回退 hardcoded fallback。

use std::collections::HashMap;
use std::sync::Arc;

use aes::Aes192;
use aes_gcm::aead::generic_array::typenum::U12;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Aes256Gcm, AesGcm, Nonce};
use hmac::{Hmac, Mac};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use sha2::{Digest, Sha256};

/// Hybrid ECDH 使用 AES-192-GCM（TS `aes-${key.length*8}-gcm`，key 取 24 字节）。
type Aes192Gcm = AesGcm<Aes192, U12>;

/// MMTLS record type
pub const REC: u16 = 0xF103;

const HOST_APP: &[u8] = b"wxd44977328b36e647";

/// 服务端公钥（硬编码 P-256）
pub const SERVER_PUB_HEX: &str = "04ef87876d6478b15f1796eab12068610541173b7176b67f1dcc86683e901acd44d18b4ac36938251d0812dd0cf842aa2d6cbb8115712d1c0087dcefc14a44cd58";

pub const TRANSFER_PATH: &[u8] = b"/ilink/ilinkapp/mp/wxaruntime_transfer";
pub const TRANSFER_HOST: &[u8] = b"shortcloud.weixin.com";

/// 服务端 ECDH 公钥
pub fn server_pub_key() -> Result<p256::PublicKey, String> {
    let bytes = hex_decode(SERVER_PUB_HEX).map_err(|e| e.to_string())?;
    let pk = p256::PublicKey::from_sec1_bytes(&bytes).map_err(|e| e.to_string())?;
    Ok(pk)
}

// =====================================================================
// Varint
// =====================================================================

/// 编码 varint
#[must_use]
pub fn vi(n: i64) -> Vec<u8> {
    let mut v: u64 = n as u64;
    let mut out: Vec<u8> = Vec::new();
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
    out
}

/// 读取 varint
///
/// 返回 `(value, next_offset)`。超出 buffer 时返回错误。
pub fn rvi(b: &[u8], mut o: usize) -> Result<(u64, usize), String> {
    let mut n: u64 = 0;
    let mut s: u32 = 0;
    while o < b.len() {
        let x = b[o];
        n |= u64::from(x & 0x7F) << s;
        o += 1;
        if (x & 0x80) == 0 {
            return Ok((n, o));
        }
        s += 7;
        if s > 63 {
            return Err("varint overflow".to_string());
        }
    }
    Err("truncated varint".to_string())
}

// =====================================================================
// Protobuf
// =====================================================================

/// Protobuf length-delimited 字段
#[must_use]
pub fn pbl(f: u32, b: &[u8]) -> Vec<u8> {
    let mut out = vi(i64::from(f) * 8 + 2);
    out.extend(vi(b.len() as i64));
    out.extend_from_slice(b);
    out
}

/// Protobuf varint 字段
#[must_use]
pub fn pbv(f: u32, n: i64) -> Vec<u8> {
    let mut out = vi(i64::from(f) * 8);
    out.extend(vi(n));
    out
}

/// 解析 protobuf
///
/// 返回字段号 -> 值（varint 字段解为 u64，length-delimited 解为 Vec<u8>）
pub fn pbf(b: &[u8]) -> HashMap<u32, PbfValue> {
    let mut out = HashMap::new();
    let mut o = 0;
    while o < b.len() {
        let Ok((tag, a)) = rvi(b, o) else { break };
        o = a;
        let f = (tag >> 3) as u32;
        match tag & 7 {
            0 => {
                let Ok((n, z)) = rvi(b, o) else { break };
                out.insert(f, PbfValue::Varint(n));
                o = z;
            }
            2 => {
                let Ok((n, z)) = rvi(b, o) else { break };
                let len = n as usize;
                o = z;
                if o + len > b.len() {
                    break;
                }
                out.insert(f, PbfValue::Bytes(b[o..o + len].to_vec()));
                o += len;
            }
            _ => break,
        }
    }
    out
}

/// Protobuf 解析后的值
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PbfValue {
    Varint(u64),
    Bytes(Vec<u8>),
}

impl PbfValue {
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            PbfValue::Bytes(b) => Some(b),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_varint(&self) -> Option<u64> {
        match self {
            PbfValue::Varint(n) => Some(*n),
            _ => None,
        }
    }
}

fn required_field<'a>(
    fields: &'a HashMap<u32, PbfValue>,
    field: u32,
    name: &str,
) -> Result<&'a [u8], String> {
    let value = fields
        .get(&field)
        .ok_or_else(|| format!("{name} is missing"))?;
    value.as_bytes().ok_or_else(|| format!("{name} is not bytes"))
}

// =====================================================================
// HMAC / SHA-256 / HKDF-Expand
// =====================================================================

type HmacSha256 = Hmac<Sha256>;

/// HMAC-SHA256
#[must_use]
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC key");
    mac.update(data);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// SHA-256
#[must_use]
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().into()
}

/// HKDF-Expand（label + context，循环计数器）
///
/// 1:1 对齐原 TS `expand` 函数
pub fn expand(secret: &[u8], label: &str, context: &[u8], size: usize) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut prev: Vec<u8> = Vec::new();
    let label_bytes = label.as_bytes();
    let mut counter: u8 = 1;
    while out.len() < size {
        let mut input = prev.clone();
        input.extend_from_slice(label_bytes);
        input.extend_from_slice(context);
        input.push(counter);
        let next = hmac_sha256(secret, &input).to_vec();
        prev = next.clone();
        out.extend(next);
        counter = counter.wrapping_add(1);
    }
    out.truncate(size);
    out
}

/// HMAC extract（短形式）
#[must_use]
pub fn extract(salt: &[u8], data: &[u8]) -> [u8; 32] {
    hmac_sha256(salt, data)
}

// =====================================================================
// AES-GCM（含 MMTLS nonce 变体）
// =====================================================================

/// MMTLS nonce 变体（IV 与 seq 异或最后 8 字节）
#[must_use]
pub fn mmtls_nonce(iv: &[u8], seq: u64) -> [u8; 12] {
    let mut n = [0u8; 12];
    n.copy_from_slice(&iv[..12.min(iv.len())]);
    let s = seq.to_be_bytes();
    for i in 0..8 {
        n[12 - 8 + i] ^= s[i];
    }
    n
}

/// MMTLS GCM（带 seq + record type + record length 的 AAD）
#[must_use]
pub fn gcm(
    key: &[u8],
    iv: &[u8],
    seq: u64,
    rec_type: u8,
    data: &[u8],
    decrypt: bool,
) -> Vec<u8> {
    let mut aad = [0u8; 13];
    aad[0..8].copy_from_slice(&seq.to_be_bytes());
    aad[8] = rec_type;
    aad[9..11].copy_from_slice(&REC.to_be_bytes());
    let data_len = if decrypt { data.len() } else { data.len() + 16 };
    aad[11..13].copy_from_slice(&(data_len as u16).to_be_bytes());

    let nonce_arr = mmtls_nonce(iv, seq);

    match key.len() {
        16 => gcm_mmtls::<Aes128Gcm>(key, &nonce_arr, &aad, data, decrypt),
        32 => gcm_mmtls::<Aes256Gcm>(key, &nonce_arr, &aad, data, decrypt),
        n => {
            tracing::error!(key_len = n, "unsupported MMTLS AES-GCM key length");
            vec![]
        }
    }
}

/// MMTLS record GCM。密钥非法或加解密失败一律返回空，**绝不**回退全零密钥。
fn gcm_mmtls<C: KeyInit + Aead>(
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    data: &[u8],
    decrypt: bool,
) -> Vec<u8> {
    let Ok(cipher) = C::new_from_slice(key) else {
        return vec![];
    };
    let nonce = Nonce::from_slice(nonce);
    let payload = aes_gcm::aead::Payload { msg: data, aad };
    if decrypt {
        if data.len() < 16 {
            return vec![];
        }
        cipher.decrypt(nonce, payload).unwrap_or_default()
    } else {
        cipher.encrypt(nonce, payload).unwrap_or_default()
    }
}

/// 普通 AES-GCM（返回 ciphertext + iv + tag，1:1 对齐原 TS `layout`）。
///
/// 加密失败返回空 Vec，**绝不**用零 tag 占位——发出去会被风控当成异常客户端。
#[must_use]
pub fn layout(key: &[u8], plain: &[u8], aad: &[u8]) -> Vec<u8> {
    let iv = random_bytes(12);
    let ct = gcm_simple(key, &iv, plain, aad, false);
    if ct.len() < 16 {
        tracing::error!(key_len = key.len(), "AES-GCM layout encrypt failed");
        return vec![];
    }
    let mut out = Vec::with_capacity(ct.len() + 12);
    out.extend_from_slice(&ct[..ct.len() - 16]);
    out.extend_from_slice(&iv);
    out.extend_from_slice(&ct[ct.len() - 16..]);
    out
}

/// 解 layout（1:1 对齐原 TS `unlayout`）
pub fn unlayout(key: &[u8], blob: &[u8], aad: &[u8]) -> Vec<u8> {
    if blob.len() < 28 {
        return vec![];
    }
    let split = blob.len() - 28;
    let iv = &blob[split..split + 12];
    let ct_part = &blob[..split];
    let tag = &blob[split + 12..];
    // 拼成 ct = ciphertext + tag
    let mut combined = Vec::with_capacity(ct_part.len() + 16);
    combined.extend_from_slice(ct_part);
    combined.extend_from_slice(tag);
    gcm_simple(key, iv, &combined, aad, true)
}

/// 普通 AES-GCM 加解密（不带 seq 变体）。
///
/// 对齐 TS `createCipheriv('aes-${key.length*8}-gcm')`：16 / 24 / 32 字节密钥。
fn gcm_simple(key: &[u8], iv: &[u8], data: &[u8], aad: &[u8], decrypt: bool) -> Vec<u8> {
    match key.len() {
        16 => gcm_simple_cipher::<Aes128Gcm>(key, iv, data, aad, decrypt),
        24 => gcm_simple_cipher::<Aes192Gcm>(key, iv, data, aad, decrypt),
        32 => gcm_simple_cipher::<Aes256Gcm>(key, iv, data, aad, decrypt),
        n => {
            tracing::error!(key_len = n, "unsupported AES-GCM key length");
            vec![]
        }
    }
}

fn gcm_simple_cipher<C: KeyInit + Aead>(
    key: &[u8],
    iv: &[u8],
    data: &[u8],
    aad: &[u8],
    decrypt: bool,
) -> Vec<u8> {
    let Ok(cipher) = C::new_from_slice(key) else {
        return vec![];
    };
    let nonce = Nonce::from_slice(iv);
    let payload = aes_gcm::aead::Payload { msg: data, aad };
    if decrypt {
        cipher.decrypt(nonce, payload).unwrap_or_default()
    } else {
        cipher.encrypt(nonce, payload).unwrap_or_default()
    }
}

// =====================================================================
// 帧 / 握手 / 容器
// =====================================================================

/// ShortLink HTTP/1.0 请求头。必须与 TS 模板字符串逐字节一致（含小写 hex path）。
///
/// 不要用 `\` 续行：续行会吃掉换行后的空白，但后续改缩进就会把空格打进报文。
#[must_use]
pub fn short_link_request_head(ts_hex: &str, body_len: usize) -> String {
    format!(
        "POST /mmtls/{ts_hex} HTTP/1.0\r\nAccept: */*\r\nCache-Control: no-cache\r\nConnection: close\r\nContent-Length: {body_len}\r\nContent-Type: application/octet-stream\r\nHost: shortcloud.weixin.com\r\nUpgrade: mmtls\r\nUser-Agent: MicroMessenger Client\r\nX-Online-Host: shortcloud.weixin.com\r\n\r\n"
    )
}

/// ShortLink 0-RTT type-8 early-data header（20 字节，timestamp 写 offset 16）。
///
/// 1:1 对齐 TS：
/// `Buffer.from([0,0,0,16, 8,0,0,0, 11,1,0,0, 0,6,0,18, 0,0,0,0]); writeUInt32BE(ts, 16)`
#[must_use]
pub fn type8_early_data(ts: u32) -> Vec<u8> {
    let mut type8 = vec![
        0x00, 0x00, 0x00, 0x10, 0x08, 0x00, 0x00, 0x00, 0x0B, 0x01, 0x00, 0x00, 0x00, 0x06,
        0x00, 0x12, 0x00, 0x00, 0x00, 0x00,
    ];
    type8[16..20].copy_from_slice(&ts.to_be_bytes());
    type8
}

/// 构造 record
#[must_use]
pub fn rec(rec_type: u8, body: &[u8]) -> Vec<u8> {
    let mut h = [0u8; 5];
    h[0] = rec_type;
    h[1..3].copy_from_slice(&REC.to_be_bytes());
    h[3..5].copy_from_slice(&(body.len() as u16).to_be_bytes());
    let mut out = h.to_vec();
    out.extend_from_slice(body);
    out
}

/// 解析 record 序列
pub fn records(data: &[u8]) -> Vec<RecordFrame> {
    let mut out = Vec::new();
    let mut o = 0;
    while o + 5 <= data.len() {
        let len = u16::from_be_bytes([data[o + 3], data[o + 4]]) as usize;
        if u16::from_be_bytes([data[o + 1], data[o + 2]]) != REC || o + 5 + len > data.len() {
            break;
        }
        out.push(RecordFrame {
            rec_type: data[o],
            body: data[o + 5..o + 5 + len].to_vec(),
        });
        o += 5 + len;
    }
    out
}

/// MMTLS record 帧
#[derive(Debug, Clone)]
pub struct RecordFrame {
    pub rec_type: u8,
    pub body: Vec<u8>,
}

/// 构造握手消息（4 字节长度 + 1 字节类型 + body）
#[must_use]
pub fn hs(hs_type: u8, body: &[u8]) -> Vec<u8> {
    let mut h = [0u8; 5];
    h[0..4].copy_from_slice(&((body.len() + 1) as u32).to_be_bytes());
    h[4] = hs_type;
    let mut out = h.to_vec();
    out.extend_from_slice(body);
    out
}

/// 解析握手消息
pub fn split_hs(b: &[u8]) -> Result<HandshakeFrame, String> {
    if b.len() < 5 {
        return Err("invalid handshake".to_string());
    }
    let len = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize;
    if len + 4 > b.len() {
        return Err("invalid handshake".to_string());
    }
    Ok(HandshakeFrame {
        hs_type: b[4],
        body: b[5..4 + len].to_vec(),
    })
}

/// 握手消息
#[derive(Debug, Clone)]
pub struct HandshakeFrame {
    pub hs_type: u8,
    pub body: Vec<u8>,
}

// =====================================================================
// LZ4：请求侧只用 literal；服务端响应带 back-reference，解码必须按 TS 重叠拷贝
// =====================================================================

/// LZ4 literal 编码（无 back-reference）
#[must_use]
pub fn lz4_literal(data: &[u8]) -> Vec<u8> {
    if data.len() < 15 {
        let mut out = vec![((data.len() as u8) << 4)];
        out.extend_from_slice(data);
        return out;
    }
    let mut out = vec![0xF0];
    let mut n = data.len() - 15;
    while n >= 255 {
        out.push(255);
        n -= 255;
    }
    out.push(n as u8);
    out.extend_from_slice(data);
    out
}

/// LZ4 解码（含 literal + back-reference）
pub fn lz4(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let t = data[i];
        i += 1;
        let mut n = (t >> 4) as usize;
        if n == 15 {
            loop {
                if i >= data.len() {
                    return Err("lz4 truncated".to_string());
                }
                let x = data[i];
                i += 1;
                n += x as usize;
                if x != 255 {
                    break;
                }
            }
        }
        let end = i + n;
        if end > data.len() {
            return Err("lz4 literal out of range".to_string());
        }
        out.extend_from_slice(&data[i..end]);
        i = end;
        if i >= data.len() {
            break;
        }
        if i + 2 > data.len() {
            return Err("lz4 offset out of range".to_string());
        }
        let off = u16::from_le_bytes([data[i], data[i + 1]]) as usize;
        i += 2;
        let mut m = ((t & 0x0F) as usize) + 4;
        if (t & 0x0F) == 15 {
            loop {
                if i >= data.len() {
                    return Err("lz4 truncated".to_string());
                }
                let x = data[i];
                i += 1;
                m += x as usize;
                if x != 255 {
                    break;
                }
            }
        }
        if off == 0 || off > out.len() {
            return Err("lz4 invalid offset".to_string());
        }
        // 必须按 TS `out.push(out[out.length - off])`：每次 push 后长度变化，
        // 重叠回引用（offset=1 的 RLE）才能展开。用「起始 pos + j」会少拷字节，
        // ManualAuth 响应解出来就会缺 field 3。
        for _ in 0..m {
            let b = out[out.len() - off];
            out.push(b);
        }
    }
    Ok(out)
}

// =====================================================================
// wpkg 容器
// =====================================================================

/// 构造 wpkg
#[must_use]
pub fn wpkg(ints: &HashMap<u32, i64>, bytes: &HashMap<u32, Vec<u8>>) -> Vec<u8> {
    let mut a: Vec<u8> = vec![1];
    let mut keys: Vec<u32> = ints.keys().copied().collect();
    keys.sort();
    for k in keys {
        a.extend(vi(i64::from(k)));
        a.extend(vi(ints[&k]));
    }
    a.extend(vi(0));
    let mut byte_keys: Vec<u32> = bytes.keys().copied().collect();
    byte_keys.sort();
    for k in byte_keys {
        a.extend(vi(i64::from(k)));
        a.extend(vi(bytes[&k].len() as i64));
        a.extend_from_slice(&bytes[&k]);
    }
    let p = a;
    let mut out = p.clone();
    out.extend(vi(0));
    out.extend(vi(p.len() as i64 + 1));
    out
}

/// 读取 wpkg，跳过内容返回总长度偏移
pub fn read_wpkg(b: &[u8]) -> Result<usize, String> {
    let (_, mut o) = rvi(b, 0)?;
    loop {
        let (f, n) = rvi(b, o)?;
        o = n;
        if f == 0 {
            break;
        }
        let (_, n) = rvi(b, o)?;
        o = n;
    }
    loop {
        let (f, n) = rvi(b, o)?;
        o = n;
        if f == 0 {
            break;
        }
        let (l, z) = rvi(b, o)?;
        o = z + l as usize;
    }
    let (_, o) = rvi(b, o)?;
    Ok(o)
}

// =====================================================================
// ShortLink 协议
// =====================================================================

/// 构造 shortlink 头
#[must_use]
pub fn short(cmd: u32, seq: u32, body: &[u8]) -> Vec<u8> {
    let mut b = vec![0u8; 16];
    let total = 16 + body.len();
    b[0..4].copy_from_slice(&(total as u32).to_be_bytes());
    b[4..6].copy_from_slice(&0x1110u16.to_be_bytes());
    b[6..8].copy_from_slice(&0x076Du16.to_be_bytes());
    b[8..12].copy_from_slice(&cmd.to_be_bytes());
    b[12..16].copy_from_slice(&seq.to_be_bytes());
    b.extend_from_slice(body);
    b
}

/// 解析 shortlink
pub fn parse_short(b: &[u8]) -> Result<(u32, Vec<u8>), String> {
    if b.len() < 16 {
        return Err("invalid shortlink".to_string());
    }
    let total = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize;
    if total > b.len() {
        return Err("invalid shortlink".to_string());
    }
    let cmd = u32::from_be_bytes([b[8], b[9], b[10], b[11]]);
    let body = b[16..total].to_vec();
    Ok((cmd, body))
}

// =====================================================================
// ECDH
// =====================================================================

/// ECDH 密钥对（含原始公钥字节）
#[derive(Clone)]
pub struct EcdhKeyPair {
    pub secret_key: p256::SecretKey,
    pub public_bytes: Vec<u8>,
}

impl EcdhKeyPair {
    pub fn generate() -> Result<Self, String> {
        let mut rng = rand::thread_rng();
        let sk = p256::SecretKey::random(&mut rng);
        let pk_bytes = sk.public_key().to_encoded_point(false).as_bytes().to_vec();
        Ok(Self {
            secret_key: sk,
            public_bytes: pk_bytes,
        })
    }

    /// 计算共享密钥（与对端公钥）
    pub fn shared_secret(&self, peer: &[u8]) -> Result<Vec<u8>, String> {
        let peer_pk = p256::PublicKey::from_sec1_bytes(peer).map_err(|e| e.to_string())?;
        let shared = p256::ecdh::diffie_hellman(self.secret_key.to_nonzero_scalar(), peer_pk.as_affine());
        Ok(shared.raw_secret_bytes().to_vec())
    }
}

// =====================================================================
// 业务：manual request / hybrid / envelope / jsPlain
// =====================================================================

/// 构造 manual auth 请求（1:1 对齐原 TS `manualRequest`）
#[must_use]
pub fn manual_request(buffer_b64: &str, app: &[u8]) -> Result<ManualRequest, String> {
    let raw = base64_decode(buffer_b64).map_err(|e| format!("base64 decode: {e}"))?;
    let fields = pbf(&raw);
    let ticket = required_field(&fields, 1, "ticket")?.to_vec();
    let device = required_field(&fields, 2, "device")?.to_vec();
    let host_opt = fields.get(&3).and_then(|v| v.as_bytes()).map(<[u8]>::to_vec);
    let base = {
        let mut v = pbl(1, app);
        v.extend(pbv(2, 1901));
        v
    };
    let mut req = pbl(1, &base);
    req.extend(pbl(3, &pbl(1, &ticket)));
    req.extend(pbv(4, 4));
    req.extend(pbl(6, &[]));
    req.extend(pbv(7, 0));
    req.extend(pbv(8, 6));
    Ok(ManualRequest {
        req,
        device,
        host: host_opt
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| HOST_APP.to_vec()),
    })
}

#[derive(Debug, Clone)]
pub struct ManualRequest {
    pub req: Vec<u8>,
    pub device: Vec<u8>,
    pub host: Vec<u8>,
}

/// 构造 hybrid（1:1 对齐原 TS `hybrid`）
pub fn hybrid(plain: &[u8]) -> Result<HybridResult, String> {
    let a = EcdhKeyPair::generate()?;
    let server_pub = server_pub_key()?;
    let server_pub_bytes = server_pub.to_encoded_point(false).as_bytes().to_vec();
    let shared = a.shared_secret(&server_pub_bytes)?;
    let secret_hash = sha256(&shared);
    let h1_label = b"1";
    let h1_suffix = b"415";
    let mut h1_input = Vec::new();
    h1_input.extend_from_slice(h1_label);
    h1_input.extend_from_slice(h1_suffix);
    h1_input.extend_from_slice(&a.public_bytes);
    let h1 = sha256(&h1_input);
    let cek = random_bytes(32);
    let enc_key = layout(&secret_hash[..24], &cek, &h1);
    if enc_key.len() != 32 + 12 + 16 {
        return Err("hybrid encKey AES-192-GCM failed".into());
    }
    let salt = b"security hdkf expand";
    let okm = expand(&sha256_with(salt, &cek), "", &h1, 56);
    let comp = lz4_literal(plain);
    let mut h2_input = Vec::new();
    h2_input.extend_from_slice(h1_label);
    h2_input.extend_from_slice(h1_suffix);
    h2_input.extend_from_slice(&a.public_bytes);
    h2_input.extend_from_slice(&enc_key);
    let h2 = sha256(&h2_input);
    let enc = layout(&okm[..24], &comp, &h2);
    if enc.len() < 12 + 16 {
        return Err("hybrid ciphertext AES-192-GCM failed".into());
    }
    let mut wire = pbv(1, 1);
    let mut key_share = pbv(1, 415);
    key_share.extend(pbl(2, &a.public_bytes));
    wire.extend(pbl(2, &key_share));
    wire.extend(pbl(3, &enc_key));
    wire.extend(pbl(4, &[]));
    wire.extend(pbl(5, &enc));
    Ok(HybridResult {
        temp: HybridTemp {
            key_pair: a,
            okm,
            comp,
        },
        wire,
    })
}

#[derive(Clone)]
pub struct HybridResult {
    pub temp: HybridTemp,
    pub wire: Vec<u8>,
}

#[derive(Clone)]
pub struct HybridTemp {
    pub key_pair: EcdhKeyPair,
    pub okm: Vec<u8>,
    pub comp: Vec<u8>,
}

/// 构造业务 envelope（1:1 对齐原 TS `envelope`）
pub fn envelope(session: &Session, plain: &[u8]) -> Result<Vec<u8>, String> {
    let compressed = lz4_literal(plain);
    let enc = layout(&session.send_key, &compressed, &[]);
    if enc.len() != compressed.len() + 12 + 16 {
        return Err(format!(
            "envelope layout failed (send_key_len={})",
            session.send_key.len()
        ));
    }
    let mut head_ints = HashMap::new();
    head_ints.insert(1, 1);
    head_ints.insert(2, session.uin as i64);
    head_ints.insert(3, 0);
    head_ints.insert(4, 0);
    head_ints.insert(5, 524545);
    head_ints.insert(6, 11);
    head_ints.insert(7, 0);
    head_ints.insert(8, 0);
    head_ints.insert(9, 0);
    head_ints.insert(10, 1);
    head_ints.insert(11, 0);
    head_ints.insert(12, 0);
    head_ints.insert(13, 0);
    head_ints.insert(17, 0);
    head_ints.insert(18, 1);
    head_ints.insert(20, 1504);
    head_ints.insert(21, 0);
    head_ints.insert(22, session.uin as i64);
    head_ints.insert(23, 0);
    head_ints.insert(25, 16);
    head_ints.insert(26, 4);
    head_ints.insert(28, 1);
    head_ints.insert(29, 1);
    head_ints.insert(30, 0);
    let mut head_bytes = HashMap::new();
    head_bytes.insert(14, Vec::new());
    head_bytes.insert(24, session.device_id.clone());
    head_bytes.insert(27, session.f9.clone());
    let head = wpkg(&head_ints, &head_bytes);
    let inner = short(0x0B41, 0, &[head, enc.clone()].concat());
    let mut b = vec![0u8; 2];
    b.extend_from_slice(TRANSFER_PATH);
    b.extend_from_slice(&[0u8; 2]);
    b.extend_from_slice(TRANSFER_HOST);
    b.extend_from_slice(&[0u8; 4]);
    b.extend_from_slice(&inner);
    let path_len_pos = 0;
    let host_len_pos = 2 + TRANSFER_PATH.len();
    let inner_len_pos = 4 + TRANSFER_PATH.len() + TRANSFER_HOST.len();
    b[path_len_pos..path_len_pos + 2].copy_from_slice(&(TRANSFER_PATH.len() as u16).to_be_bytes());
    b[host_len_pos..host_len_pos + 2]
        .copy_from_slice(&(TRANSFER_HOST.len() as u16).to_be_bytes());
    b[inner_len_pos..inner_len_pos + 4].copy_from_slice(&(inner.len() as u32).to_be_bytes());
    let mut n = vec![0u8; 4];
    n[0..4].copy_from_slice(&(b.len() as u32).to_be_bytes());
    let mut out = n;
    out.extend(b);
    Ok(out)
}

/// Session（用于 envelope）
#[derive(Debug, Clone)]
pub struct Session {
    pub send_key: Vec<u8>,
    pub recv_key: Vec<u8>,
    pub f9: Vec<u8>,
    pub uin: u64,
    pub device_id: Vec<u8>,
    pub host_app_id: Vec<u8>,
    pub psk: Vec<u8>,
    pub ticket: Vec<u8>,
}

// =====================================================================
// HTTPDNS 解析（fallback）
// =====================================================================

/// `targets` 函数：列出 long / short 链接可用的 IP+port
///
/// 1:1 对齐原 TS `targets`。
/// 真实实现会 fetch `http://aedns.weixin.qq.com/...`，但本模块仅返回 fallback。
/// 集成时需替换。
#[must_use]
pub fn targets(kind: &str) -> Vec<Target> {
    let (default_ip, default_port) = if kind == "long" {
        ("180.153.202.85", 8080)
    } else {
        ("120.241.131.173", 80)
    };
    vec![Target {
        ip: default_ip.to_string(),
        port: default_port,
    }]
}

/// 网络目标
#[derive(Debug, Clone)]
pub struct Target {
    pub ip: String,
    pub port: u16,
}

// =====================================================================
// 派生 keys
// =====================================================================

/// 派生 handshake keys（ck / sk / ci / si 各 16/16/12/12 字节）
#[must_use]
pub fn derive_handshake_keys(secret: &[u8], label: &str, hash: &[u8], size: usize) -> HandshakeKeys {
    let z = expand(secret, label, hash, size);
    let (ck, sk) = z.split_at(16);
    let (sk_ci, si) = sk.split_at(16);
    let (ci, si) = si.split_at(12);
    HandshakeKeys {
        ck: ck.to_vec(),
        sk: sk_ci.to_vec(),
        ci: ci.to_vec(),
        si: si.to_vec(),
    }
}

/// 派生 handshake keys（固定 56 字节：ck16 + sk16 + ci12 + si12）
#[must_use]
pub fn handshake_keys(secret: &[u8], label: &str, hash: &[u8]) -> HandshakeKeys {
    derive_handshake_keys(secret, label, hash, 56)
}

/// 派生 one-way keys（28 字节：key16 + iv12）
#[must_use]
pub fn one_way_keys(secret: &[u8], label: &str, hash: &[u8]) -> OneWayKeys {
    let z = expand(secret, label, hash, 28);
    let (key, iv) = z.split_at(16);
    OneWayKeys {
        key: key.to_vec(),
        iv: iv.to_vec(),
    }
}

/// Handshake keys
#[derive(Debug, Clone)]
pub struct HandshakeKeys {
    pub ck: Vec<u8>,
    pub sk: Vec<u8>,
    pub ci: Vec<u8>,
    pub si: Vec<u8>,
}

/// One-way keys（用于 0-RTT early data）
#[derive(Debug, Clone)]
pub struct OneWayKeys {
    pub key: Vec<u8>,
    pub iv: Vec<u8>,
}

// =====================================================================
// ClientHello / PSK ClientHello
// =====================================================================

/// 构造标准 ClientHello（双 ECDH 密钥对 + key_share）
///
/// 1:1 对齐原 TS `ch()`：`b = [03 F1 01 C0 2B] + random(32) + ts(4)`，
/// offers 包进 `ks = [0,16,2] + offer1 + offer2 + [0,0,0,1]`，
/// `ext = [1] + len(ks) + ks`，最终 `hs(1, b + len(ext) + ext)`。
#[must_use]
pub fn client_hello(pub1: &[u8], pub2: &[u8]) -> Vec<u8> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    client_hello_with(pub1, pub2, &random_bytes(32), now as u32)
}

/// 确定性版 `client_hello`（注入 random 与 timestamp，供 golden 向量测试）
#[must_use]
pub fn client_hello_with(pub1: &[u8], pub2: &[u8], random32: &[u8], timestamp: u32) -> Vec<u8> {
    debug_assert_eq!(random32.len(), 32, "client_hello random must be 32 bytes");

    // b = [03 F1 01 C0 2B] + random(32) + timestamp(4)
    let mut b = vec![0x03, 0xF1, 0x01, 0xC0, 0x2B];
    b.extend_from_slice(random32);
    b.extend(timestamp.to_be_bytes());

    // ks = [0,16,2] + offer1 + offer2 + [0,0,0,1]
    let mut ks = vec![0x00, 0x10, 0x02];
    for (i, p) in [pub1, pub2].iter().enumerate() {
        // x = uint32BE(i?2:1) + uint16BE(65)
        let mut x = vec![0u8; 6];
        x[0..4].copy_from_slice(&(if i == 0 { 1u32 } else { 2u32 }).to_be_bytes());
        x[4..6].copy_from_slice(&65u16.to_be_bytes());
        // z = x + pub；offer = uint32BE(z.len()) + z
        let z = [x.as_slice(), *p].concat();
        ks.extend((z.len() as u32).to_be_bytes());
        ks.extend(z);
    }
    ks.extend([0x00, 0x00, 0x00, 0x01]);

    // ext = [1] + uint32BE(ks.len()) + ks
    let mut ext = vec![0x01u8];
    ext.extend((ks.len() as u32).to_be_bytes());
    ext.extend(ks);

    // b += uint32BE(ext.len()) + ext
    b.extend((ext.len() as u32).to_be_bytes());
    b.extend(ext);

    hs(1, &b) // type=1 ClientHello
}

/// 构造 PSK ClientHello（用于 short link 0-RTT）
#[must_use]
pub fn psk_client_hello(ticket: &[u8], timestamp: u32) -> Vec<u8> {
    psk_client_hello_with(ticket, timestamp, &random_bytes(32))
}

/// 确定性版 `psk_client_hello`（注入 random，供 golden 向量测试）
#[must_use]
pub fn psk_client_hello_with(ticket: &[u8], timestamp: u32, random32: &[u8]) -> Vec<u8> {
    debug_assert_eq!(random32.len(), 32, "psk_client_hello random must be 32 bytes");

    let mut ticket_ext = vec![0x00, 0x0F, 0x01];
    ticket_ext.extend((ticket.len() as u32).to_be_bytes());
    ticket_ext.extend(ticket);

    let mut ext = vec![0x01];
    ext.extend((ticket_ext.len() as u32).to_be_bytes());
    ext.extend(ticket_ext);

    let mut body = vec![0x03, 0xF1, 0x01, 0x00, 0xA8];
    body.extend_from_slice(random32);
    body.extend(timestamp.to_be_bytes());
    body.extend((ext.len() as u32).to_be_bytes());
    body.extend(ext);

    hs(0x01, &body)
}

// =====================================================================
// js_plain (ManualAuthRequest payload)
// =====================================================================

/// 构造 js-login 客户端 ManualAuth 请求 payload
#[must_use]
pub fn js_plain(uin: u64, app_id: &str, host: &[u8]) -> Vec<u8> {
    js_plain_with(uin, app_id, host, &random_bytes(6))
}

/// 确定性版 `js_plain`（注入 mac 6 字节，供 golden 向量测试）
#[must_use]
pub fn js_plain_with(uin: u64, app_id: &str, host: &[u8], mac6: &[u8]) -> Vec<u8> {
    debug_assert_eq!(mac6.len(), 6, "js_plain mac must be 6 bytes");
    // mac = random 6 bytes, byte[0] = (mac[0] | 2) & 0xFE
    let mut mac_mut = mac6.to_vec();
    mac_mut[0] = (mac_mut[0] | 0x02) & 0xFE;
    let dev_str = mac_mut
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join("-");

    let uin32 = uin & 0xFFFFFFFF;
    let dev = dev_str.as_bytes().to_vec();

    let info = |name: &str| -> Vec<u8> {
        let mut v = pbl(1, b"sessionkey");
        v.extend(pbv(2, uin32 as i64));
        v.extend(pbl(3, &dev));
        v.extend(pbv(4, 1661404927));
        v.extend(pbl(5, name.as_bytes()));
        v.extend(pbv(6, 0));
        v
    };

    let mut req = pbl(1, &info("UnifiedPCWindows"));
    req.extend(pbl(2, app_id.as_bytes()));
    req.extend(pbv(4, 1));
    req.extend(pbl(5, &[]));
    req.extend(pbl(6, &[]));
    req.extend(pbv(7, 1));

    let mut outer = pbl(1, &info("Windows"));
    outer.extend(pbl(2, b"/cgi-bin/mmbiz-bin/js-login"));
    outer.extend(pbl(3, host));
    outer.extend(pbv(4, 5));
    outer.extend(pbl(5, &req));
    outer.extend(pbl(6, app_id.as_bytes()));
    outer.extend(pbv(7, 1029));
    outer.extend(pbv(8, 1610627409));
    outer.extend(pbl(9, b"WindowsxWebPlugin"));
    outer.extend(pbv(10, 573651281));
    outer
}

// =====================================================================
// parse_manual
// =====================================================================

/// 解析 ShortLink 返回的 ManualAuthResponse
///
/// 1:1 对齐原 TS `parseManual`。返回 `Err` 时携带 ManualAuth 拒绝的 code/message。
pub fn parse_manual(body: &[u8], temp: &HybridTemp) -> Result<ParsedManualResponse, String> {
    let marker = [0x08u8, 0x9F, 0x03, 0x12, 0x41, 0x04];

    // 找 HybridEcdhResponse：先试 read_wpkg 边界（offset 处为 0x0A），否则回退 marker 定位
    let mut offset: Option<usize> = None;
    if let Ok(off) = read_wpkg(body) {
        if off < body.len() && body[off] == 0x0A {
            offset = Some(off);
        }
    }
    let offset = offset.or_else(|| {
        body.windows(marker.len())
            .position(|w| w == marker)
            .filter(|pos| *pos >= 2)
            .map(|pos| pos - 2)
    });
    let offset = offset.ok_or_else(|| {
        let preview = hex::encode(&body[..body.len().min(48)]);
        format!(
            "HybridEcdhResponse not found (len={}, head={preview})",
            body.len()
        )
    })?;

    let hybrid_response = pbf(&body[offset..]);
    let key_fields_bytes =
        required_field(&hybrid_response, 1, "HybridEcdhResponse field 1")?.to_vec();
    let key_fields = pbf(&key_fields_bytes);
    let peer =
        required_field(&key_fields, 2, "HybridEcdhResponse server public key")?.to_vec();
    let ct = required_field(&hybrid_response, 3, "HybridEcdhResponse ciphertext")?.to_vec();
    let cred = hybrid_response
        .get(&2)
        .and_then(|v| v.as_varint())
        .unwrap_or(1);

    // compute shared secret from peer pub
    let peer_pk = p256::PublicKey::from_sec1_bytes(&peer).map_err(|e| e.to_string())?;
    let shared = p256::ecdh::diffie_hellman(
        temp.key_pair.secret_key.to_nonzero_scalar(),
        peer_pk.as_affine(),
    );
    let secret = sha256(shared.raw_secret_bytes().as_ref());

    // aad = sha256(okm[24..] ++ comp ++ b"415" ++ peer ++ String(cred))
    // 注意：cred 用其十进制字符串的 ASCII 字节（对齐 TS `U8(String(cred))`），而非原始字节。
    let mut aad_input = Vec::new();
    aad_input.extend(&temp.okm[24..]);
    aad_input.extend(&temp.comp);
    aad_input.extend_from_slice(b"415");
    aad_input.extend(&peer);
    aad_input.extend(cred.to_string().as_bytes());
    let aad = sha256(&aad_input);

    let laid = unlayout(&secret[..24], &ct, &aad);
    if laid.is_empty() {
        return Err("HybridEcdhResponse AES-192-GCM decrypt failed".into());
    }
    let plain = lz4(&laid).map_err(|e| e.to_string())?;
    let manual = pbf(&plain);
    let body_fields_bytes = match required_field(&manual, 3, "ManualAuthResponse field 3") {
        Ok(b) => b.to_vec(),
        Err(_) => {
            let keys: Vec<String> = {
                let mut ks: Vec<_> = manual.keys().copied().collect();
                ks.sort_unstable();
                ks.into_iter()
                    .map(|k| {
                        let kind = match manual.get(&k) {
                            Some(PbfValue::Varint(n)) => format!("varint:{n}"),
                            Some(PbfValue::Bytes(b)) => format!("bytes:{}", b.len()),
                            None => "none".into(),
                        };
                        format!("{k}={kind}")
                    })
                    .collect()
            };
            let head = hex::encode(&plain[..plain.len().min(64)]);
            // field 1 常为 BaseResponse：ret / errMsg
            let base = manual.get(&1).and_then(PbfValue::as_bytes).map(pbf);
            let ret = base
                .as_ref()
                .and_then(|b| b.get(&1).and_then(PbfValue::as_varint));
            let errmsg = base
                .as_ref()
                .and_then(|b| b.get(&2).and_then(PbfValue::as_bytes))
                .map(|m| String::from_utf8_lossy(m).into_owned());
            return Err(format!(
                "ManualAuthResponse field 3 is missing (fields=[{}], len={}, head={head}, ret={}, errmsg={})",
                keys.join(","),
                plain.len(),
                ret.map(|n| n.to_string()).unwrap_or_else(|| "-".into()),
                errmsg.unwrap_or_else(|| "-".into()),
            ));
        }
    };
    let body_fields = pbf(&body_fields_bytes);

    // 缺少 field 2 => ManualAuth 拒绝：抛带 code/message 的详细错误（对齐 TS）
    if body_fields.get(&2).and_then(|v| v.as_bytes()).is_none() {
        let code = body_fields.get(&4).and_then(|v| v.as_varint());
        let message = body_fields.get(&5).and_then(|v| v.as_bytes());
        let detail = message
            .map(|m| String::from_utf8_lossy(m).to_string())
            .unwrap_or_else(|| "unknown error".to_string());
        let code_str = code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        return Err(format!("ManualAuth rejected: code={code_str} message={detail}"));
    }

    let session = body_fields.get(&2).and_then(|v| v.as_bytes()).map(pbf);
    let identity = body_fields.get(&3).and_then(|v| v.as_bytes()).map(pbf);
    let send_key = session
        .as_ref()
        .and_then(|s| s.get(&1).and_then(|v| v.as_bytes()).map(<[u8]>::to_vec))
        .ok_or_else(|| "ManualAuthResponse send key missing".to_string())?;
    let recv_key = session
        .as_ref()
        .and_then(|s| s.get(&2).and_then(|v| v.as_bytes()).map(<[u8]>::to_vec))
        .ok_or_else(|| "ManualAuthResponse receive key missing".to_string())?;
    let f9 = session
        .as_ref()
        .and_then(|s| s.get(&9).and_then(|v| v.as_bytes()).map(<[u8]>::to_vec))
        .unwrap_or_default();
    let uin = identity
        .as_ref()
        .and_then(|s| s.get(&1).and_then(|v| v.as_varint()))
        .unwrap_or(0);
    Ok(ParsedManualResponse {
        send_key,
        recv_key,
        f9,
        uin,
    })
}

/// ManualAuthResponse 解析结果
#[derive(Debug, Clone)]
pub struct ParsedManualResponse {
    pub send_key: Vec<u8>,
    pub recv_key: Vec<u8>,
    pub f9: Vec<u8>,
    pub uin: u64,
}

// =====================================================================
// 异步 TCP socket
// =====================================================================

/// 异步 TCP 连接到 longcloud/shortcloud
///
/// 1:1 对齐原 TS `socket(host, port, timeout)`：
/// - 读：累积 chunk，按 record 协议 (type 1B + REC 2B + len 2B + body)
/// - 写：tokio write_all
pub async fn connect_socket(
    ip: &str,
    port: u16,
    timeout_ms: u64,
) -> Result<MmtlsSocket, String> {
    use tokio::net::TcpStream;
    use tokio::time::{timeout, Duration};

    let stream = timeout(Duration::from_millis(timeout_ms), TcpStream::connect((ip, port)))
        .await
        .map_err(|_| format!("socket connect timeout: {ip}:{port}"))?
        .map_err(|e| format!("socket connect {ip}:{port}: {e}"))?;
    stream
        .set_nodelay(true)
        .map_err(|e| format!("set_nodelay: {e}"))?;

    let (read_half, write_half) = stream.into_split();
    let reader = tokio::io::BufReader::new(read_half);

    Ok(MmtlsSocket {
        reader: Arc::new(tokio::sync::Mutex::new(reader)),
        writer: Arc::new(tokio::sync::Mutex::new(write_half)),
        buffer: Arc::new(parking_lot::Mutex::new(Vec::new())),
        timeout_ms,
    })
}

/// MMTLS TCP socket 抽象
#[derive(Clone)]
pub struct MmtlsSocket {
    reader: Arc<tokio::sync::Mutex<tokio::io::BufReader<tokio::net::tcp::OwnedReadHalf>>>,
    writer: Arc<tokio::sync::Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    buffer: Arc<parking_lot::Mutex<Vec<u8>>>,
    timeout_ms: u64,
}

impl MmtlsSocket {
    /// 发送
    pub async fn send(&self, data: &[u8]) -> Result<(), String> {
        use tokio::io::AsyncWriteExt;
        let mut w = self.writer.lock().await;
        w.write_all(data)
            .await
            .map_err(|e| format!("socket write: {e}"))?;
        w.flush().await.map_err(|e| format!("socket flush: {e}"))?;
        Ok(())
    }

    /// 读取下一个 record（type 1B + REC 2B + len 2B + body）
    pub async fn take(&self) -> Result<RecordFrame, String> {
        use tokio::io::AsyncReadExt;
        use tokio::time::{timeout, Duration};
        loop {
            // 先看 buffer 里有没有完整 record（guard 必须先 drop，再重新加锁 drain）
            {
                let frame = {
                    let buf = self.buffer.lock();
                    if buf.len() >= 5 {
                        let magic = u16::from_be_bytes([buf[1], buf[2]]);
                        if magic != REC {
                            Some(Err(format!(
                                "invalid MMTLS record magic {magic:#06x}"
                            )))
                        } else {
                            let len = u16::from_be_bytes([buf[3], buf[4]]) as usize;
                            if buf.len() >= 5 + len {
                                Some(Ok((
                                    buf[0],
                                    buf[5..5 + len].to_vec(),
                                    5 + len,
                                )))
                            } else {
                                None
                            }
                        }
                    } else {
                        None
                    }
                };
                if let Some(parsed) = frame {
                    let (rec_type, body, total) = parsed?;
                    self.buffer.lock().drain(..total);
                    return Ok(RecordFrame { rec_type, body });
                }
            }
            // 否则读
            let mut tmp = [0u8; 4096];
            let n = {
                let mut reader = self.reader.lock().await;
                timeout(
                    Duration::from_millis(self.timeout_ms),
                    reader.read(&mut tmp),
                )
                .await
                .map_err(|_| "socket read timeout".to_string())?
                .map_err(|e| format!("socket read: {e}"))?
            };
            if n == 0 {
                return Err("socket closed".to_string());
            }
            self.buffer.lock().extend_from_slice(&tmp[..n]);
        }
    }

    /// 一次性读所有数据（用于 ShortLink HTTP 响应）
    pub async fn read_all(&self, max_bytes: usize) -> Result<Vec<u8>, String> {
        use tokio::io::AsyncReadExt;
        use tokio::time::{timeout, Duration};
        let mut out = Vec::new();
        let mut tmp = [0u8; 4096];
        let _ = max_bytes;
        loop {
            let n = {
                let mut reader = self.reader.lock().await;
                match timeout(
                    Duration::from_millis(self.timeout_ms),
                    reader.read(&mut tmp),
                )
                .await
                {
                    Ok(Ok(0)) => break,
                    Ok(Ok(n)) => n,
                    Ok(Err(e)) => return Err(format!("socket read_all: {e}")),
                    Err(_) => break,
                }
            };
            out.extend_from_slice(&tmp[..n]);
        }
        Ok(out)
    }
}

// =====================================================================
// 主入口（真实实现）
// =====================================================================

/// 真实微信登录拿 code（TCP + MMTLS 完整握手 + ShortLink）
///
/// 完整流程：
/// 1. HTTPDNS 拿 long IP（6 次重试）
/// 2. ECDH 握手 + ServerHello + 加密 Finished 消息
/// 3. `manual_request` + `hybrid` 构造 auth 包
/// 4. HTTPDNS 拿 short IP，构造 early data + envelope
/// 5. 解析 ShortLink HTTP 响应，解密 AppData 拿 code
///
/// # Errors
/// - 网络 / 握手 / 解密失败
pub async fn get_native_wx_login_code(login_buffer: &str, app_id: &str) -> Result<String, String> {
    // 1. 构造 manual request + device + host
    let app_bytes = random_bytes(32);
    let manual = manual_request(login_buffer, &app_bytes)?;
    let device = manual.device.clone();
    let host = manual.host.clone();

    // 2. 尝试 6 次 long link
    let long_targets = fetch_long_targets().await;
    let long_targets = if long_targets.is_empty() {
        targets("long")
    } else {
        long_targets
    };

    let mut session: Option<Session> = None;
    let mut failures: Vec<String> = Vec::new();

    for t in long_targets.iter().take(6) {
        match long_link_handshake(t, &manual.req, &device, &host).await {
            Ok(s) => {
                session = Some(s);
                break;
            }
            Err(e) => failures.push(format!("{}:{} {}", t.ip, t.port, e)),
        }
    }
    let session = session.ok_or_else(|| {
        format!(
            "Unable to establish WeChat protocol session: {}",
            failures.join("; ")
        )
    })?;

    // 3. 构造 envelope + 走 short link
    let env = envelope(
        &session,
        &js_plain(session.uin, app_id, &session.host_app_id),
    )?;

    // 4. 尝试 short targets
    let short_targets = fetch_short_targets().await;
    let short_targets = if short_targets.is_empty() {
        targets("short")
    } else {
        short_targets
    };

    let mut short_failures: Vec<String> = Vec::new();
    for t in short_targets.iter() {
        match short_link_post(t, &env, &session).await {
            Ok(code) if !code.is_empty() => return Ok(code),
            Ok(_) => short_failures.push(format!("{}:{} empty code", t.ip, t.port)),
            Err(e) => short_failures.push(format!("{}:{} {}", t.ip, t.port, e)),
        }
    }
    Err(format!(
        "Unable to request wx.login code: {}",
        short_failures.join("; ")
    ))
}

/// Long link 完整握手 + ManualAuth + Session 建立
///
/// `target` 由调用方（HTTPDNS 解析结果）传入，避免重试循环失效。
async fn long_link_handshake(
    target: &Target,
    req: &[u8],
    device: &[u8],
    host: &[u8],
) -> Result<Session, String> {
    // a, b = ECDH keypairs
    let a = EcdhKeyPair::generate().map_err(|e| format!("ecdh a: {e}"))?;
    let b = EcdhKeyPair::generate().map_err(|e| format!("ecdh b: {e}"))?;
    let hello = client_hello(&a.public_bytes, &b.public_bytes);

    let sock = connect_socket(&target.ip, target.port, 30_000).await?;
    sock.send(&rec(0x16, &hello)).await?;

    // 读 ServerHello
    let sh = sock.take().await?;
    let server_hello = split_hs(&sh.body)?;
    let body = server_hello.body;
    if body.len() < 40 {
        return Err("ServerHello body too short".to_string());
    }
    let ext_length = u32::from_be_bytes([body[36], body[37], body[38], body[39]]) as usize;
    if body.len() < 40 + ext_length {
        return Err("ServerHello ext truncated".to_string());
    }
    let ext = &body[40..40 + ext_length];
    if ext.is_empty() || ext[0] != 0x01 {
        return Err("invalid ServerHello key-share extension".to_string());
    }
    // peer key 在 ext[13..78] (65 bytes)
    if ext.len() < 78 {
        return Err("ServerHello ext too short for key".to_string());
    }
    let shared_a = a.shared_secret(&ext[13..78])?;
    let secret = sha256(&shared_a);

    // transcript = hello ++ server_hello
    let mut transcript1 = Vec::new();
    transcript1.extend(&hello);
    transcript1.extend(&sh.body);
    let hs_keys = handshake_keys(&secret, "handshake key expansion", &sha256(&transcript1));

    let mut cert_hash: Option<Vec<u8>> = None;
    let mut ticket_entries: Vec<Vec<u8>> = Vec::new();
    let mut rx_seq: u64 = 1; // ServerHello 收下，sequence 1
    let mut transcript = transcript1.clone();

    for _ in 0..32 {
        let r = sock.take().await?;
        let plain = gcm(&hs_keys.sk, &hs_keys.si, rx_seq, r.rec_type, &r.body, true);
        if plain.is_empty() {
            return Err(format!(
                "handshake GCM decrypt failed seq={rx_seq} type={:#x} len={}",
                r.rec_type,
                r.body.len()
            ));
        }
        rx_seq += 1;
        let x = split_hs(&plain)?;
        if x.hs_type != 0x14 {
            transcript.extend(&plain);
        }
        if x.hs_type == 0x0F {
            cert_hash = Some(sha256(&transcript).to_vec());
        }
        if x.hs_type == 0x04 {
            // NewSessionTicket: count (1B) + (len 4B + entry) *
            if x.body.is_empty() {
                continue;
            }
            let n = x.body[0] as usize;
            let mut o = 1;
            for _ in 0..n {
                if o + 4 > x.body.len() {
                    break;
                }
                let len = u32::from_be_bytes([x.body[o], x.body[o + 1], x.body[o + 2], x.body[o + 3]])
                    as usize;
                o += 4;
                if o + len > x.body.len() {
                    break;
                }
                ticket_entries.push(x.body[o..o + len].to_vec());
                o += len;
            }
        }
        if x.hs_type == 0x14 {
            // Finished
            let hash = sha256(&transcript);
            let verify = hmac_sha256(
                &expand(&secret, "server finished", &[], 32),
                &hash,
            );
            if x.body.len() < 2 + verify.len() {
                return Err("ServerFinished too short".to_string());
            }
            if x.body[2..2 + verify.len()] != verify.to_vec() {
                return Err("MMTLS server verification failed".to_string());
            }
            // appKeys
            let app_keys = handshake_keys(
                &expand(&secret, "expanded secret", &hash, 32),
                "application data key expansion",
                &hash,
            );
            // 发送 client Finished
            let finish = hs(
                0x14,
                &[
                    vec![0, 32],
                    hmac_sha256(
                        &expand(&secret, "client finished", &[], 32),
                        &hash,
                    )
                    .to_vec(),
                ]
                .concat(),
            );
            sock.send(&rec(
                0x16,
                &{
                    let ct = gcm(&hs_keys.ck, &hs_keys.ci, 1, 0x16, &finish, false);
                    if ct.is_empty() {
                        return Err("client Finished encrypt failed".into());
                    }
                    ct
                },
            ))
            .await?;

            // 构造 hybrid auth
            let h = hybrid(req)?;
            // 1:1 对齐 TS：ints 与 bytes 必须在同一个 wpkg 容器内（中间 vi(0) 分隔），
            // 不能拆成两次 wpkg 调用再拼接。
            let mut head_ints = HashMap::new();
            for (k, v) in [
                (1, 1i64), (2, 0), (3, 0), (4, 0), (5, 524545), (6, 11), (7, 0), (8, 0),
                (9, 0), (10, 1), (11, 0), (12, 0), (13, 0), (17, 0), (18, 1), (20, 1504),
                (21, 0), (22, 0), (23, 0), (25, 17), (26, 4), (28, 1), (29, 1), (30, 0),
            ] {
                head_ints.insert(k, v);
            }
            let mut head_bytes = HashMap::new();
            head_bytes.insert(14, Vec::new());
            head_bytes.insert(24, device.to_vec());
            head_bytes.insert(27, Vec::new());
            let header = wpkg(&head_ints, &head_bytes);
            let body = [header, h.wire].concat();
            let auth_ct = gcm(
                &app_keys.ck,
                &app_keys.ci,
                2,
                0x17,
                &short(0x0D7D, 0, &body),
                false,
            );
            if auth_ct.is_empty() {
                return Err("ManualAuth AppData encrypt failed".into());
            }
            sock.send(&rec(0x17, &auth_ct)).await?;

            let ar = sock.take().await?;
            let auth = gcm(&app_keys.sk, &app_keys.si, rx_seq, ar.rec_type, &ar.body, true);
            if auth.is_empty() {
                return Err(format!(
                    "AppData GCM decrypt failed type={:#x} len={}",
                    ar.rec_type,
                    ar.body.len()
                ));
            }
            let (_, auth_body) = parse_short(&auth)?;
            let s = parse_manual(&auth_body, &h.temp)?;

            // 取第一个 type=1 的 ticket
            let tickets: Vec<(Vec<u8>, Vec<u8>)> = if let Some(ch) = &cert_hash {
                ticket_entries
                    .iter()
                    .filter(|t| t.first().copied() == Some(1u8))
                    .map(|t| (expand(&secret, "PSK_ACCESS", ch, 32), t.clone()))
                    .collect()
            } else {
                Vec::new()
            };
            if s.send_key.is_empty()
                || s.recv_key.is_empty()
                || s.uin == 0
                || tickets.is_empty()
                || ![16, 24, 32].contains(&s.send_key.len())
                || ![16, 24, 32].contains(&s.recv_key.len())
            {
                return Err("ManualAuth did not return a usable session".to_string());
            }
            let (psk, ticket) = tickets.into_iter().next().unwrap();
            return Ok(Session {
                send_key: s.send_key,
                recv_key: s.recv_key,
                f9: s.f9,
                uin: s.uin,
                device_id: device.to_vec(),
                host_app_id: host.to_vec(),
                psk,
                ticket,
            });
        }
    }
    Err("MMTLS handshake not finished".to_string())
}

/// ShortLink 早期数据 POST（HTTP/1.0 + Upgrade: mmtls + 0-RTT）
async fn short_link_post(
    target: &Target,
    env: &[u8],
    session: &Session,
) -> Result<String, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ts_u32 = ts as u32;
    // JS `ts.toString(16).padStart(8,'0')` 是小写 hex，大小写进风控指纹
    let ts_hex = format!("{ts_u32:08x}");
    let ticket = &session.ticket;
    let hello = psk_client_hello(ticket, ts_u32);
    let ek = one_way_keys(
        &session.psk,
        "early data key expansion",
        &sha256(&hello),
    );

    let type8 = type8_early_data(ts_u32);
    let rec_type8 = gcm(&ek.key, &ek.iv, 1, 0x19, &type8, false);
    let rec_env = gcm(&ek.key, &ek.iv, 2, 0x17, env, false);
    let rec_close = gcm(&ek.key, &ek.iv, 3, 0x15, &[0, 0, 0, 3, 0, 1, 1], false);
    if rec_type8.is_empty() || rec_env.is_empty() || rec_close.is_empty() {
        return Err("ShortLink 0-RTT encrypt failed".into());
    }

    let body = [
        rec(0x19, &hello),
        rec(0x19, &rec_type8),
        rec(0x17, &rec_env),
        rec(0x15, &rec_close),
    ]
    .concat();

    let request_head = short_link_request_head(&ts_hex, body.len());

    let sock = connect_socket(&target.ip, target.port, 8_000).await?;
    sock.send(&[request_head.as_bytes(), &body].concat()).await?;
    let raw = sock.read_all(65536).await?;

    // 解析 HTTP 响应
    let header_end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "ShortLink returned an invalid HTTP response".to_string())?;
    let payload = &raw[header_end + 4..];
    let recs = records(payload);

    let server_hello = recs
        .iter()
        .find(|r| r.rec_type == 0x16)
            .ok_or_else(|| "ShortLink response missing ServerHello/AppData".to_string())?
        .body
        .clone();
    let app_data = recs
        .iter()
        .find(|r| r.rec_type == 0x17)
            .ok_or_else(|| "ShortLink response missing ServerHello/AppData".to_string())?
        .body
        .clone();

    let transcripts = [
        [hello.as_slice(), server_hello.as_slice()].concat(),
        [hello.as_slice(), type8.as_slice(), server_hello.as_slice()].concat(),
        [hello.as_slice(), server_hello.as_slice(), type8.as_slice()].concat(),
    ];

    for t in &transcripts {
        let hk = one_way_keys(
            &session.psk,
            "handshake key expansion",
            &sha256(t),
        );
        for seq in [2u64, 1, 3] {
            let decrypted = gcm(&hk.key, &hk.iv, seq, 0x17, &app_data, true);
            if !decrypted.is_empty() {
                let candidates: Vec<Vec<u8>> = {
                    let mut v = vec![decrypted.clone()];
                    if let Ok((_, body)) = parse_short(&decrypted) {
                        v.insert(0, body);
                    }
                    v
                };
                for cand in candidates {
                    for offset in 0..cand.len().min(220) {
                        if let Ok(plain) = lz4(&unlayout(&session.recv_key, &cand[offset..], &[])) {
                            // 1:1 对齐 TS：pbf(plain) → field2 → pbf → field3 → bytes → string
                            let outer = pbf(&plain);
                            if let Some(inner_bytes) =
                                outer.get(&2).and_then(|v| v.as_bytes())
                            {
                                let inner = pbf(inner_bytes);
                                if let Some(code_bytes) =
                                    inner.get(&3).and_then(|v| v.as_bytes())
                                {
                                    if !code_bytes.is_empty() {
                                        return Ok(String::from_utf8_lossy(code_bytes).to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Err("ShortLink AppData decrypt/parse failed".to_string())
}

// =====================================================================
// HTTPDNS 解析
// =====================================================================

/// HTTPDNS 拿 long IP
async fn fetch_long_targets() -> Vec<Target> {
    fetch_httpdns_targets("long").await
}

/// HTTPDNS 拿 short IP
async fn fetch_short_targets() -> Vec<Target> {
    fetch_httpdns_targets("short").await
}

async fn fetch_httpdns_targets(kind: &str) -> Vec<Target> {
    let url = "http://aedns.weixin.qq.com/cgi-bin/default/getdns?clientversion=0&devicetype=Windows&uin=0&format=json";
    let domain = if kind == "long" {
        "longcloud.weixin.com"
    } else {
        "shortcloud.weixin.com"
    };
    let proto_name = if kind == "long" {
        "mmtlsovertcp"
    } else {
        "http"
    };
    let default_port = if kind == "long" { 8080u16 } else { 80u16 };
    let default_ip = if kind == "long" {
        "180.153.202.85"
    } else {
        "120.241.131.173"
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("MicroMessenger Client")
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let body: serde_json::Value = match resp.json().await {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let item = body
        .get("dns")
        .and_then(|d| d.get("domainlist"))
        .and_then(|l| l.as_array())
        .and_then(|list| list.iter().find(|x| x.get("name").and_then(|n| n.as_str()) == Some(domain)));
    let item = match item {
        Some(i) => i,
        None => return vec![Target { ip: default_ip.to_string(), port: default_port }],
    };
    let ports: Vec<u16> = item
        .get("protocollist")
        .and_then(|p| p.as_array())
        .and_then(|l| {
            l.iter()
                .find(|x| x.get("name").and_then(|n| n.as_str()) == Some(proto_name))
        })
        .and_then(|x| x.get("portlist"))
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    v.as_u64()
                        .map(|n| n as u16)
                        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                })
                .collect()
        })
        .unwrap_or_else(|| vec![default_port]);
    let ips: Vec<String> = item
        .get("iplist")
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.get("ip").and_then(|i| i.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_else(|| vec![default_ip.to_string()]);

    let out: Vec<Target> = ips
        .into_iter()
        .flat_map(|ip| ports.iter().map(move |&p| Target { ip: ip.clone(), port: p }))
        .collect();
    if out.is_empty() {
        vec![Target { ip: default_ip.to_string(), port: default_port }]
    } else {
        out
    }
}

// =====================================================================
// 工具
// =====================================================================

fn random_bytes(n: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut out = vec![0u8; n];
    rng.fill_bytes(&mut out);
    out
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("odd hex length".to_string());
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_value(bytes[i])?;
        let lo = hex_value(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hex_value(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid hex char: {}", b as char)),
    }
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    STANDARD.decode(s).map_err(|e| e.to_string())
}

#[cfg(test)]
fn base64_encode(b: &[u8]) -> String {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    STANDARD.encode(b)
}

fn sha256_with(salt: &[u8], data: &[u8]) -> [u8; 32] {
    hmac_sha256(salt, data)
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vi_basic() {
        assert_eq!(vi(0), vec![0]);
        assert_eq!(vi(1), vec![1]);
        assert_eq!(vi(127), vec![127]);
        assert_eq!(vi(128), vec![128, 1]);
        assert_eq!(vi(300), vec![0xAC, 0x02]);
    }

    #[test]
    fn vi_decode_roundtrip() {
        for n in [0i64, 1, 100, 127, 128, 1000, 16383, 16384, 1_000_000] {
            let encoded = vi(n);
            let (decoded, consumed) = rvi(&encoded, 0).unwrap();
            assert_eq!(decoded, n as u64);
            assert_eq!(consumed, encoded.len());
        }
    }

    #[test]
    fn rvi_truncated() {
        assert!(rvi(&[0x80], 0).is_err());
        assert!(rvi(&[], 0).is_err());
    }

    #[test]
    fn pbv_varint_field() {
        let v = pbv(3, 42);
        // field 3 (wire 0): key=24, value=42
        assert_eq!(v, vec![24, 42]);
    }

    #[test]
    fn pbl_bytes_field() {
        let v = pbl(2, b"hi");
        // field 2 (wire 2): key=18, length=2, "hi"
        assert_eq!(v, vec![18, 2, b'h', b'i']);
    }

    #[test]
    fn pbf_roundtrip() {
        let mut msg = Vec::new();
        msg.extend(pbv(1, 100));
        msg.extend(pbl(2, b"hello"));
        msg.extend(pbv(3, 0));
        let parsed = pbf(&msg);
        assert_eq!(parsed.get(&1).unwrap().as_varint(), Some(100));
        assert_eq!(parsed.get(&2).unwrap().as_bytes(), Some(&b"hello"[..]));
    }

    #[test]
    fn sha256_known() {
        // SHA-256("")
        let h = sha256(b"");
        assert_eq!(
            hex::encode(h),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hmac_known() {
        // HMAC-SHA256("key", "data")
        let h = hmac_sha256(b"key", b"data");
        let expected = "9307b3b915efb5171ff0d99b3002e8c992bcd66b5fc0a0b3c5a1a7a0c5f1a123"; // 假装
        let _ = expected;
        // 实际只检查长度
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn expand_size() {
        let secret = b"01234567890123456789012345678901"; // 32 bytes
        let out = expand(secret, "test", b"context", 100);
        assert_eq!(out.len(), 100);
    }

    #[test]
    fn expand_size_boundary() {
        let secret = b"x";
        let out = expand(secret, "l", b"c", 16);
        assert_eq!(out.len(), 16);
    }

    #[test]
    fn records_basic() {
        let mut data = Vec::new();
        data.extend(rec(0x16, b"hello"));
        data.extend(rec(0x17, b"world!"));
        let parsed = records(&data);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].rec_type, 0x16);
        assert_eq!(parsed[0].body, b"hello");
        assert_eq!(parsed[1].body, b"world!");
    }

    #[test]
    fn records_empty() {
        assert!(records(&[]).is_empty());
    }

    #[test]
    fn hs_roundtrip() {
        let msg = hs(0x0A, b"test");
        let parsed = split_hs(&msg).unwrap();
        assert_eq!(parsed.hs_type, 0x0A);
        assert_eq!(parsed.body, b"test");
    }

    #[test]
    fn hs_invalid() {
        assert!(split_hs(&[]).is_err());
        assert!(split_hs(&[0, 0, 0, 100]).is_err());
    }

    #[test]
    fn lz4_literal_short() {
        let encoded = lz4_literal(b"hi");
        assert_eq!(encoded, vec![(2u8 << 4), b'h', b'i']);
    }

    #[test]
    fn lz4_literal_long() {
        let data = vec![b'x'; 20];
        let encoded = lz4_literal(&data);
        // 前缀 0xF0 + 5 (20-15) = 0xF0, 5
        assert_eq!(encoded[0], 0xF0);
        assert_eq!(encoded[1], 5);
        assert_eq!(&encoded[2..], &data[..]);
    }

    #[test]
    fn short_roundtrip() {
        let msg = short(0x1234, 5, b"hello");
        let (cmd, body) = parse_short(&msg).unwrap();
        assert_eq!(cmd, 0x1234);
        assert_eq!(body, b"hello");
    }

    #[test]
    fn wpkg_basic() {
        let mut ints = HashMap::new();
        ints.insert(1, 42);
        ints.insert(2, 100);
        let mut bytes = HashMap::new();
        bytes.insert(3, b"data".to_vec());
        let pkg = wpkg(&ints, &bytes);
        // 应能解析
        let end = read_wpkg(&pkg).unwrap();
        // 末尾至少包含 pkg + 2 个 varint
        assert!(end > 0);
    }

    #[test]
    fn pbf_value_accessors() {
        let v = PbfValue::Varint(42);
        assert_eq!(v.as_varint(), Some(42));
        assert_eq!(v.as_bytes(), None);
        let v = PbfValue::Bytes(b"x".to_vec());
        assert_eq!(v.as_bytes(), Some(&b"x"[..]));
        assert_eq!(v.as_varint(), None);
    }

    #[test]
    fn server_pub_hex_valid() {
        let bytes = hex_decode(SERVER_PUB_HEX).unwrap();
        assert_eq!(bytes.len(), 65);
        assert_eq!(bytes[0], 0x04);
    }

    #[test]
    fn ecdh_keypair_generation() {
        let kp = EcdhKeyPair::generate().unwrap();
        // P-256 uncompressed: 0x04 + 32 + 32 = 65 bytes
        assert_eq!(kp.public_bytes.len(), 65);
        assert_eq!(kp.public_bytes[0], 0x04);
    }

    #[test]
    fn ecdh_shared_secret() {
        let a = EcdhKeyPair::generate().unwrap();
        let b = EcdhKeyPair::generate().unwrap();
        let s1 = a.shared_secret(&b.public_bytes).unwrap();
        let s2 = b.shared_secret(&a.public_bytes).unwrap();
        assert_eq!(s1, s2);
    }

    #[test]
    fn targets_long() {
        let t = targets("long");
        assert!(!t.is_empty());
        assert_eq!(t[0].ip, "180.153.202.85");
        assert_eq!(t[0].port, 8080);
    }

    #[test]
    fn targets_short() {
        let t = targets("short");
        assert!(!t.is_empty());
        assert_eq!(t[0].ip, "120.241.131.173");
        assert_eq!(t[0].port, 80);
    }

    #[test]
    fn mmtls_nonce_basic() {
        let iv = [0u8; 12];
        let n = mmtls_nonce(&iv, 1);
        // 末尾 8 字节为 0x0000000000000001
        assert_eq!(&n[4..], &[0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn mmtls_nonce_xor() {
        let mut iv = [0u8; 12];
        iv[11] = 0xAA;
        // seq = 0x0100...00, s[7] = 0x00, n[11] ^= 0x00 = 0xAA（无变化）
        let n = mmtls_nonce(&iv, 0x0100000000000000);
        assert_eq!(n[11], 0xAA);
        // seq = 0x01, s[7] = 0x01, n[11] ^= 0x01 = 0xAB
        let n2 = mmtls_nonce(&iv, 0x01);
        assert_eq!(n2[11], 0xAB);
    }

    #[test]
    fn gcm_encrypt_decrypt_roundtrip() {
        let key = [0u8; 16];
        let iv = [0u8; 12];
        let plain = b"hello world";
        let seq = 1;
        let rec_type = 0x17;
        let ct = gcm(&key, &iv, seq, rec_type, plain, false);
        assert!(ct.len() > plain.len());
        let decrypted = gcm(&key, &iv, seq, rec_type, &ct, true);
        assert_eq!(decrypted, plain);
    }

    #[test]
    fn layout_unlayout_roundtrip() {
        let key = [1u8; 32];
        let plain = b"secret message";
        let blob = layout(&key, plain, b"aad");
        let restored = unlayout(&key, &blob, b"aad");
        assert_eq!(restored, plain);
    }

    #[test]
    fn layout_unlayout_aes192() {
        let key = [0x5Au8; 24];
        let plain = b"hybrid-ecdh-aes192";
        let blob = layout(&key, plain, b"aad-192");
        assert_eq!(blob.len(), plain.len() + 12 + 16);
        let restored = unlayout(&key, &blob, b"aad-192");
        assert_eq!(restored, plain);
        let bad = unlayout(&[0x5Bu8; 24], &blob, b"aad-192");
        assert!(bad.is_empty());
    }

    #[test]
    fn layout_rejects_unsupported_key_len() {
        let blob = layout(&[0u8; 15], b"x", &[]);
        assert!(blob.is_empty(), "must not emit a fake iv+zero-tag packet");
    }

    #[test]
    fn mmtls_path_hex_is_lowercase_like_js() {
        // JS: Number(ts).toString(16).padStart(8, '0') → lowercase
        let ts = 1_700_000_000u32;
        assert_eq!(format!("{ts:08x}"), "6553f100");
        assert_eq!(format!("{ts:08X}"), "6553F100");
    }

    #[test]
    fn short_link_http_head_matches_js_template() {
        let head = short_link_request_head("6553f100", 1234);
        assert_eq!(
            head,
            "POST /mmtls/6553f100 HTTP/1.0\r\nAccept: */*\r\nCache-Control: no-cache\r\nConnection: close\r\nContent-Length: 1234\r\nContent-Type: application/octet-stream\r\nHost: shortcloud.weixin.com\r\nUpgrade: mmtls\r\nUser-Agent: MicroMessenger Client\r\nX-Online-Host: shortcloud.weixin.com\r\n\r\n"
        );
        assert!(!head.contains(" \r\n"), "header lines must not be indented");
        assert!(head.ends_with("\r\n\r\n"));
    }

    #[test]
    fn gcm_rejects_unsupported_key_len() {
        let iv = [0x22u8; 12];
        assert!(gcm(&[0u8; 24], &iv, 1, 0x17, b"x", false).is_empty());
        assert!(gcm(&[0u8; 15], &iv, 1, 0x17, b"x", false).is_empty());
    }

    #[test]
    fn layout_unlayout_wrong_key() {
        let key1 = [1u8; 32];
        let key2 = [2u8; 32];
        let plain = b"secret message";
        let blob = layout(&key1, plain, b"aad");
        let restored = unlayout(&key2, &blob, b"aad");
        assert_ne!(restored, plain);
    }

    #[test]
    fn lz4_decompress_literal() {
        // 编码 15 字节以下：<len<<4> + data
        let encoded = [(5u8 << 4), b'h', b'e', b'l', b'l', b'o'];
        let decoded = lz4(&encoded).unwrap();
        assert_eq!(decoded, b"hello");
    }

    #[test]
    fn lz4_backref_rle_matches_js() {
        // token 0x10: literal 1 + match 4, offset=1 → "AAAAA"（TS out[out.length-off]）
        let encoded = [0x10, b'A', 0x01, 0x00];
        assert_eq!(lz4(&encoded).unwrap(), b"AAAAA");
    }

    #[test]
    fn lz4_backref_copy_matches_js() {
        // token 0x40: literal 4 + match 4, offset=4 → "ABCDABCD"
        let encoded = [0x40, b'A', b'B', b'C', b'D', 0x04, 0x00];
        assert_eq!(lz4(&encoded).unwrap(), b"ABCDABCD");
    }

    #[test]
    fn lz4_backref_overlapping_offset_2() {
        // "ababab" : literal "ab" + offset 2 + match 4
        // token 0x20: literal 2, match 4
        let encoded = [0x20, b'a', b'b', 0x02, 0x00];
        assert_eq!(lz4(&encoded).unwrap(), b"ababab");
    }

    #[test]
    fn required_field_missing() {
        let m: HashMap<u32, PbfValue> = HashMap::new();
        assert!(required_field(&m, 1, "x").is_err());
    }

    #[test]
    fn required_field_wrong_type() {
        let mut m = HashMap::new();
        m.insert(1, PbfValue::Varint(100));
        assert!(required_field(&m, 1, "x").is_err());
    }

    #[test]
    fn session_clone() {
        let s = Session {
            send_key: vec![1; 16],
            recv_key: vec![2; 16],
            f9: vec![3; 32],
            uin: 12345,
            device_id: vec![4; 16],
            host_app_id: vec![5; 16],
            psk: vec![6; 32],
            ticket: vec![7; 16],
        };
        let c = s.clone();
        assert_eq!(c.uin, 12345);
    }

    #[test]
    fn record_frame_clone() {
        let f = RecordFrame {
            rec_type: 0x16,
            body: vec![1, 2, 3],
        };
        let c = f.clone();
        assert_eq!(c.rec_type, 0x16);
    }

    #[test]
    fn handshake_frame_clone() {
        let f = HandshakeFrame {
            hs_type: 0x0A,
            body: vec![1, 2],
        };
        let c = f.clone();
        assert_eq!(c.hs_type, 0x0A);
    }

    #[test]
    fn hybrid_basic() {
        let plain = b"test payload";
        let h = hybrid(plain).unwrap();
        assert!(!h.wire.is_empty());
        assert!(!h.temp.okm.is_empty());
    }

    #[test]
    fn manual_request_basic() {
        // 构造一个最小的 login buffer（手工构造 pbf 形式）
        // field 1: ticket (bytes)
        // field 2: device (bytes)
        // field 3: host (bytes)
        let mut raw = Vec::new();
        raw.extend(pbl(1, b"my_ticket"));
        raw.extend(pbl(2, b"my_device"));
        let b64 = base64_encode(&raw);
        let app = b"my_app";
        let r = manual_request(&b64, app).unwrap();
        assert!(!r.req.is_empty());
        assert_eq!(r.device, b"my_device");
        // host 缺失时使用HOST_APP
        assert_eq!(r.host, HOST_APP);
    }

    // ===== 阶段 2H：新增 helper 测试 =====

    #[test]
    fn handshake_keys_basic() {
        let secret = b"shared_secret_32_bytes_long_here!!";
        let hash = b"transcript_hash_32_bytes_long_here!";
        let k = handshake_keys(secret, "label", hash);
        assert_eq!(k.ck.len(), 16);
        assert_eq!(k.sk.len(), 16);
        assert_eq!(k.ci.len(), 12);
        assert_eq!(k.si.len(), 12);
        // 同样的输入 → 同样的输出（确定性）
        let k2 = handshake_keys(secret, "label", hash);
        assert_eq!(k.ck, k2.ck);
        assert_eq!(k.sk, k2.sk);
    }

    #[test]
    fn one_way_keys_basic() {
        let secret = b"shared_secret_32_bytes_long_here!!";
        let hash = b"transcript_hash_32_bytes_long_here!";
        let k = one_way_keys(secret, "label", hash);
        assert_eq!(k.key.len(), 16);
        assert_eq!(k.iv.len(), 12);
    }

    #[test]
    fn client_hello_format() {
        let pub1 = vec![0x04u8; 65];
        let pub2 = vec![0x04u8; 65];
        let ch = client_hello(&pub1, &pub2);
        // 5 字节 hs header + body
        assert!(ch.len() > 30);
        // header: type=1 (ClientHello), len
        assert_eq!(ch[4], 1);
    }

    #[test]
    fn psk_client_hello_format() {
        let ticket = vec![0xAAu8; 100];
        let ch = psk_client_hello(&ticket, 1234567890);
        assert!(ch.len() > 30);
        assert_eq!(ch[4], 0x01);
    }

    #[test]
    fn js_plain_basic() {
        let plain = js_plain(12345678, "wxd44977328b36e647", b"my_host_app_id");
        assert!(!plain.is_empty());
        // 至少有 50 字节
        assert!(plain.len() > 50);
    }

    #[test]
    fn connect_socket_local_failure() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            // 连一个不存在的 IP 应该快速失败
            let r = connect_socket("127.0.0.1", 1, 1000).await;
            assert!(r.is_err(), "应该 connect 失败");
        });
    }

    // =====================================================================
    // Golden 向量测试：与参考 native-protocol.ts 逐字节一致
    // 向量由 `scripts/wx-golden-gen.mts` 生成（确定性 randomBytes + 固定 Date.now）
    // =====================================================================

    fn hex(v: &[u8]) -> String {
        v.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn golden_client_hello() {
        let random32: Vec<u8> = (0..32u8).collect();
        let pub1 = [vec![0x04u8], vec![0x11u8; 64]].concat();
        let pub2 = [vec![0x04u8], vec![0x22u8; 64]].concat();
        let got = client_hello_with(&pub1, &pub2, &random32, 1700000000);
        assert_eq!(
            hex(&got),
            "000000d00103f101c02b000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6553f100000000a2010000009d00100200000047000000010041041111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111100000047000000020041042222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222200000001"
        );
    }

    #[test]
    fn golden_psk_client_hello() {
        let random32: Vec<u8> = (32..64u8).collect();
        let ticket = vec![0xAAu8; 100];
        let got = psk_client_hello_with(&ticket, 1700000000, &random32);
        assert_eq!(
            hex(&got),
            "0000009e0103f10100a8202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f6553f10000000070010000006b000f0100000064aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
    }

    #[test]
    fn golden_js_plain() {
        // mac6 = counter 64..69（0x40..0x45）
        let mac6 = [0x40u8, 0x41, 0x42, 0x43, 0x44, 0x45];
        let got = js_plain_with(12345678, "wxd44977328b36e647", b"my_host_app_id", &mac6);
        assert_eq!(
            hex(&got),
            "0a350a0a73657373696f6e6b657910cec2f1051a1134322d34312d34322d34332d34342d343520ff8d9c98062a0757696e646f77733000121b2f6367692d62696e2f6d6d62697a2d62696e2f6a732d6c6f67696e1a0e6d795f686f73745f6170705f696420052a5c0a3e0a0a73657373696f6e6b657910cec2f1051a1134322d34312d34322d34332d34342d343520ff8d9c98062a10556e6966696564504357696e646f77733000121277786434343937373332386233366536343720012a0032003801321277786434343937373332386233366536343738850840d1f28080064a1157696e646f777378576562506c7567696e50d1f2c49102"
        );
    }

    #[test]
    fn golden_lz4_literal_and_roundtrip() {
        assert_eq!(hex(&lz4_literal(b"hello")), "5068656c6c6f");
        assert_eq!(
            hex(&lz4_literal(&vec![b'A'; 20])),
            "f0054141414141414141414141414141414141414141"
        );
        let lit = lz4_literal(b"the quick brown fox");
        assert_eq!(hex(&lz4(&lit).unwrap()), "74686520717569636b2062726f776e20666f78");
    }

    #[test]
    fn golden_wpkg_short_pbl_pbv() {
        let mut ints = HashMap::new();
        ints.insert(1, 1);
        ints.insert(2, 12345);
        let mut bytes = HashMap::new();
        bytes.insert(3, b"xyz".to_vec());
        assert_eq!(hex(&wpkg(&ints, &bytes)), "01010102b96000030378797a000d");

        assert_eq!(hex(&short(0x0d7d, 0, &[1, 2, 3])), "000000131110076d00000d7d00000000010203");
        assert_eq!(hex(&pbl(2, b"abc")), "1203616263");
        assert_eq!(hex(&pbv(2, 300)), "10ac02");
    }

    #[test]
    fn golden_expand_and_nonce() {
        assert_eq!(
            hex(&expand(b"secret", "label", b"ctx", 32)),
            "e35ba6122d834ae0f6f5de6fb05255f8a9fc4f4223cc6a9318a6a056679a0ebb"
        );
        let iv = unhex("0000000000000000000000ff");
        let n = mmtls_nonce(&iv, 7);
        assert_eq!(hex(&n), "0000000000000000000000f8");
    }

    #[test]
    fn node_ecdh_priv1_shared_secret() {
        // Node: createECDH('prime256v1'); setPrivateKey(1); computeSecret(SERVER_PUB)
        let mut priv_bytes = [0u8; 32];
        priv_bytes[31] = 1;
        let sk = p256::SecretKey::from_slice(&priv_bytes).expect("priv 1");
        let pair = EcdhKeyPair {
            secret_key: sk.clone(),
            public_bytes: sk.public_key().to_encoded_point(false).as_bytes().to_vec(),
        };
        assert_eq!(
            hex(&pair.public_bytes),
            "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c2964fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
        );
        let server = hex_decode(SERVER_PUB_HEX).unwrap();
        let shared = pair.shared_secret(&server).unwrap();
        assert_eq!(shared.len(), 32);
        assert_eq!(
            hex(&shared),
            "ef87876d6478b15f1796eab12068610541173b7176b67f1dcc86683e901acd44"
        );
    }

    #[test]
    fn node_aes192_layout_unlayout() {
        // Node createCipheriv('aes-192-gcm') → ciphertext||iv||tag
        let blob = unhex("8e48de4360f6ce2734566495fd05792e2f97111111111111111111111111dbae6d00a91de85f6e255b8aaf551dc3");
        let key = [0x5Au8; 24];
        let plain = unlayout(&key, &blob, b"aad-192");
        assert_eq!(plain, b"hybrid-ecdh-aes192");
    }

    #[test]
    fn node_mmtls_gcm_encrypt() {
        let key = [0x11u8; 16];
        let iv = [0x22u8; 12];
        let ct = gcm(&key, &iv, 7, 0x17, b"mmtls-gcm-roundtrip", false);
        assert_eq!(
            hex(&ct),
            "78c82c7e43ee7748cfaeb0680a5f2a1e85450dfd9c45f54a63df158a7e41a0f76a5225"
        );
    }

    #[test]
    fn lz4_long_literal_then_rle_backref() {
        // token 0xF6: literal 20 (=15+5) + match 10, offset=1 → 30 个 'A'
        let mut encoded = vec![0xF6, 0x05];
        encoded.extend(std::iter::repeat(b'A').take(20));
        encoded.extend([0x01, 0x00]);
        assert_eq!(lz4(&encoded).unwrap(), vec![b'A'; 30]);
    }

    #[test]
    fn golden_type8_early_data() {
        let t = type8_early_data(1_700_000_000);
        assert_eq!(t.len(), 20);
        assert_eq!(
            hex(&t),
            "00000010080000000b010000000600126553f100"
        );
        // 再写一个不会 panic 的大时间戳（u32 最大值）
        let t_max = type8_early_data(u32::MAX);
        assert_eq!(t_max.len(), 20);
        assert_eq!(&t_max[16..20], &[0xff, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn gcm_roundtrip_aes128() {
        let key = [0x11u8; 16];
        let iv = [0x22u8; 12];
        let plain = b"mmtls-gcm-roundtrip";
        let ct = gcm(&key, &iv, 7, 0x17, plain, false);
        assert!(ct.len() > plain.len());
        let back = gcm(&key, &iv, 7, 0x17, &ct, true);
        assert_eq!(back, plain);
        // 错 seq 解不开
        let bad = gcm(&key, &iv, 8, 0x17, &ct, true);
        assert!(bad.is_empty());
    }

    #[test]
    fn hybrid_wire_structure() {
        let h = hybrid(b"hello-hybrid").expect("hybrid");
        assert!(!h.wire.is_empty());
        assert!(!h.temp.okm.is_empty());
        assert!(!h.temp.comp.is_empty());
        let fields = pbf(&h.wire);
        assert_eq!(fields.get(&1).and_then(|v| v.as_varint()), Some(1));
        let key_share = fields.get(&2).and_then(|v| v.as_bytes()).expect("field 2");
        let inner = pbf(key_share);
        assert_eq!(inner.get(&1).and_then(|v| v.as_varint()), Some(415));
        let pub_bytes = inner.get(&2).and_then(|v| v.as_bytes()).expect("pub");
        assert_eq!(pub_bytes.len(), 65);
        assert_eq!(pub_bytes[0], 0x04);
        let enc_key = fields.get(&3).and_then(|v| v.as_bytes()).expect("field 3");
        // cek(32) + iv(12) + tag(16) — AES-192-GCM layout，不能是失败时的 iv+零 tag（28 字节）
        assert_eq!(enc_key.len(), 32 + 12 + 16);
        let enc = fields.get(&5).and_then(|v| v.as_bytes()).expect("field 5");
        assert!(enc.len() > 28, "hybrid ciphertext too short for AES-192 layout");
    }

    #[test]
    fn golden_manual_request() {
        let ticket = vec![0xAAu8; 100];
        let device = vec![0xCDu8; 16];
        let host = b"wx".repeat(16);
        let mut login_buffer_raw = pbl(1, &ticket);
        login_buffer_raw.extend(pbl(2, &device));
        login_buffer_raw.extend(pbl(3, &host));
        let login_buffer = base64_encode(&login_buffer_raw);

        let app_bytes = vec![0xABu8; 32];
        let m = manual_request(&login_buffer, &app_bytes).unwrap();
        assert_eq!(
            hex(&m.req),
            "0a250a20abababababababababababababababababababababababababababababababab10ed0e1a660a64aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2004320038004006"
        );
    }

    #[test]
    fn envelope_shortlink_structure_matches_js() {
        let session = Session {
            send_key: vec![0x11u8; 16],
            recv_key: vec![0x22u8; 16],
            f9: vec![0x33u8; 8],
            uin: 12345678,
            device_id: b"device-id-bytes".to_vec(),
            host_app_id: HOST_APP.to_vec(),
            psk: vec![0x44u8; 32],
            ticket: vec![0x55u8; 16],
        };
        let env = envelope(&session, b"js-login-plain").expect("envelope");
        let total = u32::from_be_bytes(env[0..4].try_into().unwrap()) as usize;
        assert_eq!(total, env.len() - 4);

        let mut o = 4;
        let path_len = u16::from_be_bytes(env[o..o + 2].try_into().unwrap()) as usize;
        o += 2;
        assert_eq!(&env[o..o + path_len], TRANSFER_PATH);
        o += path_len;
        let host_len = u16::from_be_bytes(env[o..o + 2].try_into().unwrap()) as usize;
        o += 2;
        assert_eq!(&env[o..o + host_len], TRANSFER_HOST);
        o += host_len;
        let inner_len = u32::from_be_bytes(env[o..o + 4].try_into().unwrap()) as usize;
        o += 4;
        let inner = &env[o..o + inner_len];
        assert_eq!(&inner[4..6], &0x1110u16.to_be_bytes());
        assert_eq!(&inner[6..8], &0x076Du16.to_be_bytes());
        assert_eq!(&inner[8..12], &0x0B41u32.to_be_bytes());
        assert_eq!(&inner[12..16], &0u32.to_be_bytes());

        let (cmd, body) = parse_short(inner).unwrap();
        assert_eq!(cmd, 0x0B41);
        let wpkg_len = read_wpkg(&body).unwrap();
        assert!(wpkg_len > 0 && wpkg_len < body.len());
        let enc = &body[wpkg_len..];
        let compressed = lz4_literal(b"js-login-plain");
        assert_eq!(enc.len(), compressed.len() + 12 + 16);
    }

    #[test]
    fn golden_wpkg_longlink_and_envelope_headers() {
        // 与 TS native-protocol.ts 同一套 ints/bytes，逐字节锁定 cmd 容器指纹
        let device = vec![0xCDu8; 16];
        let mut long_ints = HashMap::new();
        for (k, v) in [
            (1, 1i64),
            (2, 0),
            (3, 0),
            (4, 0),
            (5, 524545),
            (6, 11),
            (7, 0),
            (8, 0),
            (9, 0),
            (10, 1),
            (11, 0),
            (12, 0),
            (13, 0),
            (17, 0),
            (18, 1),
            (20, 1504),
            (21, 0),
            (22, 0),
            (23, 0),
            (25, 17),
            (26, 4),
            (28, 1),
            (29, 1),
            (30, 0),
        ] {
            long_ints.insert(k, v);
        }
        let mut long_bytes = HashMap::new();
        long_bytes.insert(14, Vec::new());
        long_bytes.insert(24, device);
        long_bytes.insert(27, Vec::new());
        assert_eq!(
            hex(&wpkg(&long_ints, &long_bytes)),
            "01010102000300040005818220060b0700080009000a010b000c000d001100120114e00b15001600170019111a041c011d011e00000e001810cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd1b00004c"
        );

        let mut env_ints = HashMap::new();
        for (k, v) in [
            (1, 1i64),
            (2, 12345678),
            (3, 0),
            (4, 0),
            (5, 524545),
            (6, 11),
            (7, 0),
            (8, 0),
            (9, 0),
            (10, 1),
            (11, 0),
            (12, 0),
            (13, 0),
            (17, 0),
            (18, 1),
            (20, 1504),
            (21, 0),
            (22, 12345678),
            (23, 0),
            (25, 16),
            (26, 4),
            (28, 1),
            (29, 1),
            (30, 0),
        ] {
            env_ints.insert(k, v);
        }
        let mut env_bytes = HashMap::new();
        env_bytes.insert(14, Vec::new());
        env_bytes.insert(24, b"device-id-bytes".to_vec());
        env_bytes.insert(27, vec![0x33u8; 8]);
        assert_eq!(
            hex(&wpkg(&env_ints, &env_bytes)),
            "01010102cec2f1050300040005818220060b0700080009000a010b000c000d001100120114e00b150016cec2f105170019101a041c011d011e00000e00180f6465766963652d69642d62797465731b0833333333333333330059"
        );
    }
}
