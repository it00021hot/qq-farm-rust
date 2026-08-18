//! 本机微信 `localhost.weixin.qq.com` HTTPS 代理。
//!
//! 浏览器 / WebView 直连会撞 CORS、Private Network Access 和微信自签证书；
//! 桌面进程连 `127.0.0.1`，SNI/Host 仍用 `localhost.weixin.qq.com`。

use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use reqwest::redirect::Policy;
use serde_json::{json, Value};

use crate::constants::game_ids::{
    LOCAL_WECHAT_AUTHORIZE_PATH, LOCAL_WECHAT_CHECK_PATH, LOCAL_WECHAT_HOST, WX_OAUTH_APP_ID,
    WX_OAUTH_REDIRECT_URI, WX_OAUTH_SCOPE, WX_OAUTH_STATE,
};
use crate::constants::{LOCAL_WECHAT_AUTHORIZE_TIMEOUT_MS, LOCAL_WECHAT_DETECT_TIMEOUT_MS};

use super::wx_auth::WxAuthError;

/// 应用宝 OAuth 参数（与扫码 / YYB `scan.html` 一致）。
#[derive(Debug, Clone)]
pub struct LocalWechatOAuth {
    pub app_id: String,
    pub scope: String,
    pub redirect_uri: String,
    pub state: String,
}

impl LocalWechatOAuth {
    #[must_use]
    pub fn yyb() -> Self {
        Self {
            app_id: WX_OAUTH_APP_ID.to_string(),
            scope: WX_OAUTH_SCOPE.to_string(),
            redirect_uri: WX_OAUTH_REDIRECT_URI.to_string(),
            state: WX_OAUTH_STATE.to_string(),
        }
    }
}

/// 本机微信检测成功后的资料。
#[derive(Debug, Clone)]
pub struct LocalWechatProfile {
    pub port: u16,
    pub authorize_uuid: String,
    pub nickname: String,
    pub headimgurl: String,
}

/// 微信确认弹窗位置（相对屏幕，对齐 YYB `authorizePosition`）。
#[derive(Debug, Clone, Copy)]
pub struct LocalWechatPosition {
    pub x: i32,
    pub y: i32,
}

/// 本机微信 JSON 响应。
#[derive(Debug, Clone)]
pub struct LocalWechatPayload {
    pub errcode: i64,
    pub jsdata: Value,
}

/// `/api/authorize` 成功结果。
#[derive(Debug, Clone)]
pub struct LocalWechatAuthorizeResult {
    pub redirect_url: String,
}

/// 访问本机微信 HTTPS 的传输配置。
#[derive(Debug, Clone)]
pub struct LocalWechatClient {
    scheme: String,
    host: String,
    resolve_loopback: bool,
    accept_invalid_certs: bool,
}

impl Default for LocalWechatClient {
    fn default() -> Self {
        Self::production()
    }
}

impl LocalWechatClient {
    /// 生产：HTTPS + 解析到 127.0.0.1 + 接受微信自签证书。
    #[must_use]
    pub fn production() -> Self {
        Self {
            scheme: "https".into(),
            host: LOCAL_WECHAT_HOST.to_string(),
            resolve_loopback: true,
            accept_invalid_certs: true,
        }
    }

    /// 测试：明文 HTTP 打到给定 host（通常 `127.0.0.1`）。
    #[must_use]
    pub fn loopback_http(host: impl Into<String>) -> Self {
        Self {
            scheme: "http".into(),
            host: host.into(),
            resolve_loopback: false,
            accept_invalid_certs: false,
        }
    }

    fn request_client(&self, port: u16, timeout: Duration) -> Result<reqwest::Client, WxAuthError> {
        let mut builder = reqwest::Client::builder().timeout(timeout).redirect(Policy::none());
        if self.accept_invalid_certs {
            builder = builder.danger_accept_invalid_certs(true);
        }
        if self.resolve_loopback {
            builder = builder.resolve(&self.host, SocketAddr::from((Ipv4Addr::LOCALHOST, port)));
        }
        builder
            .build()
            .map_err(|e| WxAuthError::transient(format!("本机微信客户端初始化失败: {e}")))
    }

