//! 商业域 — 商城、神秘商店、支付与编排服务。
//!
//! 底层模块文件仍位于 `services/` 根目录；本模块提供域入口与编排 [`service`]。

pub mod service;

pub use service::*;

pub use super::mall;
pub use super::mystery_shop;
pub use super::pay;
