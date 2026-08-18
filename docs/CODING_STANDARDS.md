# Coding Standards — qq-farm-rust

企业级约定。新增代码必须遵守；存量按治理阶段收敛。

## Crate 边界

| Crate | 允许 | 禁止 |
|-------|------|------|
| `qq-farm-core` | 领域、协议、运行时、持久化、常量 | `axum` / `tauri` / 任何 UI |
| `qq-farm-app` | Command / Query / Event 门面；只依赖 core | HTTP Status、Tauri API、路由、View |
| `qq-farm-server` | Axum / Socket.IO 协议适配 → 调 app | 领域算法、复制 start/stop 编排 |
| `qq-farm-desktop` | Tauri IPC → 调 **同一套** app；默认内嵌 RuntimeEngine | 依赖 server；把 Tauri 泄漏进 app；经 localhost HTTP（非默认） |

**硬性规则：** `server` 与 `desktop` **禁止**互相依赖；`qq-farm-app` **禁止**依赖 `tauri`。

## 常量

- 游戏协议相关（RPC 服务名、活动/道具 ID、操作码、TTL）放 `qq_farm_core::constants`。
- 进程/部署配置（端口、CORS、max_workers）放各入口的 `*Config`（如 `ServerConfig`），不写死在 handler。
- 禁止新增与 `PHASE_NAMES` / 活动 ID 同类的散落字面量；先查 constants。

## 静态变量分级

见 [STATICS_INVENTORY.md](./STATICS_INVENTORY.md)。

- **L1** 进程单例（引擎、路径）：允许。
- **L2** JSON store 全局：短期可接受；统一访问入口。
- **L3** 应按账号隔离却用全局：禁止新增；存量迁到 per-account 状态。

## 错误与 API

- core 热路径统一 `qq_farm_core::error::Error`，避免 `anyhow` 泄漏到公共 API。
- server：`core::Error` / `AppError` → `ApiError`；不要靠解析 `Display` 字符串映射业务码。
- 新公共 API **禁止**以 `serde_json::Value` 作为主返回类型；JSON 仅在 HTTP 边界序列化。

## 模块分包

- 业务按域：`farm` / `friend` / `activity` / `commerce` / `daily` / `auth` / `task` / `warehouse`。
- 基础设施（`json_db`、`rate_limiter`、`automation`、`stats`、`status`、`panel_log`）属 `infra`，不是业务 service。

## 测试

- 行为变更：`cargo test --workspace`；server E2E 用 `--test-threads=1`。
- 跨用户 ACL 负例必须覆盖。