    /// POST 本机微信 API，解析可能双重编码的 JSON。
    pub async fn request(
        &self,
        port: u16,
        path: &str,
        body: &Value,
        timeout: Duration,
    ) -> Result<LocalWechatPayload, WxAuthError> {
        let client = self.request_client(port, timeout)?;
        let url = format!("{}://{}:{}{path}", self.scheme, self.host, port);
        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Cache-Control", "no-store")
            .json(body)
            .send()
            .await
            .map_err(|e| WxAuthError::transient(format!("连接本机微信失败（端口 {port}）: {e}")))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| WxAuthError::transient(format!("读取本机微信响应失败: {e}")))?;
        if !status.is_success() {
            return Err(WxAuthError::transient(format!(
                "本机微信 HTTP {}（端口 {port}）",
                status.as_u16()
            )));
        }
        parse_local_wechat_response(&text)
    }

    /// 并行探测端口，命中 `errcode==0 && authorize_uuid`。
    pub async fn detect(
        &self,
        oauth: &LocalWechatOAuth,
        ports: &[u16],
    ) -> Result<LocalWechatProfile, WxAuthError> {
        if ports.is_empty() {
            return Err(WxAuthError::dead("未检测到可用的桌面微信"));
        }
        let timeout = Duration::from_millis(LOCAL_WECHAT_DETECT_TIMEOUT_MS);
        let body = check_login_body(oauth);
        let mut last_err = None;
        let mut futs = Vec::with_capacity(ports.len());
        for port in ports {
            let port = *port;
            let client = self.clone();
            let body = body.clone();
            futs.push(async move {
                match client.request(port, LOCAL_WECHAT_CHECK_PATH, &body, timeout).await {
                    Ok(payload) => Ok((port, payload)),
                    Err(e) => Err(e),
                }
            });
        }
        let results = futures::future::join_all(futs).await;
        for result in results {
            match result {
                Ok((port, payload)) => {
                    if let Some(profile) = profile_from_payload(port, &payload) {
                        return Ok(profile);
                    }
                    last_err = Some(WxAuthError::dead(format!(
                        "本机微信端口 {port} 返回 errcode={}",
                        payload.errcode
                    )));
                }
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| {
            WxAuthError::dead("未检测到可用的桌面微信（请确认 Windows 桌面微信已登录且未锁定）")
        }))
    }

    /// POST `/api/authorize`，成功则返回 `redirect_url`。
    pub async fn authorize(
        &self,
        port: u16,
        oauth: &LocalWechatOAuth,
        authorize_uuid: &str,
        position: LocalWechatPosition,
    ) -> Result<LocalWechatAuthorizeResult, WxAuthError> {
        let uuid = authorize_uuid.trim();
        if uuid.is_empty() {
            return Err(WxAuthError::dead("请先检测本机微信"));
        }
        let timeout = Duration::from_millis(LOCAL_WECHAT_AUTHORIZE_TIMEOUT_MS);
        let body = authorize_body(oauth, uuid, position);
        let payload = self.request(port, LOCAL_WECHAT_AUTHORIZE_PATH, &body, timeout).await?;
        match payload.errcode {
            0 => {}
            10050 => return Err(WxAuthError::dead("已在微信中拒绝授权")),
            10046 => return Err(WxAuthError::dead("授权已超时，请重新检测")),
            10057 => return Err(WxAuthError::dead("当前应用仅支持扫码授权")),
            other => {
                return Err(WxAuthError::dead(format!(
                    "桌面微信未返回有效授权结果（errcode={other}）"
                )));
            }
        }
        let redirect_url = payload
            .jsdata
            .get("redirect_url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| WxAuthError::dead("桌面微信未返回有效授权结果"))?;
        Ok(LocalWechatAuthorizeResult { redirect_url: redirect_url.to_string() })
    }
}

/// 解析本机微信响应（可能是 JSON，或 JSON 字符串再包一层）。
pub fn parse_local_wechat_response(text: &str) -> Result<LocalWechatPayload, WxAuthError> {
    let raw = text.trim();
    if raw.is_empty() {
        return Err(WxAuthError::dead("本机微信返回空响应"));
    }
    let mut value: Value =
        serde_json::from_str(raw).map_err(|_| WxAuthError::dead("本机微信返回了无法解析的响应"))?;
    if let Some(inner) = value.as_str() {
        value = serde_json::from_str(inner)
            .map_err(|_| WxAuthError::dead("本机微信返回了无法解析的响应"))?;
    }
    let obj = value.as_object().ok_or_else(|| WxAuthError::dead("本机微信返回了无法解析的响应"))?;
    let errcode = obj.get("errcode").and_then(Value::as_i64).unwrap_or(0);
    let jsdata = obj.get("jsdata").cloned().unwrap_or(Value::Null);
    Ok(LocalWechatPayload { errcode, jsdata })
}

fn check_login_body(oauth: &LocalWechatOAuth) -> Value {
    json!({
        "apiname": "qrconnectchecklogin",
        "jsdata": {
            "appid": oauth.app_id,
            "scope": oauth.scope,
            "redirect_uri": oauth.redirect_uri,
            "state": oauth.state,
        }
    })
}

