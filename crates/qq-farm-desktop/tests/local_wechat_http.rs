use qq_farm_core::constants::game_ids::{
    DESKTOP_WECHAT_PORTS, LOCAL_WECHAT_AUTHORIZE_PATH, LOCAL_WECHAT_CHECK_PATH, LOCAL_WECHAT_HOST,
};

#[test]
fn http_scope_only_allows_local_wechat_endpoints() {
    let capability: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
    let http = capability["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["identifier"] == "http:default")
        .unwrap();
    let mut actual: Vec<_> = http["allow"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["url"].as_str().unwrap().to_string())
        .collect();
    let mut expected = Vec::new();
    for port in DESKTOP_WECHAT_PORTS {
        for path in [LOCAL_WECHAT_CHECK_PATH, LOCAL_WECHAT_AUTHORIZE_PATH] {
            expected.push(format!("https://{LOCAL_WECHAT_HOST}:{port}{path}"));
        }
    }
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

/// 只检测登录状态，不弹授权窗口、不换票、不保存账号。
/// cargo test -p qq-farm-desktop --test local_wechat_http live_wechat -- --ignored
#[tokio::test]
#[ignore = "需要本机微信已登录且未锁定"]
async fn live_wechat_responds_to_plugin_http_client() {
    use qq_farm_core::services::wx_login::local_wechat::{
        parse_local_wechat_response, LocalWechatOAuth,
    };
    use tauri_plugin_http::reqwest;

    let oauth = LocalWechatOAuth::yyb();
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .unwrap();
    let body = serde_json::json!({
        "apiname": "qrconnectchecklogin",
        "jsdata": {
            "appid": oauth.app_id,
            "scope": oauth.scope,
            "redirect_uri": oauth.redirect_uri,
            "state": oauth.state
        }
    });
    let mut failures = Vec::new();
    for port in DESKTOP_WECHAT_PORTS {
        let response = client
            .post(format!("https://{LOCAL_WECHAT_HOST}:{port}{LOCAL_WECHAT_CHECK_PATH}"))
            .header("Content-Type", "application/json")
            .header("Origin", "https://open.weixin.qq.com")
            .header("Referer", "https://open.weixin.qq.com/")
            .header("User-Agent", "tauri-plugin-http/2.6.0")
            .body(body.to_string())
            .send()
            .await;
        match response {
            Ok(response) => {
                let text = response.error_for_status().unwrap().text().await.unwrap();
                let payload = parse_local_wechat_response(&text).unwrap();
                if payload.errcode == 0
                    && payload.jsdata["authorize_uuid"].as_str().is_some_and(|s| !s.is_empty())
                {
                    return;
                }
                failures.push(format!("端口 {port}: errcode={}", payload.errcode));
            }
            Err(error) => failures.push(format!("端口 {port}: {error}")),
        }
    }
    panic!("未检测到本机微信: {}", failures.join("; "));
}
