# qq-farm-rust

QQ 农场多账号挂机工具的 **Rust 重写版**。

目标：在保留原项目 [qq-farm-bot](https://github.com/XyhTender/qq-farm-bot) 功能与 API 兼容性的前提下，把后端从 Node.js 迁移到 Rust，以**显著降低运行时内存占用**（10 账号场景预计从 ~300-500MB 压到 ~30-80MB）。

> 🚧 **阶段 0 — 最小可验证骨架**
>
> 当前只完成 workspace 骨架 + 关键技术点（prost 全量编译 + wasmtime 加载 tsdk.wasm）的 PoC。
> 业务模块（farm / friend / runtime / controllers）尚未迁移。

---

## 目录结构

```
qq-farm-rust/
├── Cargo.toml                  # workspace 根
├── rustfmt.toml
├── clippy.toml
├── .cargo/config.toml
├── proto/                      # 34 个 .proto（从原项目迁移）
├── assets/
│   └── tsdk.wasm               # 微信 SDK 加密模块（include_bytes! 嵌入）
└── crates/
    ├── qq-farm-core/           # 业务核心库
    │   ├── build.rs            # prost 编译
    │   └── src/
    │       ├── config/         # 配置层
    │       ├── models/         # 领域模型
    │       ├── proto/          # protobuf 生成的 Rust 类型
    │       ├── network/        # WebSocket 客户端 + 编解码（占位）
    │       ├── crypto/         # tsdk.wasm 封装（本阶段重点）
    │       ├── runtime/        # 多账号调度引擎（占位）
    │       ├── services/       # 业务服务（占位）
    │       └── utils/          # 日志、时间等
    ├── qq-farm-server/         # HTTP + WebSocket 服务（占位）
    │   └── src/
    │       ├── routes/
    │       └── socket/
    └── qq-farm-cli/            # CLI 工具
        └── src/
            └── commands/       # demo 子命令
```

---

## 阶段 0 验证

### 1. 编译全部 34 个 proto

```bash
cargo build -p qq-farm-core
```

这会触发 `build.rs` 跑 `prost-build`，把所有 .proto 编译为 Rust 类型，输出到 `crates/qq-farm-core/src/proto/generated/`。

### 2. 跑加密 demo

```bash
cargo run -p qq-farm-cli -- demo-crypto
```

预期输出：
- 加载 `assets/tsdk.wasm` 成功
- 加密一段明文 → 输出 hex
- 解密回明文 → **必须等于原明文**（往返一致）
- 打印耗时

### 3. 跑测试

```bash
cargo test --workspace
```

---

## 后续阶段

- **阶段 1** — 迁移 `network/`（WebSocket 客户端 + 编解码器）
- **阶段 2** — 迁移 `runtime/`（多账号调度引擎 + worker）
- **阶段 3** — 迁移 `services/farm` / `services/friend` 等业务
- **阶段 4** — 迁移 `controllers/`（axum + socketioxide，保持 Vue 前端 API 兼容）
- **阶段 5** — 端到端联调，整机内存对比

---

## 技术选型

| 用途 | crate |
|------|-------|
| 异步运行时 | tokio 1 |
| HTTP | axum 0.8 |
| Protobuf | prost 0.13 |
| WASM | wasmtime 33 |
| 错误 | thiserror 2 / anyhow 1 |
| 日志 | tracing 0.1 |
| 序列化 | serde 1 / serde_json 1 |
| CLI | clap 4 |
| HTTP 客户端 | reqwest 0.12 |

---

## 免责声明

本项目仅供学习与研究用途，与原项目一致。
