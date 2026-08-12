//! 业务服务层。
//!
//! - [`json_db`] — 原子 JSON 文件读写
//! - [`rate_limiter`] — 令牌桶 + 优先级队列 + 服务队列
//! - [`farm`] — 农场服务（1C 阶段）
//! - [`friend`] — 好友服务（1D 阶段）
//! - [`analytics`] — 作物效率分析（1G-1）
//! - [`stats`] — 每日操作统计（1G-1）
//! - [`status`] — 终端状态栏（1G-1）
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
//! - [`automation`] — 自动化开关（category → bool 映射）
//! - [`activity_center`] — 赛季 / 商店 / 星座 / 节气（1G-6，仅复刻生效活动）
//! - [`security`] — 密码 / 限流 / 会话 token（1G-7）
//! - [`qrlogin`] — QQ 小程序扫码登录（1G-7）
//! - [`login_url_profile`] — 登录 URL hints 写入系统配置（1G-7）
//! - [`randomdrop`] — 随机掉落活动（1G-7）
//! - [`wx_login`] — 微信扫码登录（1G-8：协议层 + QR 流程）

pub mod ace;
pub mod account_resolver;
pub mod activity_center;
pub mod activity_center_state;
pub mod analytics;
pub mod automation;
pub mod commerce;
pub mod email;
pub mod farm;
pub mod friend;
pub mod guide;
pub mod interact;
pub mod invite;
pub mod json_db;
pub mod login_url_profile;
pub mod mall;
pub mod monthcard;
pub mod mystery_shop;
pub mod pay;
pub mod push;
pub mod qrlogin;
pub mod qqvip;
pub mod randomdrop;
pub mod rate_limiter;
pub mod security;
pub mod share;
pub mod stats;
pub mod status;
pub mod task;
pub mod warehouse;
pub mod wx_login;

pub use json_db::{
    ensure_parent_dir, file_exists, file_size, list_files_with_ext, read_json_or,
    read_json_with_default, read_text_file, write_json_file_atomic, write_text_file_atomic,
};
pub use rate_limiter::{
    get_farm_optimizer, get_friend_optimizer, get_service_config, get_service_queue,
    PriorityQueue, QueueStatus, RateLimiterConfig, RequestQueue, ServiceConfig, TaskEntry,
    TokenBucket,
};
