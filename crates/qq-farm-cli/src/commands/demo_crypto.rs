//! 阶段 0 验证：加载 `tsdk.wasm`，跑一次加密 + 解密往返。
//!
//! 流程：
//! 1. 定位 `assets/tsdk.wasm`
//! 2. 用 `qq_farm_core::crypto::tsdk::TsdkRuntime::load` 加载
//! 3. 加密一段明文 → 输出 hex
//! 4. 解密回明文 → **必须等于原明文**
//! 5. 打印耗时与 wasm memory 大小

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use clap::Args as ClapArgs;
use qq_farm_core::config::get_resource_path;
use qq_farm_core::crypto::tsdk::TsdkRuntime;

fn default_wasm_path() -> PathBuf {
    std::env::var("TSDK_WASM_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| get_resource_path(&["assets", "tsdk.wasm"]))
}

#[derive(Debug, ClapArgs)]
pub struct CryptoArgs {
    /// 自定义 wasm 路径（默认 workspace `assets/tsdk.wasm`）
    #[arg(long)]
    pub wasm: Option<PathBuf>,

    /// 自定义明文（十六进制字符串；默认用一段中文）
    #[arg(long)]
    pub input_hex: Option<String>,

    /// 自定义明文（UTF-8 字符串；与 `--input-hex` 互斥）
    #[arg(long, conflicts_with = "input_hex")]
    pub input: Option<String>,

    /// 数据目录（传给 TSDK runtime）
    #[arg(long, default_value = "./data/tsdk-demo")]
    pub data_dir: PathBuf,
}

pub fn execute(args: CryptoArgs) -> Result<()> {
    let wasm_candidate = args.wasm.unwrap_or_else(default_wasm_path);
    let wasm_path = locate_wasm(&wasm_candidate)?;
    println!("[demo] wasm path: {}", wasm_path.display());

    // 2. 加载 + 初始化
    let load_start = Instant::now();
    let runtime = TsdkRuntime::load(&wasm_path, args.data_dir.as_os_str().to_string_lossy().to_string())
        .context("TSDK 加载失败")?;
    println!(
        "[demo] TSDK 初始化耗时: {} ms, wasm memory: {} bytes ({:.2} KB)",
        load_start.elapsed().as_millis(),
        runtime.memory_size(),
        runtime.memory_size() as f64 / 1024.0
    );

    // 3. 准备明文
    let plaintext = match (args.input.as_deref(), args.input_hex.as_deref()) {
        (_, Some(hex)) => hex::decode(hex).context("input-hex 解析失败")?,
        (Some(s), _) => s.as_bytes().to_vec(),
        (None, None) => "你好，QQ 农场！hello qq farm 🚜".as_bytes().to_vec(),
    };
    println!(
        "[demo] 明文长度: {} bytes, 前 32 字节: {}",
        plaintext.len(),
        hex_preview(&plaintext, 32)
    );

    // 4. 加密
    let enc_start = Instant::now();
    let ciphertext = runtime.encrypt(&plaintext).context("加密失败")?;
    let enc_elapsed = enc_start.elapsed();
    println!(
        "[demo] 加密耗时: {:.3} ms, 密文长度: {} bytes, hex: {}",
        enc_elapsed.as_secs_f64() * 1000.0,
        ciphertext.len(),
        hex_preview(&ciphertext, 64)
    );

    // 5. 解密
    let dec_start = Instant::now();
    let decrypted = runtime.decrypt(&ciphertext).context("解密失败")?;
    let dec_elapsed = dec_start.elapsed();
    println!(
        "[demo] 解密耗时: {:.3} ms, 回得明文: {}",
        dec_elapsed.as_secs_f64() * 1000.0,
        hex_preview(&decrypted, 32)
    );

    // 6. 验证往返一致
    if decrypted != plaintext {
        return Err(anyhow!(
            "往返一致性校验失败：明文 {} bytes，解密得 {} bytes，前 32 字节：{} vs {}",
            plaintext.len(),
            decrypted.len(),
            hex_preview(&plaintext, 32),
            hex_preview(&decrypted, 32)
        ));
    }
    println!("[demo] ✓ 往返一致性校验通过");

    // 7. 清理
    runtime.destroy();
    println!("[demo] 完成，runtime 已销毁");

    Ok(())
}

fn locate_wasm(custom: &std::path::Path) -> Result<PathBuf> {
    if custom.exists() {
        return Ok(custom.to_path_buf());
    }
    let fallback = get_resource_path(&["assets", "tsdk.wasm"]);
    if fallback.exists() {
        return Ok(fallback);
    }
    Err(anyhow!(
        "找不到 tsdk.wasm：试过 {} 和 {}",
        custom.display(),
        fallback.display()
    ))
}

fn hex_preview(bytes: &[u8], max: usize) -> String {
    let len = bytes.len().min(max);
    let head = hex::encode(&bytes[..len]);
    if bytes.len() > max {
        format!("{head}...({} bytes total)", bytes.len())
    } else {
        head
    }
}
