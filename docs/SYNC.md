# qq-farm-rust 与 qq-farm-bot/core 同步状态

> **维护约定**：业务对齐或 parity 相关热修后，必须在文末「更新记录」追加一条。  
> 本文是同步状态的**唯一滚动源**。

## 对齐原则

**以业务目标一致为准，不论实现技术细节。验收只认「齐」，不接受「基本齐」收口已知代码差。**

| 要对齐 | 不记为缺口 |
|--------|------------|
| 自动化行为（种什么、何时帮助/偷菜、出售策略） | 进程模型（多进程 IPC vs 进程内任务） |
| 游戏协议效果（请求语义与结果） | 文件是否同名、行数是否 1:1 |
| 面板 API 语义与配置开关效果 | 内部模块拆分方式 |

## 对照基准

| 仓库 | Commit | 日期 | 说明 |
|------|--------|------|------|
| **qq-farm-rust** | `main`（本提交：parity 总检修复） | 2026-08-15 | 默认值/门控/SEED 生命周期/封禁落盘/帮助经验/捣乱启动 + 既有心跳/统一 tick/青梅/偷菜空转 |
| **qq-farm-bot** | `04f9d90` | 2026-08-12 | 修复果实是否可售；`core` 包版本 `20260812` |

文档范围：**业务行为 + 面板 HTTP/Socket 契约**（不含改 Vue 面板本身）。

### 版本语义（避免混用）

| 概念 | 当前值 | 用途 |
|------|--------|------|
| 客户端版本 `FARM_CLIENT_VERSION` | 默认 `1.13.1.6_20260723`（与 bot `config.ts` 默认一致） | 进游戏网关声明 |
| bot `core` 包版本号 | `20260812` | 原项目发布标签，≠ 客户端版本字符串 |
| 青梅活动 ID | 每日 `2026081201` / 酿造 `2026081202` | 活动协议 |

---

## 复刻时间线

| 阶段 | Commit 区间 | 业务内容 |
|------|-------------|----------|
| 0/1A–1B | `15a8018` | 网关、加解密、运行时骨架 |
| 1C–1D | `1213933`–`30db725` | 本田务农 + 好友帮助/偷菜核心 |
| 1E–1F | `378eae6`–`3137a37` | 游戏配置、账号/用户持久化、限流 |
| 1G | `39cb409`–`90e020f` | 仓库/商城/任务/活动/登录等业务层 |
| 1H | `b339744`–`8f1c82f` | 多账号编排、离线提醒、worker 主循环 |
| 2A–2B | `22a6ccd`–`2285154` | 面板 API + E2E |
| 2C–2G | `4e4b776`–`4aacef7` | 真实网关、种植/op、联调完善 |
| 2H–2I | `9ee0cae`–`25dda13` | 微信扫码真接 + 扫码后启动 worker |
| 热修 | `c5386e6`、`c19f52c` | 登录后农场调度、好友列表大包超时 |
| parity | 本提交 | 总检修复：默认值单一源、sell/静默/email 门控、SEED/left_inorc/施肥 fail-closed、封禁落盘、帮助经验、捣乱启动、访客 GID、配置后补肥；保留偷菜空访/青梅落盘增强 |

业务是否「齐」以本文矩阵为准。

---

## 业务能力同步矩阵

状态说明：

- **齐**：代码侧与 bot 业务目标一致（已知无行为差）；实机对照见下方 L 清单
- **未齐**：能力缺失或明显偏离业务目标

