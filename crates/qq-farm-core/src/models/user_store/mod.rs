//! 用户系统（注册 / 登录 / 卡密 / 鉴权 / 限流）。
//!
//! 1:1 翻译原 `core/src/models/user-store/` 下 4 个文件：
//!
//! - [`auth`] — 密码哈希（PBKDF2-SHA512 + legacy SHA256）、登录限流、登录日志
//! - [`users`] — 用户 CRUD、卡密管理、注册 / 续费
//! - [`card_claim`] — 按 UA 限流领取卡密（24h 内限一次）

pub mod auth;
pub mod card_claim;
pub mod users;

pub use auth::{
    add_login_log, check_account_lockout, check_rate_limit, clear_failed_attempts,
    clear_login_logs, get_login_logs, hash_password, needs_rehash, record_failed_attempt,
    validate_password_strength, verify_password, FailedAttemptResult, LockoutResult,
    LoginAttempt, LoginLogEntry, PasswordStrengthResult, RateLimitResult,
};
pub use card_claim::{
    check_ua_claim_limit, claim_card_by_ua, clear_expired_claim_records, get_card_claim_records,
    get_card_claim_status, set_card_claim_status, CardClaimRecord, ClaimResult,
    UaClaimCheckResult,
};
pub use users::{
    change_password, create_card, create_cards_batch, delete_card, delete_cards_batch,
    delete_user, edit_user, get_all_cards, get_all_users, get_session_user, init_default_admin,
    register_user, renew_user, update_card, update_user, validate_user, Card, DEFAULT_ACCOUNT_LIMIT,
    EditResult, EditUpdates, RegisterResult, RenewResult, User, UserCard, UserSummary, ValidationResult,
};

/// 初始化用户系统（创建默认 admin 账号 + 加载持久化数据）
pub fn init() {
    users::init_default_admin();
}
