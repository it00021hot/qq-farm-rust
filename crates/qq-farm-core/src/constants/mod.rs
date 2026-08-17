//! 跨模块静态常量唯一入口。
//!
//! 游戏协议 ID、RPC 服务名、TTL、生长阶段名等放此处；
//! 进程部署配置（端口、CORS）不属于本模块。

pub mod game_ids;
pub mod panel_events;
pub mod plant;
pub mod rpc;
pub mod timing;

pub use game_ids::*;
pub use panel_events::*;
pub use plant::*;
pub use rpc::*;
pub use timing::*;
