mod loader;
mod mapper;
mod model;

#[cfg(test)]
#[path = "model_tests.rs"]
mod model_tests;

pub use loader::load_page_for_mall_slot;
pub(crate) use mapper::{page_data, sort_friend_rows};
pub use model::{DisplayRow, Page, PageData};
