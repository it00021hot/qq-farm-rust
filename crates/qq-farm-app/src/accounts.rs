//! 账号 ACL 与生命周期 helpers。

use std::collections::HashSet;

use qq_farm_core::models::store::accounts::{self, AccountRecord};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::session::AppContext;

/// 账号访问策略。
#[derive(Debug, Clone)]
pub enum AclPolicy {
    /// 面板用户：admin 全放行，普通用户仅自己的账号。
    PanelUser { username: String, role: String },
    /// 桌面端本地所有者：允许所有本地账号。
    LocalOwner,
}

/// 账号 ACL：admin / LocalOwner 全放行；普通用户只能访问自己的账号。
#[must_use]
pub fn account_accessible(policy: &AclPolicy, account_id: &str) -> bool {
    if account_id.is_empty() {
        return true;
    }
    match policy {
        AclPolicy::LocalOwner => true,
        AclPolicy::PanelUser { username, role } => {
            if role == "admin" {
                return true;
            }
            accounts::get_accounts()
                .into_iter()
                .any(|a| a.id == account_id && a.username == *username)
        }
    }
}

/// 无权限时 Forbidden。
pub fn ensure_account_access(policy: &AclPolicy, account_id: &str) -> AppResult<()> {
    if account_accessible(policy, account_id) {
        Ok(())
    } else {
        Err(AppError::Forbidden("无权访问该账号".to_string()))
    }
}

/// 当前策略可访问的账号 ID 列表。
#[must_use]
pub fn accessible_account_ids(policy: &AclPolicy) -> Vec<String> {
    let all = accounts::get_accounts();
    match policy {
        AclPolicy::LocalOwner => all.into_iter().map(|a| a.id).collect(),
        AclPolicy::PanelUser { role, .. } if role == "admin" => {
            all.into_iter().map(|a| a.id).collect()
        }
        AclPolicy::PanelUser { username, .. } => {
            all.into_iter().filter(|a| a.username == *username).map(|a| a.id).collect()
        }
    }
}

/// 启动账号 worker。
pub fn start_account(ctx: &AppContext, policy: &AclPolicy, id: &str) -> AppResult<AccountRecord> {
    ensure_account_access(policy, id)?;
    let acc = accounts::get_accounts()
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| AppError::NotFound(format!("account not found: {id}")))?;
    let account = qq_farm_core::models::AccountSession::from_store(&acc);
    ctx.engine.start_worker(account).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(acc)
}

/// 停止账号 worker。
pub fn stop_account(ctx: &AppContext, policy: &AclPolicy, id: &str) -> AppResult<()> {
    ensure_account_access(policy, id)?;
    ctx.engine.stop_worker(id);
    Ok(())
}

/// 更新账号备注。
pub fn remark_account(policy: &AclPolicy, id: &str, name: String) -> AppResult<AccountRecord> {
    ensure_account_access(policy, id)?;
    let acc = accounts::get_accounts()
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| AppError::NotFound(format!("account not found: {id}")))?;
    let updated = AccountRecord { name, ..acc };
    let saved = accounts::add_or_update_account(updated);
    accounts::persist_global();
    Ok(saved)
}

/// 删除账号（先停 worker，再删 store）。
pub fn delete_account(ctx: &AppContext, policy: &AclPolicy, id: &str) -> AppResult<()> {
    ensure_account_access(policy, id)?;
    ctx.engine.stop_worker(id);
    let _ = accounts::delete_account(id);
    accounts::persist_global();
    Ok(())
}

/// 创建 / 更新账号请求。
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertAccountRequest {
    pub id: Option<String>,
    pub name: Option<String>,
    pub code: Option<String>,
    pub platform: Option<String>,
    pub qq: Option<String>,
    pub uin: Option<String>,
    pub avatar: Option<String>,
    /// 归属用户名；LocalOwner 可用空或 `"local"`。
    pub username: Option<String>,
    /// 非 admin 时用于额度校验；None 表示不校验额度。
    pub account_limit: Option<i64>,
}

