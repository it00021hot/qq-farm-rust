---
name: qq-farm-rust
description: >-
  Develop and refactor the qq-farm-rust workspace (QQ farm multi-account bot in Rust):
  crate boundaries (core/app/server/desktop/cli), constants/infra/domain packaging,
  panel API parity with qq-farm-bot, ACL, AppEvent, and Tauri v2 desktop + SoybeanAdmin.
  Use when editing qq-farm-rust, qq-farm-core, qq-farm-app, qq-farm-server,
  qq-farm-desktop, desktop-ui, farm/friend/activity automation, or SYNC/ARCHITECTURE/CODING_STANDARDS.
---

# qq-farm-rust

QQ 农场多账号挂机 Rust 重写。改代码前先对齐分层与契约，再动手。

## Always read first

| Doc | When |
|-----|------|
| [`docs/ARCHITECTURE.md`](../../../docs/ARCHITECTURE.md) | 新功能落在哪一层 / 多前端 |
| [`docs/CODING_STANDARDS.md`](../../../docs/CODING_STANDARDS.md) | 写/改任何 Rust |
| [`docs/SYNC.md`](../../../docs/SYNC.md) | 业务行为或面板 API 对齐 |
| [`docs/STATICS_INVENTORY.md`](../../../docs/STATICS_INVENTORY.md) | 新增全局状态 / 多账号隔离 |

（路径均相对仓库根 `qq-farm-rust/`。）

## Crate topology (hard rules)

```text
Vue (bot/web) ──HTTP/Socket──► qq-farm-server ──► qq-farm-app ──► qq-farm-core
desktop-ui ──────Tauri IPC──► qq-farm-desktop ──► qq-farm-app ──► qq-farm-core
CLI / demo ─────────────────────────────────────► core（运维可调 app）
```

| Crate | Do | Don't |
|-------|----|-------|
| `qq-farm-core` | 领域、协议、runtime、store、`constants`、`infra` | `axum` / `tauri` / UI |
| `qq-farm-app` | Command/Query/Event；ACL；start/stop；聚合 | HTTP Status、Tauri、路由 |
| `qq-farm-server` | 协议适配 → 调 app → `ApiError` | 领域算法、复制编排 |
| `qq-farm-desktop` | Tauri IPC → **同一套** app；默认内嵌 `RuntimeEngine` | 依赖 server；把 Tauri 放进 app |
| `qq-farm-cli` | demo / 运维 | 复制 core mock（用共用辅助） |
| `desktop-ui/` | SoybeanAdmin 风格；IPC service/store/views 分层 | 私自主题；单文件堆业务；复制 bot web 组件 |

**server ↔ desktop 禁止互相依赖。`qq-farm-app` 禁止依赖 `tauri`。**

## Where to put code

1. **常量**（RPC 名、活动/道具 ID、TTL、阶段名）→ `qq_farm_core::constants`，禁止散落魔法值。
2. **部署配置**（端口、CORS、max_workers）→ `ServerConfig` / 各入口 `*Config`。
3. **基础设施** → `core::infra`（`json_db`、`rate_limiter`、`automation`、`stats`、`status`、`panel_log`）。
4. **业务域** → `core::services::{farm,friend,activity,commerce,daily,auth,tasks,warehouse}`。
5. **多前端共用编排**（ACL、start/stop、礼包聚合、事件）→ `qq-farm-app`，不要堆在 Axum handler 或 Tauri command。
6. **类型**：store 用 `AccountRecord`，runtime 用 `AccountSession`；错误 `core::Error` → `AppError` → `ApiError` / desktop `IpcError`。
7. **新公共 API** 禁止以 `serde_json::Value` 为主返回类型；JSON 只在 HTTP / IPC 边界。

## Statics policy

- **L1** 进程单例：允许（TSDK、路径、logger）。
- **L2** JSON store 全局：短期可接受，统一入口。
- **L3** 应按账号隔离：禁止新增 `pub static`；Friend/automation 用 per-account API（`*_for(account_id)` / `FriendRuntimeState`）。

## Parity / SYNC

- 对齐对象：自动化行为 + 面板 HTTP/Socket **语义**（不是文件 1:1）。
- 行为或契约变更：在 `docs/SYNC.md` 文末「更新记录」追加一条。
- 验收只认「齐」，不接受「基本齐」留下已知差。
- Vue 面板在原 `qq-farm-bot/web`；本仓默认 `ADMIN_PORT=3007`，鉴权头 `x-admin-token`。
- 桌面前端在本仓 `desktop-ui/`，经 Tauri IPC，不用面板 token。

## Implementation checklist

```
- [ ] 变更落在正确 crate（core / app / server / desktop / cli）或 desktop-ui 分层
- [ ] 无新增 L3 全局；无 handler/command 内领域算法
- [ ] 常量进 constants；配置进 *Config
- [ ] 账号操作走 ACL（app::accounts + PanelUser / LocalOwner）
- [ ] 错误经 AppError/ApiError/IpcError，不解析 Display 字符串
- [ ] cargo check / 相关 test；server E2E 用 --test-threads=1
- [ ] desktop-ui：Soybean/Naive 组件优先；无私自主题；typings/service/store/views 分层
- [ ] 业务/契约变更已写 SYNC.md
```

## Common commands

```bash
cargo check --workspace
cargo test --workspace
cargo test -p qq-farm-server --test e2e_integration -- --test-threads=1
RUST_LOG=info ADMIN_PORT=3007 cargo run -p qq-farm-server
pnpm -C desktop-ui i && pnpm -C desktop-ui build
cd crates/qq-farm-desktop && cargo tauri dev
```

## Desktop (Tauri) notes

- **定位**：个人免费开源；无面板登录 / RBAC / 用户管理 / 卡密；仅多农场账号 + 农场业务。
- Crate：`crates/qq-farm-desktop`；只依赖 `qq-farm-app` + Tauri；前端 `desktop-ui/`。
- ACL：进程内固定 `AclPolicy::LocalOwner`；不暴露 users / login / cards IPC。
- 菜单：10 项（home + 9 个 farm_*），对齐 qq-farm-web 农场页；无 `/system/*`、无登录门闸。
- 订阅状态：`AppContext::subscribe_events` → `emit("app-event")`，不要复制 Socket.IO。
- IPC：按域 `account` / `farm` / `friend` / `activity` / `commerce` / `settings` / `config` / `snapshot`（含 wx_login）。
- UI：沿用 Soybean 主题与组件；`service/api` → `invoke`；views 来自 web 农场页改接 Tauri。
