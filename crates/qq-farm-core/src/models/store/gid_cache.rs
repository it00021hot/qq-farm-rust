//! 已知好友 GID 缓存（文件持久化）。
//!
//! 1:1 翻译原 `core/src/models/store/gid-cache.ts`（52 行）。
//!
//! - 每个账号单独存一份 `data/known_friend_gids/<accountId>.json`
//! - 当 `accountConfig.knownFriendGids` 为空时，回退到本地缓存

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::paths::get_data_dir;

/// 缓存目录
#[must_use]
pub fn cache_dir() -> PathBuf {
    get_data_dir().join("known_friend_gids")
}

/// 单账号缓存文件
#[must_use]
pub fn cache_file(account_id: &str) -> PathBuf {
    cache_dir().join(format!("{account_id}.json"))
}

/// 读取缓存
pub fn read_cache(account_id: &str) -> Option<Vec<i64>> {
    let path = cache_file(account_id);
    let raw = fs::read_to_string(&path).ok()?;
    let v: Vec<i64> = serde_json::from_str(&raw).ok()?;
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

/// 写入缓存（自动创建目录）
pub fn write_cache(account_id: &str, gids: &[i64]) -> std::io::Result<()> {
    let dir = cache_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let path = cache_file(account_id);
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_string(gids).map_err(std::io::Error::other)?;
    fs::write(&tmp, body)?;
    // 原子 rename
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// 删除某账号缓存
pub fn remove_cache(account_id: &str) -> std::io::Result<()> {
    let path = cache_file(account_id);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// 全部缓存目录是否存在
#[must_use]
pub fn cache_dir_exists() -> bool {
    Path::new(&cache_dir()).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_file_path_ends_with_json() {
        let p = cache_file("test_acc");
        assert!(p.ends_with("known_friend_gids/test_acc.json"));
    }

    #[test]
    fn write_and_read_cache() {
        let aid = "test_acc_cache_42";
        // 清理
        let _ = remove_cache(aid);
        // 写
        write_cache(aid, &[100, 200, 300]).expect("write");
        // 读
        let r = read_cache(aid);
        assert_eq!(r, Some(vec![100, 200, 300]));
        // 删
        remove_cache(aid).expect("remove");
        assert!(read_cache(aid).is_none());
    }

    #[test]
    fn read_cache_empty_returns_none() {
        let aid = "test_empty_acc";
        let _ = remove_cache(aid);
        write_cache(aid, &[]).expect("write empty");
        assert!(read_cache(aid).is_none());
    }
}
