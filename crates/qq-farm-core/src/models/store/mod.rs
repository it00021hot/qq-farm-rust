//! 存储层（1:1 翻译原 `core/src/models/store/`）。
//!
//! - [`accounts`] — 账号列表 CRUD（`data/accounts.json`）
//! - [`account_config`] — 单账号配置（自动化、间隔、黑名单等）
//! - [`global_config`] — 全局配置（UI / 公告 / 管理员密码 / 系统配置）
//! - [`gid_cache`] — 已知好友 GID 文件缓存
//! - [`normalize`] — 配置字段 normalization 纯函数

pub mod account_config;
pub mod accounts;
pub mod gid_cache;
pub mod global_config;
pub mod normalize;
