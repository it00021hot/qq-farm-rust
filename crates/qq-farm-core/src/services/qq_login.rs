//! QQ 扫码登录 — NapCat 接口代理（1:1 翻译 bot `services/qq-login/service.ts`）。
//!
//! 对接外部 NapCat 服务完成 QQ 扫码登录：
//!
//! 1. `POST /api/qq/login/qrcode` 创建登录任务（拿二维码）
//! 2. `POST /api/qq/login/status` 轮询扫码/确认状态
//! 3. `POST /api/qq/miniapp/code` 用 taskId 换 QQ 小程序授权 code（用于登录游戏）
//! 4. `POST /api/qq/logout` 取消登录任务
//!
//! NapCat 错误码统一翻译成中文提示（对齐 bot `napCatErrorMessage`）。

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::models::store::global_config::{get_login_settings, LoginSettings};

/// 读取登录设置并校验 NapCat 配置（对齐 bot `loginSettings()`）；
/// 未开启或配置缺失时返回中文错误。
pub fn client_from_settings() -> Result<NapCatClient> {
    NapCatClient::from_settings(&get_login_settings())
}

/// QQ 小程序 appId（对齐 bot `QQ_MINIAPP_APP_ID`）
pub const QQ_MINIAPP_APP_ID: &str = "1112386029";

/// NapCat 请求超时（对齐 bot `REQUEST_TIMEOUT_MS`）
const REQUEST_TIMEOUT_MS: u64 = 120_000;

/// 登录任务状态：waiting_scan / scanned / confirmed / cancelled / expired / failed
pub type QqLoginTaskStatus = String;

/// NapCat 登录任务
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct QqLoginTask {
    pub task_id: String,
    pub status: QqLoginTaskStatus,
    pub qr_image: String,
    pub expires_at: i64,
}

impl Default for QqLoginTask {
    fn default() -> Self {
        Self {
            task_id: String::new(),
            status: String::new(),
            qr_image: String::new(),
            expires_at: 0,
        }
    }
}

/// NapCat 接口客户端（endpoint + 签名在创建时校验，对齐 bot `loginSettings()`）。
pub struct NapCatClient {
    endpoint: String,
    signature: String,
    http: reqwest::Client,
}

impl NapCatClient {
    /// 从登录设置构造；未开启或配置缺失时报错（文案对齐 bot）。
    pub fn from_settings(settings: &LoginSettings) -> Result<Self> {
        if !settings.qq_qr_login {
            return Err(Error::Business("QQ扫码登录未开启".to_string()));
        }
        if settings.nap_cat_endpoint.trim().is_empty()
            || settings.nap_cat_signature.trim().is_empty()
        {
            return Err(Error::Business("请先配置 NapCat 接口地址和接口签名".to_string()));
        }
        Ok(Self::from_parts(settings.nap_cat_endpoint.trim(), settings.nap_cat_signature.trim()))
    }

    /// 从登录设置构造；配置不完整时返回 None（供 UI 判断可用性）。
    #[must_use]
    pub fn try_from_settings(settings: &LoginSettings) -> Option<Self> {
        Self::from_settings(settings).ok()
    }

    #[must_use]
    pub fn from_parts(endpoint: &str, signature: &str) -> Self {
        let endpoint = endpoint.trim_end_matches('/').to_string();
        Self {
            endpoint,
            signature: signature.to_string(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(REQUEST_TIMEOUT_MS))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}{path}", self.endpoint)
    }

    fn nap_cat_error_message(data: &Value) -> Error {
        let error_code =
            data.get("code").and_then(Value::as_str).unwrap_or_default().trim().to_uppercase();
        let message = match error_code.as_str() {
            "SIGNATURE_REQUIRED" => "NapCat 接口签名缺失，请检查配置",
            "INVALID_SIGNATURE" => "NapCat 接口签名无效，请检查配置",
            "WORKFLOW_BUSY" => "NapCat 登录工作流繁忙，请稍后重试",
            "LOGIN_REQUIRED" => "QQ 登录尚未确认，请先完成扫码确认",
            "LOGOUT_REQUIRED" => "上一位 QQ 登录尚未注销，请稍后重试",
            "TASK_EXPIRED" => "QQ 登录任务已过期，请重新获取二维码",
            other => {
                return Error::Business(if other.is_empty() {
                    "NapCat 接口返回失败".to_string()
                } else {
                    format!("NapCat 接口返回失败（{other}）")
                })
            }
        };
        Error::Business(message.to_string())
    }

