//! 路由模块。
//!
//! 1:1 对应原 `controllers/admin/*-routes.ts`。
//!
//! ## 子模块
//!
//! - `farm` — 农场 / 自动化 / 化肥 / 土地 / 种子 / 背包 / 每日礼包 / config（35 路由）

pub mod account;
pub mod activity_center;
pub mod admin;
pub mod auth;
pub mod commerce;
pub mod farm;
pub mod friend;
pub mod placeholder;
pub mod wx_login;

use std::sync::Arc;

use axum::Router;

use crate::context::AdminContext;

/// 构造全部 admin 路由
pub fn build() -> Router<Arc<AdminContext>> {
    Router::new()
        .merge(farm::router())
        .merge(friend::router())
        .merge(account::router())
        .merge(auth::router())
        .merge(admin::router())
        .merge(activity_center::router())
        .merge(commerce::router())
        .merge(wx_login::router())
}
