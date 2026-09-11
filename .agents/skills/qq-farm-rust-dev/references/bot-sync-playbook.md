# bot 功能同步手册（分层接线全表）

以 2026-09-10 萌宠成长日记（`pet.rs`）移植为完整实例锚点；更早的样板是 `charity.rs`。移植新活动/新功能时按本表逐层落地，每层给出确切文件路径。

## 0. 调研（动手前必做）

```
qq-farm-bot/  git log --oneline -30            # 找 SYNC.md 基准之后的新提交
qq-farm-rust/docs/SYNC.md                      # 当前基准 commit + 已同步范围 + 明确不对齐项
qq-farm-bot/core/src/services/<模块>.ts        # 业务实现（门控逻辑 1:1 照抄）
qq-farm-bot/core/src/proto/*.proto             # 协议定义
qq-farm-bot/core/src/activity-data/*.json      # 数值配置
qq-farm-bot/docs/*.md                          # 协议来源/抓包证据（有则必读）
qq-farm-bot/core/tests/fixtures/*.json         # 官方向量（测试可用）
```

确认三件事：活动/功能 ID 常量、每个操作的 RPC 服务+方法+操作码+selector、是否有自动化任务（大多数活动是纯面板驱动，**不要**自作主张加进 `run_daily_routines`）。

## 1. 资源镜像（bot → rust 整份拷贝，不手抄）

| bot 侧 | rust 侧 | 说明 |
|---|---|---|
| `core/src/proto/<name>.proto` | `proto/<name>.proto` | 整份拷贝；共享 proto（activitypb 等）只做 bot 同款 diff |
| `core/src/activity-data/<name>.json` | `assets/activity-data/` | core 用 `include_str!` + OnceLock 解析 |
| 面板素材（web/public/activity-assets/...） | `desktop-ui/public/activity-assets/<活动>/` | 静态图，vite build 自动带进 dist |
| `core/src/utils/tsdk.wasm` | `assets/tsdk.wasm` | 必须校验 SHA-256 与 bot `tsdk-runtime.ts` 的 TSDK_SHA256 一致 |

build.rs（prost）自动编译 proto，本机需要 protoc 在 PATH。

## 2. core 层（crates/qq-farm-core）

接线顺序：

1. **常量** `src/constants/game_ids.rs`：活动组/子活动 ID、每个 operate_type、专属道具 ID、已知错误码。命名 `<功能>_<含义>_ID / _OPERATE_TYPE`。
2. **服务** `src/services/activity_center/<name>.rs`：
   - 挂在 `ActivityCenterService` 上（`impl ActivityCenterService`），不是独立服务；
   - 读取：`gateway.request(ACTIVITY_SERVICE, "GetGroup"/"List"/..., &request.encode_to_vec())` → prost decode → 校验回包 id/类型/selector；
   - 写操作模板：`mutation_lock` 串行 → 重查状态 → 活动窗口校验 → **bot 同款门控**（逐分支照抄，含防钻石/限购/白名单/计数一致性校验）→ 发 Operate → 校验回包 → 快照刷新（失败返回 `refreshError` 字段，不吞成功）；
   - 成本校验：含钻石（道具 1004 / id 0 / 负数 / `diamond_cost_count>0`）一律拒绝——这是用户决策，不是建议；
   - 数值配置与素材映射用 `include_str!` + `OnceLock`（见 `pet.rs` 头部的 `PetCatalog` / `pet_asset_url`）。
3. **注册** `src/services/activity_center/mod.rs`：`mod <name>;`；快照聚合是否包含按 bot 行为定（bot 有独立端点的活动不进总快照，如 pet）；需要独立单飞的加专用 `AsyncMutex` 字段。
4. **目录绑定** `src/services/activity_center/directory.rs`：`push_binding(&mut bindings, [IDs], "<gameplayKey>", "<detailTarget>", priority)`。priority 对齐 bot `core/src/services/activity-gameplay-registry.ts`。**绑定后目录条目自动继承 List 窗口真实起止时间**——没有绑定会导致前端兜底入口状态错误（见 pitfalls「目录状态」）。
5. **错误码** `src/services/activity_center/error.rs`：新变体 + `as_str()`（字符串对齐 bot 的错误码字面量）。
6. 状态持久化（仅服务端不保存状态的活动）：`src/services/activity_center_state.rs`，参照 charity 的 claimed/pending 三函数。

## 3. app 门面（crates/qq-farm-app/src/activity.rs）

每个操作一个自由函数：

```rust
pub async fn <name>(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().<core 方法>.await.map_err(AppError::from_core)
}
```

