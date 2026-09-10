//! 微信 TSDK (`tsdk.wasm`) 封装。
//!
//! # 指针所有权约定（务必遵守，违者会破坏 wasm 堆导致 64MB 内存耗尽）
//!
//! wasm 内存是**固定 1029 页（67,436,544 字节）**，不可增长；内部堆由 wasm 自己管理。
//! host 侧与 wasm 交换指针时分三类：
//!
//! | 指针来源 | 所有权 | host 义务 |
//! |---|---|---|
//! | `A(len)` 返回的指针 | host 临时借用 | 用完**必须** `B(ptr)` 释放（`AllocGuard` 负责兜底） |
//! | `N(lengthPtr)` 返回的 data_ptr | wasm 内部队列 | **绝不 free**（2026-08 曾误 free 导致堆损坏，1.5h 内 64MB 耗尽） |
//! | `H()` 返回的字符串指针 | wasm 内部 | 只读，**不 free**（对齐 node `tsdk-runtime.ts`） |
//!
//! 判定原则：凡不是 `A()` 分配的指针，一律不得传给 `B()`。node 版
//! `tsdk-runtime.ts` 是唯一行为基准，改动此处前先对照它。
//!
//! 用 `wasmtime` 加载 157KB 的 `tsdk.wasm`，提供与原 Node.js 版本对齐的：
//! - 初始化（host function 注入 + merged data 解密）
//! - `transform` 加密/解密
//!
//! ## 阶段 0 范围
//!
//! 仅保证 `transform(encrypt)` + `transform(decrypt)` 往返一致。bind_user /
//! heartbeat / 握手 等接口留到阶段 1 业务迁移时再补完。

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use anyhow::anyhow;
use wasmtime::{Config, Engine, Func, Instance, Linker, Memory, Module, Store, Val};

use crate::error::{Error, Result};

/// wasmtime 闭包返回类型（避免与本 crate 的 `Result` 冲突）
type WasmResult<T> = std::result::Result<T, wasmtime::Error>;

/// 在 [`WasmResult`] 上下文构造错误。
///
/// wasmtime 48 起 `wasmtime::Error` 不再是从 `anyhow::Error` 的透明别名
/// （trait 一致性不允许 `From<anyhow::Error>`），须显式走 [`wasmtime::Error::from_anyhow`]。
macro_rules! werr {
    ($($arg:tt)*) => {
        wasmtime::Error::from_anyhow(anyhow!($($arg)*))
    };
}

/// wasm 单次回灌的合理上限（与 Node `readCString` 默认 `maxLength=64 * 1024` 对齐）
pub const MAX_SANE_LEN: usize = 64 * 1024;

/// 同一 wasm 错误连续出现 N 次即触发 [`TsdkRuntime::request_reset`]
pub const WASM_CONSECUTIVE_FAIL_THRESHOLD: u32 = 3;

// ===== TSDK 元信息（与原项目保持一致） =====

const TSDK_VERSION: &str = "v3.9.0.1788165223";
const MINI_PROGRAM_APP_ID: &str = "wx5306c5978fdb76e4";
/// QQ 小程序 App ID（QQ 平台宿主初始化，对齐 bot `MINI_PROGRAM_APP_IDS.qq`）
const QQ_MINI_PROGRAM_APP_ID: &str = "1112386029";
const TSDK_GAME_ID: u32 = 3167;
const TSDK_APP_KEY: &str = "0";
const MERGED_DATA_KEY: u32 = 1_871_261_153;
/// QQ 宿主用户目录（QQ 平台 dataDir 回调返回它而不是本地目录）
const QQ_USER_DATA_PATH: &str = "qqfile://usr/";
/// QQ 桌面端设备文本（QQ 平台 deviceText 回调固定值）
const QQ_DEVICE_TEXT: &str = "windows;windows;windows 10.0;0;";
/// QQ 宿主特征状态（wasm 数据段内 current/reference 两块 64 字节，
/// Node 宿主在 index 1 处与官方客户端差 1，发送 init token 前须归一）
const QQ_HOST_CURRENT_PTR: usize = 17_288;
const QQ_HOST_REFERENCE_PTR: usize = 17_352;
const QQ_HOST_STATE_LENGTH: usize = 64;
const QQ_HOST_NODE_MISMATCH_INDEX: usize = 1;

/// 平台宿主画像（对齐 bot `resolveTsdkHostProfile`）：
/// QQ 账号走 QQ 宿主（App ID/设备文本/用户目录/非调试），其余默认微信宿主。
#[derive(Clone)]
struct HostProfile {
    app_id: &'static str,
    debug_mode: i32,
    device_text: Option<&'static str>,
    user_data_path: Option<&'static str>,
    platform: &'static str,
}

impl Default for HostProfile {
    fn default() -> Self {
        Self {
            app_id: MINI_PROGRAM_APP_ID,
            debug_mode: 2,
            device_text: None,
            user_data_path: None,
            platform: "wx",
        }
    }
}

impl HostProfile {
    fn resolve(platform: &str) -> Self {
        let normalized = platform.trim().to_ascii_lowercase();
        if normalized == "qq" {
            Self {
                app_id: QQ_MINI_PROGRAM_APP_ID,
                debug_mode: 0,
                device_text: Some(QQ_DEVICE_TEXT),
                user_data_path: Some(QQ_USER_DATA_PATH),
                platform: "qq",
            }
        } else {
            Self::default()
        }
    }
}

/// QQ 宿主特征状态归一（对齐 bot `normalizeQqHostFeatureState`）：
/// wasm 数据段内 current/reference 两块 64 字节只允许在 index 1 处相差 1（Node 宿主特征），
/// 发送 init token 前把它归一为官方客户端值；布局与已验证版本不符则报错。
fn normalize_qq_host_feature_state(store: &mut Store<HostState>, memory: &Memory) -> Result<()> {
    let data = memory.data_mut(&mut *store);
    let end = QQ_HOST_REFERENCE_PTR + QQ_HOST_STATE_LENGTH;
    if QQ_HOST_CURRENT_PTR + QQ_HOST_STATE_LENGTH > data.len() || end > data.len() {
        return Err(Error::crypto("TSDK QQ 宿主特征区越界"));
    }
    let mismatches: Vec<usize> = (0..QQ_HOST_STATE_LENGTH)
        .filter(|&i| data[QQ_HOST_CURRENT_PTR + i] != data[QQ_HOST_REFERENCE_PTR + i])
        .collect();
    if mismatches.is_empty() {
        return Ok(());
    }
    let idx = QQ_HOST_NODE_MISMATCH_INDEX;
    if mismatches.len() != 1
        || mismatches[0] != idx
        || data[QQ_HOST_CURRENT_PTR + idx] != data[QQ_HOST_REFERENCE_PTR + idx].wrapping_add(1)
    {
        return Err(Error::crypto("TSDK QQ 宿主特征布局与已验证版本不一致"));
    }
    data[QQ_HOST_CURRENT_PTR + idx] = data[QQ_HOST_REFERENCE_PTR + idx];
    Ok(())
}

/// Runtime table (59 bytes)
const RUNTIME_TABLE: [u8; 59] = [
    93, 86, 110, 34, 65, 129, 8, 113, 53, 192, 121, 32, 86, 162, 255, 139, 217, 70, 223, 0, 45,
    176, 85, 103, 234, 116, 120, 194, 206, 7, 176, 222, 56, 6, 161, 159, 154, 231, 93, 229, 39,
    107, 197, 136, 167, 52, 155, 228, 209, 117, 218, 8, 107, 241, 32, 62, 53, 200, 238,
];

/// Merged data segments: (offset, length) —— 17 段
const MERGED_DATA_SEGMENTS: &[(u32, u32)] = &[
    (1024, 5541),
    (6580, 8989),
    (15585, 33),
    (15643, 1),
    (15655, 21),
    (15701, 1),
    (15713, 21),
    (15759, 1),
    (15771, 30),
    (15826, 14),
    (15875, 1),
    (15887, 21),
    (15933, 1),
    (15945, 671),
    (16632, 400),
    (17040, 103),
    (67_371_008, 404),
];

// ===== Store 状态 =====

/// Store 持有的 host 端数据
#[derive(Default)]
struct HostState {
    /// TSDK 内存（实例化后回填）
    memory: Option<Memory>,
    /// 数据目录
    data_dir: String,
    /// 平台宿主画像（QQ / 微信）
    host: HostProfile,
}

// ===== Engine 单例 =====

static ENGINE: OnceLock<Engine> = OnceLock::new();

