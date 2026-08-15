//! 常用类型的便捷 re-export。
//!
//! ```ignore
//! use qq_farm_core::prelude::*;
//! ```

pub use crate::config::{AppConfig, GameConfig};
pub use crate::error::{Error, Result};
pub use crate::models::{Account, AccountSession, Friend, Land};
pub use crate::utils::logger;
