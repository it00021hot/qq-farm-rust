//! 可选的 NapCat QQ 扫码。协议照 `qq-farm-bot` `services/qq-login/service.ts`。
//! 默认关闭，登录码和微信扫码不经过这里。

use std::future::Future;
use std::time::Duration;

use serde_json::{json, Value};

use crate::models::store::global_config::{get_qq_login_settings, QqLoginSettings};

pub const QQ_MINIAPP_APP_ID: &str = "1112386029";
pub const REQUEST_TIMEOUT: Duration = Duration::from_millis(120_000);
pub const POLL_GAP: Duration = Duration::from_millis(1_200);

const QRCODE_PATH: &str = "/api/qq/login/qrcode";
const STATUS_PATH: &str = "/api/qq/login/status";
const CODE_PATH: &str = "/api/qq/miniapp/code";
const LOGOUT_PATH: &str = "/api/qq/logout";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QqLoginTask {
    pub task_id: String,
    pub status: String,
    pub qr_image: String,
    pub expires_at: i64,
}

#[derive(Clone, PartialEq, Eq)]
pub struct QqMiniappCode {
    pub code: String,
    pub nickname: String,
    pub app_id: String,
}

impl std::fmt::Debug for QqMiniappCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("QqMiniappCode")
            .field("code", &"<redacted>")
            .field("nickname", &self.nickname)
            .field("app_id", &self.app_id)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QqScanAction {
    Waiting,
    Scanned,
    FetchCode,
    Expired,
    Invalid,
    Continue,
}

#[derive(Debug, PartialEq, Eq)]
pub enum NapCatLoginError {
    Disabled,
    MissingConfig,
    EmptyTask,
    InvalidTask,
    MissingQr,
    MissingCode,
    Timeout,
    Unreachable,
    Remote(String),
}

impl std::fmt::Display for NapCatLoginError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Disabled => "QQ扫码登录未开启",
            Self::MissingConfig => "请先配置 NapCat 接口地址和接口签名",
            Self::EmptyTask => "登录任务 ID 不能为空",
            Self::InvalidTask => "NapCat 返回的登录任务无效",
            Self::MissingQr => "NapCat 未返回登录二维码",
            Self::MissingCode => "NapCat 未返回小程序授权 Code",
            Self::Timeout => "NapCat 接口请求超时，请检查服务状态",
            Self::Unreachable => "无法连接 NapCat 接口，请检查地址和服务状态",
            Self::Remote(message) => message.as_str(),
        };
        formatter.write_str(message)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct NapCatReply {
    pub status: u16,
    pub body: String,
}

