---
name: qq-farm-rust-dev
description: qq-farm-rust（QQ 农场 Tauri 桌面版）开发规范：项目架构、代码规范、从 qq-farm-bot 同步功能的标准化流程、验证门槛、历史踩坑红线（bot 镜像文件禁改、图片 farmcfg 协议、IPC ACL 注册、TSDK、活动移植、SYNC.md）。凡是在 qq-farm-rust 仓库内开发、修 bug、移植 bot 新活动/新功能、新增 Tauri IPC 命令、改动图片显示、升级客户端协议版本、或更新 SYNC.md 时都必须使用本 skill——即使用户没有明确提到"规范"二字。
---

# qq-farm-rust 开发规范

本 skill 是 `qq-farm-rust` 仓库的开发宪法。目标只有一个：**业务行为与 qq-farm-bot 对齐，不重复犯已犯过的错**。

## 仓库定位

```
qq-farm 工作区（本机：C:\Users\liu13\projects\qq-farm\）
├── qq-farm-bot\     # 参照实现（TypeScript monorepo：core 后端 + web 面板）。业务逻辑唯一真源，只读。
└── qq-farm-rust\    # 本仓库（Rust 重写）。qq-farm-rust 内部：
    ├── proto/                  # 36+ 个 .proto，prost 编译（需本机 protoc）
    ├── assets/                 # game_config（游戏配置+978张种子图）、activity-data、tsdk.wasm
    ├── crates/
    │   ├── qq-farm-core/       # 业务核心（零 UI、零进程入口；协议/服务/自动化全在这）
    │   ├── qq-farm-app/        # UI 无关门面（只依赖 core，禁止 tauri）
    │   └── qq-farm-desktop/    # Tauri v2 宿主（IPC → app；ACL/托盘/更新器）
    ├── desktop-ui/             # SoybeanAdmin (Vue3 + NaiveUI) 前端，pnpm
    ├── docs/                   # SYNC.md（同步状态唯一滚动源）、ARCHITECTURE、CODING_STANDARDS
    └── .agents/skills/qq-farm-rust-dev/  # 本 skill 随仓库分发（规范唯一真源）
```

数据流：`desktop-ui ─Tauri IPC→ qq-farm-desktop → qq-farm-app → qq-farm-core`。
游戏协议**不走 HTTP**：WebSocket + 自定义二进制帧 + protobuf + TSDK(wasmtime) 加密。

## 黄金规则（每条都用真实事故换来的）

1. **bot 源码是业务唯一真源**。移植任何功能前先读 bot 对应 TS 实现（`qq-farm-bot/core/src/services/...`），门控逻辑、错误文案、操作码 **1:1 照抄**，不要"优化"。有疑问查 bot 的 `docs/`（协议抓包说明）和 `core/tests/fixtures/`（官方向量）。
2. **图片路径三规则**（历史上挂过 3 次，见 `references/pitfalls.md` 第一条）：
   - 后端 DTO 的 `item.image` 是 `/game-config/...` 相对路径，前端**必须**经 `resolveCatalogImage()`（`desktop-ui/src/views/farm/game-config/shared.ts`）转换后才能当 `src`；
   - 该函数 Windows 分支必须返回 `http://farmcfg.localhost/<rel>` 映射形式，macOS 返回 `farmcfg://localhost/<rel>`——不许改回原始形式；
   - `/activity-assets/...` 是 dist 内静态文件、外链 `http(s)://` 和 `data:` 原样使用，**不**转换。新增页面图片统一：`resolveCatalogImage(item.image)`，禁止裸 `:src="item.image"`。
