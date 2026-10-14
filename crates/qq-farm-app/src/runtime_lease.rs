//! 跨桌面宿主的运行时互斥锁。

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use thiserror::Error;

const WAKE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
pub enum EngineLeaseError {
    #[error("QQ Farm is already running with this data directory")]
    AlreadyRunning,
    #[error("could not lock the QQ Farm data directory: {0}")]
    Io(#[from] std::io::Error),
}

/// 进程持有此对象时，其数据目录不会被另一桌面宿主同时打开。
pub struct EngineLease {
    _file: File,
    path: PathBuf,
    wake_receiver: Mutex<Option<mpsc::Receiver<()>>>,
    wake_shutdown: Arc<AtomicBool>,
    wake_thread: Option<JoinHandle<()>>,
}

impl EngineLease {
    pub fn acquire(data_dir: &Path) -> Result<Self, EngineLeaseError> {
        Self::acquire_for(data_dir, "unknown")
    }

    /// Acquire the shared data directory lease for a named desktop host.
    pub fn acquire_for(data_dir: &Path, owner: &str) -> Result<Self, EngineLeaseError> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("engine.lock");
        let mut file =
            OpenOptions::new().create(true).truncate(false).read(true).write(true).open(&path)?;
        match file.try_lock() {
            Ok(()) => {
                let listener = TcpListener::bind(("127.0.0.1", 0))?;
                listener.set_nonblocking(true)?;
                let port = listener.local_addr()?.port();
                let token = uuid::Uuid::new_v4().to_string();
                file.set_len(0)?;
                file.seek(SeekFrom::Start(0))?;
                writeln!(&file, "{port} {token} {owner}")?;
                file.sync_all()?;

                let (wake_sender, wake_receiver) = mpsc::channel();
                let wake_shutdown = Arc::new(AtomicBool::new(false));
                let shutdown = Arc::clone(&wake_shutdown);
                let wake_thread = thread::Builder::new()
                    .name("qq-farm-single-instance".into())
                    .spawn(move || {
                        while !shutdown.load(Ordering::Relaxed) {
                            match listener.accept() {
                                Ok((mut stream, _)) => {
                                    let _ = stream.set_read_timeout(Some(WAKE_TIMEOUT));
                                    let mut request = String::new();
                                    let accepted = stream.read_to_string(&mut request).is_ok()
                                        && request.trim() == token
                                        && wake_sender.send(()).is_ok();
                                    let _ = stream.write_all(if accepted { b"ok" } else { b"no" });
                                }
                                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                    thread::sleep(Duration::from_millis(35));
                                }
                                Err(_) => break,
                            }
                        }
                    })?;
                Ok(Self {
                    _file: file,
                    path,
                    wake_receiver: Mutex::new(Some(wake_receiver)),
                    wake_shutdown,
                    wake_thread: Some(wake_thread),
                })
            }
            Err(std::fs::TryLockError::WouldBlock) => Err(EngineLeaseError::AlreadyRunning),
            Err(std::fs::TryLockError::Error(error)) => Err(EngineLeaseError::Io(error)),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Take the receiver for launch requests from another desktop process.
    pub fn take_wake_receiver(&self) -> Option<mpsc::Receiver<()>> {
        self.wake_receiver.lock().ok()?.take()
    }

    /// Signal the process that currently owns this data directory's lease.
    /// Returns `true` when its local wake endpoint accepted the request.
    pub fn wake_existing(data_dir: &Path) -> bool {
        Self::wake_existing_owner(data_dir).is_some()
    }

    /// Wake the owning host and return its owner identifier when successful.
    pub fn wake_existing_owner(data_dir: &Path) -> Option<String> {
        let Ok(metadata) = std::fs::read_to_string(data_dir.join("engine.lock")) else {
            return None;
        };
        let mut parts = metadata.split_whitespace();
        let (Some(port), Some(token)) = (parts.next(), parts.next()) else { return None };
        let owner = parts.next().unwrap_or("unknown").to_string();
        let Ok(port) = port.parse::<u16>() else { return None };
        let Ok(mut stream) = TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
            WAKE_TIMEOUT,
        ) else {
            return None;
        };
        if stream.set_write_timeout(Some(WAKE_TIMEOUT)).is_err()
            || stream.write_all(token.as_bytes()).is_err()
            || stream.shutdown(std::net::Shutdown::Write).is_err()
        {
            return None;
        }
        stream.set_read_timeout(Some(WAKE_TIMEOUT)).ok()?;
        let mut response = [0; 2];
        stream.read_exact(&mut response).ok()?;
        (response == *b"ok").then_some(owner)
    }
}

