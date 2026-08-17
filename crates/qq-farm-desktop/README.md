# qq-farm-desktop

Tauri v2 桌面端：进程内嵌 `RuntimeEngine`，经 `qq-farm-app` 暴露 IPC，前端为仓库根目录 `desktop-ui/`（SoybeanAdmin）。

## 依赖

- Tauri 2
- `qq-farm-app` / `qq-farm-core`（**禁止**依赖 `qq-farm-server`；**禁止**把 Tauri 泄漏进 app）

## 开发

```bash
# 仓库根目录：先装前端依赖
pnpm -C desktop-ui i

# 启动桌面（会拉起 Vite + Rust）
cargo tauri dev -p qq-farm-desktop
# 或在本 crate 目录：
cd crates/qq-farm-desktop && cargo tauri dev
```

环境变量与 server 共用（如 `TSDK_WASM_PATH`、`FARM_SERVER_URL`、`MAX_WORKERS`）。数据目录同 `qq-farm-core` 的 `get_data_dir()`。

## 架构

```text
desktop-ui (Soybean)
    │ invoke / listen("app-event")
    ▼
Tauri commands / event bridge
    │
    ▼
qq-farm-app facades → RuntimeEngine (core)
```

- ACL：`AclPolicy::LocalOwner`
- Scaffold 命令：`desktop_ready` / `get_snapshot` / `list_accounts` / `get_settings`