/// 带 running / nick 的账号列表。
#[must_use]
pub fn list_accounts_enriched(ctx: &AppContext, username_filter: Option<&str>) -> Value {
    let running: HashSet<String> =
        ctx.engine.list_workers().into_iter().map(|w| w.account_id).collect();
    let data = accounts::accounts_data();
    let account_list: Vec<Value> = data
        .accounts
        .iter()
        .filter(|a| username_filter.is_none_or(|u| a.username == u))
        .map(|a| {
            let mut v = serde_json::to_value(a).unwrap_or(json!({}));
            if let Some(obj) = v.as_object_mut() {
                redact_wx_auth_fields(obj, a.has_wx_auth());
                obj.insert("running".to_string(), json!(running.contains(&a.id)));
                let status = ctx.engine.panel_status(&a.id);
                if let Some(nick) = status
                    .pointer("/status/name")
                    .and_then(|n| n.as_str())
                    .filter(|s| !s.is_empty())
                {
                    obj.insert("nick".to_string(), json!(nick));
                }
            }
            v
        })
        .collect();
    json!({
        "accounts": account_list,
        "nextId": data.next_id.max(1),
    })
}

fn redact_wx_auth_fields(obj: &mut serde_json::Map<String, Value>, authorized: bool) {
    obj.remove("wx_login_buffer");
    obj.remove("wx_access_token");
    obj.insert("wxAuthorized".to_string(), json!(authorized));
}

/// 账号 JSON（去掉应用宝敏感字段）。
#[must_use]
pub fn account_to_public_json(acc: &AccountRecord) -> Value {
    let mut v = serde_json::to_value(acc).unwrap_or(json!({}));
    if let Some(obj) = v.as_object_mut() {
        redact_wx_auth_fields(obj, acc.has_wx_auth());
    }
    v
}

fn policy_username_filter(policy: &AclPolicy) -> Option<&str> {
    match policy {
        AclPolicy::LocalOwner => None,
        AclPolicy::PanelUser { role, .. } if role == "admin" => None,
        AclPolicy::PanelUser { username, .. } => Some(username.as_str()),
    }
}