impl Drop for EngineLease {
    fn drop(&mut self) {
        self.wake_shutdown.store(true, Ordering::Relaxed);
        if let Some(thread) = self.wake_thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::time::Instant;

    #[test]
    fn shared_lease_rejects_a_second_process_and_confirms_each_same_host_wake() {
        let dir = std::env::temp_dir().join(format!("qq-farm-lease-{}", uuid::Uuid::new_v4()));
        let first = EngineLease::acquire_for(&dir, "gpui").expect("first lease");
        assert!(matches!(EngineLease::acquire(&dir), Err(EngineLeaseError::AlreadyRunning)));
        let receiver_guard = first.wake_receiver.lock().expect("wake receiver lock");
        let receiver = receiver_guard.as_ref().expect("wake receiver");
        let metadata = std::fs::read_to_string(dir.join("engine.lock")).expect("lease metadata");
        let port = metadata.split_whitespace().next().expect("port").parse::<u16>().unwrap();
        let mut invalid = TcpStream::connect(("127.0.0.1", port)).expect("wake endpoint");
        invalid.write_all(b"invalid-token").expect("send invalid token");
        invalid.shutdown(std::net::Shutdown::Write).expect("finish invalid request");
        let mut response = [0; 2];
        invalid.read_exact(&mut response).expect("invalid request response");
        assert_eq!(&response, b"no");
        assert!(receiver.try_recv().is_err(), "an invalid token must not wake the owner");

        assert_eq!(EngineLease::wake_existing_owner(&dir).as_deref(), Some("gpui"));
        receiver.recv_timeout(Duration::from_secs(1)).expect("receive wake request");
        assert_eq!(EngineLease::wake_existing_owner(&dir).as_deref(), Some("gpui"));
        receiver.recv_timeout(Duration::from_secs(1)).expect("receive repeated same-host wake");
        drop(receiver_guard);
        drop(first);
        assert!(EngineLease::acquire(&dir).is_ok());
        std::fs::remove_dir_all(dir).expect("remove test directory");
    }

    #[test]
    fn gpui_and_tauri_owners_cannot_run_the_same_data_directory() {
        let dir =
            std::env::temp_dir().join(format!("qq-farm-client-lease-{}", uuid::Uuid::new_v4()));

        let tauri = EngineLease::acquire_for(&dir, "tauri").expect("Tauri lease");
        assert!(matches!(
            EngineLease::acquire_for(&dir, "gpui"),
            Err(EngineLeaseError::AlreadyRunning)
        ));
        assert_eq!(EngineLease::wake_existing_owner(&dir).as_deref(), Some("tauri"));

        drop(tauri);
        let gpui = EngineLease::acquire_for(&dir, "gpui").expect("GPUI lease after Tauri exits");
        assert!(matches!(
            EngineLease::acquire_for(&dir, "tauri"),
            Err(EngineLeaseError::AlreadyRunning)
        ));
        assert_eq!(EngineLease::wake_existing_owner(&dir).as_deref(), Some("gpui"));

        drop(gpui);
        std::fs::remove_dir_all(dir).expect("remove test directory");
    }

    #[test]
    fn operating_system_releases_the_data_directory_lock_after_process_exit() {
        const CHILD_DIR: &str = "QQ_FARM_ENGINE_LEASE_CHILD_DIR";
        let dir =
            std::env::temp_dir().join(format!("qq-farm-lease-child-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create child lease directory");
        let ready = dir.join("child.ready");
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "runtime_lease::tests::lease_child_holds_directory_until_process_exit",
                "--nocapture",
            ])
            .env(CHILD_DIR, &dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start child lease process");

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut child_acquired = false;
        while Instant::now() < deadline {
            if ready.exists() {
                child_acquired = true;
                break;
            }
            if child.try_wait().expect("check child process").is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let lock_is_held = child_acquired
            && matches!(EngineLease::acquire(&dir), Err(EngineLeaseError::AlreadyRunning));
        if child.try_wait().expect("check child process before stopping").is_none() {
            let _ = child.kill();
        }
        let _ = child.wait();

        let parent_can_acquire = EngineLease::acquire(&dir).is_ok();
        assert!(child_acquired, "child process did not acquire the lease");
        assert!(lock_is_held, "a second process acquired a live child's lease");
        assert!(parent_can_acquire, "OS did not release the child's lock after exit");
        std::fs::remove_dir_all(dir).expect("remove child lease directory");
    }

    #[test]
    fn lease_child_holds_directory_until_process_exit() {
        let Ok(path) = std::env::var("QQ_FARM_ENGINE_LEASE_CHILD_DIR") else { return };
        let dir = PathBuf::from(path);
        let _lease = EngineLease::acquire_for(&dir, "lease-child").expect("child lease");
        std::fs::write(dir.join("child.ready"), "ready").expect("signal lease readiness");
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }
}
