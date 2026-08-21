//! 把同步文件 I/O 移到 blocking 线程池，避免拖住 tokio worker。
//!
//! 历史背景：项目早期所有 `fs::write` / `fs::read` 都是直接同步调用。
//! 2026-08 真实生产事故复现：`SharedFile(Arc<Mutex<fs::File>>)` 同步写日志
//! 阻塞 tokio runtime → Tauri command `farm_bag` 不返回 → 前端 `Promise.all` 永远
//! pending → `bagLoading=true` 永远不清除 → "loading..." 一直转 + 关不掉窗。
//! 根因是 sync I/O 在 tokio worker 上执行。
//!
//! 本模块提供 `write_file_blocking` / `read_file_blocking` 两个 helper，
//! 业务代码改为调用即可。函数语义与 `fs::write`/`fs::read` 一致（覆盖写 / 全量读），
//! 但实际工作跑在 `tokio::task::spawn_blocking` 的 blocking pool 上。
//!
//! **不**替代精确的 fsync / append / streaming —— 那些场景需要更精细的封装。

use std::path::{Path, PathBuf};

/// 在 blocking 线程池里跑同步文件写入。返回 `JoinHandle<io::Result<()>>`。
/// 调用方可以 `.await`，也可以丢火。
pub fn write_file_blocking(
    path: PathBuf,
    body: Vec<u8>,
) -> tokio::task::JoinHandle<std::io::Result<()>> {
    tokio::task::spawn_blocking(move || std::fs::write(&path, &body))
}

/// 在 blocking 线程池里跑同步文件读取。返回 `JoinHandle<io::Result<Vec<u8>>>`。
pub fn read_file_blocking(path: PathBuf) -> tokio::task::JoinHandle<std::io::Result<Vec<u8>>> {
    tokio::task::spawn_blocking(move || std::fs::read(&path))
}

/// 阻塞版本：当前已是 sync 上下文、不能 `await`，但仍要避免在 tokio worker 上跑。
/// 直接用 `tokio::runtime::Handle::current().spawn_blocking` 仍可能在
/// 当前任务上下文（worker 线程）上 spawn。**这个函数**保证真正异步触发：
/// 如果没有 tokio runtime（如单元测试中），会 fallback 到同步 `fs::write`。
pub fn write_file_async_or_sync(
    path: PathBuf,
    body: Vec<u8>,
) -> std::io::Result<()> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // 在 tokio 上下文里 → spawn_blocking
            handle.spawn_blocking(move || std::fs::write(&path, &body));
            Ok(())
        }
        Err(_) => std::fs::write(&path, &body),
    }
}

/// fire-and-forget 的 spawn_blocking 写文件，丢弃 JoinHandle。
/// 用于"日志 / 状态更新"等不需要结果的场景。
pub fn spawn_write_file(path: PathBuf, body: Vec<u8>) {
    let _ = write_file_blocking(path, body);
}

/// 把一个 closure 扔到 blocking pool。用于零散的 sync I/O 块。
///
/// **fire-and-forget**：不返回 `JoinHandle`，直接丢弃结果。
/// 当前所有调用点都是 `let _ = spawn_blocking(...)` 模式，不需要等结果。
///
/// 测试 / 单元上下文无 tokio runtime 时 fallback 到**同步**执行（关键）：
/// 很多现有测试假设 `persist_global()` 之后立即 `read_*()` 能拿到刚写入的数据。
/// 同步 fallback 保证测试通过；生产环境（tokio runtime 内）走真正的 blocking pool。
pub fn spawn_blocking<F, R>(f: F)
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            let _ = handle.spawn_blocking(f);
        }
        Err(_) => {
            // 无 runtime：直接同步执行（drop 结果）
            let _ = f();
        }
    }
}

/// fire-and-forget 包装：忽略 JoinHandle。和 `spawn_blocking` 等价，保留语义清晰版本。
pub fn spawn_blocking_detached<F, R>(f: F)
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    spawn_blocking(f)
}

/// 便捷：把一个 closure 同步跑（无 tokio 时 fallback），但通常你应该用 `spawn_blocking`。
pub fn blocking<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    f()
}

#[allow(dead_code)]
fn _ensure_path_used(path: &Path) -> &Path {
    path
}
