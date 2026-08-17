# qq-farm-desktop

GPUI 桌面端：进程内嵌 `RuntimeEngine`，经 `qq-farm-app` 调用业务，功能对齐 `qq-farm-bot/web`。

## 依赖

- `gpui` 0.2.2（crates.io）
- `gpui-component` 0.5.1

## 运行

```bash
# 在 qq-farm-rust 仓库根目录
cargo run -p qq-farm-desktop
```

环境变量与 server 共用（如 `TSDK_WASM_PATH`、`FARM_SERVER_URL`、`MAX_WORKERS`）。数据目录同 `qq-farm-core` 的 `get_data_dir()`。

## 架构

```text
GPUI Views → AppState → qq-farm-app facades → RuntimeEngine (core)
                ↑
         AppEvent / 定时刷新
```

- ACL：`AclPolicy::LocalOwner`（无面板登录页）
- **禁止**依赖 `qq-farm-server`；默认不走 localhost HTTP
- 导航：概览 / 个人 / 活动 / 商城 / 神秘商人 / 好友 / 分析 / 设置 / 游戏配置 / 本机运维

## Windows 构建说明

首次编译会拉取大量 gpui 原生依赖，耗时较长。若依赖解析失败，确认使用 crates.io 的 `gpui = "0.2.2"` 与 `gpui-component = "0.5.1"`（见 workspace `Cargo.toml`），勿混用未 pin 的 zed git 主线。