impl std::fmt::Debug for NapCatReply {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NapCatReply")
            .field("status", &self.status)
            .field("body_len", &self.body.len())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NapCatTransportError {
    Timeout,
    Unreachable,
}

pub trait NapCatExchange {
    fn post(
        &self,
        url: &str,
        signature: &str,
        body: &str,
    ) -> impl Future<Output = Result<NapCatReply, NapCatTransportError>> + Send;
}

#[must_use]
pub fn action_for_status(status: &str) -> QqScanAction {
    match status {
        "waiting_scan" => QqScanAction::Waiting,
        "scanned" => QqScanAction::Scanned,
        "confirmed" => QqScanAction::FetchCode,
        "cancelled" | "expired" | "failed" => QqScanAction::Expired,
        "" => QqScanAction::Invalid,
        _ => QqScanAction::Continue,
    }
}

#[must_use]
pub fn flow_still_current(started: u64, current: u64, task_id: &str, active_task: &str) -> bool {
    started == current && !task_id.is_empty() && task_id == active_task
}

pub fn require_settings(settings: &QqLoginSettings) -> Result<QqLoginSettings, NapCatLoginError> {
    let settings = settings.clone().normalized();
    if !settings.qq_qr_login {
        return Err(NapCatLoginError::Disabled);
    }
    if settings.nap_cat_endpoint.is_empty() || settings.nap_cat_signature.is_empty() {
        return Err(NapCatLoginError::MissingConfig);
    }
    Ok(settings)
}

pub async fn create_login_task<E: NapCatExchange>(
    settings: &QqLoginSettings,
    exchange: &E,
) -> Result<QqLoginTask, NapCatLoginError> {
    let data = request_nap_cat(settings, exchange, QRCODE_PATH, &json!({}), true).await?;
    normalize_task(&data)
}

pub async fn query_login_status<E: NapCatExchange>(
    settings: &QqLoginSettings,
    exchange: &E,
    task_id: &str,
) -> Result<QqLoginTask, NapCatLoginError> {
    let task_id = required_task_id(task_id)?;
    let data = request_nap_cat(
        settings,
        exchange,
        STATUS_PATH,
        &json!({ "taskId": task_id, "refresh": false }),
        true,
    )
    .await?;
    normalize_task(&data)
}

pub async fn get_miniapp_code<E: NapCatExchange>(
    settings: &QqLoginSettings,
    exchange: &E,
    task_id: &str,
) -> Result<QqMiniappCode, NapCatLoginError> {
    let task_id = required_task_id(task_id)?;
    let data = request_nap_cat(
        settings,
        exchange,
        CODE_PATH,
        &json!({ "taskId": task_id, "appId": QQ_MINIAPP_APP_ID }),
        true,
    )
    .await?;
    let code = data.get("code").and_then(Value::as_str).unwrap_or("").trim().to_owned();
    if code.is_empty() {
        return Err(NapCatLoginError::MissingCode);
    }
    let nickname = data.get("nickname").and_then(Value::as_str).unwrap_or("").trim().to_owned();
    Ok(QqMiniappCode { code, nickname, app_id: QQ_MINIAPP_APP_ID.to_owned() })
}

pub async fn cancel_login_task<E: NapCatExchange>(
    settings: &QqLoginSettings,
    exchange: &E,
    task_id: &str,
) -> Result<(), NapCatLoginError> {
    let task_id = required_task_id(task_id)?;
    request_nap_cat(settings, exchange, LOGOUT_PATH, &json!({ "taskId": task_id }), false).await?;
    Ok(())
}

pub async fn create_login_task_with_saved<E: NapCatExchange>(
    exchange: &E,
) -> Result<QqLoginTask, NapCatLoginError> {
    create_login_task(&get_qq_login_settings(), exchange).await
}

struct ReqwestExchange;

impl NapCatExchange for ReqwestExchange {
    async fn post(
        &self,
        url: &str,
        signature: &str,
        body: &str,
    ) -> Result<NapCatReply, NapCatTransportError> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| NapCatTransportError::Unreachable)?;
        let response = client
            .post(url)
            .header("Content-Type", "application/json")
            .header("X-API-Signature", signature)
            .body(body.to_owned())
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    NapCatTransportError::Timeout
                } else {
                    NapCatTransportError::Unreachable
                }
            })?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(|_| NapCatTransportError::Unreachable)?;
        Ok(NapCatReply { status, body })
    }
}

pub async fn create_saved_login_task() -> Result<QqLoginTask, NapCatLoginError> {
    create_login_task_with_saved(&ReqwestExchange).await
}

pub async fn query_saved_login_status(task_id: &str) -> Result<QqLoginTask, NapCatLoginError> {
    query_login_status(&get_qq_login_settings(), &ReqwestExchange, task_id).await
}

pub async fn get_saved_miniapp_code(task_id: &str) -> Result<QqMiniappCode, NapCatLoginError> {
    get_miniapp_code(&get_qq_login_settings(), &ReqwestExchange, task_id).await
}

pub async fn cancel_saved_login_task(task_id: &str) -> Result<(), NapCatLoginError> {
    cancel_login_task(&get_qq_login_settings(), &ReqwestExchange, task_id).await
}

async fn request_nap_cat<E: NapCatExchange>(
    settings: &QqLoginSettings,
    exchange: &E,
    path: &str,
    body: &Value,
    require_ok: bool,
) -> Result<Value, NapCatLoginError> {
    let settings = require_settings(settings)?;
    let url = api_url(&settings.nap_cat_endpoint, path);
    let payload = body.to_string();
    let reply =
        exchange.post(&url, &settings.nap_cat_signature, &payload).await.map_err(|error| {
            match error {
                NapCatTransportError::Timeout => NapCatLoginError::Timeout,
                NapCatTransportError::Unreachable => NapCatLoginError::Unreachable,
            }
        })?;
    if reply.status != 200 {
        return Err(NapCatLoginError::Unreachable);
    }
    let data = serde_json::from_str::<Value>(&reply.body).unwrap_or(Value::Null);
    if require_ok && data.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(NapCatLoginError::Remote(nap_cat_error_message(&data)));
    }
    Ok(data)
}

fn api_url(endpoint: &str, path: &str) -> String {
    format!("{}{path}", endpoint.trim_end_matches('/'))
}

fn required_task_id(task_id: &str) -> Result<String, NapCatLoginError> {
    let task_id = task_id.trim().to_owned();
    if task_id.is_empty() {
        Err(NapCatLoginError::EmptyTask)
    } else {
        Ok(task_id)
    }
}