fn shared_engine() -> Result<&'static Engine> {
    if let Some(e) = ENGINE.get() {
        return Ok(e);
    }
    let mut config = Config::new();
    config.wasm_multi_memory(false);
    config.wasm_multi_value(true);
    match wasmtime::Cache::new(wasmtime::CacheConfig::new()) {
        Ok(cache) => {
            config.cache(Some(cache));
        }
        Err(e) => tracing::warn!(error = %e, "TSDK wasm 编译缓存不可用"),
    }
    let engine =
        Engine::new(&config).map_err(|e| Error::crypto(format!("create engine failed: {e}")))?;
    let _ = ENGINE.set(engine);
    Ok(ENGINE.get().expect("engine initialized"))
}

static MODULE: OnceLock<Module> = OnceLock::new();

fn shared_module(engine: &Engine, wasm_path: &Path) -> Result<&'static Module> {
    if let Some(m) = MODULE.get() {
        return Ok(m);
    }
    let module = Module::from_file(engine, wasm_path)
        .map_err(|e| Error::crypto(format!("load wasm failed: {e}")))?;
    let _ = MODULE.set(module);
    Ok(MODULE.get().expect("module initialized"))
}

// ===== TSDK 运行时 =====

/// TSDK 单实例。每个账号对应一个 runtime。
pub struct TsdkRuntime {
    data_dir: String,
    /// 平台宿主画像（QQ / 微信）
    host_profile: HostProfile,
    /// wasm 文件路径（`init` 时填充，`rebuild` 用它重新实例化）
    wasm_path: parking_lot::Mutex<Option<std::path::PathBuf>>,
    /// 上次成功 bind_user 的 open_id（`rebuild` 用它重绑用户）
    last_open_id: parking_lot::Mutex<Option<String>>,
    inner: parking_lot::Mutex<Option<Inner>>,
    /// 当前 TSDK 调用是否处于失败累计阶段。
    /// 任一 `call_wasm_*` helper 失败 +1，成功清零；达到 [`WASM_CONSECUTIVE_FAIL_THRESHOLD`] 时
    /// 由 worker 调度器观察 [`TsdkRuntime::pending_reset`] 并重建 runtime。
    consecutive_fail_count: AtomicU32,
    /// 是否已请求重置。被 worker 拉起重置后清零。
    /// 在 pending_reset 期间所有 host-side helper 直接返回 `Err(WasmResetPending)`。
    pending_reset: AtomicBool,
}

struct Inner {
    store: Store<HostState>,
    #[allow(dead_code)]
    instance: Instance,
    exports: Exports,
    /// 是否已 bindUser
    user_bound: bool,
}

struct Exports {
    memory: Memory,
    alloc: Func,
    free: Func,
    x: Func,
    g: Func,
    encrypt: Func,
    decrypt: Func,
    decrypt_strings: Func,
    // === ACE 协议接口 ===
    /// `H()` → ptr (string)
    h: Func,
    /// `M()` → heartbeat tick
    m: Func,
    /// `P()` → process received data
    p: Func,
    /// `E()` → send status
    e: Func,
    /// `O(ptr, length)` → send data from server
    o: Func,
    /// `fa(elapsedMs)` → detect speed hack
    fa: Func,
    /// `N(lengthPtr)` → ptr (with length written to lengthPtr); used by getDataToServer
    n: Func,
}

impl TsdkRuntime {
    /// 创建 runtime（不加载 wasm）；默认微信宿主，QQ 账号用 [`Self::for_platform`]。
    #[must_use]
    pub fn new(data_dir: impl Into<String>) -> Self {
        Self::for_platform(data_dir, "wx")
    }

    /// 按账号平台创建 runtime（`qq` → QQ 宿主，其余 → 微信宿主）。
    #[must_use]
    pub fn for_platform(data_dir: impl Into<String>, platform: &str) -> Self {
        Self {
            data_dir: data_dir.into(),
            host_profile: HostProfile::resolve(platform),
            wasm_path: parking_lot::Mutex::new(None),
            last_open_id: parking_lot::Mutex::new(None),
            inner: parking_lot::Mutex::new(None),
            consecutive_fail_count: AtomicU32::new(0),
            pending_reset: AtomicBool::new(false),
        }
    }

    /// 便捷构造：创建 + 初始化（微信宿主）
    pub fn load(wasm_path: &Path, data_dir: impl Into<String>) -> Result<Self> {
        Self::load_for_platform(wasm_path, data_dir, "wx")
    }

    /// 便捷构造：创建 + 初始化（按账号平台选择宿主）
    pub fn load_for_platform(
        wasm_path: &Path,
        data_dir: impl Into<String>,
        platform: &str,
    ) -> Result<Self> {
        let rt = Self::for_platform(data_dir, platform);
        let start = Instant::now();
        rt.init(wasm_path)?;
        tracing::info!(
            elapsed_ms = start.elapsed().as_millis() as u64,
            version = TSDK_VERSION,
            platform = rt.host_profile.platform,
            "TSDK 初始化完成"
        );
        Ok(rt)
    }

    /// 记录一次 wasm 调用的成功结果（清零失败计数）。
    pub fn record_wasm_success(&self) {
        self.consecutive_fail_count.store(0, Ordering::Release);
    }

    /// 记录一次 wasm 调用的失败结果；返回 `true` 表示已连续失败达到阈值，需要调用 [`Self::request_reset`]。
    pub fn record_wasm_failure(&self) -> bool {
        let n = self.consecutive_fail_count.fetch_add(1, Ordering::AcqRel) + 1;
        n >= WASM_CONSECUTIVE_FAIL_THRESHOLD
    }

    /// 当前连续失败计数（用于诊断）
    #[must_use]
    pub fn consecutive_fail_count(&self) -> u32 {
        self.consecutive_fail_count.load(Ordering::Acquire)
    }

    /// 是否已请求重置
    #[must_use]
    pub fn is_reset_pending(&self) -> bool {
        self.pending_reset.load(Ordering::Acquire)
    }

    /// 请求重置。下次 host 侧调用会先观察该标志。worker 重建完成后必须调 [`Self::mark_reset_completed`]。
    pub fn request_reset(&self) {
        self.pending_reset.store(true, Ordering::Release);
    }

    /// worker 重建 wasm 完成后调用，清零 reset 状态和失败计数。
    pub fn mark_reset_completed(&self) {
        self.consecutive_fail_count.store(0, Ordering::Release);
        self.pending_reset.store(false, Ordering::Release);
    }

    /// 同步阻塞地销毁旧 runtime 并用同一个 wasm path + data_dir 重建一个新的。
    /// 重建完成后旧的所有 wasm 内存（heap + Stack）随旧 Store 析构，腾出 RSS。
    ///
    /// 调用方必须在 `tokio::task::spawn_blocking` 里跑（涉及 wasm 编译 + 实例化 ~12ms 起步）。
    /// 调用前必须先 `Gateway::begin_rebuild()` 让 WorkerLoop 放宽 silence 阈值。
    /// 调用后必须：
    /// 1. `Gateway::replace_encryptor(new TsdkEncryptor(rebuilt))`
    /// 2. `Gateway::end_rebuild()`
    /// 3. 重启 AceShared
    pub fn rebuild(&self) -> Result<()> {
        // 复制一份路径和 open_id（不持锁跨函数）
        let wasm_path = self.wasm_path.lock().clone();
        let open_id = self.last_open_id.lock().clone();
        let wasm_path = wasm_path.ok_or_else(|| {
            Error::crypto("rebuild 失败：从未 init 过 TSDK，没有 wasm_path".to_string())
        })?;

        // 1. 销毁旧 runtime（旧 Inner.drop 触发 wasm Store 析构 → 释放 wasm 内存）
        self.destroy();
        // 2. 重新 init（重新编译 wasm、重新解密 merged data、重新创建 Store）
        self.init(&wasm_path)?;
        // 3. 重新 bind_user（让 wasm 内部 state 绑定到当前账号）
        if let Some(open_id) = open_id.as_deref() {
            if !open_id.is_empty() {
                // 直接调内部方法（避免再走一次 pending_reset 短路检查）
                self.bind_user_inner(open_id)?;
            }
        }
        // 4. 清零 reset 状态和失败计数
        self.mark_reset_completed();
        Ok(())
    }

    /// `bind_user` 的内部版本：跳过 `pending_reset` 短路检查。
    /// 仅 `rebuild` 内部使用（rebuild 期间其他 host-side 调用被强制短路，
    /// 但 rebuild 自己必须能 bind_user）。
    fn bind_user_inner(&self, open_id: &str) -> Result<()> {
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        if inner.user_bound {
            return Ok(());
        }
        let store = &mut inner.store;
        let exports = &inner.exports;

        let cstr = format!("{open_id}\0");
        let cap = (cstr.len() as i32).max(64);
        let alloc_result = AllocGuard::alloc(store, exports, cap);
        let mut ptr_guard = alloc_result?;
        let ptr = ptr_guard.ptr();
        if let Err(e) = write_bytes(store, &exports.memory, ptr, cstr.as_bytes()) {
            ptr_guard.free_now(store, exports);
            return Err(e);
        }
        if let Err(e) = exports
            .g
            .call(&mut *store, &[Val::I32(TSDK_GAME_ID as i32), Val::I32(ptr)], &mut [])
            .map_err(|e| Error::crypto(format!("G(bind) failed: {e}")))
        {
            ptr_guard.free_now(store, exports);
            return Err(e);
        }
        ptr_guard.free_now(store, exports);
        inner.user_bound = true;
        *self.last_open_id.lock() = Some(open_id.to_string());
        Ok(())
    }

