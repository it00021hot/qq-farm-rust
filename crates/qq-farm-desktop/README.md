# qq-farm-desktop

Tauri v2 桌面端：进程内嵌 `RuntimeEngine`，经 `qq-farm-app` 暴露 IPC，前端为仓库根目录 `desktop-ui/`（SoybeanAdmin）。

## 依赖

- Tauri 2（托盘 / 菜单 / updater）
- `qq-farm-app` / `qq-farm-core`（**禁止**依赖 `qq-farm-server`；**禁止**把 Tauri 泄漏进 app）

## 开发

```bash
# 仓库根目录：先装前端依赖
pnpm -C desktop-ui i

cd crates/qq-farm-desktop && cargo tauri dev
```

环境变量与 server 共用（如 `TSDK_WASM_PATH`、`FARM_SERVER_URL`、`MAX_WORKERS`）。

- 开发：数据目录同 `qq-farm-core::get_data_dir()`（默认仓库 `data/`，可用 `FARM_DATA_DIR` 覆盖）
- 安装包：未设 `FARM_DATA_DIR` 时为 macOS `~/Library/Application Support/QQFarmRust`、Windows `%LOCALAPPDATA%\QQFarmRust`；`tsdk.wasm` / `game_config` 从 bundle resources 加载

## 壳层

- macOS：App / Edit / Window + **应用**（打开数据目录、检查更新）；Cmd+Q 退出
- 全平台托盘：显示主窗口 / 打开数据目录 / 检查更新 / 关于 / 退出；左键显隐主窗口
- 关窗隐藏到托盘，不退出
- 启动约 5 秒后静默检查 GitHub Releases；托盘/菜单「检查更新」总是给出结果

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
- 无面板登录 / 无「在浏览器中打开」（IPC 内嵌，不跑 localhost HTTP）