| 能力 | 状态 | 期望行为（摘要） | 定位（非验收标准） |
|------|------|------------------|-------------------|
| 连网进游戏 | 齐 | 经 Gateway + TSDK 登录并维持心跳 | `network/*`, `crypto/tsdk.rs` |
| QQ 小程序扫码拿码 | 齐 | 面板可走 QQ 码登录流程 | `services/qrlogin.rs` |
| 微信扫码拿码并启动 | 齐 | 扫码 → 应用宝 login_buffer 落盘 → 换一次性网关 code 启动；掉线/重启可用授权再换码重连 | `services/wx_login/*`, `routes/wx_login.rs` |
| 本田务农循环 | 齐 | 除草除虫浇水 → 收获 → 铲除 → 种植（含多格）→ 施肥/解锁升级；默认策略/skip_own_weed_bug/smart 秒数对齐 bot | `services/farm/*`, `runtime/worker_loop.rs` |
| 好友帮助 / 偷菜 / 捣乱 | 齐 | 列表、访问、帮助（经验门控）、偷菜（stealers/空访跳过）、静默、黑名单落盘；捣乱按日限/启动筛选/`1001046` 停 | `services/friend/*` |
| 背包展示与操作 | 齐 | 按 UID 堆分行；含 `key`/`uid`/`mutantTypes`/`groupKey`；系统物品分离 | `services/warehouse.rs` |
| 自动/手动出售果实 | 齐 | 自动受 `sell` 开关；跳过不可售；手动 `sell_items` 预检拒绝不可售 | `warehouse` + `game_config` + `worker_loop` |
| 商城 / 神秘商店 / 月卡 / 钻石 | 齐 | 列表、购买（神秘 Buy 无回包）、月卡、充值信息 | `mall`, `mystery_shop`, `monthcard`, `pay`, `commerce` |
| 日常领取 | 齐 | 任务（成长 claim 后刷新 TaskInfo + `currentTask`）、邮件、分享等 | `task`, `email`, `share`, … |
| 活动中心 | 齐 | 千星游记、观星、星砂、节令、青梅（含已领幂等） | `activity_center*` |
| 面板鉴权与账号 | 齐 | 登录注册（无卡密）、账号 CRUD、设置 | `routes/auth`, `account`, `admin` |
| 面板农场/好友/活动/商业 API | 齐 | 与 Vue 面板契约兼容，可挂 3007 | `qq-farm-server/src/routes/*` |
| Socket 状态/日志推送 | 齐 | `status:update` / `log:new` 等 | `socket.rs` |
| 离线重登提醒 | 齐 | 掉线可提醒 | `runtime/relogin_reminder.rs` |
| 推送通知 | 齐 | 面板 `channelOptions` 全部 19 渠道可路由发送 | `services/push.rs` |
| 统计 / 状态汇总 | 齐 | 效率与状态可给面板 | `stats`, `status`, `analytics` |

---

## 已知业务缺口

### 1–3. 已关闭（2026-08-14 parity）

原缺口（背包 UID、bot HEAD 行为回归、推送渠道）已在代码侧对齐关闭，详见更新记录。

### 4. 真实账号业务回归（持续）

同配置下与原版对比；**通过后在更新记录注明**，无需再改矩阵为未齐。代码侧已知业务差已清。

| # | 场景 | 状态 |
|---|------|------|
| L1 | 微信/QQ 扫码登录并启动 worker | 待验 |
| L2 | 本田完整务农一轮（含多格种植） | 待验 |
| L3 | 好友帮助 + 偷菜 + 捣乱日限（含静默时段） | 待验 |
| L4 | 自动/手动出售（含不可售跳过/拒绝） | 待验 |
| L5 | 活动领取（赛季/观星/节令/青梅） | 待验 |
| L6 | 商城或神秘商店购买 | 待验 |
| L7 | 掉线后重登提醒 / 重新扫码 | 待验 |
| L8 | 面板 Socket 状态与日志实时更新 | 待验 |

---

## 更新记录

### 2026-08-14 — 建立同步文档

- 基准：rust `c19f52c` / bot `04f9d90`
- 业务变更：无代码变更；盘点当前能力与缺口，确立「业务目标一致」对齐原则。
- 能力状态：矩阵初版；登记缺口 #1–#4；登记真实账号回归清单 L1–L8。
- 关联：进度滚动以本文为准。

### 2026-08-14 — 清理过时文档

- 基准：rust `c19f52c` / bot `04f9d90`
- 业务变更：无；删除 `docs/audit-2026-08-11.md` 与 `docs/PERFORMANCE.md`。
- 能力状态：不变；同步文档仅保留本文。

### 2026-08-14 — 关闭缺口 #1–#3（完全对齐）

- 基准：rust `c19f52c` + 工作区改动 / bot `04f9d90`
- 业务变更：
  - 背包 `get_bag_detail` 按 UID 分行，补 `key`/`uid`/`mutantTypes`/`groupKey`；`systemItems` 精简字段
  - `sell_items` 不可售预检；成长 `do_claim` 后刷新 TaskInfo；`GrowthTaskStateLikeApp` 含 `currentTask`、`doneToday: false`
  - 捣乱日限落盘、剩余次数切片、按地确认、`1001046` 停；神秘商店 `Buy` → `send_no_reply`
  - 推送实现面板全部 19 渠道
- 能力状态：缺口 #1–#3 关闭；相关矩阵项 → **齐**；L1–L8 仍待实机勾选。

### 2026-08-15 — 本地启动冒烟 + E2E 修正

- 基准：rust `c19f52c` + 工作区 parity / bot `04f9d90`
- 业务变更：无新业务逻辑；修正 `e2e_integration` 对 `{ ok, data }` 响应包裹的断言。
- 验证：
  - `qq-farm-server` 启动正常（`ADMIN_PORT=3007`，独立 `FARM_DATA_DIR`）
  - `GET /health`、`/api/ping`、`/api/game-version` OK
  - 管理登录（`x-admin-token`）→ 开卡 → 注册 → 用户登录 → `/api/accounts` OK
  - `cargo test -p qq-farm-server --test e2e_integration`：**10/10 通过**
  - `cargo test -p qq-farm-core --lib warehouse`：含背包 UID / 不可售预检 **15/15 通过**