    /// 初始化
    pub fn init(&self, wasm_path: &Path) -> Result<()> {
        if self.inner.lock().is_some() {
            return Ok(());
        }
        let engine = shared_engine()?;
        let module = shared_module(engine, wasm_path)?;

        let host = HostState {
            data_dir: self.data_dir.clone(),
            host: self.host_profile.clone(),
            ..Default::default()
        };
        let mut store = Store::new(engine, host);
        let linker = create_linker(engine)?;
        let instance = linker
            .instantiate(&mut store, module)
            .map_err(|e| Error::crypto(format!("instantiate failed: {e}")))?;

        let exports = extract_exports(&instance, &mut store)?;
        store.data_mut().memory = Some(exports.memory);

        // 校验 merged data 段范围
        let mem_size = exports.memory.data(&store).len();
        for (offset, length) in MERGED_DATA_SEGMENTS {
            let end = (*offset as usize).saturating_add(*length as usize);
            if end > mem_size {
                return Err(Error::crypto(format!(
                    "merged data segment out of bounds: offset={offset}, length={length}, mem={mem_size}"
                )));
            }
        }
        // 解密 merged data
        for (offset, length) in MERGED_DATA_SEGMENTS {
            exports
                .decrypt_strings
                .call(
                    &mut store,
                    &[
                        Val::I32(*offset as i32),
                        Val::I32(*length as i32),
                        Val::I32(MERGED_DATA_KEY as i32),
                    ],
                    &mut [],
                )
                .map_err(|e| Error::crypto(format!("decrypt_strings failed: {e}")))?;
        }

        // 调 x() 初始化
        exports
            .x
            .call(&mut store, &[], &mut [])
            .map_err(|e| Error::crypto(format!("init x() failed: {e}")))?;

        // 设置 game id + app key
        let mut app_key_guard =
            AllocGuard::alloc(&mut store, &exports, TSDK_APP_KEY.len() as i32 + 1)?;
        let app_key_ptr = app_key_guard.ptr();
        write_cstring(&mut store, &exports.memory, app_key_ptr, TSDK_APP_KEY.as_bytes())?;
        exports
            .g
            .call(&mut store, &[Val::I32(TSDK_GAME_ID as i32), Val::I32(app_key_ptr)], &mut [])
            .map_err(|e| Error::crypto(format!("G(gameId) failed: {e}")))?;
        app_key_guard.free_now(&mut store, &exports);

        *self.inner.lock() = Some(Inner { store, instance, exports, user_bound: false });
        // 记录 wasm 路径供 rebuild 使用
        *self.wasm_path.lock() = Some(wasm_path.to_path_buf());
        Ok(())
    }

    /// 加密
    pub fn encrypt(&self, input: &[u8]) -> Result<Vec<u8>> {
        self.transform(input, false)
    }

    /// 解密
    pub fn decrypt(&self, input: &[u8]) -> Result<Vec<u8>> {
        self.transform(input, true)
    }

    /// 加解密主逻辑
    pub fn transform(&self, input: &[u8], decrypt: bool) -> Result<Vec<u8>> {
        if self.pending_reset.load(Ordering::Acquire) {
            return Err(Error::crypto("TSDK 已请求重置，等待 worker 重建"));
        }
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;

        let store = &mut inner.store;
        let exports = &inner.exports;

        // 诊断：alloc failed 往往意味着 wasm 内存持续增长
        let memory_bytes = exports.memory.data(&*store).len();
        tracing::debug!(decrypt, input_len = input.len(), memory_bytes, "tsdk transform begin");

        // 1. alloc（用 guard 保证后续所有 early-return 都会 free）
        let len = (input.len().max(1)) as i32;
        let alloc_result = AllocGuard::alloc(store, exports, len);
        let mut ptr_guard = match alloc_result {
            Ok(g) => g,
            Err(e) => {
                if self.record_wasm_failure() {
                    self.request_reset();
                    tracing::error!(
                        "TSDK alloc 连续失败达到 {} 次，请求 wasm 重建",
                        self.consecutive_fail_count()
                    );
                }
                return Err(e);
            }
        };
        let ptr = ptr_guard.ptr();

        // 2. 写入
        if let Err(e) = write_bytes(store, &exports.memory, ptr, input) {
            ptr_guard.free_now(store, exports);
            if self.record_wasm_failure() {
                self.request_reset();
            }
            return Err(e);
        }

        // 3. 加密/解密
        let func = if decrypt { &exports.decrypt } else { &exports.encrypt };
        let enc_res = func
            .call(&mut *store, &[Val::I32(ptr), Val::I32(input.len() as i32)], &mut [])
            .map_err(|e| {
                Error::crypto(format!(
                    "{} failed: {e}",
                    if decrypt { "decrypt" } else { "encrypt" }
                ))
            });
        if let Err(e) = enc_res {
            ptr_guard.free_now(store, exports);
            if self.record_wasm_failure() {
                self.request_reset();
            }
            return Err(e);
        }

        // 4. 读出
        let result = match read_bytes(store, &exports.memory, ptr, input.len()) {
            Ok(r) => r,
            Err(e) => {
                ptr_guard.free_now(store, exports);
                if self.record_wasm_failure() {
                    self.request_reset();
                }
                return Err(e);
            }
        };

        // 5. 释放（成功路径；guard free 失败仅记日志）
        ptr_guard.free_now(store, exports);
        self.record_wasm_success();
        Ok(result)
    }

    // =============================================================
    // === ACE 协议接口（1:1 对应原 tsdk-runtime.ts）==================
    // =============================================================

    /// 绑定 openid 到 TSDK（不可重复绑定）
    /// 原 TS: `bindUser(openId: string)`
    pub fn bind_user(&self, open_id: &str) -> Result<()> {
        let value = open_id.trim();
        if value.is_empty() {
            return Ok(());
        }
        if self.pending_reset.load(Ordering::Acquire) {
            return Err(Error::crypto("TSDK 已请求重置，等待 worker 重建"));
        }
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        if inner.user_bound {
            return Ok(());
        }
        let store = &mut inner.store;
        let exports = &inner.exports;

        // alloc cstring → 写 openid → G(game_id, ptr)（对齐 bot allocCString：精确 len+1）
        let cstr = format!("{value}\0");
        let cap = cstr.len() as i32;
        let alloc_result = AllocGuard::alloc(store, exports, cap);
        let mut ptr_guard = match alloc_result {
            Ok(g) => g,
            Err(e) => {
                if self.record_wasm_failure() {
                    self.request_reset();
                }
                return Err(e);
            }
        };
        let ptr = ptr_guard.ptr();
        if let Err(e) = write_bytes(store, &exports.memory, ptr, cstr.as_bytes()) {
            ptr_guard.free_now(store, exports);
            if self.record_wasm_failure() {
                self.request_reset();
            }
            return Err(e);
        }

        // 调 G(game_id, app_key_ptr) —— 把用户绑到 wasm
        if let Err(e) = exports
            .g
            .call(&mut *store, &[Val::I32(TSDK_GAME_ID as i32), Val::I32(ptr)], &mut [])
            .map_err(|e| Error::crypto(format!("G(bind) failed: {e}")))
        {
            ptr_guard.free_now(store, exports);
            if self.record_wasm_failure() {
                self.request_reset();
            }
            return Err(e);
        }

        ptr_guard.free_now(store, exports);
        self.record_wasm_success();
        inner.user_bound = true;
        // 记录 open_id 供 rebuild 使用
        *self.last_open_id.lock() = Some(value.to_string());
        Ok(())
    }

    /// 拿加密的 init info（base64 字符串，原 TS 调 H()）。
    /// QQ 宿主在发送前须归一宿主特征状态（对齐 bot `normalizeQqHostFeatureState`）。
    pub fn get_encrypted_init_info(&self) -> Result<String> {
        if self.pending_reset.load(Ordering::Acquire) {
            return Err(Error::crypto("TSDK 已请求重置，等待 worker 重建"));
        }
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        let exports = &inner.exports;
        if self.host_profile.platform == "qq" {
            normalize_qq_host_feature_state(store, &exports.memory)?;
        }

        let mut ret = [Val::I32(0); 1];
        if let Err(e) = exports
            .h
            .call(&mut *store, &[], &mut ret)
            .map_err(|e| Error::crypto(format!("H() failed: {e}")))
        {
            if self.record_wasm_failure() {
                self.request_reset();
            }
            return Err(e);
        }
        let ptr = i32_val(&ret, 0)?;
        if ptr == 0 {
            self.record_wasm_success();
            return Ok(String::new());
        }
        let result = read_cstring(store, &exports.memory, ptr, MAX_SANE_LEN);
        if result.is_err() && self.record_wasm_failure() {
            self.request_reset();
        } else {
            self.record_wasm_success();
        }
        result
    }

