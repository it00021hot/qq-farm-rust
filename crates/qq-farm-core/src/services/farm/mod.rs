//! 农场服务模块。
//!
//! - [`api`] — 底层农场/商店 API（protobuf 请求 + 解码响应）
//! - [`land_analysis`] — 土地状态分析（地块映射、阶段判断、布局）
//! - [`scheduler`] — 调度循环（定时检查 + 触发操作）
//! - [`planting`] — 种植引擎（选种子、拖动种植、按配置施肥）
//!
//! 阶段 1C.1 范围：api + 框架骨架 + 最小可工作测试。
//! 阶段 1C.2 范围：land_analysis + planting 核心策略。
//! 阶段 1C.3 范围：scheduler 完整循环 + 端到端 demo。

pub mod api;
pub mod land_analysis;
pub mod planting;
pub mod scheduler;
