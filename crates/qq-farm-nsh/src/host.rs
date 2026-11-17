use std::sync::Arc;
use std::time::Duration;

use qq_farm_app::runtime_lease::{EngineLease, EngineLeaseError};
use thiserror::Error;

use crate::session::{self, Session};

const MAX_WORKERS: usize = 16;
const GATEWAY_ORIGIN: &str = "https://game.qq.com";
const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);
const RUNTIME_WAIT: Duration = Duration::from_secs(1);

#[derive(Debug, Error)]
enum HostError {
    #[error("QQ Farm could not prepare its data directory: {0}")]
    Prepare(#[source] std::io::Error),
    #[error("{0}")]
    Lease(#[from] EngineLeaseError),
    #[error("QQ Farm could not create its runtime: {0}")]
    Runtime(#[source] std::io::Error),
}

struct Host {
    runtime: tokio::runtime::Runtime,
    lease: EngineLease,
}

pub fn run() -> impl std::process::Termination {
    match Host::boot() {
        Ok(host) => host.launch(),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

impl Host {
    fn boot() -> Result<Self, HostError> {
        prepare_environment().map_err(HostError::Prepare)?;
        let data_dir = qq_farm_core::config::get_data_dir();
        let lease = EngineLease::acquire_for(&data_dir, "nsh")?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("qq-farm-nsh-tokio")
            .build()
            .map_err(HostError::Runtime)?;
        let _enter = runtime.enter();
        let context = qq_farm_app::bootstrap::assemble_app_context(MAX_WORKERS, GATEWAY_ORIGIN);
        context.engine.schedule_wx_authorized_start();
        session::install(Session { context: Arc::new(context), runtime: runtime.handle().clone() });
        Ok(Self { runtime, lease })
    }

    fn launch(self) -> impl std::process::Termination {
        let termination = desktop_app();
        self.runtime
            .block_on(qq_farm_app::lifecycle::shutdown(&session::session().context, SHUTDOWN_WAIT));
        self.runtime.shutdown_timeout(RUNTIME_WAIT);
        drop(self.lease);
        termination
    }
}

fn prepare_environment() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    let data_dir = farm_data_dir();
    std::fs::create_dir_all(&data_dir)?;
    if std::env::var("FARM_DATA_DIR").ok().filter(|value| !value.is_empty()).is_none() {
        std::env::set_var("FARM_DATA_DIR", &data_dir);
    }
    let resources = resource_root();
    set_if_missing("QQ_FARM_ASSET_DIR", &resources);
    set_if_missing("FARM_GAME_CONFIG_DIR", &resources.join("assets/game_config"));
    set_if_missing("TSDK_WASM_PATH", &wasm_path(&resources, "tsdk.wasm"));
    set_if_missing("TSDK_WX_WASM_PATH", &wasm_path(&resources, "tsdk-wx.wasm"));
    Ok(())
}

fn farm_data_dir() -> std::path::PathBuf {
    std::env::var_os("FARM_DATA_DIR")
        .filter(|path| !path.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(platform_data_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("data"))
}

fn platform_data_dir() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        dirs::data_local_dir().map(|path| path.join("QQFarmRust"))
    }
    #[cfg(not(windows))]
    {
        dirs::data_dir().map(|path| path.join("QQFarmRust"))
    }
}

fn resource_root() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("QQ_FARM_ASSET_DIR") {
        return std::path::PathBuf::from(path);
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(binary_dir) = executable.parent() {
            #[cfg(target_os = "macos")]
            {
                let bundle_resources = binary_dir.join("../Resources");
                if bundle_resources.join("assets/game_config").is_dir() {
                    return bundle_resources;
                }
            }
            if binary_dir.join("assets/game_config").is_dir() {
                return binary_dir.to_path_buf();
            }
        }
    }
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn wasm_path(resources: &std::path::Path, filename: &str) -> std::path::PathBuf {
    let packaged = resources.join(filename);
    if packaged.is_file() {
        packaged
    } else {
        resources.join("assets").join(filename)
    }
}

fn set_if_missing(name: &str, value: &std::path::Path) {
    if std::env::var(name).ok().filter(|value| !value.is_empty()).is_none() {
        std::env::set_var(name, value);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn desktop_app() -> impl std::process::Termination {
    #[cfg(target_os = "linux")]
    use nsh_platform_linux as platform;
    #[cfg(target_os = "macos")]
    use nsh_platform_macos as platform;
    #[cfg(target_os = "windows")]
    use nsh_platform_windows as platform;

    platform::run_app(
        platform::AppSource::Rust(platform::Root::of::<crate::App>().app_id("com.qqfarm.rust")),
        platform::AppOptions {
            title: "QQ Farm".to_owned(),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            app_id: "com.qqfarm.rust".to_owned(),
            size: (1280, 840),
            ..Default::default()
        },
    )
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn desktop_app() -> impl std::process::Termination {
    std::process::ExitCode::from(2)
}