- 能力状态：不变；**L1–L8 仍待真实游戏账号实机勾选**（本次未接网关务农）。

### 2026-08-15 — 修复忙时心跳误杀导致掉线

- 现象：登录后偷菜/出售/任务正常，约 50s 后面板报 `disconnect:ws_close` 并等待重扫码。
- 根因（对照 bot `network.ts`）：
  - Heartbeat RPC 超时用了 **5s**（bot `sendMsgAsync` 默认 **20s**），忙时易失败且失败被静默吞掉
  - 心跳超时后 `force_disconnect` 被统一记成 `ws_close`，掩盖真实原因
- 修复：Heartbeat 超时改 20s；失败/超时写面板日志；断开原因区分 `heartbeat_timeout`；队列满时 Heartbeat 短暂等待空位
- 补充：心跳发送改为 fire-and-forget（`tokio::spawn`），对齐 bot `sendMsgAsync().then().catch()` 不阻塞 interval
- 能力状态：连接保活对齐；L1–L8 仍需重扫码后实机确认

### 2026-08-15 — 调度/统一 tick 机制对齐 bot

- 对照发现并修复：
  1. **Scheduler**：默认 `preventOverlap=true`；ticker **不 await** 回调（对齐 Node `setInterval`）；支持 `runImmediately`
  2. **统一 farm/help/steal**：由 500ms 轮询改为 bot 的 `scheduleUnifiedNextTick`（timeout 链，最低 1s）
  3. **心跳**：仅 `Online` 且 `gid!=0` 才发；超时告警带 `pending=`；RPC 超时 20s；fire-and-forget
  4. 去掉 Heartbeat「队列满等待」特例，恢复与 bot 相同的 pending≥5 立即失败

### 2026-08-15 — 青梅每日种子领取状态对齐

- 现象：种子实际已领（或返回 `1034014`）时面板仍报错，且「领取」按钮可点。
- 根因：已领幂等未稳定写成 `dailySeed.claimed=true`；worker 重启后内存标记丢失；snapshot 在 mutation 锁内拉取易失败。
- 修复：
  - `1034014` / 「已经领取」一律幂等成功，并强制 snapshot `dailySeed.claimed=true`、禁用 claimSeed
  - 今日已领落盘 `qingmei-seed-claimed-*.json`，`set_account_id` 时恢复
  - 本地已领则不再打 RPC；mutation 锁在拉 snapshot 前释放
- 能力状态：青梅领种状态对齐；需重扫码后点一次验证按钮变「今日已领取」

### 2026-08-15 — 偷菜空转循环对齐

- 现象：每隔约 12–16s 反复刷「开始批量偷菜，共 1 个好友有可偷」，无好友偷菜结果 / 巡查完成。
- 根因（对照 bot `visit-strategy.ts`）：
  1. GetAll 的 `steal_plant_num` 表示「仍有可被偷的地」，不等于「我还能偷」；进场后无可偷却每 tick 重入
  2. 从地占用判断过粗（只要 `master_land_id` 就跳过），未对齐 bot「master 有植物才跳过从地」
  3. 未解析 `stealers` / `steal_num` 判断我是否已达每人上限
  4. rust 仍输出 bot 已注释掉的「开始批量偷菜」日志，放大空转感
- 修复：
  - 占用判断改用 `display_land_context`（TS `isOccupiedSlaveLand`）
  - 可偷判定加入 stealers/steal_num
  - 进场无可偷时按 `(gid, steal_plant_num)` 记空访标记，指标不变则跳过；有偷成功或指标变化后恢复
  - 去掉「开始批量偷菜」面板日志；进入失败补 `log_warn`（对齐 bot）
- 能力状态：偷菜空转应对齐；需重扫码后观察不再刷开始日志，有可偷时应出现 `好友名: 偷N(...)`
### 2026-08-15 — 业务对齐总检修复（代码侧）

- 基准：rust 工作区 / bot `04f9d90`
- 策略：业务目标一致；保留偷菜 `stealers`/空访与青梅已领落盘增强
- 修复（意外偏离）：
  1. **默认值**：`normalize::default_account_config` 对齐 bot；`types` Default 委托单一源（含 `skip_own_weed_bug`/`max_exp`/`steal 20–25`/`friend_help_exp_limit`/`smart=300`/`bagSeedPriority=[]`）
  2. **worker 门控**：收获出售看 `sell`；静默不再挡住本田 tick；farm tick 不再领邮件
  3. **本田**：SEED→growing；`left_inorc` 用 optional presence；背包失败不误购；拉地失败施肥 fail-closed
  4. **好友**：封禁写账号黑名单落盘；失效 GID 移除+冷却；帮助经验 `canGetExp*`；`help_farm` 用 `results` + `1001057` noop；捣乱用真实 `my_gid`；启动捣乱按 idle+等级 top20 + `visit_friend`
  5. **其它**：访客补充 known GID；配置保存后施肥模式变更立即补肥；分享 Report 失败中止 Claim；删除未接线的 season_progress 死配置