## 4. desktop IPC（crates/qq-farm-desktop）

1. `src/commands/activity.rs`：`#[tauri::command] pub async fn <cmd>(state: State<'_, DesktopState>, account_id: String, ...) -> IpcResult<Value>`，开头 `ensure(&state, &account_id)?`（ACL）；数字参数用现有 `json_i64` 兼容 int/float/string。
2. **两处注册**：`src/lib.rs` `generate_handler![...]` + `permissions/desktop.toml`（黄金规则 3）。

## 5. desktop-ui

1. **API** `src/service/api/farm.ts`：`invokeFlat('<ipc 命令>', { accountId: aid(accountId), ... })`。
2. **视图** `src/views/farm/activity/<name>-view.vue`：
   - 自包含视图样板 `weather-view.vue` / `pet-diary-view.vue`：`useFarmAccountStore` 取账号、`onMounted` 拉快照、操作后用回包 snapshot 原地刷新；
   - 调 fetch 前先 `const accountId = farmAccountStore.currentAccountId; if (!accountId) return;`（类型收窄）；
   - 时间格式化用 `dayjs`；图片遵守黄金规则 2；
   - 紧凑功能风格（对齐 charity/qixi/weather），不移植 bot web 的纯视觉件（动画/CG）。
3. **接入** `src/views/farm/activity/index.vue`：`GameplayKey` 联合类型加新键、`resolveGameplay` 的 detailTarget 映射加分支、白名单判断加键、模板 `v-else-if="selectedGameplay === '<key>'"` 挂视图。

## 6. 单测（写在 `<name>.rs` 的 `#[cfg(test)] mod tests`）

- 常量对齐 TS 字面量（`assert_eq!(XXX_ID, 2_026_090_101)` 风格）；
- 数值配置解析结果对齐 bot JSON（feed 成本/阈值/日限/刷新参数）；
- 门控纯函数：钻石拒绝、同币种合并、限购、计数校验、白名单；
- normalize 边界：用 prost 结构构造最小数据，断言 canFeed/canDraw/exchangeable 等派生标志；
- 离线 service 构造：

```rust
fn service() -> ActivityCenterService {
    let gateway = Gateway::new(GatewayConfig {
        server_url: "wss://gate.example.com/ws".into(), platform: "qq".into(),
        os: "Windows".into(), client_version: "1.14.0.1_20260909".into(),
        auth_code: "test".into(), headers: HashMap::new(),
    }, Arc::new(NoopEncryptor));
    ActivityCenterService::new(Arc::new(gateway))
}
```

注意：断言失败先怀疑**测试数据**再怀疑实现（合并成本 300×2=600 ≤ 余额 700 本来就该可用，别写反）。

## 7. 全量验证 + SYNC.md

验证门槛命令见 SKILL.md。SYNC.md 更新四处：

1. 「对照基准」表：rust 行说明 + bot commit（同步时 bot main 的 HEAD）；
2. 「版本语义」表：客户端版本（若变）、新活动 ID 行；
3. 「业务能力同步矩阵」：对应行补新功能名；
4. 文末「更新记录」追加：基准、业务明细、验证命令与结果、实机待验项。

热修（实机发现 bug 后的修复）也要追加记录，标注「热修」和根因。

## 常用路径速查

| 用途 | 路径 |
|---|---|
| 活动常量 | `crates/qq-farm-core/src/constants/game_ids.rs` |
| RPC 服务名 | `crates/qq-farm-core/src/constants/rpc.rs` |
| 活动服务样板（简单） | `crates/qq-farm-core/src/services/activity_center/charity.rs` |
| 活动服务样板（完整/最新） | `crates/qq-farm-core/src/services/activity_center/pet.rs` |
| DTO 工具（item_dto/positive_decimal/text_content） | `crates/qq-farm-core/src/services/activity_center/dto.rs` |
| 背包读取（注入优先，回退 get_bag_via） | `crates/qq-farm-core/src/services/warehouse.rs` |
| 节令 | `season.rs::get_current_solar_terms`（复用，勿重写） |
| 服务器时间 | `crate::utils::time::get_server_time_secs()` |
| 每日自动化挂点 | `crates/qq-farm-core/src/runtime/worker_loop/mod.rs::run_daily_routines` |
| TSDK 宿主/平台 | `crates/qq-farm-core/src/crypto/tsdk.rs`（HostProfile） |
| 前端图片转换 | `desktop-ui/src/views/farm/game-config/shared.ts::resolveCatalogImage` |
