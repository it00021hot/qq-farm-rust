//! Protobuf 生成的 Rust 类型。
//!
//! 实际代码由 `build.rs` 调 `prost-build` 编译 `../../proto/*.proto` 生成，
//! build.rs 写入 `OUT_DIR/generated/mod.rs`，按 proto package 结构嵌套。
//!
//! ## 模块布局
//!
//! - `crate::proto::generated::corepb` —— `corepb.proto`
//! - `crate::proto::generated::gatepb` —— `game.proto`（gatepb）
//! - `crate::proto::generated::gamepb::acepb` —— `acepb.proto`
//! - `crate::proto::generated::gamepb::activitypb` —— `activitypb.proto`
//! - ... 共 33 个 mod
//!
//! 这个嵌套结构对齐 prost-build 内部 `super::xxx` 相对路径，**不要随意扁平化**。

#![allow(
    clippy::all,
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    clippy::pedantic,
    clippy::nursery
)]

/// 自动生成的 protobuf 模块集合（按 proto package 嵌套）
pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated/mod.rs"));
}

pub use generated::*;