- 能力状态：矩阵保持 **齐**（代码侧已知业务差已清）；**L1–L8 仍待真实账号实机勾选**
### 2026-08-15 — 残留高影响差修复

- 基准：bot `04f9d90`；上轮 P0–P3 已落地后对照复测
- 修复：
  1. **本田静默**：`check_farm` 改 `in_friend_quiet_hours_for(account_id)`，账号静默配置生效（worker 仍跑 task/补肥）
  2. **visit_friend**：帮/偷/捣乱改 `is_automation_on_for`；增加 `can_get_exp_by_candidates`；启动捣乱传入帮助经验门控
  3. **访客 GID 同步**：`knownFriendGidSyncCooldownSec` 进程内按账号节流；失败缩短冷却
- 明确不改：施肥拉地失败 fail-closed（比 bot 更严，保守）
- 能力状态：矩阵保持 **齐**；实机冒烟未见异常，**L1–L8 清单仍待逐项勾选**
### 2026-08-15 — 人机「小果」头像结论（暂缓落地）

- 实机（账号3 wx）：真·人机 `gid=10001` 名称「小果」；另有同名真人 `gid=1226150960`
- 游戏下发 `avatarUrl=gui/texture/common/img_botHead5/spriteFrame`（Cocos 包内路径，非 http）→ 面板无法直接显示
- 仓库静态资源无 `img_botHead5`；手机解包提取暂缓
- 代码：好友列表拉取时对 gid=10001 / 名称含「小果」打诊断日志（`人机头像诊断`）
- 前端（bot web）：好友/访客头像补 `referrerpolicy=no-referrer`，并规范化 `//` 协议相对 URL（利于微信 http 头像）
- 能力状态：矩阵仍 **齐**；人机本地头像映射 **未做**（待后续从游戏包导出或 CDN 映射）
### 2026-08-15 — 企业级质量治理（分层重构，业务目标不变）

- 范围：全 workspace + GPUI 预留；**不改**面板契约与游戏协议语义
- 分层：
  - `qq-farm-core`：`constants/`、`infra/`、业务域聚合；拆分 `activity_center` / `visit_strategy`
  - `qq-farm-app`：UI 无关门面（ACL、start/stop、daily gifts、AppEvent）
  - `qq-farm-server`：ACL 补齐、`/ws` 鉴权、`ServerConfig`、farm 路由拆分；会话明确内存-only
  - `qq-farm-desktop`：占位 crate（无 GPUI UI）
- 质量：AccountRecord/AccountSession 命名；rate_limiter 改用 `core::Error`；Friend/automation 按账号隔离；CLI mock 去重
- 能力状态：矩阵保持 **齐**（重构不引入已知业务差）；实机 L 清单仍待勾选
### 2026-08-15 — GPUI 桌面端落地（进程内嵌）

- `qq-farm-desktop`：gpui 0.2.2 + gpui-component 0.5.1；`LocalOwner` ACL；导航对齐 web（概览/个人/活动/商城/神秘商人/好友/分析/设置/配置/本机运维）
- `qq-farm-app`：补齐 `bootstrap`、`settings`、`farm`（status/lands/bag/operate/analytics/logs）、`friend`、`activity`、`commerce`、`config`、`admin`；账号 `list_accounts_enriched` / `upsert_account`
- server：账号列表、settings 面板、farm status 改调 app 门面
- 能力状态：面板 HTTP 契约 **不变**；desktop 与 web **语义对齐**（UI 为 gpui 实现，非像素级还原）
### 2026-08-15 — 桌面端微信扫码登录（对齐 web AccountModal）

- `qq-farm-app::wx_login`：create / poll / confirm / issue_code / destroy，语义对齐 `/api/wx-login/tasks*`
- 设置 → 账号：双 Tab「输入 code」/「微信扫码」；扫码展示 JPEG QR、状态文案、刷新/取消；成功后 `upsert_account(platform=wx)` 并启动
- 概览补点券/钻石资产卡；个人农场操作按钮保持与 web FarmPanel 同名
- 能力状态：桌面扫码与 web 同协议；server HTTP 契约 **不变**

### 2026-08-15 — 桌面账号管理语义修正（停机≠可启动）