    /// 拿到要发给服务器的数据（ACE AntiDataRequest.data）
    /// 原 TS: `getDataToServer(): Buffer`
    /// 实现:  alloc 4 字节 lengthPtr → N(lengthPtr) → 读 ptr + length
    ///
    /// **与 Node 行为对齐**：
    /// - `data_ptr <= 0` 或 wasm 写入的 `length <= 0` → 返回 `Ok(Vec::new())`，而不是 `Err`。
    /// - `length` 大于 [`MAX_SANE_LEN`] 视为 wasm 异常：记 warn 日志、退化为空数据。
    /// - **N() 返回的 data_ptr 是 wasm 内部队列指针，不属于宿主堆，绝不能 free**
    ///   （对齐 node `getDataToServer` 只 free lengthPtr；rust 曾误调 B(data_ptr)
    ///   破坏 wasm 堆管理，是 64MB 固定内存被漏满的根因）。
    /// - 宿主自己分配的 lengthPtr 由 [`AllocGuard`] 释放。
    pub fn get_data_to_server(&self) -> Result<Vec<u8>> {
        if self.pending_reset.load(Ordering::Acquire) {
            return Err(Error::crypto("TSDK 已请求重置，等待 worker 重建"));
        }
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        let exports = &inner.exports;

        // 1. 分配 4 字节 lengthPtr
        let alloc_result = AllocGuard::alloc(store, exports, 4);
        let mut length_guard = match alloc_result {
            Ok(g) => g,
            Err(e) => {
                if self.record_wasm_failure() {
                    self.request_reset();
                }
                return Err(e);
            }
        };
        let length_ptr = length_guard.ptr();

        // 对齐 bot tsdk-runtime.ts:357-364：调 N() 前先把 length 槽清零，
        // 避免 wasm 未写入时读到陈旧堆数据 → 发出垃圾长度的 AntiData。
        if let Err(e) = write_bytes(store, &exports.memory, length_ptr, &[0, 0, 0, 0]) {
            length_guard.free_now(store, exports);
            if self.record_wasm_failure() {
                self.request_reset();
            }
            return Err(e);
        }

        // 2. 调 N(lengthPtr) → 返回 data_ptr（length 由 wasm 写入 lengthPtr）
        let mut ret = [Val::I32(0); 1];
        let n_result = exports
            .n
            .call(&mut *store, &[Val::I32(length_ptr)], &mut ret)
            .map_err(|e| Error::crypto(format!("N() failed: {e}")));
        if let Err(e) = n_result {
            length_guard.free_now(store, exports);
            if self.record_wasm_failure() {
                self.request_reset();
            }
            return Err(e);
        }
        let data_ptr = i32_val(&ret, 0)?;
        if data_ptr <= 0 {
            // 对齐 Node：`!dataPtr` → 返回空 buffer
            length_guard.free_now(store, exports);
            self.record_wasm_success();
            return Ok(Vec::new());
        }

        // 3. 读 length (i32 little-endian)
        let len_bytes = match read_bytes(store, &exports.memory, length_ptr, 4) {
            Ok(b) => b,
            Err(e) => {
                length_guard.free_now(store, exports);
                if self.record_wasm_failure() {
                    self.request_reset();
                }
                return Err(e);
            }
        };
        let raw_len = i32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]);

        // 4. 对齐 Node：`length <= 0` → 返回空 buffer
        if raw_len <= 0 {
            length_guard.free_now(store, exports);
            self.record_wasm_success();
            return Ok(Vec::new());
        }
        let data_len = raw_len as usize;
        // 对齐 bot：只做内存边界检查（ensureBounds），无 64KB 上限；
        // 越界由下方 read_bytes 的 ensure_bounds 自然报错。

        // 5. 读 data
        let data_result = read_bytes(store, &exports.memory, data_ptr, data_len);
        let data = match data_result {
            Ok(d) => d,
            Err(e) => {
                length_guard.free_now(store, exports);
                if self.record_wasm_failure() {
                    self.request_reset();
                }
                return Err(e);
            }
        };

        // 6. 释放宿主分配的 lengthPtr（data_ptr 归 wasm 内部管理，不能 free）
        length_guard.free_now(store, exports);
        self.record_wasm_success();
        Ok(data)
    }

    /// 服务器回灌数据到 TSDK
    /// 原 TS: `sendDataFromServer(value: Uint8Array)` → O(ptr, length)
    pub fn send_data_from_server(&self, data: &[u8]) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        if self.pending_reset.load(Ordering::Acquire) {
            return Err(Error::crypto("TSDK 已请求重置，等待 worker 重建"));
        }
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        let exports = &inner.exports;

        let memory_bytes = exports.memory.data(&*store).len();
        tracing::debug!(input_len = data.len(), memory_bytes, "tsdk send_data_from_server begin");

        let cap = data.len() as i32;
        let alloc_result = AllocGuard::alloc(store, exports, cap);
        let mut ptr_guard = match alloc_result {
            Ok(g) => g,
            Err(e) => {
                if self.record_wasm_failure() {
                    self.request_reset();
                }
                return Err(e);
            }
        };
        let ptr = ptr_guard.ptr();
        if let Err(e) = write_bytes(store, &exports.memory, ptr, data) {
            ptr_guard.free_now(store, exports);
            if self.record_wasm_failure() {
                self.request_reset();
            }
            return Err(e);
        }

        if let Err(e) = exports
            .o
            .call(&mut *store, &[Val::I32(ptr), Val::I32(data.len() as i32)], &mut [])
            .map_err(|e| Error::crypto(format!("O() failed: {e}")))
        {
            ptr_guard.free_now(store, exports);
            if self.record_wasm_failure() {
                self.request_reset();
            }
            return Err(e);
        }

        ptr_guard.free_now(store, exports);
        self.record_wasm_success();
        Ok(())
    }

    /// 心跳 tick
    /// 原 TS: `heartbeatTick()` → M()
    pub fn heartbeat_tick(&self) -> Result<()> {
        if self.pending_reset.load(Ordering::Acquire) {
            return Err(Error::crypto("TSDK 已请求重置，等待 worker 重建"));
        }
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        let call_result = inner
            .exports
            .m
            .call(&mut *store, &[], &mut [])
            .map_err(|e| Error::crypto(format!("M() failed: {e}")));
        match &call_result {
            Ok(()) => self.record_wasm_success(),
            Err(_) => {
                if self.record_wasm_failure() {
                    self.request_reset();
                }
            }
        }
        call_result
    }

    /// 处理收到的数据
    /// 原 TS: `processReceivedData()` → P()
    pub fn process_received_data(&self) -> Result<()> {
        if self.pending_reset.load(Ordering::Acquire) {
            return Err(Error::crypto("TSDK 已请求重置，等待 worker 重建"));
        }
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        let call_result = inner
            .exports
            .p
            .call(&mut *store, &[], &mut [])
            .map_err(|e| Error::crypto(format!("P() failed: {e}")));
        match &call_result {
            Ok(()) => self.record_wasm_success(),
            Err(_) => {
                if self.record_wasm_failure() {
                    self.request_reset();
                }
            }
        }
        call_result
    }

    /// 发送状态
    /// 原 TS: `sendStatus()` → E()
    pub fn send_status(&self) -> Result<()> {
        if self.pending_reset.load(Ordering::Acquire) {
            return Err(Error::crypto("TSDK 已请求重置，等待 worker 重建"));
        }
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        let call_result = inner
            .exports
            .e
            .call(&mut *store, &[], &mut [])
            .map_err(|e| Error::crypto(format!("E() failed: {e}")));
        match &call_result {
            Ok(()) => self.record_wasm_success(),
            Err(_) => {
                if self.record_wasm_failure() {
                    self.request_reset();
                }
            }
        }
        call_result
    }

    /// 速度作弊检测
    /// 原 TS: `detectSpeedHack(elapsedMs)` → fa(elapsedMs)
    pub fn detect_speed_hack(&self, elapsed_ms: u64) -> Result<()> {
        if self.pending_reset.load(Ordering::Acquire) {
            return Err(Error::crypto("TSDK 已请求重置，等待 worker 重建"));
        }
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        let call_result = inner
            .exports
            .fa
            .call(&mut *store, &[Val::I32(elapsed_ms as i32)], &mut [])
            .map_err(|e| Error::crypto(format!("fa() failed: {e}")));
        match &call_result {
            Ok(()) => self.record_wasm_success(),
            Err(_) => {
                if self.record_wasm_failure() {
                    self.request_reset();
                }
            }
        }
        call_result
    }

    /// 是否已 bindUser
    #[must_use]
    pub fn is_user_bound(&self) -> bool {
        self.inner.lock().as_ref().map(|i| i.user_bound).unwrap_or(false)
    }

    /// 销毁
    pub fn destroy(&self) {
        *self.inner.lock() = None;
    }

    /// 当前 memory 大小（调试用）
    #[must_use]
    pub fn memory_size(&self) -> usize {
        self.inner.lock().as_ref().map_or(0, |i| i.exports.memory.data(&i.store).len())
    }
}

