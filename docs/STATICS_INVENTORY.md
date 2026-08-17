# Process Globals Inventory

分级用于治理优先级。标注 **多前端敏感** 的项在 server / desktop 共用进程内嵌引擎时更需收敛。

## L1 — 允许（进程级单例）

| 位置 | 说明 |
|------|------|
| `crypto/tsdk.rs` ENGINE/MODULE | wasmtime 单例 |
| `network/encryptor.rs` RUNTIME | TSDK encryptor |
| `config/system_config.rs` GLOBAL_CONFIG | 运行时系统配置 |
| `config/game_config.rs` GLOBAL | 游戏静态表 |
| `utils/logger.rs` INIT/LOG_DIR | 日志初始化 |
| `utils/time.rs` SERVER_TIME_* | 服务器时间同步 |
| `config/paths` / `FARM_DATA_DIR` | 数据根路径 |
| `runtime/scheduler.rs` REG | 调度注册表 |

## L2 — 短期可接受（JSON store）

| 位置 | 说明 |
|------|------|
| `models/store/accounts.rs` ACCOUNTS | 账号列表 |
| `models/store/account_config.rs` STATE | 每账号配置（仍是进程全局 map） |
| `models/store/global_config.rs` STATE | 全局配置 |
| `models/user_store/users.rs` USERS/CARDS | 面板用户 |
| `models/user_store/auth.rs` LOGIN_* | 登录限流/日志 |
| `models/user_store/card_claim.rs` | 卡密领取 |
| `services/security.rs` SECURITY_CONFIG | 安全参数常量结构 |

本轮不改为全 DI Repository；统一访问入口即可。

## L3 — 待治理（应按账号隔离）**多前端敏感**

| 位置 | 问题 |
|------|------|
| `friend/visit_strategy.rs` `FRIENDS_LIST_CACHE` / `FRIEND_QUIET_HOURS` | 多账号串缓存 |
| `friend/visit_strategy` 冷却 / 活动植物 | 失效 GID 冷却仍按 gid；活动植物已按 account_id 分槽 |
| `friend/api.rs` coalesce MAP | 请求合并全局 |
| `services/automation.rs` FLAGS | 已按 account 分 key；空 id 走默认 |
| `services/stats.rs` SLOTS | 按账号分槽；CLI/单测走 `"_"` 兼容槽，无「当前账号」 |
| `services/status.rs` STATUS_BY_ACCOUNT | 按账号 HashMap；CLI 读 `""` 槽 |
| `services/rate_limiter.rs` SERVICE_QUEUES | 进程级队列（可接受为 L2.5，但需文档化） |
| `services/panel_log.rs` HOOKS | 按账号 hook，结构尚可 |

## Server 进程状态

| 位置 | 级别 | 说明 |
|------|------|------|
| `sessions::SessionStore` | L2 | 内存会话；持久化 API 须接线或删除 |
| `wx_login::WxLoginHub` | L2 | 扫码任务内存表（server/desktop 共用） |
| `auth::STARTED_AT` | L1 | 进程启动时间 |

## 治理原则

1. 禁止新增 L3 `pub static`。
2. Friend / automation 迁入 per-worker / `FriendRuntimeState`。
3. stats/status 所有读写带 `account_id`。
4. 推送经 `AppEvent`，避免各处直接摸全局 hook。