- 概览：去掉假「启动账号」；离线 CTA 改为「扫码重新登录」（同备注 upsert）
- 设置 → 账号管理：表格（序号/备注/平台/运行状态/最近更新/操作）；工具栏「+ 新增」「刷新」；新增面板按需展开
- 运行中 →「停止」；已停止 →「重新登录」（扫码/新 code），不再用旧一次性 code 点「启动」
- 能力状态：桌面语义更正；server HTTP 契约 **不变**

### 2026-08-17 — 桌面端切换 Tauri v2 + SoybeanAdmin（Scaffold）

- 删除 GPUI 实现与 workspace `gpui` / `gpui-component` 依赖
- `qq-farm-desktop`：Tauri v2 宿主；IPC 命令 `desktop_ready` / `get_snapshot` / `list_accounts` / `get_settings`；`AppEvent` → `emit("app-event")`；ACL 仍为 `LocalOwner`
- `desktop-ui/`：SoybeanAdmin（NaiveUI）裁剪壳；分层 `typings` / `service/tauri` / `store/desktop` / `views`；登录为本地进入；scaffold 页：概览 + 设置只读
- **不改** `qq-farm-bot/web`；**不**在 `qq-farm-app` 引入 Tauri；面板 HTTP/Socket 契约 **不变**
- 能力状态：桌面 Scaffold 可开窗打通 IPC；完整业务页待后续迁入

### 2026-08-17 — 移除面板卡密（license card）

- 注册/登录不再要求 `cardCode`；去掉卡密管理、卡密领取、续费-by-card 路由
- 保留面板用户鉴权与账号 ACL；`DEFAULT_ACCOUNT_LIMIT` 提高到 100；admin 仍无限额
- **不改**游戏内月卡（`monthcard`）；**不改** Go `qq-farm`
- 能力状态：面板鉴权仍可用；卡密相关 HTTP 契约已移除

### 2026-08-17 — 桌面功能对齐（个人免费 / 无权限）

- 桌面产品：无登录门闸、无用户管理、无面板 RBAC；ACL 固定 `LocalOwner`
- `desktop-ui`：侧栏 10 项对齐 qq-farm-web 农场菜单（去掉 `/system/admin`）；迁入 farm/home 页，HTTP → Tauri `invoke`
- `qq-farm-desktop`：按域扩面 IPC（account/farm/friend/activity/commerce/settings/config + wx_login_code）
- **不改** `qq-farm-bot/web`；server 面板 token 登录可保留（与桌面无关）
- 能力状态：开窗直达首页；多农场账号主路径 IPC 通；卡密已从 rust 面板栈移除

### 2026-08-17 — 桌面农场页功能打通

- `farm.ts` 适配层：对齐桌面 IPC 参数/返回（status 扁平化、automation→settings_panel、friend/mall/bag/activity）
- 侧栏账号切换器；`farm_diamond`、青梅酿造 IPC；config overlay 增删改
- `app-event` 触发状态/日志刷新（不全量塞 Go 形状 payload）
- 能力状态：各农场页主路径可走 IPC；真实游戏数据仍依赖账号在线 worker

### 2026-08-17 — 扫码更新后自动启动（对齐 Go）

- `upsert_account`：更新时若提交了新 code（`code_changed`），即使账号原先已停止也 `restart_worker`；失败返回错误（对齐 Go `Start`）
- `desktop-ui` 账号抽屉：微信扫码「编辑/重新登录」成功后补一次 `start_account`
- 能力状态：已停止账号扫码换 code 后应进入运行中；server HTTP 契约不变

### 2026-08-17 — 好友列表偷菜后刷新

- Rust：偷菜成功 `mark_friend_steal_cleared`（覆盖游戏 GetAll 滞后的 stealNum）；列表 API `force`；好友页「刷新列表」+ 监听偷菜事件防抖刷新
- Go：Session `friendStealCleared` 覆盖 Friends()；web 好友页同样监听 `friend_interact` +「刷新列表」
- 能力状态：自动/手动偷菜后气泡应清零；手动按钮可强制拉新列表

### 2026-08-17 — 好友 help 操作 / 日志 event / 桌面全屏

- `FriendOperation::from_str_opt`：`help` → 一键务农（对齐 Go），`bug` → 除虫
- 看板日志：英文 event key 映射中文（对齐 bot Dashboard）
- 全屏：Tauri `setFullscreen`，不再用 WebView `requestFullscreen`
- 能力状态：好友页一键务农可调用；日志 chip 可读；全屏切原生窗口

### 2026-08-17 — 架构卫生（事件信封 / app 编排 / DTO / PanelEvent / L3 / 资源）

