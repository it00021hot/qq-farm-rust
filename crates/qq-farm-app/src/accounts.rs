//! 账号 ACL 与生命周期 helpers。

use qq_farm_core::models::store::accounts::{self, AccountRecord};

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
        AclPolicy::PanelUser { username, .. } => all
            .into_iter()
            .filter(|a| a.username == *username)
            .map(|a| a.id)
            .collect(),
    }
}

/// 启动账号 worker。
pub fn start_account(
    ctx: &AppContext,
    policy: &AclPolicy,
    id: &str,
) -> AppResult<AccountRecord> {
    ensure_account_access(policy, id)?;
    let acc = accounts::get_accounts()
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| AppError::NotFound(format!("account not found: {id}")))?;
    let account = qq_farm_core::models::AccountSession::from_store(&acc);
    ctx.engine
        .start_worker(account)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(acc)
}

/// 停止账号 worker。
pub fn stop_account(ctx: &AppContext, policy: &AclPolicy, id: &str) -> AppResult<()> {
    ensure_account_access(policy, id)?;
    ctx.engine.stop_worker(id);
    Ok(())
}

/// 更新账号备注。
pub fn remark_account(
    policy: &AclPolicy,
    id: &str,
    name: String,
) -> AppResult<AccountRecord> {
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
