# qq-farm-rust

QQ 农场多账号挂机的 Rust 重写。协议、调度和 HTTP/Socket.IO 合约对齐原 [qq-farm-bot](https://github.com/it00021hot/qq-farm-bot)（TypeScript + Vue），管理面板继续用原项目的 Vue，不改前端去迁就后端。

与原 `core` 的**业务同步状态、缺口与更新记录**见 [docs/SYNC.md](docs/SYNC.md)（对齐以业务目标一致为准；每次业务对齐请追加更新记录）。

## 仓库结构

```
qq-farm-rust/
├── crates/
│   ├── qq-farm-core/      # 网关、登录、农场/好友调度、活动中心、统计
│   ├── qq-farm-app/       # UI 无关应用门面（server / desktop 共用）
│   ├── qq-farm-server/    # HTTP API + Socket.IO（默认 3007）
│   ├── qq-farm-desktop/   # Tauri v2 桌面宿主（IPC → app）
│   └── qq-farm-cli/       # 调试命令（crypto / farm / friend / wx-code）
├── desktop-ui/            # SoybeanAdmin 桌面前端（与客户端共存）
├── proto/                 # 游戏 protobuf
├── assets/activity-data/  # 活动静态数据
├── scripts/               # 辅助脚本
├── docs/ARCHITECTURE.md   # 多前端拓扑
├── docs/CODING_STANDARDS.md
└── .env.example
```

数据目录默认 `~/.qq-farm-rust/`（可用 `FARM_DATA_DIR` 覆盖），不要提交进去。

## 环境

- Rust 1.75+（建议用当前 stable）
- `protoc`（protobuf compiler，生成协议代码；Windows 可装到 `%USERPROFILE%\tools\protoc\bin` 并加入 PATH / 设置 `PROTOC`）
- macOS / Linux
- 管理面板：原项目 `qq-farm-bot/web`（Vite 5173，把 `/api` 和 `/socket.io` 代理到 3007）

## 编译与启动

```bash
cp .env.example .env
cargo build --release
RUST_LOG=info ADMIN_PORT=3007 ./target/release/qq-farm-server
```

健康检查：

```bash
curl http://127.0.0.1:3007/health
```

开发时也可以直接：

```bash
cargo run -p qq-farm-server
```

### 管理面板

在 `qq-farm-bot/web`：

```bash
pnpm install
pnpm dev
```

浏览器打开 `http://127.0.0.1:5173`。面板登录、加号、农场操作都打到本仓库的 3007。

### 桌面端（Tauri v2 + SoybeanAdmin）

```bash
# 前端依赖
pnpm -C desktop-ui i

# 需已安装 Tauri CLI：cargo install tauri-cli --version "^2"
cd crates/qq-farm-desktop && cargo tauri dev
```

桌面端经 IPC 调 `qq-farm-app`（LocalOwner），**不**走 3007 HTTP。浏览器面板与桌面前端并存。

### 微信扫码登录

1. 面板里给账号选微信平台，走扫码登录。
2. 网关登录码仍是一次性的；应用宝 `login_buffer` 会随账号落盘。掉线或进程重启后会自动换新码重连，无需再扫。授权失效时才需要重新扫码。

## 环境变量

见 `.env.example`。常用项：

| 变量 | 默认 | 说明 |
|------|------|------|
| `FARM_SERVER_URL` | `wss://gate-obt.nqf.qq.com/prod/ws` | 游戏网关 |
| `FARM_OS` | `Windows` | 客户端 OS |
| `FARM_CLIENT_VERSION` | `1.13.1.6_20260723` | 客户端版本 |
| `ADMIN_PORT` | `3007` | HTTP / Socket.IO 端口 |
| `RUST_LOG` | `info` | 日志级别 |
| `FARM_DATA_DIR` | `~/.qq-farm-rust` | 账号、用户、配置 |

## 运行时行为

登录成功后，worker 按账号配置串行跑农场 / 帮助 / 偷菜：

- **农场**：除草除虫浇水 → 收获 → 铲除枯株 → 种植 → 施肥 / 解锁升级
- **偷菜**：按好友列表 `steal_plant_num` 筛选并排序；进场后再用 `stealers`/`steal_num` 判断「我还能偷」；进场无可偷则跳过同指标空转（不再刷「开始批量偷菜」）
- **出售**：收获或偷菜成功后，按「果实 + 可出售」自动卖出（需打开 `sell`）
- **面板**：`status:update` 推送效率（`sessionExpGained` / `uptime`），`log:new` 推送运行日志

安静时段、好友总开关、蔬菜黑名单与原版配置项一致。

## CLI

`qq-farm-cli` 里的 demo 子命令（`demo-crypto`、`worker-demo`、`farm-demo`、`friend-demo`）仅用于本地开发与阶段验证，依赖 mock WebSocket，**不是生产入口**。生产入口：`qq-farm-server`（面板）或 `qq-farm-desktop`（Tauri）。

```bash
cargo run -p qq-farm-cli -- farm-demo
cargo run -p qq-farm-cli -- friend-demo
cargo run -p qq-farm-cli -- wx-code --help
```

## 测试

```bash
# 需本机已安装 protoc（protobuf 编译器），并在 PATH / PROTOC 中可见
cargo test --workspace

# 面板 API 冒烟（独立进程，默认会起临时端口）
cargo test -p qq-farm-server --test e2e_integration -- --test-threads=1
```

鉴权请求头为 `x-admin-token: <token>`（登录接口返回的 `data.token`），不是 `Authorization: Bearer`。

## 许可

仅供学习使用。