- 桌面实时事件对齐 web 信封 `{ type, payload, accountId }`；补 `status:update` 体、`friend_interact` / `farm_operation`（由日志派生）
- server farm/friend/commerce/activity/账号 upsert 只调 `qq-farm-app`；wx-login 共用 `WxLoginHub`
- 第一批面板 DTO：status / lands / bag / friend list / logs；desktop-ui 去掉双键读
- 日志 event 改为 `PanelEvent` 英文 snake_case；中文只在 UI 映射
- 去掉 stats「当前账号」槽与 status 单槽；好友黑名单只走 per-account store；活动植物按账号分槽
- `assets/game_config/seed_images_named` 进仓；`tauri dev` 走 `frontendDist`（先 `pnpm build`），不占用 Vite 端口；去掉 Soybean `hasAuth` / 未用 desktop store / HTTP `fetchLogin`
- 能力状态：分层执行更接近 `desktop/server → app → core`；面板 HTTP 契约不变

### 2026-08-17 — 游戏网关 WS 握手对齐 Go（User-Agent 大小写）

- 现象：微信扫码后立刻「系统连接已断开… WS 连接失败: HTTP 400 Bad Request」
- 原因：tungstenite 把额外头写成小写 `user-agent`（只特判 `Origin`）；Go gorilla / Node `ws` 发 `User-Agent`，腾讯网关按大小写校验会 400
- 修复：`WsClient` 按 Go 写法手写握手（`Origin` / `User-Agent` 规范大小写），15s 超时与 Go `HandshakeTimeout` 一致
- 能力状态：扫码拿到 code 后应能完成网关 upgrade；需重启桌面进程后实机验证

### 2026-08-17 — 自动化默认对齐 Go 面板，保存热更新

- 默认开关改回 Go / 面板截图：种植收获、任务、卖果实、好友互动、推送巡田、升级土地、填充化肥、跳过一键务农、偷菜开启；帮忙 / 捣乱 / 经验满不帮忙 / 自动买肥关闭；智能施肥 360s
- 已落盘且仍是旧 rust（bot）默认组合的账号，启动时自动迁到上述默认，不覆盖用户手动改过的组合
- 保存设置：`ReloadConfig` 失败会异步补发，并立刻 `sync_status`；帮忙/偷菜关闭时仍改下次调度，买肥开关随保存启停定时器——运行中账号不用停再开
- 能力状态：新账号与未改过的旧账号设置页应与截图一致；改开关保存后当轮即生效

### 2026-08-17 — 点好友列表不再把心跳打死

- 现象：打开好友列表立刻 GetAll/Heartbeat 请求超时，约 49s 无响应后 `heartbeat_timeout` 停号，之后任何接口都调不了
- 与 Go 的差：
  1. 微信 GetAll 失败后 Rust 再打空 `SyncAll`（回包还在路上时把连接堵死）；Go 只对已知 GID 走 `GetGameFriends`
  2. 心跳 30s 无 Heartbeat 回包就杀号；Go 明确「Bare RPC timeout 不是 socket 已死」
  3. 普通 RPC `pending>=5` 直接 QueueFull（Go 无此硬限），Heartbeat 也被挡住
  4. 桌面进好友页 `force: true` 每次都打网关；Go web 用缓存/DB，失败仍展示旧列表
- 修复：GetAll 等 60s；失败走 GetGameFriends；有入站帧或 in-flight RPC 时不因心跳静默杀号；Heartbeat 不受排队上限；列表失败回缓存；进页不再 force
- 能力状态：点好友列表不应掉线；需重启桌面后再试

### 2026-08-17 — 策略对比作物图标（去掉 Vite 后 404）

- 现象：分析页策略对比瓶子树 / 山竹变成问号；图标文件仍在 `assets/game_config/seed_images_named`
- 原因：`tauri dev` 无 `devUrl` 时 CLI 用内置静态站托管 `desktop-ui/dist`（`http://127.0.0.1`）。前端把 localhost 当成 Vite，请求 `/game-config/…`，但 `dist` 里没有这些 PNG（原 Vite 中间件也不会跑）
- 修复：`pnpm build` 把 `assets/game_config` 拷进 `dist/game-config`；`resolveCatalogImage` 一律同源 `/game-config/…`（打包 `tauri://` 同样走前端资源）
- 能力状态：策略对比 / 背包 / 好友田应显示作物图；需重新 `pnpm build` 或重启 `cargo tauri dev`

### 2026-08-18 — 看板日志顺序 / 头像 / 游戏配置入口