fn authorize_body(
    oauth: &LocalWechatOAuth,
    authorize_uuid: &str,
    position: LocalWechatPosition,
) -> Value {
    json!({
        "apiname": "qrconnectfastauthorize",
        "jsdata": {
            "data": json!({ "x": position.x, "y": position.y }).to_string(),
            "appid": oauth.app_id,
            "scope": oauth.scope,
            "redirect_uri": oauth.redirect_uri,
            "state": oauth.state,
            "authorize_uuid": authorize_uuid,
        }
    })
}

fn profile_from_payload(port: u16, payload: &LocalWechatPayload) -> Option<LocalWechatProfile> {
    if payload.errcode != 0 {
        return None;
    }
    let uuid = payload.jsdata.get("authorize_uuid").and_then(Value::as_str)?.trim();
    if uuid.is_empty() {
        return None;
    }
    Some(LocalWechatProfile {
        port,
        authorize_uuid: uuid.to_string(),
        nickname: payload.jsdata.get("nickname").and_then(Value::as_str).unwrap_or("").to_string(),
        headimgurl: payload
            .jsdata
            .get("headimgurl")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn ok_check_json() -> String {
        json!({
            "errcode": 0,
            "jsdata": {
                "authorize_uuid": "uuid-1",
                "nickname": "测试号",
                "headimgurl": "https://img.example/a.png"
            }
        })
        .to_string()
    }

    #[test]
    fn parse_plain_json() {
        let p = parse_local_wechat_response(&ok_check_json()).unwrap();
        assert_eq!(p.errcode, 0);
        assert_eq!(p.jsdata["authorize_uuid"], "uuid-1");
        assert!(profile_from_payload(14013, &p).is_some());
    }

    #[test]
    fn parse_double_encoded_json() {
        let inner = ok_check_json();
        let wrapped = serde_json::to_string(&inner).unwrap();
        let p = parse_local_wechat_response(&wrapped).unwrap();
        assert_eq!(p.errcode, 0);
        assert_eq!(profile_from_payload(1, &p).unwrap().authorize_uuid, "uuid-1");
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(parse_local_wechat_response("not-json").is_err());
        assert!(parse_local_wechat_response("").is_err());
    }

    #[test]
    fn profile_requires_uuid_and_zero_errcode() {
        let p = LocalWechatPayload { errcode: 1, jsdata: json!({"authorize_uuid": "x"}) };
        assert!(profile_from_payload(1, &p).is_none());
        let p = LocalWechatPayload { errcode: 0, jsdata: json!({"authorize_uuid": ""}) };
        assert!(profile_from_payload(1, &p).is_none());
    }

    async fn spawn_json_server(status: u16, body: String) -> u16 {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = vec![0_u8; 2048];
                let _ = stream.read(&mut buf).await;
                let header = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.write_all(body.as_bytes()).await;
                let _ = stream.flush().await;
            }
        });
        port
    }

    #[tokio::test]
    async fn detect_picks_first_success_port() {
        let good = spawn_json_server(200, ok_check_json()).await;
        let client = LocalWechatClient::loopback_http("127.0.0.1");
        let profile = client.detect(&LocalWechatOAuth::yyb(), &[good]).await.unwrap();
        assert_eq!(profile.port, good);
        assert_eq!(profile.authorize_uuid, "uuid-1");
        assert_eq!(profile.nickname, "测试号");
    }

    #[tokio::test]
    async fn detect_skips_dead_port_then_matches() {
        let dead = spawn_json_server(200, json!({"errcode": 1, "jsdata": {}}).to_string()).await;
        let good = spawn_json_server(200, ok_check_json()).await;
        let client = LocalWechatClient::loopback_http("127.0.0.1");
        let profile = client.detect(&LocalWechatOAuth::yyb(), &[dead, good]).await.unwrap();
        assert_eq!(profile.port, good);
    }

    #[tokio::test]
    async fn authorize_returns_redirect_url() {
        let body = json!({
            "errcode": 0,
            "jsdata": { "redirect_url": "https://yybadaccess.3g.qq.com/pc_yyb/pcyyb_oauth?login_type=WX&state=web&code=abc" }
        })
        .to_string();
        let port = spawn_json_server(200, body).await;
        let client = LocalWechatClient::loopback_http("127.0.0.1");
        let result = client
            .authorize(port, &LocalWechatOAuth::yyb(), "uuid-1", LocalWechatPosition { x: 1, y: 2 })
            .await
            .unwrap();
        assert!(result.redirect_url.contains("code=abc"));
    }

    #[tokio::test]
    async fn authorize_maps_reject_errcode() {
        let port =
            spawn_json_server(200, json!({"errcode": 10050, "jsdata": {}}).to_string()).await;
        let client = LocalWechatClient::loopback_http("127.0.0.1");
        let err = client
            .authorize(port, &LocalWechatOAuth::yyb(), "uuid-1", LocalWechatPosition { x: 0, y: 0 })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("拒绝授权"));
    }
}
