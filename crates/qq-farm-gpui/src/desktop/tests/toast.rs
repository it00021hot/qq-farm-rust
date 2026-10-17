use std::time::Duration;

use super::support::open_test_desktop;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn toast_autohides_quickly_and_pauses_while_hovered(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| this.show_toast("已保存", true, cx));
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("app-toast").visible());
    })
    .expect("toast renders");

    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.toast.as_mut().expect("toast state").hovered = true;
            cx.notify();
        });
    });
    for _ in 0..60 {
        cx.background_executor.advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
    }
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("app-toast").visible(), "hover pauses the timer");
    })
    .expect("hovered toast stays");

    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.toast.as_mut().expect("toast state").hovered = false;
            cx.notify();
        });
    });
    for _ in 0..30 {
        cx.background_executor.advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
    }
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("app-toast").is_none(), "toast expires after the short delay");
    })
    .expect("toast auto-hides");
}