- 日志：刷新后看板倒序。Rust `engine_global_logs` / HTTP `get_logs` 按新→旧截断，直播 `pushLog` 却是旧→新追加。改为 last N、旧→新（对齐 Go `hub.Logs.Query`）；Socket `logs:snapshot` 同样升序；看板 `applyLogEntries` 再按 `ts` 升序
- 头像：登录回包 `BasicInfo.avatar_url` 未进 `StatusData` / `get_stats` / `PanelStatus`，看板 `v-if="status?.avatar"` 不渲染。登录写入 avatar，status JSON 带出，DTO 读 nested + `AccountRecord` 兜底；看板补 `https:`、`no-referrer` 与字母占位
- 游戏配置：Tauri 无 `devUrl` 托管 `dist`，history 路由会撞 `dist/game-config/` 静态目录。桌面改为 hash（对齐 Go `.env.desktop`）；页面仍 `/farm/game-config`，资源仍 `/game-config/*`
- 能力状态：刷新看板日志新在下；在线账号个人信息有头像或字母占位；侧栏能进游戏配置（需重新 `pnpm build` / `cargo tauri dev`）

### 2026-08-18 — 看板断线日志可读化

- 现象：心跳超时后看板出现英文 `(source=heartbeat_timeout, phase=online)`，并把 `account-log:new` JSON 原样刷成系统日志
- 修复：断开原因改中文（心跳超时 / 被踢下线 / 连接关闭）；`account-log:new` 补 `message`/`event`；看板不再把账号审计日志当运行日志展示（避免与系统日志重复）
- 能力状态：断线应显示「连接已断开，不再使用旧 Code 重连（心跳超时）」；不再出现 `account-log:new {json}`

### 2026-08-18 — 应用宝授权落盘与掉线换码重连

- 网关 `code` 仍一次性；扫码 confirm 后把应用宝 `openid` / `login_buffer` / `accesstoken` 写入 `accounts.json`
- 连网关前用 `login_buffer` 换新 code；buffer 失效时用 accesstoken 向应用宝换票再试
- 传输断开 / HTTP 400 / 踢号 / 进程重启自动换码重连（最多 3 次）；仅手动停止不重连
- 列表 API 脱敏，不返回 buffer/token，仅 `wxAuthorized`
- 相对 Go：这是 Rust 增强，不是缺口
- 能力状态：微信扫码账号掉线后应自动重连；授权失效才提示重新扫码

### 2026-08-18 — 应用宝 token 续期 / 失败清授权 / 本机微信快速授权

- 扫码与本机快速授权 confirm 后落盘 `refreshtoken` + `expires_at`；列表 API 继续脱敏（不返回 buffer/token/refresh）
- mint 失败：有 refresh 时先 `pcyyb_refresh_token_auth` 再换 `login_buffer` 再 mint；仍失败则清 buffer/token/refresh、保留 openid，**不**排 5 分钟重连，推送 `account_status` / `wxAuthorized=false`
- 后台保活：每 30 分钟检查，token 剩余不足 45 分钟则续 token+buffer（不 mint 网关 code）
- 桌面端微信 Tab：「本机微信 | 扫码」；WebView 调 `localhost.weixin.qq.com`（Windows + 已登录桌面微信）；检测失败自动回退扫码
- HTTP：`POST /api/wx-login/quick-tasks`、`POST .../confirm`；桌面 IPC：`wx_quick_login_create` / `wx_quick_login_confirm`
- 能力状态：授权失效后账号页授权列应变为未授权；Windows 本机微信可一键添加；旧账号无 refresh 跳过保活续期

### 2026-08-18 — 授权状态列 / 5 分钟再重连 / 策略落盘

- 账号列表「启用」改为「授权状态」（应用宝 login_buffer 是否在）
- 已授权账号：桌面/服务重启后先打日志，等 5 分钟再自动换码重连；踢号/断线同样等 5 分钟（最多 3 次）
- 重连开始、换码成功/失败、启动失败都写入运行日志
- 策略页保存补上偷菜黑名单；账号配置反序列化加 default，解析失败打 warn，避免整份配置被跳过看起来像「重启重置」
- 能力状态：列表应显示已授权/未授权；重启后看板出现「将在 5 分钟后自动重连」，到期后有成功或失败日志；改策略保存后再重启应保持

### 2026-08-18 — 下线提醒可配置

- 现象：踢号/掉线后运行日志报「下线提醒配置不完整：channel=webhook, token=未设置」，桌面设置页没有入口
- 原因：全局默认 `channel=webhook` 且标题/正文已填，但 endpoint/token 为空；触发逻辑把这当成「已配置但不完整」写错误日志。桌面 IPC 也没有保存/测试命令
- 修复：endpoint 与 token 都空（或渠道为 none）视为未配置，静默跳过；Webhook 只校验接口地址。自动化设置增加「下线提醒」页，IPC `get/set/test_offline_reminder`
- 能力状态：未填 webhook 不再刷运行日志；设置页可保存渠道并测试推送

