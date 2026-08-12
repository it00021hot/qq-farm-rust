//! 路由模块。
//!
//! 1:1 对应原 `controllers/admin/*-routes.ts`。

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

use axum::{extract::Request, middleware, Router};

use crate::context::AdminContext;

/// 构造全部 admin 路由
pub fn build(ctx: Arc<AdminContext>) -> Router<Arc<AdminContext>> {
    let ctx_for_inject = ctx.clone();
    let ctx_for_admin = ctx.clone();
    let ctx_for_auth = ctx.clone();

    // 顶层注入 ctx 到 request extensions
    let inject_layer = middleware::from_fn(move |mut req: Request, next: axum::middleware::Next| {
        let ctx = ctx_for_inject.clone();
        async move {
            req.extensions_mut().insert(ctx);
            next.run(req).await
        }
    });

    // admin 鉴权
    let admin_check = middleware::from_fn(move |req: Request, next: axum::middleware::Next| {
        let ctx = ctx_for_admin.clone();
        async move {
            crate::middleware::admin_required_strict_ext(ctx, req, next).await
        }
    });

    // 普通用户鉴权
    let auth_check = middleware::from_fn(move |req: Request, next: axum::middleware::Next| {
        let ctx = ctx_for_auth.clone();
        async move {
            crate::middleware::auth_required_strict_ext(ctx, req, next).await
        }
    });

    // auth 路由不挂 auth_check（login/register 公开）
    // admin 挂 admin_check
    // account / friend 挂 auth_check
    // 其余挂 inject（不需要登录）
    let authed = Router::new()
        .merge(account::router())
        .merge(friend::router())
        .route_layer(auth_check);

    Router::new()
        .merge(farm::router())
        .merge(auth::router())
        .merge(authed)
        .merge(admin::router().route_layer(admin_check))
        .merge(activity_center::router())
        .merge(commerce::router())
        .merge(wx_login::router())
        .route_layer(inject_layer)
}
