//! 使用原生字体与 GPU 输出界面预览；隔离数据目录，仅注入展示用快照。
mod fixtures;
mod scenes;

use super::*;
use gpui_kit::component::theme::{Theme, ThemeMode};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{Bounds, HeadlessAppContext, Result, WindowBounds, WindowOptions};
use std::path::Path;

pub(crate) fn render_previews(output: &Path) -> Result<()> {
    std::fs::create_dir_all(output)?;
    let data = std::env::temp_dir().join(format!("qq-farm-gpui-preview-{}", std::process::id()));
    std::env::set_var("FARM_DATA_DIR", &data);
    crate::runtime::prepare_environment()?;
    let lease = Arc::new(EngineLease::acquire(&data)?);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().enable_all().build()?);
    let app = {
        let _enter = runtime.enter();
        Arc::new(qq_farm_app::bootstrap::assemble_app_context(1, "https://game.qq.com"))
    };
    for case in scenes::cases() {
        render_case(&case, &app, &runtime, &lease, output)?;
    }
    runtime.block_on(qq_farm_app::lifecycle::shutdown(&app, Duration::from_secs(1)));
    drop(app);
    if let Ok(runtime) = Arc::try_unwrap(runtime) {
        runtime.shutdown_timeout(Duration::from_secs(1));
    }
    drop(lease);
    std::fs::remove_dir_all(data)?;
    Ok(())
}

fn render_case(
    case: &scenes::PreviewCase,
    app: &Arc<AppContext>,
    runtime: &Arc<tokio::runtime::Runtime>,
    lease: &Arc<EngineLease>,
    output: &Path,
) -> Result<()> {
    let language = if case.name.ends_with("-english") { "en-US" } else { "zh-CN" };
    let preferences = crate::runtime::UiPreferences {
        appearance: case.mode.name().into(),
        language: language.into(),
        ..Default::default()
    };
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::theme::init(cx);
        Theme::change(case.mode, None, cx);
        Theme::update(cx, |theme| theme.font_size *= case.font_scale);
    });
    let (handle, view) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: size(px(case.width), px(case.height)),
                })),
                focus: false,
                show: false,
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    FarmDesktop::new(
                        Arc::clone(app),
                        Arc::clone(runtime),
                        Arc::clone(lease),
                        None,
                        true,
                        preferences,
                        mpsc::unbounded().1,
                        window,
                        cx,
                    )
                })
            },
        )
    })?;
    cx.update_window(handle, |_, window, cx| {
        scenes::seed(&view, handle, case, window, cx);
        window.render_frame(cx);
    })?;
    cx.run_until_parked();
    cx.advance_clock(Duration::from_millis(200));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        scenes::arrange(case, &view, window, cx);
        window.render_frame(cx);
    })?;
    cx.run_until_parked();
    cx.advance_clock(Duration::from_millis(200));
    cx.capture_screenshot(handle)?.save(output.join(format!("{}.png", case.name)))?;
    Ok(())
}
