# qq-farm-rust

QQ 农场多账号挂机的 Rust 重写。协议、调度和 HTTP/Socket.IO 合约对齐原 [qq-farm-bot](https://github.com/it00021hot/qq-farm-bot)（TypeScript + Vue），管理面板继续用原项目的 Vue，不改前端去迁就后端。

## 仓库结构

```
qq-farm-rust/
├── crates/
│   ├── qq-farm-core/      # 网关、登录、农场/好友调度、活动中心、统计
│   ├── qq-farm-server/    # HTTP API + Socket.IO（默认 3007）
│   └── qq-farm-cli/       # 调试命令（crypto / farm / friend / wx-code）
├── proto/                 # 游戏 protobuf
├── assets/activity-data/  # 活动静态数据
├── scripts/               # 辅助脚本
└── .env.example
```

数据目录默认 `~/.qq-farm-rust/`（可用 `FARM_DATA_DIR` 覆盖），不要提交进去。

## 环境

- Rust 1.75+（建议用当前 stable）
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

### 微信扫码登录

1. 面板里给账号选微信平台，走扫码登录。
2. 扫码拿到的登录码是一次性的。服务重启后必须重新扫，不能拿已经用过的码重连。

## 环境变量

见 `.env.example`。常用项：

| 变量 | 默认 | 说明 |
|------|------|------|
| `FARM_SERVER_URL` | `wss://gate-obt.nqf.qq.com/prod/ws` | 游戏网关 |
| `FARM_OS` | `Windows` | 客户端 OS |
| `FARM_CLIENT_VERSION` | `1.13.1.6_20260723` | 客户端版本 |
| `ADMIN_PORT` | `3007` | HTTP / Socket.IO 端口 |
| `RUST_LOG` | `info` | 日志级别 |
| `FARM_DATA_DIR` | `~/.qq-farm-rust` | 账号、用户、卡密、配置 |

## 运行时行为

登录成功后，worker 按账号配置串行跑农场 / 帮助 / 偷菜：

- **农场**：除草除虫浇水 → 收获 → 铲除枯株 → 种植 → 施肥 / 解锁升级
- **偷菜**：按好友列表 `steal_plant_num` 筛选并排序，只去有菜可偷的好友
- **出售**：收获或偷菜成功后，按「果实 + 可出售」自动卖出（需打开 `sell`）
- **面板**：`status:update` 推送效率（`sessionExpGained` / `uptime`），`log:new` 推送运行日志

安静时段、好友总开关、蔬菜黑名单与原版配置项一致。

## CLI

```bash
cargo run -p qq-farm-cli -- farm-demo
cargo run -p qq-farm-cli -- friend-demo
cargo run -p qq-farm-cli -- wx-code --help
```

## 测试

```bash
cargo test --workspace
```

## 许可

仅供学习使用。