3. **IPC 命令两处注册缺一不可**：`crates/qq-farm-desktop/src/lib.rs` 的 `generate_handler![]` **和** `crates/qq-farm-desktop/permissions/desktop.toml` ACL 白名单。有防回归测试（`every_handler_command_is_allowed_by_acl`）兜底，但不要依赖它——提交前自查。
4. **tokio::sync::Mutex 不可重入**。方法 A 拿了 `mutation_lock` 后不得再调用同样拿这把锁的方法 B（哪怕"只是读"）——要么把锁内逻辑抽成 `_inner` 私有方法，要么在拿锁前分流。2026-09-10 萌宠 solar 分流死锁就是这么来的。
5. **游戏协议常量必须进 `crates/qq-farm-core/src/constants/`**（活动 ID、操作码、道具 ID、错误码），禁止散落字面量。数字风格用下划线（`2_026_090_101`）。
6. **bot 镜像文件（proto / activity-data JSON 等）一字不改，只整份拷贝**：`proto/*.proto` 等从 bot 镜像的文件**禁止任何本地修改**——包括加 `optional`、改注释、调格式，哪怕是"wire 兼容、只影响解码"的改动也不行（2026-09-11 施肥事故：擅自给 `left_inorc_fert_times` 加 `optional` 被打回）。bot 侧 proto 更新时重新整份覆盖。prost 表达不了 bot 语义（如 protobufjs `Object.hasOwn` 的字段 presence）时，**在 rust 代码层近似实现**，并用注释写明依据（官方向量见 `core/tests/fixtures/` 与抓包测试如 `farm-fertilize-proto.test.js`）。请求/回包 selector 字段号可能不同（如萌宠请求 128/回包 129），prost 类型天然区分，不用担心。
7. **客户端版本号成对改**：`crates/qq-farm-core/src/config/system_config.rs` 的 `DEFAULT_CLIENT_VERSION` 和 `DEFAULT_CLIENT_VERSION_UPDATED_AT`（bot `config.ts` 同名常量照抄）。版本是活动协议的前提，不同步会打不开新活动。
8. **SYNC.md 是同步状态的唯一记录**（`docs/SYNC.md`）。业务对齐或热修后必须文末「更新记录」追加一条：基准 commit、明细、验证命令与结果。实机没验的记「待验」，不装验过。
9. **crate 边界**：core 禁 axum/tauri/UI；app 只依赖 core；desktop 只调 app 的门面函数。完整条款见 `docs/CODING_STANDARDS.md`。
10. **rustfmt max_width=100、clippy 圈复杂度 25/函数 120 行/参数 8**。中文注释与日志是项目惯例，跟随。

## 同步 bot 新功能的标准流程

新活动、新功能移植走这 8 步（**完整分层细节和文件路径表见 `references/bot-sync-playbook.md`，开始移植前先读它**）：

1. **调研**：对比 bot `git log` 与 SYNC.md 基准 commit，读 bot 实现 + proto + 数值配置 JSON + docs；
2. **资源镜像**：proto、`assets/activity-data/*.json`、素材图 → `desktop-ui/public/activity-assets/<活动>/`、必要时 tsdk.wasm（校验 SHA-256）；
3. **core**：constants → `services/activity_center/<name>.rs`（样板：`charity.rs`，最新完整实例：`pet.rs`）→ `mod.rs` 注册 → `directory.rs` gameplay 绑定；
4. **版本/配套**：协议版本、pets 表等按需；
5. **app + desktop**：门面函数 → `#[tauri::command]` → 两处注册（规则 3）；
6. **desktop-ui**：`service/api/farm.ts` fetch 函数 → 视图组件（自加载模式样板：`weather-view.vue`）→ `index.vue` 的 `GameplayKey` 与 `resolveGameplay` 接入；
7. **单测**：常量对齐 TS 值断言、门控纯函数、normalize 边界；离线构造用 `Gateway::new(config, NoopEncryptor)`；
8. **全量验证 + SYNC.md**（见下）。

## 验证门槛（全部通过才算完）

```bash
# Rust（0 错 0 警是硬门槛）
RUSTFLAGS="-D warnings" cargo check --workspace --all-targets
cargo test --workspace          # app crate 若偶发失败用 --test-threads=1
cargo fmt --all --check
# 前端（pnpm 坏了用 corepack pnpm）
cd desktop-ui && corepack pnpm typecheck && corepack pnpm build
```

## 桌面端运行注意

- 启动：`cd crates/qq-farm-desktop && cargo tauri dev`（先确保 `corepack pnpm -C desktop-ui install` 过）。
- `beforeDevCommand`（pnpm build）**只在启动时跑一次**：之后改前端必须手动 `pnpm build` 并重启实例；只改 Rust 会自动重编译。
- dev 实例从 `data/` 加载真实账号并自动登录；**与安装版同账号双开会互相顶号**，只留一个跑。
- 弹"发现新版本"更新框属正常（本地 dev 版本号落后线上 release），点「稍后」。

## 何时读参考文件

- 开始任何 bot 功能移植/新 IPC 命令 → **必读** `references/bot-sync-playbook.md`
- 改图片/静态资源/协议相关、或想了解历史事故根因 → `references/pitfalls.md`
- 详细的分层条款、静态资源分级、错误处理约定 → `docs/CODING_STANDARDS.md` 与 `docs/ARCHITECTURE.md`（仓库内）
