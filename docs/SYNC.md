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
| 微信扫码拿码并启动 | 齐 | 扫码 → auth_code → worker；用过的码不可重连 | `services/wx_login/*`, `routes/wx_login.rs` |
| 本田务农循环 | 齐 | 除草除虫浇水 → 收获 → 铲除 → 种植（含多格）→ 施肥/解锁升级；默认策略/skip_own_weed_bug/smart 秒数对齐 bot | `services/farm/*`, `runtime/worker_loop.rs` |
| 好友帮助 / 偷菜 / 捣乱 | 齐 | 列表、访问、帮助（经验门控）、偷菜（stealers/空访跳过）、静默、黑名单落盘；捣乱按日限/启动筛选/`1001046` 停 | `services/friend/*` |
| 背包展示与操作 | 齐 | 按 UID 堆分行；含 `key`/`uid`/`mutantTypes`/`groupKey`；系统物品分离 | `services/warehouse.rs` |
| 自动/手动出售果实 | 齐 | 自动受 `sell` 开关；跳过不可售；手动 `sell_items` 预检拒绝不可售 | `warehouse` + `game_config` + `worker_loop` |
| 商城 / 神秘商店 / 月卡 / 钻石 | 齐 | 列表、购买（神秘 Buy 无回包）、月卡、充值信息 | `mall`, `mystery_shop`, `monthcard`, `pay`, `commerce` |
| 日常领取 | 齐 | 任务（成长 claim 后刷新 TaskInfo + `currentTask`）、邮件、分享等 | `task`, `email`, `share`, … |
| 活动中心 | 齐 | 千星游记、观星、星砂、节令、青梅（含已领幂等） | `activity_center*` |
| 面板鉴权与账号 | 齐 | 登录注册、卡密、账号 CRUD、设置 | `routes/auth`, `account`, `admin` |
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
