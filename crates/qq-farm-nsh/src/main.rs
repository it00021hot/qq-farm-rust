#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() -> impl std::process::Termination {
    env_logger::init();
    qq_farm_nsh::run()
}