    async fn request_nap_cat(&self, path: &str, body: Value, require_ok: bool) -> Result<Value> {
        let response = self
            .http
            .post(self.api_url(path))
            .header("Content-Type", "application/json")
            .header("X-API-Signature", &self.signature)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    Error::Business("NapCat 接口请求超时，请检查服务状态".to_string())
                } else {
                    Error::Business("无法连接 NapCat 接口，请检查地址和服务状态".to_string())
                }
            })?;

        if !response.status().is_success() {
            return Err(Error::Business(format!(
                "无法连接 NapCat 接口，请检查地址和服务状态（HTTP {}）",
                response.status().as_u16()
            )));
        }

        let data: Value = response
            .json()
            .await
            .map_err(|_| Error::Business("NapCat 接口返回失败".to_string()))?;
        if require_ok && !data.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            return Err(Self::nap_cat_error_message(&data));
        }
        Ok(data)
    }

    /// 创建登录任务（获取二维码）。
    pub async fn create_login_task(&self) -> Result<QqLoginTask> {
        let data =
            self.request_nap_cat("/api/qq/login/qrcode", serde_json::json!({}), true).await?;
        normalize_task(&data)
    }

    /// 轮询扫码/确认状态。
    pub async fn query_login_status(&self, task_id: &str) -> Result<QqLoginTask> {
        let id = task_id.trim();
        if id.is_empty() {
            return Err(Error::Business("登录任务 ID 不能为空".to_string()));
        }
        let data = self
            .request_nap_cat(
                "/api/qq/login/status",
                serde_json::json!({ "taskId": id, "refresh": false }),
                true,
            )
            .await?;
        normalize_task(&data)
    }

    /// 用 taskId 换 QQ 小程序授权 code。
    pub async fn get_miniapp_code(&self, task_id: &str) -> Result<String> {
        let id = task_id.trim();
        if id.is_empty() {
            return Err(Error::Business("登录任务 ID 不能为空".to_string()));
        }
        let data = self
            .request_nap_cat(
                "/api/qq/miniapp/code",
                serde_json::json!({ "taskId": id, "appId": QQ_MINIAPP_APP_ID }),
                true,
            )
            .await?;
        let code = data.get("code").and_then(Value::as_str).unwrap_or_default().trim();
        if code.is_empty() {
            return Err(Error::Business("NapCat 未返回小程序授权 Code".to_string()));
        }
        Ok(code.to_string())
    }

    /// 取消登录任务（对齐 bot `cancelLoginTask`：不要求 ok，尽力而为）。
    pub async fn cancel_login_task(&self, task_id: &str) -> Result<()> {
        let id = task_id.trim();
        if id.is_empty() {
            return Err(Error::Business("登录任务 ID 不能为空".to_string()));
        }
        self.request_nap_cat("/api/qq/logout", serde_json::json!({ "taskId": id }), false).await?;
        Ok(())
    }
}

/// 归一化 NapCat 返回的任务对象（对齐 bot `normalizeTask`）。
fn normalize_task(raw: &Value) -> Result<QqLoginTask> {
    let task = raw.get("task").filter(|t| t.is_object()).cloned().unwrap_or(Value::Null);
    let task_id = task.get("id").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    let status = task.get("status").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    let qr_image =
        task.get("qrImage").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    let expires_at = task.get("expiresAt").and_then(Value::as_i64).unwrap_or(0);
    if task_id.is_empty() {
        return Err(Error::Business("NapCat 返回的登录任务无效".to_string()));
    }
    if qr_image.is_empty() {
        return Err(Error::Business("NapCat 未返回登录二维码".to_string()));
    }
    Ok(QqLoginTask { task_id, status, qr_image, expires_at })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_task_parses_napcat_payload() {
        let task = normalize_task(&json!({
            "ok": true,
            "task": { "id": "t-1", "status": "waiting_scan", "qrImage": "data:image/png;base64,xx", "expiresAt": 123 }
        }))
        .expect("task");
        assert_eq!(task.task_id, "t-1");
        assert_eq!(task.status, "waiting_scan");
        assert_eq!(task.qr_image, "data:image/png;base64,xx");
        assert_eq!(task.expires_at, 123);
    }

    #[test]
    fn normalize_task_rejects_missing_qr() {
        let err = normalize_task(&json!({ "task": { "id": "t-1", "status": "x" } }));
        assert!(err.is_err());
        let err = normalize_task(&json!({}));
        assert!(err.is_err());
    }

    #[test]
    fn error_messages_match_bot() {
        let err = NapCatClient::nap_cat_error_message(
            &json!({ "ok": false, "code": "signature_required" }),
        );
        assert!(err.to_string().contains("签名缺失"));
        let err =
            NapCatClient::nap_cat_error_message(&json!({ "ok": false, "code": "TASK_EXPIRED" }));
        assert!(err.to_string().contains("已过期"));
        let err = NapCatClient::nap_cat_error_message(&json!({ "ok": false }));
        assert!(err.to_string().contains("NapCat 接口返回失败"));
    }

    #[test]
    fn url_and_client_construction() {
        let client = NapCatClient::from_parts("http://127.0.0.1:3000///", "sig");
        assert_eq!(
            client.api_url("/api/qq/login/qrcode"),
            "http://127.0.0.1:3000/api/qq/login/qrcode"
        );
        assert_eq!(client.signature, "sig");
    }

    #[test]
    fn from_settings_validates() {
        let err = match NapCatClient::from_settings(&LoginSettings::default()) {
            Err(e) => e,
            Ok(_) => panic!("expected error for disabled setting"),
        };
        assert!(err.to_string().contains("QQ扫码登录未开启"));
        let off = LoginSettings { qq_qr_login: true, ..Default::default() };
        let err = match NapCatClient::from_settings(&off) {
            Err(e) => e,
            Ok(_) => panic!("expected error for missing napcat config"),
        };
        assert!(err.to_string().contains("NapCat 接口地址和接口签名"));
    }
}
