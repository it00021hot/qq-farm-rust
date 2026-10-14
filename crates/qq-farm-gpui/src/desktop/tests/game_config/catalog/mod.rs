use super::super::super::Page;
use super::super::support::open_test_desktop;
use super::{wait_for_batch_delete_popover, wait_for_delete_popover, wait_for_sheet};
use gpui_kit::component::WindowExt;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{point, px, size, AppContext as GpuiContextExt, ScrollDelta, TestAppContext};
use std::time::Duration;

mod deletion;
mod lifecycle;
mod search;
mod table;
