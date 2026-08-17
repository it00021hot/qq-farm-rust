//! Account 路由 — 13 端点。
//!
//! 1:1 对应原 `controllers/admin/account-routes.ts`（495 行）。

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok, ok_data, ok_empty, AdminContext, ApiError, ApiResult};
use crate::routes::{
    accessible_account_ids, acl_policy_from_session, current_session, ensure_account_access,
    resolve_account_id,
};

fn accounts_list_payload(ctx: &AdminContext, username: Option<&str>) -> serde_json::Value {
    qq_farm_app::accounts::list_accounts_enriched(&ctx.app_context(), username)
}

fn persist_accounts() {
    qq_farm_core::models::store::accounts::persist_global();
}

fn username_from_headers(ctx: &AdminContext, headers: &axum::http::HeaderMap) -> String {
    let token = headers
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    ctx.sessions
        .get_username(token)
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| token.to_string())
}

/// 构造 account 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/accounts", get(list_accounts).post(create_account))
        .route("/api/accounts/{id}/start", post(post_account_start))
        .route("/api/accounts/{id}/stop", post(post_account_stop))
        .route("/api/account/remark", post(remark_account))
        .route("/api/accounts/{id}", delete(delete_account))
        .route("/api/account-logs", get(get_account_logs))
        .route("/api/logs", get(get_logs).delete(delete_logs))
        .route("/api/settings", get(get_settings).post(save_settings))
        .route("/api/settings/save", post(save_settings))
        .route("/api/settings/default", get(get_default_settings))
        .route("/api/settings/theme", post(set_theme))
        .route("/api/settings/offline-reminder", post(set_offline_reminder))
        .route("/api/settings/offline-reminder/test", post(test_offline_reminder))
        .route("/api/announcement", get(get_announcement))
        .route("/api/announcement/read", post(mark_announcement_read))
}

