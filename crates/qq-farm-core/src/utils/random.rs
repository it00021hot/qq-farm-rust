//! 随机 / 异步等待工具。
//!
//! 1:1 翻译原 `core/src/utils/utils.ts`（`sleep` / `randomDelay`）和
//! `core/src/utils/gateway-token.ts`（`createGatewayToken`）。

use std::time::Duration;

use rand::Rng;
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
    let mut rng = rand::thread_rng();
    rng.gen_range(min..=max)
}

/// 范围内随机 u64（含两端）
pub fn random_u64(min: u64, max: u64) -> u64 {
    if max <= min {
        return min;
    }
    let mut rng = rand::thread_rng();
    rng.gen_range(min..=max)
}

// =====================================================================
// Gateway Token（1:1 翻译 `gateway-token.ts`）
// =====================================================================

const TOKEN_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// 创建 gateway token（64~127 字符 + `=` 后缀，字母数字随机）
#[must_use]
pub fn create_gateway_token() -> String {
    let mut rng = rand::thread_rng();
    let length = 64 + rng.gen_range(0..64) as usize;
    let mut token = String::with_capacity(length + 1);
    for _ in 0..length {
        let idx = rng.gen_range(0..TOKEN_ALPHABET.len());
        token.push(TOKEN_ALPHABET[idx] as char);
    }
    token.push('=');
    token
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
}