/// 创建或更新账号；新建且有 code 时自动 start。
pub fn upsert_account(
    ctx: &AppContext,
    policy: &AclPolicy,
    req: UpsertAccountRequest,
) -> AppResult<Value> {
    let owner = req.username.clone().filter(|u| !u.is_empty()).unwrap_or_else(|| match policy {
        AclPolicy::LocalOwner => "local".to_string(),
        AclPolicy::PanelUser { username, .. } => username.clone(),
    });
    let name = req.name.as_deref().unwrap_or("").trim().to_string();
    let code = req.code.clone().unwrap_or_default();
    let platform_set = req.platform.is_some();
    let platform = req.platform.clone().unwrap_or_else(|| "qq".to_string());
    let mut update_id = req.id.as_deref().unwrap_or("").trim().to_string();

    let visible: Vec<_> = {
        let all = accounts::get_accounts();
        match policy_username_filter(policy) {
            None => all,
            Some(u) => all.into_iter().filter(|a| a.username == u).collect(),
        }
    };
    let remark_relogin =
        update_id.is_empty() && !name.is_empty() && visible.iter().any(|a| a.name.trim() == name);
    if update_id.is_empty() && remark_relogin {
        if let Some(matched) = visible.iter().find(|a| a.name.trim() == name) {
            update_id = matched.id.clone();
        }
    }

    if update_id.is_empty() {
        if let Some(limit) = req.account_limit {
            let count =
                accounts::get_accounts().iter().filter(|a| a.username == owner).count() as i64;
            if count >= limit {
                return Err(AppError::Forbidden(format!("账号数量已达上限（{limit}个）")));
            }
        }
    }

    let is_update = !update_id.is_empty();
    if is_update {
        ensure_account_access(policy, &update_id)?;
    }

    let qq_set = req.qq.is_some();
    let uin_set = req.uin.is_some();
    let avatar_set = req.avatar.is_some();
    let code_provided = !code.trim().is_empty();
    let mut code_changed = false;
    let mut saved = if is_update {
        let existing = accounts::get_accounts()
            .into_iter()
            .find(|a| a.id == update_id)
            .ok_or_else(|| AppError::NotFound(format!("account not found: {update_id}")))?;
        if code_provided {
            code_changed = code.trim() != existing.code.trim();
        }
        let updated = AccountRecord {
            name: if name.is_empty() { existing.name.clone() } else { name.clone() },
            code: if code.is_empty() { existing.code.clone() } else { code.clone() },
            platform: if platform_set { platform.clone() } else { existing.platform.clone() },
            qq: req.qq.clone().unwrap_or(existing.qq),
            uin: req.uin.clone().unwrap_or(existing.uin),
            avatar: req.avatar.clone().unwrap_or(existing.avatar),
            username: if owner.is_empty() { existing.username } else { owner },
            ..existing
        };
        accounts::add_or_update_account(updated)
    } else {
        let acc = AccountRecord {
            id: String::new(),
            name: name.clone(),
            code: code.clone(),
            platform: platform.clone(),
            qq: req.qq.unwrap_or_default(),
            uin: req.uin.unwrap_or_default(),
            avatar: req.avatar.unwrap_or_default(),
            username: owner,
            ..Default::default()
        };
        let mut saved = accounts::add_or_update_account(acc);
        if saved.name.trim().is_empty() {
            saved.name = format!("账号{}", saved.id);
            saved = accounts::add_or_update_account(saved);
        }
        saved
    };
    if let Some(auth) = crate::wx_login::take_pending_auth(&ctx.wx_login, &saved.code) {
        saved.wx_openid = auth.openid;
        saved.wx_login_buffer = auth.login_buffer;
        saved.wx_access_token = auth.access_token;
        saved = accounts::add_or_update_account(saved);
    }
    accounts::persist_global();

    if is_update {
        let only_remark = !code_provided && !platform_set && !qq_set && !uin_set && !avatar_set;
        let was_running = ctx.engine.has_worker(&saved.id);
        // Align Go: refreshing login code implies reconnect — start/restart even if previously stopped.
        let should_restart = remark_relogin || code_changed || (was_running && !only_remark);
        if should_restart && (!saved.code.is_empty() || saved.has_wx_auth()) {
            let models_acc = qq_farm_core::models::AccountSession::from_store(&saved);
            if let Err(e) = ctx.engine.restart_worker(models_acc) {
                tracing::warn!(account_id = %saved.id, "更新后重启 worker 失败: {e}");
                return Err(AppError::Internal(format!("账号已更新，自动启动失败: {e}")));
            }
        }
        let msg = if remark_relogin || code_changed {
            format!("通过登录凭证重新登录账号: {}", saved.name)
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
    } else {
        ctx.engine.runtime_state().add_account_log(
            "add",
            &format!("添加账号: {}", saved.name),
            Some(&saved.id),
            Some(&saved.name),
            None,
        );
        if !saved.code.is_empty() || saved.has_wx_auth() {
            let models_acc = qq_farm_core::models::AccountSession::from_store(&saved);
            if let Err(e) = ctx.engine.start_worker(models_acc) {
                tracing::warn!(account_id = %saved.id, "自动启动 worker 失败: {e}");
            }
        }
    }

    Ok(list_accounts_enriched(ctx, policy_username_filter(policy)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;

    #[test]
    fn redact_wx_auth_strips_secrets_and_flags() {
        let mut obj = Map::new();
        obj.insert("wx_login_buffer".into(), json!("secret-buf"));
        obj.insert("wx_access_token".into(), json!("secret-tok"));
        obj.insert("wx_openid".into(), json!("oid"));
        redact_wx_auth_fields(&mut obj, true);
        assert!(obj.get("wx_login_buffer").is_none());
        assert!(obj.get("wx_access_token").is_none());
        assert_eq!(obj.get("wx_openid").and_then(|v| v.as_str()), Some("oid"));
        assert_eq!(obj.get("wxAuthorized"), Some(&json!(true)));
    }

    #[test]
    fn redact_wx_auth_false_when_missing() {
        let mut obj = Map::new();
        redact_wx_auth_fields(&mut obj, false);
        assert_eq!(obj.get("wxAuthorized"), Some(&json!(false)));
    }

    #[test]
    fn account_to_public_json_hides_buffer() {
        let acc = AccountRecord {
            id: "1".into(),
            name: "n".into(),
            code: "c".into(),
            platform: "wx".into(),
            wx_openid: "oid".into(),
            wx_login_buffer: "buf".into(),
            wx_access_token: "tok".into(),
            ..Default::default()
        };
        let v = account_to_public_json(&acc);
        assert!(v.get("wx_login_buffer").is_none());
        assert!(v.get("wx_access_token").is_none());
        assert_eq!(v["wxAuthorized"], json!(true));
        assert_eq!(v["wx_openid"], "oid");
    }
}
