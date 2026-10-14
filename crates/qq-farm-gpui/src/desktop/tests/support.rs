use super::super::{mpsc, EngineLease, FarmDesktop};
use gpui_kit::{AnyWindowHandle, Entity, WindowOptions};
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};
use std::ffi::OsString;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);
static TEST_DATA_DIR_LOCK: Mutex<()> = Mutex::new(());

pub(super) struct RestoreDataDir {
    previous: Option<OsString>,
    _guard: MutexGuard<'static, ()>,
}

impl Drop for RestoreDataDir {
    fn drop(&mut self) {
        if let Some(path) = self.previous.take() {
            std::env::set_var("FARM_DATA_DIR", path);
        } else {
            std::env::remove_var("FARM_DATA_DIR");
        }
    }
}

pub(super) fn open_test_desktop(
    cx: &mut TestAppContext,
) -> (AnyWindowHandle, Entity<FarmDesktop>, RestoreDataDir) {
    // FARM_DATA_DIR is process-wide, so hold this guard through the test lifetime.
    let data_dir_guard =
        TEST_DATA_DIR_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    cx.update(crate::theme::init);
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("qq-farm-gpui-test")
            .build()
            .expect("test Tokio runtime"),
    );
    let data_dir = std::env::temp_dir().join(format!(
        "qq-farm-gpui-test-{}-{}",
        std::process::id(),
        NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed),
    ));
    let restore_data_dir =
        RestoreDataDir { previous: std::env::var_os("FARM_DATA_DIR"), _guard: data_dir_guard };
    std::env::set_var("FARM_DATA_DIR", &data_dir);
    let app = {
        let _enter = runtime.enter();
        Arc::new(qq_farm_app::bootstrap::assemble_app_context(1, "https://game.qq.com"))
    };
    let lease = Arc::new(EngineLease::acquire(&data_dir).expect("test data lease"));
    let (_tray_sender, tray_receiver) = mpsc::unbounded();
    cx.update(|cx| crate::application::configure_commands(cx, "zh-CN"));
    let (window, desktop) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                FarmDesktop::new(
                    app,
                    runtime,
                    lease,
                    None,
                    true,
                    crate::runtime::UiPreferences::default(),
                    tray_receiver,
                    window,
                    cx,
                )
            })
        })
        .expect("test window")
    });
    cx.update(|cx| {
        desktop.update(cx, |this, _| this.window_handle = Some(window));
        crate::application::register_window_commands(&desktop, window, cx);
    });
    (window, desktop, restore_data_dir)
}
