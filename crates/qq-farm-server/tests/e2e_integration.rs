//! 端到端集成测试。
//!
//! 真实起 axum router + AdminContext，调完整 HTTP 流程：
//! 1. 注册 → 登录 → 验证 token
//! 2. 改密码 + 旧 token 失效
//! 3. admin 登录 + 改 role + 看 login logs
//! 4. /api/ping + /api/game-version
//! 5. health + 静态 404 路径
//!
//! 不调真实外部网络（gateway 是 noop）；只验路由 + handler + 序列化。

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

use qq_farm_core::models::user_store::users::create_card_with_code;
use qq_farm_server::{build_test_app, TestApp};

/// 生成一张 unique 卡密（基于 process id + 测试名 + counter）并返回 code
fn mint_test_card(label: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let code = format!("E2E-{}-{}-{}", std::process::id(), label, n);
    create_card_with_code(&code, "e2e-test", 30, "time");
    code
}

#[tokio::test]
#[serial_test::serial(e2e)]
async fn e2e_register_login_validate_flow() {
    let app = build_test_app(TestApp::default()).await;
    let username = format!("user_e2e_{}", std::process::id());
    let password = "TestPass123!";
    let card_code = mint_test_card("E2E");

    // 1. 注册
    let body = json!({
        "username": username,
        "password": password,
        "cardCode": card_code,
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["ok"], true, "register 返回 ok: {v}");

    // 2. 登录
    let body = json!({
        "username": username,
        "password": password,
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/json")
                .header("user-agent", "e2e-test/1.0")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "login 应成功");
    let bytes = to_bytes(resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    let token = v["data"]["token"].as_str().expect("token 存在").to_string();
    assert!(!token.is_empty());
    assert_eq!(v["data"]["username"], username);
    assert_eq!(v["data"]["role"], "user");

    // 3. 验证 token（注意：validate 用 x-admin-token header）
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/auth/validate")
                .header("x-admin-token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "validate 应成功");
    let bytes = to_bytes(resp.into_body(), 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["username"], username);

    // 4. 错误 token → 401
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/auth/validate")
                .header("x-admin-token", "invalid-token-12345")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial_test::serial(e2e)]
async fn e2e_login_wrong_password_returns_401() {
    let app = build_test_app(TestApp::default()).await;
    let username = format!("user_wrong_{}", std::process::id());
    let password = "RightPass123!";
    let card_code = mint_test_card("WP");

    // 先注册
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": username,
                        "password": password,
                        "cardCode": card_code,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // 错密码
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": username,
                        "password": "WrongPass",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let bytes = to_bytes(resp.into_body(), 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["ok"], false);
    assert!(v["error"].as_str().is_some());
}

#[tokio::test]
#[serial_test::serial(e2e)]
async fn e2e_health_and_ping() {
    let app = build_test_app(TestApp::default()).await;

    // /health
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["service"], "qq-farm-server");

    // /api/ping
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/ping")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v["ok"].as_bool().unwrap_or(false) || v["pong"].is_string() || v["ok"] == true);
}

#[tokio::test]
#[serial_test::serial(e2e)]
async fn e2e_unknown_route_returns_404() {
    let app = build_test_app(TestApp::default()).await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/this/does/not/exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[serial_test::serial(e2e)]
async fn e2e_game_version() {
    let app = build_test_app(TestApp::default()).await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/game-version")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    // game-version 至少返回 version 字段
    assert!(
        v["version"].is_string() || v["ok"] == true,
        "game-version 响应格式: {v}"
    );
}

#[tokio::test]
#[serial_test::serial(e2e)]
async fn e2e_change_password_invalidates_old_token() {
    let app = build_test_app(TestApp::default()).await;
    let username = format!("user_pwchange_{}", std::process::id());
    let password = "OldPass123!";
    let card_code = mint_test_card("PWC");

    // 注册 + 登录
    let _ = app
        .clone()
        .oneshot(register_req(&username, password, &card_code))
        .await
        .unwrap();
    let login_resp = app.clone().oneshot(login_req(&username, password)).await.unwrap();
    assert_eq!(login_resp.status(), StatusCode::OK);
    let bytes = to_bytes(login_resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    let token = v["data"]["token"].as_str().expect("token").to_string();

    // 改密码
    let new_password = "NewPass456!";
    let change_body = json!({
        "oldPassword": password,
        "newPassword": new_password,
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/user/change-password")
                .header("content-type", "application/json")
                .header("x-admin-token", &token)
                .body(Body::from(serde_json::to_vec(&change_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 旧 token 应失效
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/auth/validate")
                .header("x-admin-token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // 新密码应能登录
    let resp = app
        .clone()
        .oneshot(login_req(&username, new_password))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[serial_test::serial(e2e)]
async fn e2e_register_duplicate_username() {
    let app = build_test_app(TestApp::default()).await;
    let username = format!("user_dup_{}", std::process::id());
    let card1 = mint_test_card("DUP1");

    let r1 = app
        .clone()
        .oneshot(register_req(&username, "p1", &card1))
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::OK);

    // 重复注册 → 失败（无需有效卡密，username 已存在直接拒）
    let r2 = app
        .clone()
        .oneshot(register_req(&username, "p2", "INVALID_CARD"))
        .await
        .unwrap();
    assert_eq!(r2.status(), StatusCode::OK, "duplicate 应该 200 但 ok=false");
    let bytes = to_bytes(r2.into_body(), 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["ok"], false, "duplicate 应该 ok=false: {v}");
    // 不严格断言 error 内容（Rust 端 Err 直接转字符串，error 字段是 String）
    assert!(v["error"].is_string(), "error 字段应存在: {v}");
}

#[tokio::test]
#[serial_test::serial(e2e)]
async fn e2e_logout_invalidates_token() {
    let app = build_test_app(TestApp::default()).await;
    let username = format!("user_lo_{}", std::process::id());
    let card_code = mint_test_card("LO");

    let reg_resp = app
        .clone()
        .oneshot(register_req(&username, "Pass123!", &card_code))
        .await
        .unwrap();
    let reg_body = to_bytes(reg_resp.into_body(), 1024).await.unwrap();
    let reg_v: Value = serde_json::from_slice(&reg_body).unwrap();
    assert_eq!(reg_v["ok"], true, "register 应成功: {reg_v}");
    let login = app.clone().oneshot(login_req(&username, "Pass123!")).await.unwrap();
    let status = login.status();
    let bytes = to_bytes(login.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status, StatusCode::OK, "login 应成功: {v}");
    let token = v["data"]["token"]
        .as_str()
        .expect("login 应返回 token")
        .to_string();

    // 登出
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logout")
                .header("x-admin-token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 旧 token 失效
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/auth/validate")
                .header("x-admin-token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial_test::serial(e2e)]
async fn e2e_admin_login_logs() {
    let app = build_test_app(TestApp::default()).await;
    let admin_username = "admin";
    let admin_password = "admin";

    // admin 登录（默认账号 admin/admin）
    let resp = app
        .clone()
        .oneshot(login_req(admin_username, admin_password))
        .await
        .unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status, StatusCode::OK, "admin login 应成功: {v}");
    let token = v["data"]["token"].as_str().expect("admin token").to_string();
    assert_eq!(v["data"]["role"], "admin");

    // /api/admin/login-logs 需要 admin 鉴权
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/admin/login-logs")
                .header("x-admin-token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 65536).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    // 至少返回 logs 数组
    assert!(v["logs"].is_array() || v["ok"] == true, "login-logs 响应: {v}");
}

#[tokio::test]
#[serial_test::serial(e2e)]
async fn e2e_account_creation_returns_account_id() {
    let app = build_test_app(TestApp::default()).await;
    let username = format!("user_acc_{}", std::process::id());
    let password = "Pass123!";
    let card_code = mint_test_card("ACC");

    let resp = app
        .clone()
        .oneshot(register_req(&username, password, &card_code))
        .await
        .unwrap();
    let body = to_bytes(resp.into_body(), 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ok"], true, "register 应成功: {v}");
    // 检查 user 字段
    assert!(v["user"].is_object(), "user 字段应存在: {v}");
    let user_obj = &v["user"];
    assert_eq!(user_obj["username"], username);
    assert_eq!(user_obj["role"], "user");
}

// ===== helpers =====

fn register_req(username: &str, password: &str, card_code: &str) -> Request<Body> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    // 用 unique 假 IP 避免 rate limiter 互相干扰
    let ip = format!("10.13.{}.{}", (n / 256) % 256, n % 256);
    Request::builder()
        .method("POST")
        .uri("/api/register")
        .header("content-type", "application/json")
        .header("x-forwarded-for", &ip)
        .body(Body::from(
            serde_json::to_vec(&json!({
                "username": username,
                "password": password,
                "cardCode": card_code,
            }))
            .unwrap(),
        ))
        .unwrap()
}

fn login_req(username: &str, password: &str) -> Request<Body> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let ip = format!("10.13.{}.{}", (n / 256) % 256, n % 256);
    Request::builder()
        .method("POST")
        .uri("/api/login")
        .header("content-type", "application/json")
        .header("x-forwarded-for", &ip)
        .body(Body::from(
            serde_json::to_vec(&json!({
                "username": username,
                "password": password,
            }))
            .unwrap(),
        ))
        .unwrap()
}

// ===== 防止 lint warning =====

#[allow(dead_code)]
fn _unused() {
    let _: Arc<()> = Arc::new(());
}