// ===== host function 实现 =====
//
// 22 个 host function `a.a` ~ `a.v`，按原 TS 实现翻译。
// 阶段 0：保持语义近似，不依赖 host 返回值的部分用 noop/0 即可。

fn create_linker(engine: &Engine) -> Result<Linker<HostState>> {
    let mut linker = Linker::new(engine);

    // a: assertion — 抛错（与原 TS 一致）
    linker.func_wrap(
        "a",
        "a",
        |_c: wasmtime::Caller<'_, HostState>,
         _e: i32,
         _f: i32,
         _l: i32,
         _fn_: i32|
         -> WasmResult<()> { Err(werr!("TSDK assertion")) },
    )?;

    // b: writeStringToFile（对齐 bot tsdk-runtime.ts:148-158：真实写文件，utf-8）
    linker.func_wrap(
        "a",
        "b",
        |mut c: wasmtime::Caller<'_, HostState>, f: i32, d: i32, _e: i32| -> WasmResult<i32> {
            let data_dir = c.data().data_dir.clone();
            let (file, content) =
                match (read_cstring_in_caller(&mut c, f), read_cstring_in_caller(&mut c, d)) {
                    (Ok(f), Ok(d)) => (f, d),
                    (Err(e), _) | (_, Err(e)) => return Err(e),
                };
            let Some(target) = resolve_data_path(&data_dir, &file) else {
                tracing::warn!(file = %file, "TSDK 文件写入失败: 路径越出账号目录");
                return Ok(0);
            };
            match std::fs::create_dir_all(target.parent().unwrap_or(std::path::Path::new(".")))
                .and_then(|()| std::fs::write(&target, content.as_bytes()))
            {
                Ok(()) => Ok(1),
                Err(e) => {
                    tracing::warn!(error = %e, "TSDK 文件写入失败");
                    Ok(0)
                }
            }
        },
    )?;

    // c: captureStackTrace — 写空字符串
    linker.func_wrap(
        "a",
        "c",
        |mut c: wasmtime::Caller<'_, HostState>, ptr: i32, _cap: i32| -> WasmResult<i32> {
            // 写一个空 cstring
            write_cstring_in_caller(&mut c, ptr, b"")?;
            Ok(1)
        },
    )?;

    // d: 写入 TSDK_VERSION
    linker.func_wrap(
        "a",
        "d",
        |mut c: wasmtime::Caller<'_, HostState>, ptr: i32, cap: i32| -> WasmResult<i32> {
            let bytes = TSDK_VERSION.as_bytes();
            if bytes.len() < cap as usize {
                write_cstring_in_caller(&mut c, ptr, bytes)?;
                Ok(1)
            } else {
                Ok(0)
            }
        },
    )?;

    // e: ACEVM 完整性 — 返回 0（参数是 wasm 内部传入的某种上下文指针）
    linker.func_wrap(
        "a",
        "e",
        |_c: wasmtime::Caller<'_, HostState>, _ctx: i32| -> WasmResult<i32> { Ok(0) },
    )?;

    // f: sensors — noop
    linker
        .func_wrap("a", "f", |_c: wasmtime::Caller<'_, HostState>| -> WasmResult<()> { Ok(()) })?;

    // g: readFileToString（对齐 bot tsdk-runtime.ts:169-176：真实读文件，失败返回 0）
    linker.func_wrap(
        "a",
        "g",
        |mut c: wasmtime::Caller<'_, HostState>,
         f: i32,
         o: i32,
         cap: i32,
         _e: i32|
         -> WasmResult<i32> {
            let data_dir = c.data().data_dir.clone();
            let file = read_cstring_in_caller(&mut c, f)?;
            let Some(target) = resolve_data_path(&data_dir, &file) else {
                return Ok(0);
            };
            match std::fs::read_to_string(&target) {
                Ok(content) => {
                    let bytes = content.as_bytes();
                    if (bytes.len() as i32) < cap {
                        write_cstring_in_caller(&mut c, o, bytes)?;
                        Ok(1)
                    } else {
                        Ok(0)
                    }
                }
                Err(_) => Ok(0),
            }
        },
    )?;

    // h: clock_gettime — 对齐 bot tsdk-runtime.ts:177-185：
    // clock 0 = 墙钟 ms（用服务器同步时钟），clock 1~3 = 进程启动以来单调 ms；
    // 值 = round(ms * 1e6)（纳秒），按 u32 low/high 写入。
    linker.func_wrap(
        "a",
        "h",
        |mut c: wasmtime::Caller<'_, HostState>,
         clock_id: i32,
         _l: i32,
         _h: i32,
         out: i32|
         -> WasmResult<i32> {
            if !(0..=3).contains(&clock_id) {
                return Ok(28);
            }
            let ms = if clock_id == 0 {
                crate::utils::time::now_ms() as u64
            } else {
                process_start().elapsed().as_millis() as u64
            };
            let value = (ms as u128) * 1_000_000;
            let mem = c.data().memory;
            if let Some(m) = mem {
                let data = m.data_mut(&mut c);
                let low = (value & 0xFFFF_FFFF) as u32;
                let high = ((value >> 32) & 0xFFFF_FFFF) as u32;
                write_u32_le(data, out, low);
                write_u32_le(data, out + 4, high);
            }
            Ok(0)
        },
    )?;

    // i: dataDir（对齐 bot：QQ 宿主返回 qqfile://usr/，其余目录 + 平台路径分隔符）
    linker.func_wrap(
        "a",
        "i",
        |mut c: wasmtime::Caller<'_, HostState>, ptr: i32, cap: i32| -> WasmResult<i32> {
            let owned;
            let bytes: &[u8] = if let Some(path) = c.data().host.user_data_path {
                path.as_bytes()
            } else {
                owned = format!("{}{}", c.data().data_dir, std::path::MAIN_SEPARATOR);
                owned.as_bytes()
            };
            if bytes.len() < cap as usize {
                write_cstring_in_caller(&mut c, ptr, bytes)?;
                Ok(1)
            } else {
                Ok(0)
            }
        },
    )?;

    // j: deviceText（对齐 bot getDeviceText：QQ 宿主固定 `windows;windows;windows 10.0;0;`；
    // 其余 `{deviceId};{os};{sysSoftware};Node.js;`，来自运行时配置——与登录 device_info 同源）
    linker.func_wrap(
        "a",
        "j",
        |mut c: wasmtime::Caller<'_, HostState>, ptr: i32, cap: i32| -> WasmResult<i32> {
            if let Some(text) = c.data().host.device_text {
                let bytes = text.as_bytes();
                if bytes.len() < cap as usize {
                    write_cstring_in_caller(&mut c, ptr, bytes)?;
                    return Ok(1);
                }
                return Ok(0);
            }
            let rt = crate::config::get_runtime_config();
            let model = if rt.device_info.device_id.is_empty() {
                format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
            } else {
                rt.device_info.device_id.clone()
            };
            let platform =
                if rt.os.is_empty() { std::env::consts::OS.to_string() } else { rt.os.clone() };
            let system = if rt.device_info.sys_software.is_empty() {
                platform.clone()
            } else {
                rt.device_info.sys_software.clone()
            };
            let text = format!("{model};{platform};{system};Node.js;");
            let bytes = text.as_bytes();
            if bytes.len() < cap as usize {
                write_cstring_in_caller(&mut c, ptr, bytes)?;
                Ok(1)
            } else {
                Ok(0)
            }
        },
    )?;

    // k: RUNTIME_TABLE（对齐 bot writeBytes：返回写入字节数 59）
    linker.func_wrap(
        "a",
        "k",
        |mut c: wasmtime::Caller<'_, HostState>, ptr: i32, cap: i32| -> WasmResult<i32> {
            if (RUNTIME_TABLE.len() as i32) <= cap {
                let mem = c.data().memory;
                if let Some(m) = mem {
                    let data = m.data_mut(&mut c);
                    let off = ptr as usize;
                    if off + RUNTIME_TABLE.len() <= data.len() {
                        data[off..off + RUNTIME_TABLE.len()].copy_from_slice(&RUNTIME_TABLE);
                        return Ok(RUNTIME_TABLE.len() as i32);
                    }
                }
            }
            Ok(0)
        },
    )?;

    // l: debugMode（对齐 bot hostProfile.debugMode：微信 2 / QQ 0）
    linker.func_wrap("a", "l", |c: wasmtime::Caller<'_, HostState>| -> WasmResult<i32> {
        Ok(c.data().host.debug_mode)
    })?;

    // m / n: appId（按宿主画像：QQ 1112386029 / 微信 wx5306c5978fdb76e4）
    for name in ["m", "n"] {
        linker.func_wrap(
            "a",
            name,
            |mut c: wasmtime::Caller<'_, HostState>, ptr: i32, cap: i32| -> WasmResult<i32> {
                let bytes = c.data().host.app_id.as_bytes();
                if bytes.len() < cap as usize {
                    write_cstring_in_caller(&mut c, ptr, bytes)?;
                    Ok(1)
                } else {
                    Ok(0)
                }
            },
        )?;
    }

    // o: integrity functions — noop（4 个 i32 参数，无返回）
    linker.func_wrap(
        "a",
        "o",
        |_caller: wasmtime::Caller<'_, HostState>,
         _a: i32,
         _b: i32,
         _c: i32,
         _d: i32|
         -> WasmResult<()> { Ok(()) },
    )?;

    // p: stat（对齐 bot tsdk-runtime.ts:193-200：真实 stat 并回调 wasm 的 y(mode,size,atime,mtime)）
    linker.func_wrap(
        "a",
        "p",
        |mut c: wasmtime::Caller<'_, HostState>, f: i32| -> WasmResult<i32> {
            let data_dir = c.data().data_dir.clone();
            let file = read_cstring_in_caller(&mut c, f)?;
            let Some(target) = resolve_data_path(&data_dir, &file) else {
                return Ok(0);
            };
            let Ok(meta) = std::fs::metadata(&target) else {
                return Ok(0);
            };
            let size = meta.len().min(0x7FFF_FFFF) as i32;
            let atime = meta
                .accessed()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            // 回调 wasm 导出 y(mode, size, atime, mtime)
            let y = c.get_export("y").and_then(wasmtime::Extern::into_func);
            let Some(y) = y else {
                return Ok(0);
            };
            let mut ret = [Val::I32(0); 1];
            // mode 沿用 Node Windows 语义（普通文件 0o100666 = 33206）
            y.call(
                &mut c,
                &[Val::I32(33_206), Val::I32(size), Val::I32(atime as i32), Val::I32(mtime as i32)],
                &mut ret,
            )
            .map_err(|e| werr!("TSDK stat 回调 y() 失败: {e}"))?;
            Ok(i32_val(&ret, 0).unwrap_or(0))
        },
    )?;

    // q: serverTime — 写服务器同步时钟秒（bot 先写本地再异步用 HTTP Date 校正；
    // rust 的 now_ms 已由 Login/Heartbeat 回包持续同步，等效且更准）
    linker.func_wrap(
        "a",
        "q",
        |mut c: wasmtime::Caller<'_, HostState>, out: i32| -> WasmResult<i32> {
            let now = (crate::utils::time::now_ms() / 1000) as u32;
            let mem = c.data().memory;
            if let Some(m) = mem {
                write_u32_le(m.data_mut(&mut c), out, now);
            }
            Ok(1)
        },
    )?;

    // r: memory.grow 失败时调用（返回 0，wasmtime 会把这个 trap 转成自己的 error）
    linker.func_wrap(
        "a",
        "r",
        |_c: wasmtime::Caller<'_, HostState>, size: i32| -> WasmResult<i32> {
            Err(werr!("TSDK 内存扩展失败: {size}"))
        },
    )?;

    // s: time (ms) —— wasm 实际要求返回 f64（与 JS Date.now() 一致）
    linker.func_wrap("a", "s", |_c: wasmtime::Caller<'_, HostState>| -> WasmResult<f64> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);
        Ok(now)
    })?;

    // t: appendStringToFile（对齐 bot tsdk-runtime.ts:215-224：真实追加写）
    linker.func_wrap(
        "a",
        "t",
        |mut c: wasmtime::Caller<'_, HostState>, f: i32, d: i32, _e: i32| -> WasmResult<i32> {
            let data_dir = c.data().data_dir.clone();
            let (file, content) =
                match (read_cstring_in_caller(&mut c, f), read_cstring_in_caller(&mut c, d)) {
                    (Ok(f), Ok(d)) => (f, d),
                    (Err(e), _) | (_, Err(e)) => return Err(e),
                };
            let Some(target) = resolve_data_path(&data_dir, &file) else {
                return Ok(0);
            };
            use std::io::Write as _;
            let append = (|| -> std::io::Result<()> {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut fh = std::fs::OpenOptions::new().create(true).append(true).open(&target)?;
                fh.write_all(content.as_bytes())
            })();
            match append {
                Ok(()) => Ok(1),
                Err(e) => {
                    tracing::warn!(error = %e, "TSDK 文件追加失败");
                    Ok(0)
                }
            }
        },
    )?;

    // u: abort
    linker.func_wrap("a", "u", |_c: wasmtime::Caller<'_, HostState>| -> WasmResult<()> {
        Err(werr!("TSDK aborted"))
    })?;

    // v: TQOS 上报（对齐 bot tsdk-runtime.ts:226-242：解析 wasm 内存 JSON
    // `{headers, message}`，fire-and-forget POST https://api.anticheatexpert.com/tqos）
    linker.func_wrap(
        "a",
        "v",
        |c: wasmtime::Caller<'_, HostState>, ptr: i32, len: i32| -> WasmResult<i32> {
            if ptr <= 0 || len <= 0 {
                return Ok(0);
            }
            let mem = c.data().memory;
            let Some(m) = mem else { return Ok(0) };
            let mem_size = m.data(&c).len();
            let off = ptr as usize;
            let take = (len as usize).min(mem_size.saturating_sub(off));
            let raw = m.data(&c)[off..off + take].to_vec();
            let parsed: serde_json::Value = match serde_json::from_slice(&raw) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(error = %e, "TSDK TQOS 数据无效");
                    return Ok(0);
                }
            };
            // fire-and-forget：不阻塞 wasm 调用线程（bot 同样异步发出后立即返回 0）
            let handle = tokio::runtime::Handle::try_current();
            if let Ok(handle) = handle {
                handle.spawn(async move {
                    let client = match reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(5))
                        .build()
                    {
                        Ok(cl) => cl,
                        Err(_) => return,
                    };
                    let mut req = client.post("https://api.anticheatexpert.com/tqos");
                    if let Some(headers) = parsed.get("headers").and_then(|h| h.as_object()) {
                        for (k, v) in headers {
                            if let Some(v) = v.as_str() {
                                req = req.header(k.as_str(), v);
                            }
                        }
                    }
                    let body = match parsed.get("message") {
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(other) => other.to_string(),
                        None => "{}".to_string(),
                    };
                    if let Err(e) = req.body(body).send().await {
                        tracing::debug!(error = %e, "TSDK TQOS 上报失败");
                    }
                });
            }
            Ok(0)
        },
    )?;

    Ok(linker)
}

