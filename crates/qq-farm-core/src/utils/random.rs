//! 随机 / 异步等待工具。
//!
//! 1:1 翻译原 `core/src/utils/utils.ts`（`sleep` / `randomDelay`）和
//! `core/src/utils/gateway-token.ts`（`createGatewayToken`）。

use std::time::Duration;

use rand::RngExt;
use tokio::time::sleep;

/// 异步 sleep
pub async fn sleep_ms(ms: u64) {
    sleep(Duration::from_millis(ms)).await;
}

/// 范围内随机延迟（毫秒，含两端）
pub async fn random_delay(min_ms: u64, max_ms: u64) {
    let delay = random_u64(min_ms, max_ms.max(min_ms));
    sleep_ms(delay).await;
}

/// 范围内随机秒数延迟
pub async fn random_delay_secs(min_secs: u64, max_secs: u64) {
    random_delay(min_secs * 1000, max_secs * 1000).await;
}

/// 范围内随机 i64（含两端）
pub fn random_i64(min: i64, max: i64) -> i64 {
    if max <= min {
        return min;
    }
    let mut rng = rand::rng();
    rng.random_range(min..=max)
}

/// 范围内随机 u64（含两端）
pub fn random_u64(min: u64, max: u64) -> u64 {
    if max <= min {
        return min;
    }
    let mut rng = rand::rng();
    rng.random_range(min..=max)
}

// =====================================================================
// Gateway Token（1:1 翻译 `gateway-token.ts`）
// =====================================================================

const TOKEN_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// 创建 gateway token（64~127 字符 + `=` 后缀，字母数字随机）
#[must_use]
pub fn create_gateway_token() -> String {
    let mut rng = rand::rng();
    let length = 64 + rng.random_range(0..64) as usize;
    let mut token = String::with_capacity(length + 1);
    for _ in 0..length {
        let idx = rng.random_range(0..TOKEN_ALPHABET.len());
        token.push(TOKEN_ALPHABET[idx] as char);
    }
    token.push('=');
    token
}

/// 一次性 TSDK 初始化凭据 + 随机 token 提供器（1:1 翻译 `gateway-token.ts::GatewayTokenProvider`）。
///
/// 登录成功后 `bindUser` 产出加密初始化凭据，stage 进来；下一条出站消息的
/// `token` 字段携带它（恰好一次，原子消费），之后恢复随机 token。
pub struct GatewayTokenProvider {
    pending_init_token: parking_lot::Mutex<Option<String>>,
}

impl GatewayTokenProvider {
    #[must_use]
    pub fn new() -> Self {
        Self { pending_init_token: parking_lot::Mutex::new(None) }
    }

    /// 暂存一次性初始化凭据，返回凭据长度（0 表示忽略）。
    ///
    /// 对齐 bot `stageInitToken`：空串忽略；超长（>64KB）或含非可打印 ASCII
    /// 视为格式无效——这里与 bot 一致抛错由调用方降级为 warn。
    pub fn stage_init_token(&self, value: &str) -> Result<usize, String> {
        let token = value.trim();
        if token.is_empty() {
            return Ok(0);
        }
        if token.len() > 64 * 1024 || !token.bytes().all(|b| (0x21..=0x7E).contains(&b)) {
            return Err("TSDK 初始化凭据格式无效".to_string());
        }
        let len = token.len();
        *self.pending_init_token.lock() = Some(token.to_string());
        Ok(len)
    }

    /// 取下一条消息的 token：有暂存凭据则原子消费返回一次，否则随机 token。
    pub fn next(&self) -> String {
        self.next_marked().0
    }

    /// 同 [`next`]，并返回该 token 是否为暂存的一次性凭据（诊断用）。
    pub fn next_marked(&self) -> (String, bool) {
        let staged = self.pending_init_token.lock().take();
        match staged {
            Some(token) => (token, true),
            None => (create_gateway_token(), false),
        }
    }

    /// 清空暂存凭据（断线时调用，对齐 bot `clearNetworkRuntime → clear()`）。
    pub fn clear(&self) {
        *self.pending_init_token.lock() = None;
    }
}

impl Default for GatewayTokenProvider {
    fn default() -> Self {
        Self::new()
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sleep_ms_runs() {
        let start = std::time::Instant::now();
        sleep_ms(50).await;
        let elapsed = start.elapsed().as_millis();
        assert!(elapsed >= 45, "sleep 50ms should take >= 45ms, got {elapsed}");
    }

    #[tokio::test]
    async fn random_delay_within_range() {
        let start = std::time::Instant::now();
        random_delay(50, 100).await;
        let elapsed = start.elapsed().as_millis();
        assert!(elapsed >= 45, "elapsed={elapsed}");
        assert!(elapsed <= 200, "elapsed={elapsed}");
    }

    #[tokio::test]
    async fn random_delay_min_eq_max() {
        let start = std::time::Instant::now();
        random_delay(50, 50).await;
        assert!(start.elapsed().as_millis() >= 45);
    }

    #[test]
    fn random_i64_range() {
        for _ in 0..100 {
            let n = random_i64(10, 20);
            assert!((10..=20).contains(&n));
        }
    }

    #[test]
    fn random_i64_min_eq_max() {
        assert_eq!(random_i64(5, 5), 5);
    }

    #[test]
    fn gateway_token_length_and_chars() {
        for _ in 0..50 {
            let t = create_gateway_token();
            // 64-127 chars + '='
            assert!(t.len() >= 65 && t.len() <= 128, "len={}", t.len());
            assert!(t.ends_with('='));
            let body = &t[..t.len() - 1];
            for c in body.chars() {
                assert!(c.is_ascii_alphanumeric(), "non-alphanumeric char: {c} in {t}");
            }
        }
    }

    #[test]
    fn gateway_token_random() {
        let a = create_gateway_token();
        let b = create_gateway_token();
        // 极小概率相同
        assert_ne!(a, b);
    }

    #[test]
    fn token_provider_stages_and_consumes_once() {
        let p = GatewayTokenProvider::new();
        // 未 stage 时返回随机 token
        let random_one = p.next();
        assert!(random_one.ends_with('=') && random_one.len() >= 65);

        let len = p.stage_init_token("  abc123  ").expect("valid token");
        assert_eq!(len, 6);
        // 恰好消费一次：第一次返回 staged，第二次恢复随机
        assert_eq!(p.next(), "abc123");
        let after = p.next();
        assert_ne!(after, "abc123");
        assert!(after.ends_with('='));
    }

    #[test]
    fn token_provider_empty_is_ignored() {
        let p = GatewayTokenProvider::new();
        assert_eq!(p.stage_init_token("   ").expect("empty ok"), 0);
        assert!(p.next().ends_with('='));
    }

    #[test]
    fn token_provider_rejects_invalid_format() {
        let p = GatewayTokenProvider::new();
        assert!(p.stage_init_token("has space").is_err());
        assert!(p.stage_init_token("中文凭据").is_err());
        let long = "x".repeat(64 * 1024 + 1);
        assert!(p.stage_init_token(&long).is_err());
        // 被拒绝后不影响随机 token 流
        assert!(p.next().ends_with('='));
    }

    #[test]
    fn token_provider_clear_drops_staged() {
        let p = GatewayTokenProvider::new();
        p.stage_init_token("cred").expect("valid");
        p.clear();
        assert_ne!(p.next(), "cred");
    }
}