fn normalize_task(raw: &Value) -> Result<QqLoginTask, NapCatLoginError> {
    let task = raw.get("task").filter(|value| value.is_object()).unwrap_or(&Value::Null);
    let task_id = task.get("id").and_then(Value::as_str).unwrap_or("").trim().to_owned();
    let status = task.get("status").and_then(Value::as_str).unwrap_or("").trim().to_owned();
    let qr_image = task.get("qrImage").and_then(Value::as_str).unwrap_or("").trim().to_owned();
    if task_id.is_empty() {
        return Err(NapCatLoginError::InvalidTask);
    }
    if qr_image.is_empty() {
        return Err(NapCatLoginError::MissingQr);
    }
    let expires_at = task.get("expiresAt").and_then(Value::as_i64).unwrap_or(0);
    Ok(QqLoginTask { task_id, status, qr_image, expires_at })
}

fn nap_cat_error_message(data: &Value) -> String {
    let error_code = data.get("code").and_then(Value::as_str).unwrap_or("").trim().to_uppercase();
    match error_code.as_str() {
        "SIGNATURE_REQUIRED" => "NapCat 接口签名缺失，请检查配置".to_owned(),
        "INVALID_SIGNATURE" => "NapCat 接口签名无效，请检查配置".to_owned(),
        "WORKFLOW_BUSY" => "NapCat 登录工作流繁忙，请稍后重试".to_owned(),
        "LOGIN_REQUIRED" => "QQ 登录尚未确认，请先完成扫码确认".to_owned(),
        "LOGOUT_REQUIRED" => "上一位 QQ 登录尚未注销，请稍后重试".to_owned(),
        "TASK_EXPIRED" => "QQ 登录任务已过期，请重新获取二维码".to_owned(),
        _ => format!(
            "NapCat 接口返回失败{}",
            if error_code.is_empty() { String::new() } else { format!("（{error_code}）") }
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct Script {
        replies: Mutex<Vec<Result<NapCatReply, NapCatTransportError>>>,
        seen: Mutex<Vec<(String, String, String)>>,
    }

    impl NapCatExchange for Script {
        async fn post(
            &self,
            url: &str,
            signature: &str,
            body: &str,
        ) -> Result<NapCatReply, NapCatTransportError> {
            self.seen.lock().expect("seen").push((
                url.to_owned(),
                signature.to_owned(),
                body.to_owned(),
            ));
            self.replies.lock().expect("replies").remove(0)
        }
    }

    fn enabled() -> QqLoginSettings {
        QqLoginSettings {
            qq_qr_login: true,
            nap_cat_endpoint: "https://napcat.example/root/".to_owned(),
            nap_cat_signature: "test-signature".to_owned(),
        }
    }

    fn task_body(status: &str) -> String {
        json!({
            "ok": true,
            "task": { "id": "task-1", "status": status, "qrImage": "data:image/png;base64,qq", "expiresAt": 1700000000000_i64 }
        })
        .to_string()
    }

    #[test]
    fn status_actions_match_the_bot_panel() {
        assert_eq!(action_for_status("waiting_scan"), QqScanAction::Waiting);
        assert_eq!(action_for_status("scanned"), QqScanAction::Scanned);
        assert_eq!(action_for_status("confirmed"), QqScanAction::FetchCode);
        assert_eq!(action_for_status("expired"), QqScanAction::Expired);
        assert_eq!(action_for_status("cancelled"), QqScanAction::Expired);
        assert_eq!(action_for_status("failed"), QqScanAction::Expired);
        assert_eq!(action_for_status(""), QqScanAction::Invalid);
        assert_eq!(action_for_status("other"), QqScanAction::Continue);
        assert!(flow_still_current(2, 2, "task-1", "task-1"));
        assert!(!flow_still_current(2, 3, "task-1", "task-1"));
        assert!(!flow_still_current(2, 2, "task-1", "task-2"));
    }

    #[tokio::test]
    async fn disabled_or_incomplete_settings_never_call_the_service() {
        let script = Script { replies: Mutex::new(vec![]), seen: Mutex::new(vec![]) };
        let disabled = QqLoginSettings::default();
        assert_eq!(
            create_login_task(&disabled, &script).await.unwrap_err(),
            NapCatLoginError::Disabled
        );
        let missing = QqLoginSettings { qq_qr_login: true, ..QqLoginSettings::default() };
        assert_eq!(
            create_login_task(&missing, &script).await.unwrap_err(),
            NapCatLoginError::MissingConfig
        );
        assert!(script.seen.lock().expect("seen").is_empty());
    }

    #[tokio::test]
    async fn qr_status_code_and_cancel_follow_the_bot_contract() {
        let script = Script {
            replies: Mutex::new(vec![
                Ok(NapCatReply { status: 200, body: task_body("waiting_scan") }),
                Ok(NapCatReply { status: 200, body: task_body("confirmed") }),
                Ok(NapCatReply {
                    status: 200,
                    body: json!({ "ok": true, "code": "mini-code", "nickname": "果农" })
                        .to_string(),
                }),
                Ok(NapCatReply { status: 200, body: json!({ "ok": false }).to_string() }),
            ]),
            seen: Mutex::new(vec![]),
        };
        let settings = enabled();
        let created = create_login_task(&settings, &script).await.expect("task");
        assert_eq!(created.task_id, "task-1");
        assert_eq!(created.qr_image, "data:image/png;base64,qq");
        let status = query_login_status(&settings, &script, " task-1 ").await.expect("status");
        assert_eq!(status.status, "confirmed");
        let code = get_miniapp_code(&settings, &script, "task-1").await.expect("code");
        assert_eq!(code.code, "mini-code");
        assert_eq!(code.nickname, "果农");
        assert_eq!(code.app_id, QQ_MINIAPP_APP_ID);
        cancel_login_task(&settings, &script, "task-1").await.expect("cancel");
        let seen = script.seen.lock().expect("seen");
        assert_eq!(seen[0].0, "https://napcat.example/root/api/qq/login/qrcode");
        assert_eq!(seen[0].1, "test-signature");
        assert_eq!(seen[1].0, "https://napcat.example/root/api/qq/login/status");
        assert!(seen[1].2.contains("\"refresh\":false"));
        assert!(seen[2].2.contains("1112386029"));
        assert_eq!(seen[3].0, "https://napcat.example/root/api/qq/logout");
        let rendered =
            format!("{}", NapCatLoginError::Remote("NapCat 接口签名无效，请检查配置".to_owned()));
        assert!(!rendered.contains("test-signature"));
        assert!(!format!("{settings:?}").contains("test-signature"));
        let issued = QqMiniappCode {
            code: "mini-code".to_owned(),
            nickname: "果农".to_owned(),
            app_id: QQ_MINIAPP_APP_ID.to_owned(),
        };
        let reply = NapCatReply { status: 200, body: json!({ "code": "mini-code" }).to_string() };
        assert!(!format!("{issued:?}").contains("mini-code"));
        assert!(!format!("{reply:?}").contains("mini-code"));
    }

    #[tokio::test]
    async fn transport_and_remote_failures_use_the_bot_messages() {
        let settings = enabled();
        let timeout = Script {
            replies: Mutex::new(vec![Err(NapCatTransportError::Timeout)]),
            seen: Mutex::new(vec![]),
        };
        assert_eq!(
            create_login_task(&settings, &timeout).await.unwrap_err(),
            NapCatLoginError::Timeout
        );
        let down = Script {
            replies: Mutex::new(vec![Ok(NapCatReply { status: 503, body: String::new() })]),
            seen: Mutex::new(vec![]),
        };
        assert_eq!(
            create_login_task(&settings, &down).await.unwrap_err(),
            NapCatLoginError::Unreachable
        );
        let invalid = Script {
            replies: Mutex::new(vec![Ok(NapCatReply {
                status: 200,
                body: json!({ "ok": false, "code": "INVALID_SIGNATURE" }).to_string(),
            })]),
            seen: Mutex::new(vec![]),
        };
        assert_eq!(
            create_login_task(&settings, &invalid).await.unwrap_err().to_string(),
            "NapCat 接口签名无效，请检查配置"
        );
        let empty = Script { replies: Mutex::new(vec![]), seen: Mutex::new(vec![]) };
        assert_eq!(
            query_login_status(&settings, &empty, " ").await.unwrap_err(),
            NapCatLoginError::EmptyTask
        );
        let no_qr = Script {
            replies: Mutex::new(vec![Ok(NapCatReply {
                status: 200,
                body: json!({ "ok": true, "task": { "id": "task-1", "status": "waiting_scan" } })
                    .to_string(),
            })]),
            seen: Mutex::new(vec![]),
        };
        assert_eq!(
            create_login_task(&settings, &no_qr).await.unwrap_err(),
            NapCatLoginError::MissingQr
        );
    }
}