fn extract_exports(instance: &Instance, store: &mut Store<HostState>) -> Result<Exports> {
    let memory = instance
        .get_memory(&mut *store, "w")
        .ok_or_else(|| Error::crypto("missing memory export 'w'"))?;

    // 必须按顺序取（store 同一时刻只能有一个 &mut 借用）
    let alloc = instance
        .get_func(&mut *store, "A")
        .ok_or_else(|| Error::crypto("missing required export: A"))?;
    let free = instance
        .get_func(&mut *store, "B")
        .ok_or_else(|| Error::crypto("missing required export: B"))?;
    let x = instance
        .get_func(&mut *store, "x")
        .ok_or_else(|| Error::crypto("missing required export: x"))?;
    let g = instance
        .get_func(&mut *store, "G")
        .ok_or_else(|| Error::crypto("missing required export: G"))?;
    let encrypt = instance
        .get_func(&mut *store, "ba")
        .ok_or_else(|| Error::crypto("missing required export: ba"))?;
    let decrypt = instance
        .get_func(&mut *store, "ca")
        .ok_or_else(|| Error::crypto("missing required export: ca"))?;
    let decrypt_strings = instance
        .get_func(&mut *store, "__mergewasm_shared____wasm_decrypt_strings")
        .ok_or_else(|| Error::crypto("missing __mergewasm_shared____wasm_decrypt_strings"))?;
    let h = instance
        .get_func(&mut *store, "H")
        .ok_or_else(|| Error::crypto("missing required export: H"))?;
    let m = instance
        .get_func(&mut *store, "M")
        .ok_or_else(|| Error::crypto("missing required export: M"))?;
    let p = instance
        .get_func(&mut *store, "P")
        .ok_or_else(|| Error::crypto("missing required export: P"))?;
    let e = instance
        .get_func(&mut *store, "E")
        .ok_or_else(|| Error::crypto("missing required export: E"))?;
    let o = instance
        .get_func(&mut *store, "O")
        .ok_or_else(|| Error::crypto("missing required export: O"))?;
    let fa = instance
        .get_func(&mut *store, "fa")
        .ok_or_else(|| Error::crypto("missing required export: fa"))?;
    let n = instance
        .get_func(&mut *store, "N")
        .ok_or_else(|| Error::crypto("missing required export: N"))?;

    Ok(Exports {
        memory,
        alloc,
        free,
        x,
        g,
        encrypt,
        decrypt,
        decrypt_strings,
        h,
        m,
        p,
        e,
        o,
        fa,
        n,
    })
}

