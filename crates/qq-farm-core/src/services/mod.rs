//! 业务服务层。
//!
//! - [`infra`](crate::infra) — JSON 持久化、限流、自动化、日志、统计、状态（自本层迁出）
//! - [`farm`] — 农场服务（1C 阶段）
//! - [`friend`] — 好友服务（1D 阶段）
//! - [`analytics`] — 作物效率分析（1G-1）
//! - [`email`] — 邮箱领取（1G-2）
//! - [`share`] — 每日分享（1G-2）
//! - [`interact`] — 访客互动记录（1G-2）
//! - [`warehouse`] — 仓库 / 出售果实 / 化肥礼包（1G-2）
//! - [`mystery_shop`] — 神秘商店 RPC 封装（1G-3）
//! - [`pay`] — 支付 / 充值 RPC 封装（1G-3）
//! - [`mall`] — 商城自动购买化肥 / 免费礼包（1G-3）
//! - [`monthcard`] — 月卡每日礼包（1G-3）
//! - [`commerce`] — 商城 + 神秘商店业务编排 + DTO（1G-3）
//! - [`push`] — 推送服务 webhook 实现（1G-3）
//! - [`qqvip`] — QQ 会员每日礼包（1G-4）
//! - [`invite`] — 邀请码处理（ReportArkClick 模拟点击分享链接）（1G-4）
//! - [`guide`] — 新手引导 / 节点奖励（1G-4）
//! - [`task`] — 任务 / 活跃度 / 图鉴自动领取（1G-5）
//! - [`activity_center`] — 赛季 / 商店 / 星座 / 节气（1G-6，仅复刻生效活动）
//! - [`security`] — 密码 / 限流 / 会话 token（1G-7）
//! - [`qrlogin`] — QQ 小程序扫码登录（1G-7）
//! - [`login_url_profile`] — 登录 URL hints 写入系统配置（1G-7）
//! - [`randomdrop`] — 随机掉落活动（1G-7）
//! - [`wx_login`] — 微信扫码登录（1G-8：协议层 + QR 流程）
//!
//! ## 域分组（Phase 2a）
//!
//! - [`commerce`] — mall / mystery_shop / pay / 编排
//! - [`daily`] — email / share / monthcard / qqvip
//! - [`activity`] — activity_center / activity_center_state
//! - [`auth`] — security / qrlogin / wx_login / login_url_profile
//! - [`tasks`] — task / guide / interact / invite / randomdrop

pub mod ace;
pub mod account_resolver;
pub mod activity;
pub mod activity_center;
pub mod activity_center_state;
pub mod analytics;
pub mod auth;
pub mod commerce;
pub mod daily;
pub mod email;
pub mod farm;
pub mod friend;
pub mod guide;
pub mod interact;
pub mod invite;
pub mod login_url_profile;
pub mod mall;
pub mod monthcard;
pub mod mystery_shop;
pub mod pay;
pub mod push;
pub mod qrlogin;
pub mod qqvip;
pub mod randomdrop;
pub mod security;
pub mod share;
pub mod task;
pub mod tasks;
pub mod warehouse;
pub mod wx_login;

// --- infra 向后兼容 re-export（已迁至 [`crate::infra`]) ---
pub use crate::infra::automation;
pub use crate::infra::json_db;
pub use crate::infra::panel_log;
pub use crate::infra::rate_limiter;
pub use crate::infra::stats;
pub use crate::infra::status;
pub use crate::infra::{
    ensure_parent_dir, file_exists, file_size, get_farm_optimizer, get_friend_optimizer,
    get_service_config, get_service_queue, list_files_with_ext, read_json_or,
    read_json_with_default, read_text_file, write_json_file_atomic, write_text_file_atomic,
    PriorityQueue, QueueStatus, RateLimiterConfig, RequestQueue, ServiceConfig, TaskEntry,
    TokenBucket,
};