### 2026-08-18 — 心跳 RPC 超时不再当掉线刷屏

- 现象：重连后看板反复出现「心跳心跳超时 Heartbeat 失败: 请求超时: Heartbeat (seq=…, pending=1)」
- 原因：不是旧 socket 没关。单条 WS 上 GetAll 等大包占着通道时，Heartbeat 20s 等不到回包；interval 里 `tokio::spawn` 不受 preventOverlap 约束会叠发。`pending=1` 是取消本心跳后还有别的 RPC 在路上。原 bot `catch(() => {})` 吞掉，Rust 曾把这类超时写进看板
- 修复：socket 忙或已有 Heartbeat 在路上则跳过本次发送；RPC 超时只打 debug，不记 `HeartbeatTimeout`。真掉线仍走 30s 无响应且 pending=0 的静默杀
- 能力状态：忙时运行日志不应刷 Heartbeat 请求超时；连接真死仍出现「连接可能已断开」并停止账号

### 2026-08-18 — 手动启动写运行日志 / 微信会换码

- 现象：点启动连上后看板没有启动/登录成功日志
- 原因：`启动账号` 只进账号审计日志（看板不展示）；换码成功走 `WorkerEvent` 且发生在 panel_log 注册之前；`WorkerEvent::Started` 被事件桥忽略；登录成功只打 tracing
- 行为：已授权微信账号每次启动都会用应用宝 login_buffer 换一次性网关 code（失败则刷新 buffer 再试）。无授权则用已保存 code
- 修复：启动立刻写运行日志；换码中/成功/失败、网关已连接、登录成功（昵称+等级）写入看板
- 能力状态：点启动后应出现「开始启动」→「换码成功」→「登录成功：… Lv…」；无授权账号则走「用已保存的登录码连接」

### 2026-08-18 — 游戏 RPC 不再 10s/20s 硬切（超时连锁 / 卡巡查中 / 日志双份）

- 现象：请求超时 → 心跳超时 → 全部超时，看板一直「巡查中」；运行日志每条打两遍
- 原因：单条游戏 WS 上几乎所有 RPC（本田/好友/背包 10s、访客记录 2.5s 探测）硬切 waiter 后继续发请求，把通道堵死；心跳在 pending>0 时 skip，pending 被 cancel 打成 0 后误杀；统一 tick 的 `clear` 会 abort 正在跑的巡查；看板同时 `pushLog` 了 `log:new` 和派生的 `farm_operation`/`friend_interact`
- 修复：WS 读/写分 task；`Gateway::request` 等到回包或断线（仅 Login/Heartbeat/握手保留短超时），业务 RPC 账号内串行；心跳只 skip 叠发，静默看入站帧不看 pending；timeout 任务开火后另 spawn，abort 用 Drop 清 running/`farm_at`；看板只从 `log:new` 写运行日志
- 能力状态：巡查中点土地/背包/好友/活动/商城/任务/邮件不应再刷游戏「请求超时」或心跳停号；真掉线显示离线；偷菜/出售/土地推送各一行

### 2026-08-18 — 桌面壳对齐 Wails（菜单 / 托盘 / 发版 / 更新）

- 对照 `qq-farm-desktop`：macOS 原生菜单 + 全平台托盘 + 关窗进托盘；不移植「在浏览器中打开」（IPC 内嵌无 HTTP）
- 安装包 `bundle.resources` 打进 `tsdk.wasm` 与 `game_config`；release 数据目录走 OS `QQFarm`
- GitHub Actions `v*` tag 打 Windows NSIS（用户级）+ macOS universal；`tauri-plugin-updater` 读 `latest.json`
- 删除无用的 `qq-farm-cli`
- 能力状态：托盘可显隐/退出；干净机器安装后能加载 TSDK 登录；打更高版本 tag 后「检查更新」能换包重启

### 2026-08-18 — Windows 发版签名与编译警告

- Windows CI 把空的 updater 密钥密码当成错误密码；仅在 Secret 非空时写入环境变量，并去掉私钥 CR
- 清掉 core/desktop 未使用字段、重名 glob、弃用 `Account` 导出；CI `RUSTFLAGS=-D warnings`
- 能力状态：Windows job 能签 updater 产物；`cargo check --workspace` 无 rustc warning

### 2026-08-18 — Windows 无边框与发版版本号对齐 tag

- Windows 使用 `tauri.windows.conf.json` 关闭原生边框，保留前端自定义最小化/最大化/关闭按钮；对齐 Wails frameless
- Release workflow 在构建前用 tag 同步 `Cargo.toml` / `tauri.conf.json` / `desktop-ui/package.json` 版本
- 能力状态：Windows 不再出现系统标题栏与虚拟按钮重复；安装包文件名与 Release tag 一致