// ===== 边界检查 =====
//
// 对齐 Node `tsdk-runtime.ts::ensureBounds`：ptr 必须非负，ptr+len 不能越过 wasm 内存。
// 用 `checked_add` 替代裸 `+`，避免 release 模式下 `off + len` 整型回绕绕过检查再触发
// slice panic（实际生产 panic 的根因之一）。

/// 检查 `[ptr, ptr+len)` 是否落在 wasm 内存内。所有 host 侧 `read_bytes` / `write_bytes`
/// / `read_cstring` / `write_cstring` / `write_cstring_in_caller` 都必须先过这一道关。
fn ensure_bounds(ptr: i32, len: usize, mem_size: usize) -> Result<()> {
    if ptr < 0 {
        return Err(Error::crypto(format!(
            "wasm pointer is negative: ptr={ptr}, len={len}, mem={mem_size}"
        )));
    }
    let off = ptr as usize;
    let end = off
        .checked_add(len)
        .ok_or_else(|| Error::crypto(format!("wasm pointer overflow: ptr={ptr}, len={len}")))?;
    if end > mem_size {
        return Err(Error::crypto(format!(
            "wasm out of bounds: ptr={ptr}, len={len}, mem={mem_size}"
        )));
    }
    Ok(())
}

/// 零拷贝取 wasm 内存的 `&mut [u8]`；长度越界返回 `Err`，永不 panic。
fn mem_slice_mut<'a>(
    store: &'a mut Store<HostState>,
    memory: &'a Memory,
    ptr: i32,
    len: usize,
) -> Result<&'a mut [u8]> {
    let mem_size = memory.data(&*store).len();
    ensure_bounds(ptr, len, mem_size)?;
    let off = ptr as usize;
    // 上一步已经验证 off + len <= mem_size
    Ok(&mut memory.data_mut(store)[off..off + len])
}

/// 零拷贝取 wasm 内存的 `&[u8]`；长度越界返回 `Err`，永不 panic。
fn mem_slice<'a>(
    store: &'a Store<HostState>,
    memory: &'a Memory,
    ptr: i32,
    len: usize,
) -> Result<&'a [u8]> {
    let mem_size = memory.data(store).len();
    ensure_bounds(ptr, len, mem_size)?;
    let off = ptr as usize;
    Ok(&memory.data(store)[off..off + len])
}

// ===== AllocGuard =====
//
// 对齐 Node `try/finally`：guard 让 wasm 分配与释放的配对在编译期可见，避免原先散落各处的
// `if let Err(e) = ... { let _ = exports.free... }` 兜底代码在新增分支时被遗漏。
//
// **设计取舍**：
// - 工作区 `unsafe_code = "forbid"`，所以不能像 Node 那样用 `try/finally` 自动收尾。
// - guard 只持有 `i32` ptr；业务代码**必须**在用完数据后调 `free_now(store, exports)` 释放，
//   或调 `defuse()` 把所有权交出去（例如返回 `Vec<u8>` 后已隐式转移）。
// - Drop 时若 ptr 仍非零，会记一条 error 日志（与原"忘 free"行为一致——但通过日志更易发现）。
// - 对 `from_existing_ptr` 路径（`N()` 返回的 ptr），guard 自身**不持有** free 闭包，
//   必须调 `free_externally(store, exports)` 显式释放。

/// wasm 内存分配 + 释放的配对跟踪。
struct AllocGuard {
    ptr: i32,
    /// 标记所有权已转交（不再要求 free）
    defused: bool,
}

impl AllocGuard {
    /// 创建空 guard（不持有任何 wasm 分配）
    #[must_use]
    #[cfg(test)]
    fn empty() -> Self {
        Self { ptr: 0, defused: true }
    }

    /// 通过调用 wasm `alloc(size)` 分配。失败时返回 `Err`，guard 保持 no-op 状态。
    fn alloc(store: &mut Store<HostState>, exports: &Exports, size: i32) -> Result<Self> {
        let mut alloc_res = [Val::I32(0); 1];
        let call_result = exports.alloc.call(&mut *store, &[Val::I32(size)], &mut alloc_res);
        if let Err(e) = &call_result {
            // 正式日志默认无 debug 级别，这里用 WARN 留下内存增长证据
            tracing::warn!(
                size,
                memory_bytes = exports.memory.data(&*store).len(),
                error = %e,
                "tsdk wasm alloc 失败"
            );
        }
        call_result.map_err(|e| Error::crypto(format!("alloc failed: {e}")))?;
        let ptr = i32_val(&alloc_res, 0)?;
        if ptr <= 0 {
            return Err(Error::crypto(format!("alloc returned non-positive ptr: {ptr}")));
        }
        Ok(Self { ptr, defused: false })
    }

    /// 用一个外部获得的 i32 ptr 构造 guard（典型来源：`N()` 返回值）。
    /// 因为没有匹配 alloc，guard 不会在 Drop 时自动 free——必须调 `free_now`。
    #[must_use]
    #[cfg(test)]
    fn from_existing_ptr(ptr: i32) -> Self {
        if ptr <= 0 {
            return Self::empty();
        }
        Self { ptr, defused: false }
    }

    /// 已分配的 wasm 指针值
    #[must_use]
    fn ptr(&self) -> i32 {
        self.ptr
    }

    /// 立即调 wasm `free` 释放 ptr。调用后 guard 变成空状态。
    fn free_now(&mut self, store: &mut Store<HostState>, exports: &Exports) {
        if self.ptr == 0 || self.defused {
            return;
        }
        let _ = exports.free.call(store, &[Val::I32(self.ptr)], &mut []);
        self.ptr = 0;
    }
}

impl Drop for AllocGuard {
    fn drop(&mut self) {
        if self.ptr == 0 || self.defused {
            return;
        }
        // 业务忘了调 free_now / free_externally：原代码同样会泄漏，这里仅记日志以便发现。
        tracing::error!(
            ptr = self.ptr,
            "AllocGuard dropped without explicit free: wasm memory will leak (call .free_now() or .defuse())"
        );
    }
}

// ===== 字节读写工具 =====

fn write_bytes(
    store: &mut Store<HostState>,
    memory: &Memory,
    ptr: i32,
    bytes: &[u8],
) -> Result<()> {
    if bytes.is_empty() {
        // 空写也要校验 ptr 本身落在内存内（ptr == mem_size 也算合法）
        let mem_size = memory.data(&*store).len();
        ensure_bounds(ptr, 0, mem_size)?;
        return Ok(());
    }
    let slice = mem_slice_mut(store, memory, ptr, bytes.len())?;
    slice.copy_from_slice(bytes);
    Ok(())
}

fn read_bytes(store: &Store<HostState>, memory: &Memory, ptr: i32, len: usize) -> Result<Vec<u8>> {
    if len == 0 {
        let mem_size = memory.data(store).len();
        ensure_bounds(ptr, 0, mem_size)?;
        return Ok(Vec::new());
    }
    let slice = mem_slice(store, memory, ptr, len)?;
    Ok(slice.to_vec())
}

fn write_cstring(
    store: &mut Store<HostState>,
    memory: &Memory,
    ptr: i32,
    bytes: &[u8],
) -> Result<()> {
    write_bytes(store, memory, ptr, bytes)?;
    // 写入 NUL 终止符：ptr + bytes.len() 必须落在 [0, mem_size] 内
    let mem_size = memory.data(&*store).len();
    let null_pos = (ptr as usize).checked_add(bytes.len()).ok_or_else(|| {
        Error::crypto(format!("cstring null_pos overflow: ptr={ptr}, len={}", bytes.len()))
    })?;
    if null_pos < mem_size {
        let data = mem_slice_mut(store, memory, ptr, 0)?; // 仅作 borrow 校验
        let _ = data;
        let mut_ref = memory.data_mut(store);
        mut_ref[null_pos] = 0;
    }
    Ok(())
}

