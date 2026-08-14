//! 帧加密/解密器。
//!
//! 对外暴露 `Encryptor` trait，业务层用 trait 注入，底层由 [`TsdkEncryptor`]
//! 提供（包装阶段 0 已实现的 `TsdkRuntime`）。
//!
//! ```ignore
//! use qq_farm_core::network::encryptor::{Encryptor, TsdkEncryptor};
//!
//! let enc = TsdkEncryptor::new(tsdk_runtime);
//! let encrypted = enc.encrypt(b"plain")?;
//! let decrypted = enc.decrypt(&encrypted)?;
//! assert_eq!(decrypted, b"plain");
//! ```

use std::sync::Arc;


use crate::crypto::tsdk::TsdkRuntime;
use crate::error::Result;

/// 加密器抽象
pub trait Encryptor: Send + Sync {
    /// 加密一段明文
    ///
    /// # Errors
    /// - 底层加密失败时返回错误
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>;

    /// 解密一段密文
    ///
    /// # Errors
    /// - 底层解密失败时返回错误
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>>;

    /// 便捷：encrypt + 失败时返回空 Vec
    fn encrypt_or_empty(&self, plaintext: &[u8]) -> Vec<u8> {
        self.encrypt(plaintext).unwrap_or_default()
    }
}

/// 基于 [`TsdkRuntime`] 的加密器实现
///
/// `TsdkRuntime` 内部用 `parking_lot::Mutex` 保护，可跨线程共享。
pub struct TsdkEncryptor {
    runtime: Arc<TsdkRuntime>,
}

impl TsdkEncryptor {
    /// 包装一个已初始化的 [`TsdkRuntime`]
    #[must_use]
    pub fn new(runtime: Arc<TsdkRuntime>) -> Self {
        Self { runtime }
    }
}

impl Encryptor for TsdkEncryptor {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        self.runtime.encrypt(plaintext)
    }
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        self.runtime.decrypt(ciphertext)
    }
}

/// 不加密的明文 Encryptor（用于测试和本地开发）
pub struct NoopEncryptor;

impl Encryptor for NoopEncryptor {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        Ok(plaintext.to_vec())
    }
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        Ok(ciphertext.to_vec())
    }
}

// 旧 impl 已被上面的 impl 替代
#[cfg(test)]
mod noop_tests {
    use super::*;

    #[test]
    fn noop_roundtrip() {
        let enc = NoopEncryptor;
        let plain = b"hello noop";
        let ct = enc.encrypt(plain).expect("encrypt");
        assert_eq!(ct, plain);
        let pt = enc.decrypt(&ct).expect("decrypt");
        assert_eq!(pt, plain);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// 真实 wasm 加载比较慢，单测里用一次性 fixture
    fn shared_runtime() -> Arc<TsdkEncryptor> {
        use std::sync::OnceLock;
        static RUNTIME: OnceLock<Arc<TsdkEncryptor>> = OnceLock::new();
        RUNTIME
            .get_or_init(|| {
                // 默认：仓库根 `assets/tsdk.wasm`（用 CARGO_MANIFEST_DIR 拼绝对路径，跨 CWD 都能跑）
                let manifest_dir = env!("CARGO_MANIFEST_DIR");
                let default_path =
                    Path::new(manifest_dir).join("..").join("..").join("assets").join("tsdk.wasm");
                let wasm_path = std::env::var("TSDK_WASM_PATH")
                    .map(std::path::PathBuf::from)
                    .unwrap_or(default_path);
                let rt = TsdkRuntime::load(&wasm_path, "./data/tsdk-encryptor-test")
                    .expect("load tsdk.wasm");
                Arc::new(TsdkEncryptor::new(Arc::new(rt)))
            })
            .clone()
    }

    #[test]
    fn roundtrip() {
        let enc = shared_runtime();
        let plain = b"hello network layer";
        let ct = enc.encrypt(plain).expect("encrypt");
        let pt = enc.decrypt(&ct).expect("decrypt");
        assert_eq!(pt, plain);
    }

    #[test]
    fn empty_buffer() {
        let enc = shared_runtime();
        let ct = enc.encrypt(b"").expect("encrypt empty");
        let pt = enc.decrypt(&ct).expect("decrypt empty");
        assert_eq!(pt, b"");
    }
}