#[derive(Debug, Deserialize)]
struct RemarkBody {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct CreateAccountBody {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    qq: Option<String>,
    #[serde(default)]
    uin: Option<String>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    username: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AccountLogsQuery {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LogsQuery {
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
    #[serde(default)]
    keyword: Option<String>,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    module: Option<String>,
    #[serde(default)]
    event: Option<String>,
    #[serde(default, alias = "isWarn")]
    is_warn: Option<bool>,
    #[serde(default, alias = "timeFrom")]
    time_from: Option<String>,
    #[serde(default, alias = "timeTo")]
    time_to: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SettingsBody {
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
    #[serde(flatten)]
    rest: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ThemeBody {
    theme: String,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OfflineReminderBody {
    #[serde(default)]
    account_id: Option<String>,
    #[serde(flatten)]
    cfg: serde_json::Value,
}

async fn list_accounts(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<serde_json::Value> {
    let Some(sess) = current_session(&ctx, &headers) else {
        return ok_data(json!({ "accounts": [], "nextId": 1 }));
    };
    let filter = if sess.role == "admin" {
        None
    } else {
        Some(sess.username)
    };
    ok_data(accounts_list_payload(&ctx, filter.as_deref()))
}

async fn create_account(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateAccountBody>,
) -> ApiResult<serde_json::Value> {
    let sess = current_session(&ctx, &headers);
    let owner = body
        .username
        .clone()
        .filter(|u| !u.is_empty())
        .or_else(|| sess.as_ref().map(|s| s.username.clone()))
        .unwrap_or_else(|| username_from_headers(&ctx, &headers));
    let name = body.name.as_deref().unwrap_or("").trim().to_string();
    let code = body.code.clone().unwrap_or_default();
    let platform_set = body.platform.is_some();
    let platform = body.platform.clone().unwrap_or_else(|| "qq".to_string());
    let mut update_id = body.id.as_deref().unwrap_or("").trim().to_string();

    let visible: Vec<_> = {
        let all = qq_farm_core::models::store::accounts::get_accounts();
        if sess.as_ref().map(|s| s.role.as_str()) == Some("admin") {
            all
        } else {
            all.into_iter().filter(|a| a.username == owner).collect()
        }
    };
    let remark_relogin = update_id.is_empty() && !name.is_empty() && visible.iter().any(|a| a.name.trim() == name);
    if update_id.is_empty() && remark_relogin {
        if let Some(matched) = visible.iter().find(|a| a.name.trim() == name) {
            update_id = matched.id.clone();
        }
    }

    if update_id.is_empty() {
        if let Some(sess) = sess.as_ref() {
            if sess.role != "admin" {
                let user = qq_farm_core::models::user_store::users::get_session_user(&sess.username);
                let limit = user
                    .map(|u| u.account_limit.max(1))
                    .unwrap_or(qq_farm_core::models::user_store::users::DEFAULT_ACCOUNT_LIMIT);
                let count = qq_farm_core::models::store::accounts::get_accounts()
                    .iter()
                    .filter(|a| a.username == sess.username)
                    .count() as i64;
                if count >= limit {
                    return Err(ApiError::Forbidden(format!(
                        "账号数量已达上限（{limit}个），请购买额度卡密增加额度"
                    )));
                }
            }
        }
    }

    let is_update = !update_id.is_empty();
    let qq_set = body.qq.is_some();
    let uin_set = body.uin.is_some();
    let avatar_set = body.avatar.is_some();
    let saved = if is_update {
        let existing = qq_farm_core::models::store::accounts::get_accounts()
            .into_iter()
            .find(|a| a.id == update_id)
            .ok_or_else(|| ApiError::NotFound(format!("account not found: {update_id}")))?;
        if let Some(sess) = sess.as_ref() {
            if sess.role != "admin" && existing.username != sess.username {
                return Err(ApiError::Forbidden("无权访问此账号".to_string()));
            }
        }
        let updated = qq_farm_core::models::store::accounts::AccountRecord {
            name: if name.is_empty() { existing.name.clone() } else { name.clone() },
            code: if code.is_empty() { existing.code.clone() } else { code.clone() },
            platform: if platform_set { platform.clone() } else { existing.platform.clone() },
            qq: body.qq.clone().unwrap_or(existing.qq),
            uin: body.uin.clone().unwrap_or(existing.uin),
            avatar: body.avatar.clone().unwrap_or(existing.avatar),
            username: if owner.is_empty() { existing.username } else { owner },
            ..existing
        };
        qq_farm_core::models::store::accounts::add_or_update_account(updated)
    } else {
        let acc = qq_farm_core::models::store::accounts::AccountRecord {
            id: String::new(),
            name: name.clone(),
            code: code.clone(),
            platform: platform.clone(),
            qq: body.qq.unwrap_or_default(),
            uin: body.uin.unwrap_or_default(),
            avatar: body.avatar.unwrap_or_default(),
            username: owner,
            nick: String::new(),
            created_at: 0,
            updated_at: 0,
        };
        let mut saved = qq_farm_core::models::store::accounts::add_or_update_account(acc);
        if saved.name.trim().is_empty() {
            saved.name = format!("账号{}", saved.id);
            saved = qq_farm_core::models::store::accounts::add_or_update_account(saved);
        }
        saved
    };
    persist_accounts();

    let list_filter = sess.as_ref().and_then(|s| {
        if s.role == "admin" {
            None
        } else {
            Some(s.username.clone())
        }
    });

    if !is_update {
        ctx.engine.runtime_state().add_account_log(
            "add",
            &format!("添加账号: {}", saved.name),
            Some(&saved.id),
            Some(&saved.name),
            None,
        );
        if !saved.code.is_empty() {
            let models_acc = qq_farm_core::models::AccountSession::from_store(&saved);
            if let Err(e) = ctx.engine.start_worker(models_acc) {
                tracing::warn!(account_id = %saved.id, "自动启动 worker 失败: {e}");
            }
        }
    } else {
        let only_remark = body.code.as_deref().unwrap_or("").is_empty()
            && !platform_set
            && !qq_set
            && !uin_set
            && !avatar_set;
        let was_running = ctx.engine.has_worker(&saved.id);
        let should_restart = remark_relogin || (was_running && !only_remark);
        if should_restart && !saved.code.is_empty() {
            let models_acc = qq_farm_core::models::AccountSession::from_store(&saved);
            if let Err(e) = ctx.engine.restart_worker(models_acc) {
                tracing::warn!(account_id = %saved.id, "更新后重启 worker 失败: {e}");
            }
        }
        let msg = if remark_relogin {
            format!("通过备注重新登录账号: {}", saved.name)
        } else {
            format!("更新账号: {}", saved.name)
        };
        ctx.engine.runtime_state().add_account_log(
            "update",
            &msg,
            Some(&saved.id),
            Some(&saved.name),
            None,
        );
    }

    ok_data(accounts_list_payload(&ctx, list_filter.as_deref()))
}

async fn remark_account(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<RemarkBody>,
) -> ApiResult<serde_json::Value> {
    ensure_account_access(&ctx, &headers, &body.id)?;
    let accounts = qq_farm_core::models::store::accounts::get_accounts();
    let acc = accounts
        .into_iter()
        .find(|a| a.id == body.id)
        .ok_or_else(|| ApiError::NotFound(format!("account not found: {}", body.id)))?;
    let updated = qq_farm_core::models::store::accounts::AccountRecord {
        name: body.name,
        ..acc
    };
    let _saved = qq_farm_core::models::store::accounts::add_or_update_account(updated);
    persist_accounts();
    let list_filter = current_session(&ctx, &headers).and_then(|s| {
        if s.role == "admin" {
            None
        } else {
            Some(s.username)
        }
    });
    ok_data(accounts_list_payload(&ctx, list_filter.as_deref()))
}

async fn delete_account(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    ensure_account_access(&ctx, &headers, &id)?;
    ctx.engine.stop_worker(&id);
    let _ = qq_farm_core::models::store::accounts::delete_account(&id);
    persist_accounts();
    let list_filter = current_session(&ctx, &headers).and_then(|s| {
        if s.role == "admin" {
            None
        } else {
            Some(s.username)
        }
    });
    ok_data(accounts_list_payload(&ctx, list_filter.as_deref()))
}

async fn post_account_start(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let sess = current_session(&ctx, &headers).ok_or_else(|| {
        ApiError::Forbidden("无权访问该账号".to_string())
    })?;
    let policy = acl_policy_from_session(&sess);
    let acc = qq_farm_app::accounts::start_account(&ctx.app_context(), &policy, &id)?;
    ok(json!({ "ok": true, "accountId": acc.id, "started": true }))
}

async fn post_account_stop(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let sess = current_session(&ctx, &headers).ok_or_else(|| {
        ApiError::Forbidden("无权访问该账号".to_string())
    })?;
    let policy = acl_policy_from_session(&sess);
    qq_farm_app::accounts::stop_account(&ctx.app_context(), &policy, &id)?;
    ok(json!({ "ok": true, "accountId": id, "stopped": true }))
}

async fn get_account_logs(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountLogsQuery>,
) -> ApiResult<serde_json::Value> {
    let sess = current_session(&ctx, &headers);
    let is_admin = sess.as_ref().is_some_and(|s| s.role == "admin");
    let owned: std::collections::HashSet<String> = if is_admin {
        Default::default()
    } else {
        accessible_account_ids(&ctx, &headers).into_iter().collect()
    };
    let state = ctx.engine.runtime_state();
    let logs = state.account_logs.lock().clone();
    let filtered: Vec<_> = logs
        .into_iter()
        .filter(|l| {
            if let Some(target) = q.account_id.as_deref() {
                if l.account_id != target {
                    return false;
                }
            }
            is_admin || owned.contains(&l.account_id)
        })
        .collect();
    let limit = q.limit.unwrap_or(100);
    let limited: Vec<_> = filtered.into_iter().rev().take(limit).collect();
    Ok(Json(serde_json::to_value(&limited).unwrap_or(json!([]))))
}

async fn get_logs(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<LogsQuery>,
) -> ApiResult<serde_json::Value> {
    let query_ref = q.account_id.as_deref().unwrap_or("").trim();
    let id = if query_ref.is_empty() || query_ref == "all" {
        resolve_account_id(&ctx, &headers, None)
    } else {
        resolve_account_id(&ctx, &headers, Some(query_ref))
    };
    let state = ctx.engine.runtime_state();
    let logs = state.global_logs.lock().clone();
    let filters = qq_farm_core::runtime::runtime_state::LogFilters {
        keyword: q.keyword,
        tag: q.tag,
        module: q.module,
        event: q.event,
        is_warn: q.is_warn,
        time_from: q.time_from,
        time_to: q.time_to,
    };
    let accessible = accessible_account_ids(&ctx, &headers);
    let scoped: Vec<_> = if id.is_empty() {
        logs.into_iter()
            .filter(|l| {
                l.account_id
                    .as_deref()
                    .map(|aid| accessible.iter().any(|x| x == aid))
                    .unwrap_or(true)
            })
            .collect()
    } else {
        logs.into_iter()
            .filter(|l| l.account_id.as_deref() == Some(id.as_str()))
            .collect()
    };
    let mut filtered = state.filter_logs(&scoped, &filters);
    filtered.sort_by(|a, b| b.ts.cmp(&a.ts));
    let limit = q.limit.unwrap_or(100).max(1);
    filtered.truncate(limit);
    ok_data(filtered)
}

async fn delete_logs(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, None);
    if id.is_empty() {
        return Err(ApiError::BadRequest("Missing x-account-id".to_string()));
    }
    let state = ctx.engine.runtime_state();
    let mut logs = state.global_logs.lock();
    let before = logs.len();
    logs.retain(|l| l.account_id.as_deref() != Some(id.as_str()));
    let cleared = before.saturating_sub(logs.len());
    ok_data(json!({ "cleared": cleared, "accountId": id }))
}

fn settings_panel_payload(account_id: &str, username: &str) -> serde_json::Value {
    qq_farm_app::settings::settings_panel(account_id, username)
}

async fn get_settings(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AccountQuery>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, q.account_id.as_deref());
    let username = current_session(&ctx, &headers)
        .map(|s| s.username)
        .unwrap_or_default();
    ok_data(settings_panel_payload(&id, &username))
}

async fn save_settings(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<SettingsBody>,
) -> ApiResult<serde_json::Value> {
    let id = resolve_account_id(&ctx, &headers, body.account_id.as_deref());
    if id.is_empty() {
        return Err(ApiError::BadRequest("Missing x-account-id".to_string()));
    }
    let _updated = qq_farm_core::models::store::account_config::apply_config_snapshot(
        body.rest,
        Some(&id),
        true,
    );
    let rev = ctx.engine.runtime_state().next_config_revision();
    let running = ctx.engine.has_worker(&id);
    if running {
        ctx.engine.reload_worker_config(&id);
    }
    let mut data = settings_panel_payload(&id, "");
    if let Some(obj) = data.as_object_mut() {
        obj.insert("saved".to_string(), json!(true));
        obj.insert("configRevision".to_string(), json!(rev));
        obj.insert("strategy".to_string(), obj.get("strategy").cloned().unwrap_or(json!(null)));
        obj.insert("preferredSeed".to_string(), obj.get("preferredSeed").cloned().unwrap_or(json!(0)));
        obj.insert("status".to_string(), json!(if running { "confirmed" } else { "stopped" }));
        obj.insert("stopped".to_string(), json!(!running));
        obj.insert("confirmed".to_string(), json!(running));
    }
    Ok(Json(json!({
        "ok": true,
        "saved": true,
        "stopped": !running,
        "confirmed": running,
        "unconfirmed": false,
        "status": if running { "confirmed" } else { "stopped" },
        "data": data,
    })))
}

async fn get_default_settings(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let cfg = qq_farm_core::models::store::account_config::get_default_account_config();
    ok_data(cfg)
}

async fn set_theme(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<ThemeBody>,
) -> ApiResult<serde_json::Value> {
    let _ = body.account_id;
    qq_farm_core::models::store::global_config::set_ui_theme(&body.theme);
    ok_empty()
}

async fn set_offline_reminder(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<OfflineReminderBody>,
) -> ApiResult<serde_json::Value> {
    let cfg: qq_farm_core::models::store::global_config::OfflineReminder =
        serde_json::from_value(body.cfg).unwrap_or_default();
    let username = current_session(&ctx, &headers)
        .map(|s| s.username)
        .unwrap_or_default();
    if username.is_empty() {
        qq_farm_core::models::store::global_config::set_offline_reminder(cfg);
    } else {
        qq_farm_core::models::store::global_config::set_user_offline_reminder(&username, cfg);
    }
    ok_empty()
}

async fn test_offline_reminder(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<OfflineReminderBody>,
) -> ApiResult<serde_json::Value> {
    let username = current_session(&ctx, &headers)
        .map(|s| s.username)
        .unwrap_or_default();
    let cfg = if username.is_empty() {
        qq_farm_core::models::store::global_config::get_offline_reminder()
    } else {
        qq_farm_core::models::store::global_config::get_user_offline_reminder(&username)
            .unwrap_or_else(qq_farm_core::models::store::global_config::get_offline_reminder)
    };
    let merged: qq_farm_core::models::store::global_config::OfflineReminder =
        serde_json::from_value(body.cfg).unwrap_or(cfg);
    let push = qq_farm_core::services::push::PushService::new();
    let result = push
        .send(&qq_farm_core::services::push::PushPayload {
            channel: merged.channel.clone(),
            endpoint: merged.endpoint.clone(),
            token: merged.token.clone(),
            title: if merged.title.is_empty() {
                "离线提醒测试".to_string()
            } else {
                merged.title.clone()
            },
            content: if merged.msg.is_empty() {
                "这是一条离线提醒测试".to_string()
            } else {
                merged.msg.clone()
            },
        })
        .await;
    if result.ok {
        ok_empty()
    } else {
        Ok(Json(json!({ "ok": false, "error": result.msg })))
    }
}

async fn get_announcement(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<serde_json::Value> {
    let username = current_session(&ctx, &headers)
        .map(|s| s.username)
        .unwrap_or_default();
    let ann = qq_farm_core::models::store::global_config::get_announcement();
    let should_show =
        qq_farm_core::models::store::global_config::should_show_announcement(&username);
    let mut data = serde_json::to_value(&ann).unwrap_or(json!({}));
    if let Some(obj) = data.as_object_mut() {
        obj.insert("shouldShow".to_string(), json!(should_show));
    }
    ok_data(data)
}

async fn mark_announcement_read(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> ApiResult<serde_json::Value> {
    if let Some(sess) = current_session(&ctx, &headers) {
        qq_farm_core::models::store::global_config::mark_announcement_read(&sess.username);
    }
    ok_empty()
}

#[derive(Debug, Deserialize)]
struct AccountQuery {
    #[serde(default, alias = "accountId")]
    account_id: Option<String>,
}
