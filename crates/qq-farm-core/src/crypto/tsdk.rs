//! 微信 TSDK (`tsdk.wasm`) 封装。
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
use std::sync::OnceLock;
use std::time::Instant;

use anyhow::anyhow;
use wasmtime::{Config, Engine, Func, Instance, Linker, Memory, Module, Store, Val};

use crate::error::{Error, Result};

/// wasmtime 闭包返回类型（避免与本 crate 的 `Result` 冲突）
type WasmResult<T> = std::result::Result<T, wasmtime::Error>;

// ===== TSDK 元信息（与原项目保持一致） =====

const TSDK_VERSION: &str = "v3.8.6.1785239995";
const MINI_PROGRAM_APP_ID: &str = "wx5306c5978fdb76e4";
const TSDK_GAME_ID: u32 = 3167;
const TSDK_APP_KEY: &str = "0";
const MERGED_DATA_KEY: u32 = 1_871_261_153;

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
    inner: parking_lot::Mutex<Option<Inner>>,
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
    /// 创建 runtime（不加载 wasm）
    #[must_use]
    pub fn new(data_dir: impl Into<String>) -> Self {
        Self { data_dir: data_dir.into(), inner: parking_lot::Mutex::new(None) }
    }

    /// 便捷构造：创建 + 初始化
    pub fn load(wasm_path: &Path, data_dir: impl Into<String>) -> Result<Self> {
        let rt = Self::new(data_dir);
        let start = Instant::now();
        rt.init(wasm_path)?;
        tracing::info!(
            elapsed_ms = start.elapsed().as_millis() as u64,
            version = TSDK_VERSION,
            "TSDK 初始化完成"
        );
        Ok(rt)
    }

    /// 初始化
    pub fn init(&self, wasm_path: &Path) -> Result<()> {
        if self.inner.lock().is_some() {
            return Ok(());
        }
        let engine = shared_engine()?;
        let module = shared_module(engine, wasm_path)?;

        let host = HostState { data_dir: self.data_dir.clone(), ..Default::default() };
        let mut store = Store::new(engine, host);
        let linker = create_linker(engine)?;
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| Error::crypto(format!("instantiate failed: {e}")))?;

        let exports = extract_exports(&instance, &mut store)?;
        store.data_mut().memory = Some(exports.memory.clone());

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
                    &mut [
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
            .call(&mut store, &mut [], &mut [])
            .map_err(|e| Error::crypto(format!("init x() failed: {e}")))?;

        // 设置 game id + app key
        let mut alloc_res = [Val::I32(0); 1];
        exports
            .alloc
            .call(&mut store, &mut [Val::I32(TSDK_APP_KEY.len() as i32 + 1)], &mut alloc_res)
            .map_err(|e| Error::crypto(format!("alloc app_key failed: {e}")))?;
        let app_key_ptr = i32_val(&alloc_res, 0)?;
        write_cstring(&mut store, &exports.memory, app_key_ptr, TSDK_APP_KEY.as_bytes())?;
        exports
            .g
            .call(&mut store, &mut [Val::I32(TSDK_GAME_ID as i32), Val::I32(app_key_ptr)], &mut [])
            .map_err(|e| Error::crypto(format!("G(gameId) failed: {e}")))?;
        exports
            .free
            .call(&mut store, &mut [Val::I32(app_key_ptr)], &mut [])
            .map_err(|e| Error::crypto(format!("free app_key failed: {e}")))?;

        *self.inner.lock() = Some(Inner { store, instance, exports, user_bound: false });
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
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;

        let store = &mut inner.store;
        let exports = &inner.exports;

        // 1. alloc
        let len = (input.len().max(1)) as i32;
        let mut alloc_res = [Val::I32(0); 1];
        exports
            .alloc
            .call(&mut *store, &mut [Val::I32(len)], &mut alloc_res)
            .map_err(|e| Error::crypto(format!("alloc failed: {e}")))?;
        let ptr = i32_val(&alloc_res, 0)?;

        // 2. 写入
        write_bytes(store, &exports.memory, ptr, input)?;

        // 3. 加密/解密
        let func = if decrypt { &exports.decrypt } else { &exports.encrypt };
        func.call(&mut *store, &mut [Val::I32(ptr), Val::I32(input.len() as i32)], &mut [])
            .map_err(|e| {
                Error::crypto(format!(
                    "{} failed: {e}",
                    if decrypt { "decrypt" } else { "encrypt" }
                ))
            })?;

        // 4. 读出
        let result = read_bytes(store, &exports.memory, ptr, input.len())?;

        // 5. 释放
        exports
            .free
            .call(&mut *store, &mut [Val::I32(ptr)], &mut [])
            .map_err(|e| Error::crypto(format!("free failed: {e}")))?;

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
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        if inner.user_bound {
            return Ok(());
        }
        let store = &mut inner.store;
        let exports = &inner.exports;

        // alloc cstring → 写 openid → g(game_id, app_key_ptr)
        let cstr = format!("{value}\0");
        let cap = (cstr.len() as i32).max(64);
        let mut alloc_res = [Val::I32(0); 1];
        exports
            .alloc
            .call(&mut *store, &mut [Val::I32(cap)], &mut alloc_res)
            .map_err(|e| Error::crypto(format!("alloc failed: {e}")))?;
        let ptr = i32_val(&alloc_res, 0)?;
        write_bytes(store, &exports.memory, ptr, cstr.as_bytes())?;

        // 调 G(game_id, app_key_ptr) —— 把用户绑到 wasm
        exports
            .g
            .call(&mut *store, &mut [Val::I32(TSDK_GAME_ID as i32), Val::I32(ptr)], &mut [])
            .map_err(|e| Error::crypto(format!("G(bind) failed: {e}")))?;

        exports
            .free
            .call(&mut *store, &mut [Val::I32(ptr)], &mut [])
            .map_err(|e| Error::crypto(format!("free failed: {e}")))?;

        inner.user_bound = true;
        Ok(())
    }

    /// 拿加密的 init info（base64 字符串，原 TS 调 H()）
    pub fn get_encrypted_init_info(&self) -> Result<String> {
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        let exports = &inner.exports;

        let mut ret = [Val::I32(0); 1];
        exports
            .h
            .call(&mut *store, &[], &mut ret)
            .map_err(|e| Error::crypto(format!("H() failed: {e}")))?;
        let ptr = i32_val(&ret, 0)?;
        if ptr == 0 {
            return Ok(String::new());
        }
        let s = read_cstring(store, &exports.memory, ptr, 64 * 1024)?;
        Ok(s)
    }

    /// 拿到要发给服务器的数据（ACE AntiDataRequest.data）
    /// 原 TS: `getDataToServer(): Buffer`
    /// 实现:  alloc 4 字节 lengthPtr → N(lengthPtr) → 读 ptr + length
    pub fn get_data_to_server(&self) -> Result<Vec<u8>> {
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        let exports = &inner.exports;

        // 分配 4 字节给 length
        let mut alloc_res = [Val::I32(0); 1];
        exports
            .alloc
            .call(&mut *store, &mut [Val::I32(4)], &mut alloc_res)
            .map_err(|e| Error::crypto(format!("alloc failed: {e}")))?;
        let length_ptr = i32_val(&alloc_res, 0)?;

        // 调 N(lengthPtr) → 返回 data ptr (length 已写入 lengthPtr)
        let mut ret = [Val::I32(0); 1];
        exports
            .n
            .call(&mut *store, &mut [Val::I32(length_ptr)], &mut ret)
            .map_err(|e| Error::crypto(format!("N() failed: {e}")))?;
        let data_ptr = i32_val(&ret, 0)?;
        if data_ptr == 0 {
            exports
                .free
                .call(&mut *store, &mut [Val::I32(length_ptr)], &mut [])
                .map_err(|e| Error::crypto(format!("free failed: {e}")))?;
            return Ok(Vec::new());
        }

        // 读 length (i32 little-endian)
        let len_bytes = read_bytes(store, &exports.memory, length_ptr, 4)?;
        let data_len =
            i32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;

        // 读 data
        let data = if data_len > 0 {
            read_bytes(store, &exports.memory, data_ptr, data_len)?
        } else {
            Vec::new()
        };

        // 释放
        exports
            .free
            .call(&mut *store, &mut [Val::I32(data_ptr)], &mut [])
            .map_err(|e| Error::crypto(format!("free failed: {e}")))?;
        exports
            .free
            .call(&mut *store, &mut [Val::I32(length_ptr)], &mut [])
            .map_err(|e| Error::crypto(format!("free failed: {e}")))?;

        Ok(data)
    }

    /// 服务器回灌数据到 TSDK
    /// 原 TS: `sendDataFromServer(value: Uint8Array)` → O(ptr, length)
    pub fn send_data_from_server(&self, data: &[u8]) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        let exports = &inner.exports;

        let cap = data.len() as i32;
        let mut alloc_res = [Val::I32(0); 1];
        exports
            .alloc
            .call(&mut *store, &mut [Val::I32(cap)], &mut alloc_res)
            .map_err(|e| Error::crypto(format!("alloc failed: {e}")))?;
        let ptr = i32_val(&alloc_res, 0)?;
        write_bytes(store, &exports.memory, ptr, data)?;

        exports
            .o
            .call(&mut *store, &mut [Val::I32(ptr), Val::I32(data.len() as i32)], &mut [])
            .map_err(|e| Error::crypto(format!("O() failed: {e}")))?;

        exports
            .free
            .call(&mut *store, &mut [Val::I32(ptr)], &mut [])
            .map_err(|e| Error::crypto(format!("free failed: {e}")))?;
        Ok(())
    }

    /// 心跳 tick
    /// 原 TS: `heartbeatTick()` → M()
    pub fn heartbeat_tick(&self) -> Result<()> {
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        inner
            .exports
            .m
            .call(&mut *store, &[], &mut [])
            .map_err(|e| Error::crypto(format!("M() failed: {e}")))?;
        Ok(())
    }

    /// 处理收到的数据
    /// 原 TS: `processReceivedData()` → P()
    pub fn process_received_data(&self) -> Result<()> {
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        inner
            .exports
            .p
            .call(&mut *store, &[], &mut [])
            .map_err(|e| Error::crypto(format!("P() failed: {e}")))?;
        Ok(())
    }

    /// 发送状态
    /// 原 TS: `sendStatus()` → E()
    pub fn send_status(&self) -> Result<()> {
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        inner
            .exports
            .e
            .call(&mut *store, &[], &mut [])
            .map_err(|e| Error::crypto(format!("E() failed: {e}")))?;
        Ok(())
    }

    /// 速度作弊检测
    /// 原 TS: `detectSpeedHack(elapsedMs)` → fa(elapsedMs)
    pub fn detect_speed_hack(&self, elapsed_ms: u64) -> Result<()> {
        let mut guard = self.inner.lock();
        let inner = guard.as_mut().ok_or_else(|| Error::crypto("TSDK 未初始化"))?;
        let store = &mut inner.store;
        inner
            .exports
            .fa
            .call(&mut *store, &mut [Val::I32(elapsed_ms as i32)], &mut [])
            .map_err(|e| Error::crypto(format!("fa() failed: {e}")))?;
        Ok(())
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
         -> WasmResult<()> { Err(anyhow!("TSDK assertion")) },
    )?;

    // b: writeStringToFile — 阶段 0 跳过（返回 0）
    linker.func_wrap(
        "a",
        "b",
        |_c: wasmtime::Caller<'_, HostState>, _f: i32, _d: i32, _e: i32| -> WasmResult<i32> {
            Ok(0)
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

    // g: readFileToString — 阶段 0 跳过
    linker.func_wrap(
        "a",
        "g",
        |_c: wasmtime::Caller<'_, HostState>,
         _f: i32,
         _o: i32,
         _cap: i32,
         _e: i32|
         -> WasmResult<i32> { Ok(0) },
    )?;

    // h: clock_gettime — 写当前时间（微秒）
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
            let micros = if clock_id == 0 {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_micros() as i64)
                    .unwrap_or(0)
            } else {
                0
            };
            let mem = c.data().memory.clone();
            if let Some(m) = mem {
                let data = m.data_mut(&mut c);
                let low = (micros as u64 & 0xFFFF_FFFF) as u32;
                let high = ((micros as u64) >> 32) as u32;
                write_u32_le(data, out, low);
                write_u32_le(data, out + 4, high);
            }
            Ok(0)
        },
    )?;

    // i: dataDir
    linker.func_wrap(
        "a",
        "i",
        |mut c: wasmtime::Caller<'_, HostState>, ptr: i32, cap: i32| -> WasmResult<i32> {
            let dir = format!("{}/", c.data().data_dir);
            let bytes = dir.as_bytes();
            if bytes.len() < cap as usize {
                write_cstring_in_caller(&mut c, ptr, bytes)?;
                Ok(1)
            } else {
                Ok(0)
            }
        },
    )?;

    // j: deviceText
    linker.func_wrap(
        "a",
        "j",
        |mut c: wasmtime::Caller<'_, HostState>, ptr: i32, cap: i32| -> WasmResult<i32> {
            let text = b"rust-runtime;darwin;1.0;Rust;";
            if text.len() < cap as usize {
                write_cstring_in_caller(&mut c, ptr, text)?;
                Ok(1)
            } else {
                Ok(0)
            }
        },
    )?;

    // k: RUNTIME_TABLE
    linker.func_wrap(
        "a",
        "k",
        |mut c: wasmtime::Caller<'_, HostState>, ptr: i32, cap: i32| -> WasmResult<i32> {
            if (RUNTIME_TABLE.len() as i32) <= cap {
                let mem = c.data().memory.clone();
                if let Some(m) = mem {
                    let data = m.data_mut(&mut c);
                    let off = ptr as usize;
                    if off + RUNTIME_TABLE.len() <= data.len() {
                        data[off..off + RUNTIME_TABLE.len()].copy_from_slice(&RUNTIME_TABLE);
                        return Ok(1);
                    }
                }
            }
            Ok(0)
        },
    )?;

    // l: arch (2 = wasm32)
    linker
        .func_wrap("a", "l", |_c: wasmtime::Caller<'_, HostState>| -> WasmResult<i32> { Ok(2) })?;

    // m: appId
    linker.func_wrap(
        "a",
        "m",
        |mut c: wasmtime::Caller<'_, HostState>, ptr: i32, cap: i32| -> WasmResult<i32> {
            let bytes = MINI_PROGRAM_APP_ID.as_bytes();
            if bytes.len() < cap as usize {
                write_cstring_in_caller(&mut c, ptr, bytes)?;
                Ok(1)
            } else {
                Ok(0)
            }
        },
    )?;

    // n: appId (另一处)
    linker.func_wrap(
        "a",
        "n",
        |mut c: wasmtime::Caller<'_, HostState>, ptr: i32, cap: i32| -> WasmResult<i32> {
            let bytes = MINI_PROGRAM_APP_ID.as_bytes();
            if bytes.len() < cap as usize {
                write_cstring_in_caller(&mut c, ptr, bytes)?;
                Ok(1)
            } else {
                Ok(0)
            }
        },
    )?;

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

    // p: stat — 阶段 0 跳过
    linker.func_wrap(
        "a",
        "p",
        |_c: wasmtime::Caller<'_, HostState>, _f: i32| -> WasmResult<i32> { Ok(0) },
    )?;

    // q: serverTime — 写本地时间（不发 anticheatexpert 请求）
    linker.func_wrap(
        "a",
        "q",
        |mut c: wasmtime::Caller<'_, HostState>, out: i32| -> WasmResult<i32> {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i32)
                .unwrap_or(0);
            let mem = c.data().memory.clone();
            if let Some(m) = mem {
                write_u32_le(m.data_mut(&mut c), out, now as u32);
            }
            Ok(1)
        },
    )?;

    // r: memory.grow 失败时调用（返回 0，wasmtime 会把这个 trap 转成自己的 error）
    linker.func_wrap(
        "a",
        "r",
        |_c: wasmtime::Caller<'_, HostState>, size: i32| -> WasmResult<i32> {
            Err(anyhow!("TSDK 内存扩展失败: {size}"))
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

    // t: appendStringToFile
    linker.func_wrap(
        "a",
        "t",
        |_c: wasmtime::Caller<'_, HostState>, _f: i32, _d: i32, _e: i32| -> WasmResult<i32> {
            Ok(0)
        },
    )?;

    // u: abort
    linker.func_wrap("a", "u", |_c: wasmtime::Caller<'_, HostState>| -> WasmResult<()> {
        Err(anyhow!("TSDK aborted"))
    })?;

    // v: reportEvent
    linker.func_wrap(
        "a",
        "v",
        |_c: wasmtime::Caller<'_, HostState>, _p: i32, _l: i32| -> WasmResult<i32> { Ok(0) },
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

// ===== 字节读写工具 =====

fn write_bytes(
    store: &mut Store<HostState>,
    memory: &Memory,
    ptr: i32,
    bytes: &[u8],
) -> Result<()> {
    let data = memory.data_mut(store);
    let off = ptr as usize;
    if off + bytes.len() > data.len() {
        return Err(Error::crypto(format!("write out of bounds: ptr={ptr}, len={}", bytes.len())));
    }
    data[off..off + bytes.len()].copy_from_slice(bytes);
    Ok(())
}

fn read_bytes(store: &Store<HostState>, memory: &Memory, ptr: i32, len: usize) -> Result<Vec<u8>> {
    let data = memory.data(store);
    let off = ptr as usize;
    if off + len > data.len() {
        return Err(Error::crypto(format!("read out of bounds: ptr={ptr}, len={len}")));
    }
    Ok(data[off..off + len].to_vec())
}

fn write_cstring(
    store: &mut Store<HostState>,
    memory: &Memory,
    ptr: i32,
    bytes: &[u8],
) -> Result<()> {
    write_bytes(store, memory, ptr, bytes)?;
    let null_pos = (ptr as usize).saturating_add(bytes.len());
    let data = memory.data_mut(store);
    if null_pos < data.len() {
        data[null_pos] = 0;
    }
    Ok(())
}

/// 在 host function 闭包内写入 cstring（caller 借用处理）
fn write_cstring_in_caller(
    caller: &mut wasmtime::Caller<'_, HostState>,
    ptr: i32,
    bytes: &[u8],
) -> WasmResult<()> {
    let mem = caller.data().memory.clone().ok_or_else(|| anyhow!("memory not set"))?;
    let data = mem.data_mut(caller);
    let off = ptr as usize;
    if off + bytes.len() + 1 > data.len() {
        return Err(anyhow!("cstring write out of bounds"));
    }
    data[off..off + bytes.len()].copy_from_slice(bytes);
    data[off + bytes.len()] = 0;
    Ok(())
}

fn write_u32_le(data: &mut [u8], ptr: i32, value: u32) {
    let off = ptr as usize;
    if off + 4 <= data.len() {
        data[off..off + 4].copy_from_slice(&value.to_le_bytes());
    }
}

/// 从 wasm 内存读 cstring（以 NUL 结尾）
fn read_cstring(
    store: &Store<HostState>,
    memory: &Memory,
    ptr: i32,
    max_length: usize,
) -> Result<String> {
    let data = memory.data(store);
    let off = ptr as usize;
    if off >= data.len() {
        return Err(Error::crypto("read_cstring: pointer out of bounds"));
    }
    let cap = (data.len() - off).min(max_length);
    let mut end = off;
    while end < off + cap && data[end] != 0 {
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
