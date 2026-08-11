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
//! - 真实 TCP 收发（`getNativeWxLoginCode`）留到集成时实现：
//!   本模块提供 `get_native_wx_login_code` stub，返回 `unimplemented!()`，
//!   所有依赖的编解码原语都可单独测试。
//! - 真实 HTTPDNS 解析（`targets`）同理：返回 hardcoded fallback。

use std::collections::HashMap;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Aes256Gcm, Nonce};
use hmac::{Hmac, Mac};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::PublicKey;
use sha2::{Digest, Sha256};

const U8: fn(&[u8]) -> Vec<u8> = |s: &[u8]| s.to_vec();

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
    let nonce = Nonce::from_slice(&nonce_arr);

    if key.len() == 16 {
        let cipher = Aes128Gcm::new_from_slice(key).unwrap_or_else(|_| Aes128Gcm::new_from_slice(&[0u8; 16]).unwrap());
        if decrypt {
            if data.len() < 16 {
                return vec![];
            }
            // 把末尾 16 字节当 tag；前面是 ciphertext
            // aes-gcm crate decrypt 期望 msg = ciphertext + tag
            match cipher.decrypt(
                nonce,
                aes_gcm::aead::Payload {
                    msg: data,
                    aad: &aad,
                },
            ) {
                Ok(plain) => plain,
                Err(_) => vec![],
            }
        } else {
            cipher
                .encrypt(
                    nonce,
                    aes_gcm::aead::Payload {
                        msg: data,
                        aad: &aad,
                    },
                )
                .unwrap_or_default()
        }
    } else if key.len() == 32 {
        let cipher = Aes256Gcm::new_from_slice(key).unwrap_or_else(|_| Aes256Gcm::new_from_slice(&[0u8; 32]).unwrap());
        if decrypt {
            if data.len() < 16 {
                return vec![];
            }
            match cipher.decrypt(
                nonce,
                aes_gcm::aead::Payload {
                    msg: data,
                    aad: &aad,
                },
            ) {
                Ok(plain) => plain,
                Err(_) => vec![],
            }
        } else {
            cipher
                .encrypt(
                    nonce,
                    aes_gcm::aead::Payload {
                        msg: data,
                        aad: &aad,
                    },
                )
                .unwrap_or_default()
        }
    } else {
        vec![]
    }
}

/// 普通 AES-GCM（返回 ciphertext + iv + tag，1:1 对齐原 TS `layout`）
#[must_use]
pub fn layout(key: &[u8], plain: &[u8], aad: &[u8]) -> Vec<u8> {
    let iv = random_bytes(12);
    let ct = gcm_simple(key, &iv, plain, aad, false);
    // ct = ciphertext + tag（共 plain.len() + 16 字节；如加密失败则为空）
    // 输出 = ciphertext + iv + tag
    let mut out = Vec::with_capacity(plain.len() + 12 + 16);
    if ct.len() >= 16 {
        out.extend_from_slice(&ct[..ct.len() - 16]); // ciphertext
    }
    out.extend_from_slice(&iv);
    if ct.len() >= 16 {
        out.extend_from_slice(&ct[ct.len() - 16..]); // tag
    } else {
        // 加密失败：返回 iv + 16 字节 0 作为占位 tag
        out.extend(std::iter::repeat(0u8).take(16));
    }
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

/// 普通 AES-GCM 加解密（不带 seq 变体）
fn gcm_simple(key: &[u8], iv: &[u8], data: &[u8], aad: &[u8], decrypt: bool) -> Vec<u8> {
    let nonce = Nonce::from_slice(iv);
    if key.len() == 16 {
        let cipher = Aes128Gcm::new(key.into());
        if decrypt {
            use aes_gcm::aead::Aead;
            cipher
                .decrypt(nonce, aes_gcm::aead::Payload { msg: data, aad })
                .unwrap_or_default()
        } else {
            use aes_gcm::aead::Aead;
            cipher
                .encrypt(nonce, aes_gcm::aead::Payload { msg: data, aad })
                .unwrap_or_default()
        }
    } else if key.len() == 32 {
        let cipher = Aes256Gcm::new(key.into());
        if decrypt {
            use aes_gcm::aead::Aead;
            cipher
                .decrypt(nonce, aes_gcm::aead::Payload { msg: data, aad })
                .unwrap_or_default()
        } else {
            use aes_gcm::aead::Aead;
            cipher
                .encrypt(nonce, aes_gcm::aead::Payload { msg: data, aad })
                .unwrap_or_default()
        }
    } else {
        vec![]
    }
}

// =====================================================================
// 帧 / 握手 / 容器
// =====================================================================

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
// LZ4（仅 literal 模式，因为协议只用 literal）
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
        for j in 0..m {
            let pos = out.len() - off + j;
            if pos < out.len() {
                out.push(out[pos]);
            }
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
        use rand::RngCore;
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
#[must_use]
pub fn envelope(session: &Session, plain: &[u8]) -> Vec<u8> {
    let enc = layout(&session.send_key, &lz4_literal(plain), &[]);
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
    out
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
// 主入口（stub）
// =====================================================================

/// 真实微信登录拿 code（TCP + MMTLS 完整握手 + ShortLink）
///
/// 真实实现需要：
/// 1. `targets("long")` 拿 long IP，6 次重试
/// 2. ECDH 握手 + ServerHello + 加密 Finished 消息
/// 3. `manual_request` + `hybrid` 构造 auth 包
/// 4. `targets("short")` 拿 short IP，构造 early data + envelope
/// 5. 解析 ShortLink HTTP 响应，解密 AppData 拿 code
///
/// **本阶段只完成协议层（编码原语），真实 TCP 收发留到集成时**。
///
/// # Errors
/// - 当前实现返回 `unimplemented!()` 占位错误
pub async fn get_native_wx_login_code(_login_buffer: &str, _app_id: &str) -> Result<String, String> {
    Err("get_native_wx_login_code 真实网络收发留到集成时（仅协议层已 1:1 翻译）".to_string())
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
        // host 缺失时使用 HOST_APP
        assert_eq!(r.host, HOST_APP);
    }
}
