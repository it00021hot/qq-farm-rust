# qq-farm-rust

QQ 农场多账号挂机的 Rust 重写。协议与调度对齐原 [qq-farm-bot](https://github.com/it00021hot/qq-farm-bot)（TypeScript + Vue），前端为内置 SoybeanAdmin 桌面 UI，**只维护桌面版**（原 HTTP API 服务 crate `qq-farm-server` 已删除）。

与原 `core` 的**业务同步状态、缺口与更新记录**见 [docs/SYNC.md](docs/SYNC.md)（对齐以业务目标一致为准；每次业务对齐请追加更新记录）。

## 仓库结构

```
qq-farm-rust/
├── crates/
│   ├── qq-farm-core/      # 网关、登录、农场/好友调度、活动中心、统计
│   ├── qq-farm-app/       # UI 无关应用门面（desktop 经 IPC 调用）
│   └── qq-farm-desktop/   # Tauri v2 桌面宿主（IPC → app）
├── desktop-ui/            # SoybeanAdmin 桌面前端（与客户端共存）
├── proto/                 # 游戏 protobuf
├── assets/activity-data/  # 活动静态数据
├── scripts/               # 辅助脚本
├── docs/ARCHITECTURE.md   # 分层拓扑
├── docs/CODING_STANDARDS.md
└── .env.example
```

数据目录可用 `FARM_DATA_DIR` 覆盖，不要提交进去。

## 环境

- Rust 1.75+（建议用当前 stable）
- `protoc`（protobuf compiler，生成协议代码；Windows 可装到 `%USERPROFILE%\tools\protoc\bin` 并加入 PATH / 设置 `PROTOC`）
- Node.js + pnpm（desktop-ui 前端）
- Windows / macOS

## 编译与启动（桌面端，Tauri v2 + SoybeanAdmin）

```bash
# 前端依赖
pnpm -C desktop-ui i

# 需已安装 Tauri CLI：cargo install tauri-cli --version "^2"
cd crates/qq-farm-desktop && cargo tauri dev
```

桌面端经 IPC 调 `qq-farm-app`（LocalOwner），**不**走 HTTP。

- macOS 有原生菜单（应用：打开数据目录 / 检查更新）；Windows 动作在托盘
- 关闭窗口隐藏到托盘；退出只走托盘「退出」或 macOS Cmd+Q
- 安装包内嵌 `tsdk.wasm` 与 `game_config`；数据目录默认 `~/Library/Application Support/QQFarmRust` 或 `%LOCALAPPDATA%\QQFarmRust`

### 发版与自动更新

打 tag 后 GitHub Actions 构建 Windows NSIS + macOS universal DMG，并上传 `latest.json` 供客户端更新。验收见 [docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md)。

```bash
git tag v0.2.0
git push origin v0.2.0
```

### 微信扫码登录

1. 桌面端给账号选微信平台，走扫码登录（或「本机微信」快速授权）。
2. 网关登录码仍是一次性的；应用宝 `login_buffer` 会随账号落盘。掉线或进程重启后会自动换新码重连，无需再扫。授权失效时才需要重新扫码。

## 环境变量

见 `.env.example`。常用项：

| 变量 | 默认 | 说明 |
|------|------|------|
| `FARM_SERVER_URL` | `wss://gate-obt.nqf.qq.com/prod/ws` | 游戏网关 |
| `FARM_OS` | `Windows` | 客户端 OS |
| `FARM_CLIENT_VERSION` | `1.13.3.14_20260826` | 客户端版本 |
| `RUST_LOG` | `info` | 日志级别 |
| `FARM_DATA_DIR` | dev：仓库 `data/`；安装包：OS 应用数据目录 `QQFarmRust` | 账号、用户、配置 |

## 运行时行为

登录成功后，worker 按账号配置串行跑农场 / 帮助 / 偷菜：

- **农场**：除草除虫浇水 → 收获 → 铲除枯株 → 种植 → 施肥 / 解锁升级
- **偷菜**：按好友列表 `steal_plant_num` 筛选并排序；进场后再用 `stealers`/`steal_num` 判断「我还能偷」；进场无可偷则跳过同指标空转（不再刷「开始批量偷菜」）
- **出售**：收获或偷菜成功后，按「果实 + 可出售」自动卖出（需打开 `sell`）
- **面板**：`status:update` 推送效率（`sessionExpGained` / `uptime`），`log:new` 推送运行日志

安静时段、好友总开关、蔬菜黑名单与原版配置项一致。

## 测试

```bash
# 需本机已安装 protoc（protobuf 编译器），并在 PATH / PROTOC 中可见
cargo test --workspace
```

## 开发规范

本仓库的开发宪法位于 [.agents/skills/qq-farm-rust-dev/SKILL.md](.agents/skills/qq-farm-rust-dev/SKILL.md)：
架构分层、从 qq-farm-bot 同步功能的标准化流程、验证门槛、历史踩坑红线（bot 镜像文件禁改、图片 farmcfg 协议、IPC ACL 注册等）。
在本仓库内开发、修 bug、移植 bot 功能前必读；配套踩坑全录见同目录 `references/pitfalls.md`，移植流程明细见 `references/bot-sync-playbook.md`。

## 许可

仅供学习使用。