/// 在 host function 闭包内写入 cstring（caller 借用处理）
fn write_cstring_in_caller(
    caller: &mut wasmtime::Caller<'_, HostState>,
    ptr: i32,
    bytes: &[u8],
) -> WasmResult<()> {
    if ptr < 0 {
        return Err(werr!("write_cstring_in_caller: negative ptr={ptr}"));
    }
    // clone Memory（cheap）以避免长生命周期借用 caller
    let mem = caller.data().memory.ok_or_else(|| werr!("memory not set"))?;
    let mem_size = mem.data(&*caller).len();
    ensure_bounds(ptr, bytes.len() + 1, mem_size)?;
    let off = ptr as usize;
    let data = mem.data_mut(&mut *caller);
    data[off..off + bytes.len()].copy_from_slice(bytes);
    data[off + bytes.len()] = 0;
    Ok(())
}

fn write_u32_le(data: &mut [u8], ptr: i32, value: u32) {
    let off = ptr as usize;
    if off.checked_add(4).is_some_and(|end| end <= data.len()) {
        data[off..off + 4].copy_from_slice(&value.to_le_bytes());
    }
}

/// 进程启动时刻（单调钟基准，对齐 bot `performance.now()`）
fn process_start() -> std::time::Instant {
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    *START.get_or_init(std::time::Instant::now)
}

/// 在 host function 闭包内读 cstring（对齐 bot readCString，默认 64KB 上限）
fn read_cstring_in_caller(
    caller: &mut wasmtime::Caller<'_, HostState>,
    ptr: i32,
) -> WasmResult<String> {
    if ptr <= 0 {
        return Ok(String::new());
    }
    let mem = caller.data().memory.ok_or_else(|| werr!("memory not set"))?;
    let mem_size = mem.data(&*caller).len();
    let off = ptr as usize;
    if off >= mem_size {
        return Err(werr!("read_cstring_in_caller: out of bounds ptr={ptr}"));
    }
    let cap = (mem_size - off).min(64 * 1024);
    let data = mem.data(&*caller);
    let mut end = off;
    while end - off < cap && data[end] != 0 {
        end += 1;
    }
    let s = std::str::from_utf8(&data[off..end]).map_err(|e| werr!("invalid utf-8: {e}"))?;
    Ok(s.to_string())
}

/// TSDK 文件路径解析（对齐 bot resolveDataPath：限制在账号数据目录内，防路径穿越）
fn resolve_data_path(data_dir: &str, input: &str) -> Option<std::path::PathBuf> {
    let relative = input.replace('\\', "/");
    let relative = relative.trim_start_matches('/');
    if relative.is_empty() || relative.contains("..") {
        return None;
    }
    let root = std::path::Path::new(data_dir);
    Some(root.join(relative))
}

/// 从 wasm 内存读 cstring（以 NUL 结尾）
fn read_cstring(
    store: &Store<HostState>,
    memory: &Memory,
    ptr: i32,
    max_length: usize,
) -> Result<String> {
    if ptr < 0 {
        return Err(Error::crypto(format!("read_cstring: negative ptr={ptr}")));
    }
    let data = memory.data(store);
    let off = ptr as usize;
    if off >= data.len() {
        return Err(Error::crypto(format!("read_cstring: pointer out of bounds, ptr={ptr}")));
    }
    let cap = (data.len() - off).min(max_length);
    let mut end = off;
    while end - off < cap && data[end] != 0 {
        end += 1;
    }
    let s = std::str::from_utf8(&data[off..end])
        .map_err(|e| Error::crypto(format!("invalid utf-8 in cstring: {e}")))?;
    Ok(s.to_string())
}

fn i32_val(vals: &[Val], idx: usize) -> Result<i32> {
    match vals.get(idx) {
        Some(Val::I32(v)) => Ok(*v),
        _ => Err(Error::crypto("expected i32 result")),
    }
}

// ===== Error 扩展 =====
impl Error {
    fn crypto<S: Into<String>>(msg: S) -> Self {
        Self::Crypto(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ensure_bounds` 必须对所有以下情形返回 `Err` 而不是 panic：
    /// - `ptr < 0`（wasm 写入负值，对齐 Node `ptr < 0` 检查）
    /// - `ptr + len` 整型回绕（release 模式下会回绕成小值绕过 > data.len()）
    /// - 超出 wasm 内存范围
    #[test]
    fn ensure_bounds_rejects_invalid_arguments() {
        // 负 ptr
        let err = ensure_bounds(-1, 0, 1024);
        assert!(err.is_err());
        assert!(format!("{}", err.unwrap_err()).contains("negative"));

        // ptr + len 整型回绕
        let err = ensure_bounds(1, usize::MAX, 1024);
        assert!(err.is_err());
        assert!(format!("{}", err.unwrap_err()).contains("overflow"));

        // 超出内存
        let err = ensure_bounds(0, 2048, 1024);
        assert!(err.is_err());
        assert!(format!("{}", err.unwrap_err()).contains("out of bounds"));

        // ptr + len 刚好等于 mem_size：允许（写入 NUL 终止符场景）
        assert!(ensure_bounds(0, 1024, 1024).is_ok());

        // 正常区间
        assert!(ensure_bounds(100, 50, 1024).is_ok());
    }

    /// `AllocGuard::from_existing_ptr(ptr <= 0)` 必须返回空 guard
    #[test]
    fn alloc_guard_from_existing_ptr_handles_non_positive() {
        let g = AllocGuard::from_existing_ptr(0);
        assert_eq!(g.ptr(), 0);
        // 空 guard 不会触发 free（Drop 时 no-op）
        drop(g);

        let g = AllocGuard::from_existing_ptr(-1);
        assert_eq!(g.ptr(), 0);
        drop(g);
    }

    /// `AllocGuard::empty()` 必须返回空 guard
    #[test]
    fn alloc_guard_empty_is_noop() {
        let g = AllocGuard::empty();
        assert_eq!(g.ptr(), 0);
        // Drop 时 no-op（defused = true）
        drop(g);
    }

    /// `i32::from_le_bytes(...) as usize` 的下溢行为 — 这正是生产 panic 的根因。
    /// Rust 本身的下溢是 defined behavior，但 `read_bytes` 必须先用 ensure_bounds 拦截，
    /// 不能让这个下溢后的 usize 进入 slice 表达式。
    /// 这里只是文档化：负 i32 转 usize 的确会变成超大值（这不是我们要修的，是 wasm 行为）。
    #[test]
    fn negative_i32_casts_to_huge_usize() {
        // 任取一个负 i32：-1
        let n: i32 = -1;
        let as_us: u64 = n as u64;
        // 负数 cast 到 u64 会回绕到 2^64 - 1
        assert_eq!(as_us, u64::MAX);
        // 这就是原 `read_bytes` 收到的"out of range"值的来源
        // —— 必须先 `ensure_bounds` 拦截，否则进入 slice 表达式直接 panic
        assert!(ensure_bounds(n, 0, 1024).is_err());
    }

    /// 关键回归测试：连续失败计数 + request_reset 流程
    #[test]
    fn wasm_failure_counter_triggers_reset() {
        let rt = TsdkRuntime::new("test_counter");
        assert_eq!(rt.consecutive_fail_count(), 0);
        assert!(!rt.is_reset_pending());

        // 单次失败：record_wasm_failure 返回 false，调用方不会触发 reset
        assert!(!rt.record_wasm_failure());
        assert_eq!(rt.consecutive_fail_count(), 1);
        assert!(!rt.is_reset_pending());

        // 成功清零
        rt.record_wasm_success();
        assert_eq!(rt.consecutive_fail_count(), 0);
        assert!(!rt.is_reset_pending());

        // 连续 WASM_CONSECUTIVE_FAIL_THRESHOLD 次失败：record_wasm_failure 返回 true
        for _ in 0..(WASM_CONSECUTIVE_FAIL_THRESHOLD - 1) {
            assert!(!rt.record_wasm_failure());
        }
        // 第 N 次必须返回 true；调用方据此触发 request_reset
        assert!(rt.record_wasm_failure());
        assert_eq!(rt.consecutive_fail_count(), WASM_CONSECUTIVE_FAIL_THRESHOLD);
        // 当前实现：record_wasm_failure 不自动 request_reset；调用方需自己调用
        assert!(!rt.is_reset_pending());
        rt.request_reset();
        assert!(rt.is_reset_pending());

        // 模拟 worker 重建完成，调用 mark_reset_completed
        rt.mark_reset_completed();
        assert!(!rt.is_reset_pending());
        assert_eq!(rt.consecutive_fail_count(), 0);
    }

    /// rebuild 在没有 init 过的情况下必须返回 Err
    #[test]
    fn rebuild_without_init_returns_error() {
        let rt = TsdkRuntime::new("test_rebuild_no_init");
        // wasm_path 为空，rebuild 应该失败
        let result = rt.rebuild();
        assert!(result.is_err());
    }
}
