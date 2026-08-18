//! 简易 JSON 文件持久化（1:1 翻译原 `core/src/services/json-db.ts`）。
//!
//! - 读：文件不存在 / 解析失败 → 返回 fallback
//! - 写：原子写（先写 tmp 文件，再 rename）
//!
//! 用于 `data/` 下各种 JSON 配置文件（store / users / cards / accounts 等）的
//! 持久化保存。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

/// 确保父目录存在
pub fn ensure_parent_dir(file_path: impl AsRef<Path>) -> io::Result<()> {
    let dir = file_path.as_ref().parent();
    if let Some(d) = dir {
        if !d.as_os_str().is_empty() && !d.exists() {
            fs::create_dir_all(d)?;
        }
    }
    Ok(())
}

/// 读取文本（不存在/失败返回 fallback）
#[must_use]
pub fn read_text_file(file_path: impl AsRef<Path>, fallback: &str) -> String {
    let path = file_path.as_ref();
    if !path.exists() {
        return fallback.to_string();
    }
    fs::read_to_string(path).unwrap_or_else(|_| fallback.to_string())
}

/// 读取 JSON（不存在/解析失败/空内容 → 返回 fallback）
///
/// `fallback` 可以是默认值或闭包返回值。
pub fn read_json_or<T: DeserializeOwned + Default>(file_path: impl AsRef<Path>) -> T {
    let path = file_path.as_ref();
    if !path.exists() {
        return T::default();
    }
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return T::default(),
    };
    if raw.trim().is_empty() {
        return T::default();
    }
    serde_json::from_str(&raw).unwrap_or_default()
}

/// 读取 JSON（带自定义 fallback 工厂）
pub fn read_json_with_default<T, F>(file_path: impl AsRef<Path>, fallback: F) -> T
where
    T: DeserializeOwned,
    F: FnOnce() -> T,
{
    let path = file_path.as_ref();
    if !path.exists() {
        return fallback();
    }
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return fallback(),
    };
    if raw.trim().is_empty() {
        return fallback();
    }
    serde_json::from_str(&raw).unwrap_or_else(|_| fallback())
}

/// 原子写文本（先写 tmp，再 rename）
pub fn write_text_file_atomic(file_path: impl AsRef<Path>, text: &str) -> io::Result<()> {
    let path = file_path.as_ref();
    ensure_parent_dir(path)?;
    let pid = std::process::id();
    let ts = crate::utils::time::now_ms();
    let tmp_path = {
        let mut p = path.to_path_buf();
        let file_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
        p.set_file_name(format!("{file_name}.{pid}.{ts}.tmp"));
        p
    };

    let result = (|| -> io::Result<()> {
        fs::write(&tmp_path, text.as_bytes())?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    })();

    if tmp_path.exists() {
        let _ = fs::remove_file(&tmp_path);
    }
    result
}

/// 原子写 JSON
pub fn write_json_file_atomic<T: Serialize>(
    file_path: impl AsRef<Path>,
    data: &T,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(data).map_err(io::Error::other)?;
    write_text_file_atomic(file_path, &json)
}

/// 检查文件存在
#[must_use]
pub fn file_exists(path: impl AsRef<Path>) -> bool {
    path.as_ref().exists()
}

/// 文件大小
#[must_use]
pub fn file_size(path: impl AsRef<Path>) -> Option<u64> {
    fs::metadata(path.as_ref()).ok().map(|m| m.len())
}

/// 列出目录下所有匹配扩展名的文件
#[must_use]
pub fn list_files_with_ext(dir: impl AsRef<Path>, ext: &str) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else {
        return vec![];
    };
    rd.filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(ext))
        .collect()
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn temp_file() -> PathBuf {
        let dir = PathBuf::from("/tmp/qq-farm-test");
        let _ = fs::create_dir_all(&dir);
        let pid = std::process::id();
        let ts = crate::utils::time::now_ms();
        dir.join(format!("json-db-{pid}-{ts}.json"))
    }

    #[test]
    fn read_text_missing_returns_fallback() {
        let p = temp_file();
        let s = read_text_file(&p, "default");
        assert_eq!(s, "default");
    }

    #[test]
    #[serial(json_db)]
    fn read_json_missing_returns_default() {
        let p = temp_file();
        let v: serde_json::Value = read_json_or(&p);
        assert_eq!(v, serde_json::Value::Null);
    }

    #[test]
    #[serial(json_db)]
    fn write_and_read_json_roundtrip() {
        let p = temp_file();
        let data = serde_json::json!({"a": 1, "b": [2, 3]});
        write_json_file_atomic(&p, &data).expect("write");
        let back: serde_json::Value = read_json_or(&p);
        assert_eq!(back, data);
        let _ = fs::remove_file(&p);
    }

    #[test]
    #[serial(json_db)]
    fn write_atomic_creates_parent_dir() {
        let p = PathBuf::from("/tmp/qq-farm-test/sub/nested/test.json");
        let data = serde_json::json!({"x": 1});
        write_json_file_atomic(&p, &data).expect("write");
        assert!(p.exists());
        let _ = fs::remove_file(&p);
        let _ = fs::remove_dir("/tmp/qq-farm-test/sub/nested");
        let _ = fs::remove_dir("/tmp/qq-farm-test/sub");
    }

    #[test]
    #[serial(json_db)]
    fn read_json_invalid_returns_default() {
        let p = temp_file();
        fs::write(&p, "not valid json {").unwrap();
        let v: serde_json::Value = read_json_or(&p);
        assert_eq!(v, serde_json::Value::Null);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn read_json_with_default_uses_factory() {
        let p = temp_file();
        let v: serde_json::Value =
            read_json_with_default(&p, || serde_json::json!({"fallback": true}));
        assert_eq!(v, serde_json::json!({"fallback": true}));
    }

    #[test]
    #[serial(json_db)]
    fn file_exists_and_size() {
        let p = temp_file();
        assert!(!file_exists(&p));
        fs::write(&p, "hello").unwrap();
        assert!(file_exists(&p));
        assert_eq!(file_size(&p), Some(5));
        let _ = fs::remove_file(&p);
    }
}
