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
| **qq-farm-rust** | `main`（本提交：bot 9-10 萌宠日记增量） | 2026-09-10 | 萌宠成长日记 + 协议 1.14.0.1_20260909 + TSDK QQ 宿主初始化 |
| **qq-farm-bot** | `3bb11e2` | 2026-09-10 | 萌宠成长日记（PR #71）+ TSDK QQ 宿主初始化（`b0a4405`）+ 版本 20260910 |

文档范围：**业务行为 + 面板 HTTP/Socket 契约**（不含改 Vue 面板本身）。

### 版本语义（避免混用）

| 概念 | 当前值 | 用途 |
|------|--------|------|
| 客户端版本 `FARM_CLIENT_VERSION` | 默认 `1.14.0.1_20260909`（与 bot `config.ts` 默认一致，UPDATED_AT `1789004223123`） | 进游戏网关声明 |
| bot `core` 包版本号 | `20260910` | 原项目发布标签，≠ 客户端版本字符串 |
| 青梅活动 ID | 每日 `2026081201` / 酿造 `2026081202` | 活动协议 |
| 公益小红花活动 ID | 活动组 `2026090900` / 活动 `2026090901` | 活动协议 |
| 萌宠成长日记活动 ID | 活动组 `2026090100` / 养成 `2026090101` / 种子赠礼 `2026090102` / 拾物小铺 `2026090103` | 活动协议 |

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
| 好友帮助 / 偷菜 / 捣乱 | 齐 | 列表、访问、帮助（经验门控）、偷菜（气泡+自巡/空访/一键 Harvest 主地回退/先偷后帮）、静默仅挡好友、黑名单落盘；捣乱按日限/启动筛选/`1001046` 停 | `services/friend/*` |
| 背包展示与操作 | 齐 | 按 UID 堆分行；含 `key`/`uid`/`mutantTypes`/`groupKey`；系统物品分离 | `services/warehouse.rs` |
| 自动/手动出售果实 | 齐 | 自动受 `sell` 开关；`sell_cond` 满足后用 `cond_sells`（活动结束后 / 道具过期后等）；手动预检拒绝不可售 | `warehouse` + `game_config` + `activity_windows` |
| 商城 / 神秘商店 / 月卡 / 钻石 | 齐 | 列表、购买（神秘 Buy 无回包）、月卡、充值信息 | `mall`, `mystery_shop`, `monthcard`, `pay`, `commerce` |
| 日常领取 | 齐 | 任务（成长 claim 后刷新 TaskInfo + `currentTask`）、邮件、分享等 | `task`, `email`, `share`, … |
| 活动中心 | 齐 | 千星游记、观星、星砂、节令、青梅（含已领幂等）、鹊桥寄情（筑桥领取 + 赠香囊）、公益小红花（领种子/捐爱心/每日礼包）、萌宠成长日记（养成/寻宝/锦囊/夺宝/种子/小铺/节令/记录） | `activity_center*` |
| 面板鉴权与账号 | 齐 | 登录注册（无卡密）、账号 CRUD、设置 | `routes/auth`, `account`, `admin` |
| 面板农场/好友/活动/商业 API | 已随 server 删除 | **只维护桌面版**（Tauri IPC 语义对齐原 HTTP 契约） | `qq-farm-desktop/src/commands/*` |
| Socket 状态/日志推送 | 齐 | `status:update` / `log:new` 等 | `socket.rs` |
| 离线重登提醒 | Rust 增强 | QQ 官方机器人主动单聊提醒；应用宝失败附重登录二维码 | `runtime/relogin_reminder.rs` |
| 推送通知 | Rust 增强 | Rust 原生 QQ Bot AccessToken + Gateway + C2C；微信 Bot 预留；**渠道面固定为 QQ Bot / 钉钉 / 微信，不对齐 bot MeoW（用户决策 2026-09-01）** | `services/qq_bot` |
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
  3. 普通 RPC 最多 5 in-flight、最多 100 排队；Heartbeat 插队不占槽
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

### 2026-08-18 — 应用宝保活提前续 token

- 现象：剩余不足 45 分钟会进保活并落盘，但 `expires_at` 不变；只换了 `login_buffer`
- 原因：`refresh_credentials_and_buffer` 内部用 `token_due_for_refresh(0)`，access_token 未过期就跳过 `pcyyb_refresh_token_auth`
- 修复：内部续期改用 `WX_KEEPALIVE_AHEAD_SECS`（45 分钟）
- 能力状态：保活成功后 `wx_token_expires_at` 应往后推；日志「应用宝 token 保活成功」

### 2026-08-18 — 保活对齐 YYB：到期必续 token + 建议重扫

- 对照 YYB-Go-Enhanced：45 分钟提前量只做保活外层门闸；`refresh_credentials_and_buffer` 有 refresh_token 就一定先 `pcyyb_refresh_token_auth` 再换 `login_buffer`
- 落盘 `wx_refresh_token_observed_at`；微信未轮换 refresh_token 时不改时钟；约 25 天后列表 `wxRescanRecommended` 显示「建议重扫」
- 能力状态：保活后过期时间应往后推；同一 refresh 连续约 25 天账号授权列变为「建议重扫」

### 2026-08-18 — 桌面本机微信快速授权改原生代理

- 现象：Tauri/Wails WebView 直连 `https://localhost.weixin.qq.com` 失败（自定义协议 CORS / Local Network Access / 微信自签证书），检测后静默回退扫码
- 对照 YYB-Go-Enhanced：OAuth 参数已齐；YYB 用浏览器碰本机微信是因为服务端可能在远程 Docker。桌面进程在本机，改为 Rust 绑 `127.0.0.1` 代理 `/api/check-login`、`/api/authorize`
- IPC：`wx_quick_login_detect` / `wx_quick_login_authorize`；失败原因回传前端
- Go Web 同样走后端 `POST .../detect`、`.../authorize`：Chrome 直连 `localhost.weixin.qq.com` 会被 CORS / Local Network Access 拦住；本机面板由 Go 绑 `127.0.0.1` 代探
- 能力状态：Windows 已登录未锁定桌面微信时，添加账号「本机微信」应检出昵称并可确认授权
- 2026-08-18 补：新版 Weixin.exe 除 14013-14015 外还会监听 14016/14019/14022/14023；探测端口扩到 14013-14025 + 13013-13015

### 2026-08-19 — 微信版偷菜对齐 Go（本田不挡静默）

- 策略：只玩微信农场——无保护罩、无每人偷满；狗咬只扣金币不挡偷。bot 不改。
- 巡逻：GetAll `steal_plant_num` 好友多会漏推 → 气泡优先 + `ceil(n/4)` 零气泡自巡（对齐 Go）。有气泡空访仍用 `(gid, steal_num)` noop，零气泡不走 noop。
- 偷菜：`Harvest is_all=true` 一键；失败后对 **主地** `is_all=false` 按地回退，不传从地。识别 `1001040`。微信确认无 10008 日限：不读 `OperationLimit`、不调 `CheckCanOperate`、不截断 `can_steal_num`。QQ 仍走配额，无数据 fail-open。
- 偷菜必帮忙：有可偷则 **先偷后帮** 一键 Farming，不受帮忙开关/经验上限；帮忙失败不挡。
- 静默：`check_farm` 不再看 `friendQuietHours`；help/steal tick 仍静默。
- 选地：微信路径只信成熟 + `stealable` + 主地，不用 stealers。`OP_NAMES` 10008 改为「偷菜」。
- 能力状态：代码侧与 Go 微信偷菜策略对齐；实机仍看 L3

### 2026-08-18 — 桌面品牌与 Go 版拆开

- 显示名 / 窗口 / 托盘 / 菜单改为 **QQ Farm**（Go 仍为「QQ农场智能助手」）
- `identifier` 改为 `com.qqfarm.rust`；release 数据目录改为 OS `QQFarmRust`
- 图标改为锈橙底绿苗 + 六边形 R 角标（侧栏 logo / favicon 同步）
- 能力状态：与 Go 桌面可并装；Dock / 托盘 tooltip / 数据目录互不覆盖

### 2026-08-19 — 微信 10008 确认无限

- 微信农场没有偷菜日配额（operation `10008`）。巡逻不再读 `OperationLimit`，进场不再调 `CheckCanOperate`，也不用 `can_steal_num` 截断可偷地。QQ 仍走配额。
- 能力状态：微信偷菜只看成熟 + `stealable` + 主地；次数门控已去掉

### 2026-08-19 — 好友气泡推送单点刷新 + 列表排序/一键偷取

- `LandsNotify` 好友田不再丢弃：对该 gid 调 `GetGameFriends`（失败则用推送土地只升不降合并），写回列表缓存与 GetAll 短缓存；不全量 GetAll。
- 推送可偷写入 `push_steal_hints`：GetAll 漏气泡时仍进偷菜队列；进场或 GetAll 追上后清除。新气泡会去掉 `steal_cleared` / 空访 noop / 自巡 visited。
- 好友页「刷新列表」`force` 会作废 GetAll 800ms 短缓存。排序：可偷 → 可帮 → 等级。单行按钮「一键偷取」（`is_all` + 主地回退）；顶栏「全部偷取」仍扫所有可偷好友。
- 能力状态：气泡推送应只改该好友行；手动刷新走强制 GetAll

### 2026-08-19 — 个人农场操作按钮按状态显示

- 桌面 / Go web 个人农场顶栏：无活不显示。收获看成熟（倒计时到 0 也算）；一键务农看草/虫/旱；种植看空地或枯株；升级看可升/可解锁；一键全收仅在收获/务农/种植有活时出现。
- Rust `op=clear` 改为真正的一键务农（除草/除虫/浇水），对齐 Go 与按钮文案；铲除仍走 `op=remove`。
- 能力状态：没活不显示对应按钮；成熟倒计时归零后「收获」应出现。

### 2026-08-19 — 同步 bot 鹊桥寄情与活动体系（`6f696bf`）

- 协议整份覆盖拷贝 bot `proto/` 后生成；客户端默认 `1.13.2.8_20260723`
- 鹊桥寄情：`GetGroup` / Operate 筑桥领取 25 / 赠香囊 26；快照串行拉 List + qixi；HTTP `/api/activity-center/qixi*` 与桌面 IPC；desktop-ui 活动页鹊桥 tab
- 出售：`sell_cond` 满足后用 `cond_sells`（活动结束后 / 道具过期后 / 活动结束前 / 活动区间外），窗口来自 `ActivityService.List`（TTL 5 分钟）
- 网关：最多 5 in-flight、最多 100 排队；Heartbeat 插队；背包 GetBag 单飞
- 能力状态：代码齐 + 单测；实机 L5 待验

### 2026-08-19 — 活动中心对齐 bot 目录入口与鹊桥互动

- 桌面活动页改为先看活动列表，再进入千星游记 / 鹊桥寄情 / 青梅；鹊桥页展示阶段消耗与奖励、香囊步进器
- 赠香囊数量按整数解析（含 JSON 浮点），IPC 同时收 `count` / `sachetCount`，避免只送 1 个
- 赠送好友列表显示头像、等级、名称，不再用 GID 当主文案
- 能力状态：面板交互对齐 bot 信息架构；未搬玻璃拟态样式

### 2026-08-19 — 同步鹊桥道具图与 ItemInfo

- 从 bot `gameConfig` 补齐 20 条 ItemInfo（鹊羽/香囊/礼包等）及对应 `seed_images_named` PNG
- 活动详情返回按钮改为带返回图标的 secondary 按钮
- 能力状态：静态资源齐；桌面需重启以嵌入新 ItemInfo 并重拷 dist/game-config

### 2026-08-19 — 鹊桥活动说明与赠送提示

- `text_content` 对齐 bot：从活动 extra 的 `tips.txt` 抽出活动说明段落
- 鹊桥页增加可展开的活动说明；赠送成功提示用好友名称，不再带 GID
- 能力状态：规则文案与赠送提示对齐 bot 语义

### 2026-08-19 — 好友页顶栏操作按钮可见

- Naive `NCard` 无 title 时不渲染 `#header-extra`，好友页「一键偷取 / 同步 / 刷新列表」因此消失
- 改为放在搜索行右侧；最近访客刷新同样迁出 header-extra
- 能力状态：桌面与 Go web 好友页工具栏应可见

### 2026-08-19 — 最近访客字段对齐 bot（camelCase）

- `NormalizedRecord` 序列化改为 camelCase（`actionType` / `actionLabel` / `avatarUrl` 等），对齐 bot `/api/interact-records`
- 修复桌面「全部显示互动」、筛选失效、头像读不到的问题
- 能力状态：访客类型标签、筛选与头像字段契约齐

### 2026-08-19 — 好友页去掉重复「同步」按钮

- bot 好友列表只有「刷新列表」；桌面 / Go web 去掉与 refresh 同路径的「同步」
- 顶栏保留全部偷取 + 刷新列表
- 能力状态：好友页工具栏对齐 bot

### 2026-08-19 — 无消费者暴露层清理（Phase 1）

- 删除 desktop 无 UI 调用的 IPC（friend_sync、known_gids、get_settings、list_accounts 等）及对应 server HTTP / app 包装
- 删除 desktop-ui `service/tauri` 脚手架与 `farm.ts` 死 export；server 保留核心路由并新增 `/api/settings/system-config*` 别名
- 能力状态：暴露面与 bot 实际消费对齐；core 运行时逻辑（访客 GID 补充、化肥定时器等）保留

### 2026-08-19 — 运行环境设置（Phase 2）

- 设置页新增「运行环境」Tab：设备预设、serverUrl、platform、OS、clientVersion、deviceId、UA、保存/重置
- 新增 desktop IPC：`get_device_presets` / `get_system_config` / `set_system_config` / `reset_system_config`
- 能力状态：对齐 bot Settings → 系统 → 运行环境

### 2026-08-19 — 化肥保存即检测 / 活动说明 / 面板 parity 补项

- 保存自动控制后调用 `farm_fertilizer_check_and_buy` IPC（开启化肥购买时）
- 千星游记 / 观星礼录活动说明 dialog；青梅活动说明 collapse
- 好友列表 25 条/页分页；概览日志 eventType 筛选；账号页清理已停止 + 批量删已停止
- 能力状态：plan v2 剩余 UI 缺口已补齐

### 2026-08-19 — 游戏配置分页 / 作物图标 / 看板日志布局

- 游戏配置页：搜索/筛选后本地分页表格（`NDataTable` + `pagination`），图标改 `<img>` 失败回退 emoji
- 图标：`resolveCatalogImage` 在 Tauri 环境走 `farmcfg://` 自定义协议，Vite dev 仍走 `/game-config/*`
- 看板日志：隐藏 `avatar_probe`（人机头像诊断 / 小果头像）日志；筛选栏 grid 对齐；日志区加高并用固定列 grid 对齐时间/模块/事件/正文
- 能力状态：游戏配置可分页浏览且图标恢复；看板不再刷头像诊断噪音，日志列对齐 bot 观感

### 2026-08-19 — 游戏配置改为账号管理式搜索+分页表

- 桌面「游戏配置」对齐账号管理布局：折叠搜索卡（`NCard` + `NCollapse` + `NForm`）+ `TableHeaderOperation` + `useNaivePaginatedTable` / `NDataTable` 远程分页
- 种子/果实/道具仍为页内 Tab；筛选与翻页走本地缓存切片，IPC `config_list_*` 不变
- 能力状态：游戏配置列表交互与账号管理一致（搜索、列设置、新增/批量删除、分页）

### 2026-08-19 — 游戏配置果实/道具价格对齐 sells

- 桌面 IPC `config_list_fruits` / `config_list_items` 原先直接序列化 Item，没有 `price`/`priceId`
- 现从 `sells`（果实无则回退 `cond_sells`）解析首个报价，对齐面板 HTTP `/api/config/fruits|items`
- 道具列表排除种子(type=5)与果实(type=6)
- 能力状态：游戏配置果实/道具价格不再全是 0 金币

### 2026-08-19 — 游戏配置种子等级取 ItemInfo.level

- `get_all_seeds` 原先用 Plant.json `land_level_need`（全为 1），桌面种子列表等级全是 Lv.1
- 改为优先 ItemInfo.level，没有再回退 land_level_need（对齐 bot `getAllSeeds` / 种植策略）
- 能力状态：种子列表等级与 ItemInfo 一致

### 2026-08-19 — 快速授权全静默回退 / bot 资源镜像工具

- 桌面本机微信快速登录的 create / detect / authorize / confirm 任一步失败均不弹错误，直接切换微信扫码；仅二维码流程自身失败才显示错误
- Rust 仓新增 `tools/sync-from-bot.mjs`：只读 qq-farm-bot，按哈希预览并以 `--apply` 镜像四份游戏配置、作物图片和 proto；bot 工具及代码不改
- 新增 `docs/OFFICIAL_RESOURCE_SYNC.md`：说明微信 4.1.x Windows/macOS 缓存路径、`--source` 反编译目录、CDN 资源下载、Rust 同步和 `capture-dir/*.bin` 协议抓包的区别
- 验证：同步工具 Node 测试 5/5；实际 dry-run 检出配置 3 处差异、图片 908/908 与 proto 36/36 一致；`cargo check -p qq-farm-core`、`pnpm build` 通过
- 已知存量：`pnpm typecheck` 仍被账号页 number/string 与活动页缺 `rules` 两处既有错误阻断，本次修改文件无新增诊断

### 2026-08-19 — 桌面窗口拖动 / Windows 作物图片

- 拖动：标题栏只使用 Electron/Wails 风格的 `-webkit-app-region: drag`，Tauri v2 没有触发原生拖动；现 macOS / Windows 顶栏非交互区域统一调用 `startDragging`，控件仍可点击
- 图片：前端硬编码 `farmcfg://localhost/...`，macOS 可用但 Windows WebView2 需要 `http://farmcfg.localhost/...`；改用 Tauri `convertFileSrc(path, "farmcfg")` 按平台生成 URL
- 验证：资源协议同时覆盖 `farmcfg://localhost/...` 与 Windows `http://farmcfg.localhost/...`
- 能力状态：macOS 可拖动窗口；Windows 作物、背包、商城和活动图片恢复

### 2026-08-20 — 重连等待细分 / 设置页交互

- 应用宝账号掉线后第 1 次重连等待 3 分钟，第 2～3 次各等待 1 分钟；客户端启动后的首次自动重连等待 1 分钟
- 设置页好友静默时段改为同排时间选择器；推送渠道官网通过 Tauri opener 交给系统默认浏览器打开
- 能力状态：重连日志与实际等待一致；桌面 WebView 点击渠道官网不再无响应

### 2026-08-20 — 下线推送渠道下架

- Qmsg 酱与 Push Plus 服务已下线，从桌面设置、渠道白名单和推送路由中移除（含 Push Plus Hxtrip）
- 历史配置仍选中已下架渠道时按未配置处理，设置页回退为「不推送」，避免持续发送失败

### 2026-08-20 — QQ Bot 扫码绑定

- 下线提醒改为扫码绑定：用户不再手填 AppID/AppSecret/user_openid，设置页仅展示绑定状态、扫码入口、解绑与测试推送
- 设置页填写 QQ 机器人 AppID/AppSecret，再扫码绑定通知对象；环境变量仍可覆盖全局凭据
- Gateway 监听 `C2C_MESSAGE_CREATE` 自动采集 `user_openid` 并保存绑定；绑定成功自动回复确认消息；发送“解绑”可清除绑定
- 绑定后固定推送三类通知：账号下线、账号上线、应用宝授权二维码；设置页不再配置标题/内容/删除秒数
- 上线/下线通知文案与日志按类型区分；踢下线并进入应用宝重连时也会立即推送下线通知
- 绑定不再使用猜测的 `q.qq.com/qqbot/{appId}` 假链接（会 404）；点绑定后用手机 QQ 直接给机器人发消息即可

### 2026-08-20 — 桌面设置持久化目录对齐

- `cargo tauri dev` 与安装包共用 OS 数据目录 `QQFarmRust`（不再默认写仓库 `data/`）；设置落在该目录下的 `store.json`
- 会写 `store.json` 的单测改为临时 `FARM_DATA_DIR`，避免覆盖真实配置；保存离线提醒时若 payload 解失败或 binding 为空则保留已有绑定

### 2026-08-28 — 同步 bot 大版本（`8dae528`）+ 删除 API 服务 crate（只维护桌面版）

- 基准：rust 本提交 / bot `8dae528`（区间 `6f696bf..8dae528`，71 个提交）
- **结构**：删除 `qq-farm-server` crate（HTTP/Socket.IO 面板服务）及其 workspace/脚本/文档引用；
  `qq-farm-app` 门面仅由 `qq-farm-desktop`（Tauri IPC）消费；E2E 随 crate 删除
- **协议/版本**：
  - proto 全量同步（weatherpb 全新；plantpb 社交事件/互动记录/变异扩展、careerpb 生涯、
    friendpb DelFriend、visitpb brief_dog_info+weather、itempb UseTarget、activitypb 天气消息）
  - `client_version` → `1.13.3.14_20260826`；新增 `clientVersionUpdatedAt` 时间戳语义
    （保存版本只在比默认新时沿用，对齐 bot `resolveClientVersion`）；TSDK 升级
    `v3.9.0.1787640848`（wasm 同步覆盖）；游戏配置镜像同步
    （ItemInfo/Plant/RoleLevel/Land + MutantEffect/BuffCfg + seed_images_named 重排为
    `seed_images/`+`mutant/` 子目录，918+13 张；`tools/sync-from-bot.mjs` 配置清单
    扩展至 6 份）
- **天气活动「雨落成诗」**（全新）：活动组 2026070300；采雨走 Activity Operate(type=9,field107)
  不走 ItemService.Use、1034040 幂等；好友现场天气以 Enter.weather 为准、field_9==4 已采标记；
  召唤/青蛙/乌云瓶走 ItemService.Use（乌云地块合格判定：生长中+无 5006 记录）；气象研究/兑换/
  任务快照严格串行构建；好友扫描批 5/间隔 300ms/TTL 600s/让位好友巡查（等不到回 deferredGids）；
  WeatherChangeNotify 清缓存；桌面 IPC `weather_*` 9 条 + 活动页雨落成诗视图
- **好友宠物体系**（全新）：`friend-pet-<sha256>.json` 按天缓存（Enter.brief_dog_info 写透，
  dog_id=0 也是结论）；经验满时仅护主犬（90021）好友继续帮（`friend_help_protect_dog_ignore_exp_limit`，
  默认开）；pet-sync 每日同步自适应节奏（批 5/配额 10→25/3min 快通道/60s 让路重试/30min 忙冷却/
  90s 启动延迟；区分抢窗口失败与服务端静默）
- **统一好友任务**：help/steal 双 tick 合并为 friend tick（friendMin/Max=20-25s，旧账号取两组
  min 迁移）；一次 GetAll 构建 visit plan（wantSteal/wantHelp/wantBad），每好友一次 Enter 完成
  帮→偷→坏（偷菜必帮忙语义随 bot 终态回归）；坏对象=无可偷无可帮按等级 top20
- **生涯收获偷菜**（全新）：CareerService.CareerInfoGet；本田 /api/lands 与好友土地回包带 career；
  面板「万」格式化与收偷比
- **好友申请过滤**（全新）：`friend_auto_accept`（默认开）+ 等级过滤（手动最低/不低于自己取严）+
  收偷比过滤（harvest×stealPart ≥ steal×harvestPart，默认 8:1）；先拒后收，生涯查询失败者搁置
- **删除好友**（全新）：FriendService.DelFriend + 成功落黑名单；桌面 `friend_delete`
- **变异体系**：MutantEffect.json 全量（含闪电 12/晶辉 14）；土地/背包展示具体变异类型（名称+图标，
  effect_name 优先）；变异展示植株映射链（多效果组合优先）；紫晶共鸣以服务端 LandInfo.buff 为准
  （level5+有变异才显示）
- **互动道具清理**：黄金虫/足球/乌云（uses+targets 实时记录）并入一键务农地块；青蛙（农场级
  AllLands.social_events）经 Farming field5 发送、无地块时回退首个有效作物地；FarmSocialEventsNotify
  触发巡查；field_40 仅作七夕灵露兜底（变异 13+历史码）
- **其它**：bagSeedLandTypes 地块限制（受限种子先种）；静默 `continueFarm`（默认 true 巡田继续，
  nextChecks 带三 quiet 标记）；`show_manual_fertilizer`；系统时区白名单（跨日键统一走
  服务器时间+配置时区）；钉钉推送渠道+加签（HMAC-SHA256，endpoint/secret）
- **网络对齐说明**：bot 请求分级并发/健康度退避在 rust 的映射——前台保护已有（5 槽/100 队列/
  心跳插队/skip 叠发）；后台让位由 pet-sync（配额节奏+网关 pending 空闲等待+好友巡查互斥）与
  天气扫描（批间隔+让位标记）行为性覆盖；重连退避沿用 v0.2.7-8 阶梯
- **桌面 UI**：活动页新增「雨落成诗」视图（天气卡/兑换/任务/研究链/背包/说明/好友扫描详情卡，
  写操作用回包 snapshot 就地刷新）；好友页删除好友+好友土地生涯卡+宠物徽标（petState/pet 经
  FriendSummary 透传）；个人农场生涯卡+土地变异徽标/紫晶共鸣/互动道具效果；背包变异名称；
  设置页申请过滤/静默继续巡查/手动施肥显示/时区/钉钉渠道；看板静默中标签
- 验证：`RUSTFLAGS=-D warnings cargo check --workspace --all-targets` 0 错 0 警；
  `cargo test -p qq-farm-core --lib` **926/926**、`-p qq-farm-app` 15/15（单线程；
  并行下 `zero_interval` 为存量 30ms 时序 flake）；`pnpm -C desktop-ui typecheck` 0 错、
  `pnpm -C desktop-ui build` 成功
- 能力状态：矩阵保持齐；实机 L 清单仍待勾选（天气活动链路、宠物同步节奏、统一巡查为新增待验项）

### 2026-09-01 — 增量同步 bot（`8dae528..e44cc12`）

- 基准：rust 本提交 / bot `e44cc12`（区间 4 个提交：`228f1b9`、`56f71a5`、`25a5cf0`、`e44cc12`）
- 前置：把 8-28 大同步（含删除 server crate）补落为独立 commit `8a3d1e6`，本次增量单独成提交
- **公益小红花**（全新，对齐 bot `e44cc12`）：
  - 协议：activitypb 同步（`ActivityData.charity_red_flower=116` + `CharityRedFlower*` 消息族 +
    `ActivityOperateReply` 135/136/138/139），proto 与 bot 哈希一致
  - core：常量 活动组 `2026090900` / 活动 `2026090901` / op 35（领种子）36（捐爱心，一次捐全部）
    38（每日礼包 send_public_fund）；状态走 `ActivityService.List` BFS（含 children）定位；
    `activity_center/charity.rs` DTO 对齐 bot `charityRedFlowerDto`（seedReward 2=可领/3=已领、
    dailyGift 以 public_fund 记录判已领、progressRewards `claimSupported:false` 只展示、
    globalProgress/settlement/actions）；写操作 mutation 串行 + 动作门控 + 回包校验
    activity_id/operate_type
  - 快照：新增 `charity` 字段、actions `charityClaimSeeds/charityDonateLove/charityClaimDailyGift`、
    capabilities、`errors.charity`；目录绑定 gameplay=charity priority 70（对齐 bot 注册表）
  - 桌面：IPC `activity_get_charity` / `activity_claim_charity_seeds` /
    `activity_donate_charity_love` / `activity_claim_charity_daily_gift`；活动页小红花视图
    （爱心/累计/结算/全服进度 + 领种子/捐爱心（二次确认）/每日礼包三操作卡 + 档位奖励 + 说明）；
    目录未命中时快照兜底入口
- **施肥协议修正**（bot `25a5cf0`）：plantpb `FertilizeReply.fertilizer` int64→`corepb.Item`、
  新增 `FertilizerUse`/`fertilizer_use=4`；rust 施肥路径本就丢弃回包（`api.rs fertilize`），
  无行为变化，仅协议文件对齐
- **QQVip 非会员**（bot `e44cc12`）：`code=1021001` → 当日 markDone + result=none，
  当天不再重试（原仅处理 1021002 已领取）
- **版本**：`DEFAULT_CLIENT_VERSION` → `1.13.3.16_20260826`（updatedAt 1788238800000）
- **明确不对齐（用户决策）**：bot 新增的 MeoW 推送渠道**不移植**；rust 推送渠道面固定为
  QQ 官方机器人 / 钉钉 / 微信（预留），后续同步不以 MeoW 为缺口
- 顺手修复（存量）：`card_claim` 测试 `reset()` 未清 `data/cards.json`，其它用例创建的卡密
  持久化后令「库存不足」断言间歇失败；reset 现同步删除卡库存文件
- 验证：`RUSTFLAGS=-D warnings cargo check --workspace --all-targets` 0 错 0 警；
  `cargo test -p qq-farm-core --lib` 新增 charity 7 / qqvip 8 用例全过（934+）；
  存量 flake 与 8-28 记录一致（`zero_interval` 30ms 时序；`init_status_bar_tty` /
  `known_friend_gids_with_file_cache` 并行竞态，隔离运行均过）；
  `pnpm -C desktop-ui typecheck` 0 错、`pnpm -C desktop-ui build` 成功
- 能力状态：矩阵保持齐；小红花链路列入实机待验（L5 扩展）

### 2026-09-01 — 手动操作结果明细（用户反馈，超越 bot 的本地增强）

- 痛点：开礼包 / 手动偷菜 / 出售只提示「操作成功」，看不到开出什么、偷到什么
- **基础设施**：
  - `Gateway::subscribe_notify_scoped()`：带生命周期的 Notify 订阅（Drop 自动退订，
    订阅表按 id 清理，不残留发送端）
  - `services/item_capture`：操作窗口内捕获 ItemNotify 物品增量（回包后 400ms 排空，
    对齐 Harvest/Use 实际所得走推送的协议行为）+ 聚合 / 命名（货币固定文案、果实走
    作物名、其余走 ItemInfo）/ DTO
- **偷菜**：`do_steal_op` 返回 `summary`（如「偷取 3 块地：白萝卜×12、南瓜×3」）与
  `items` 明细；ItemNotify 缺失时退化用被偷地块作物名；「全部偷取」跨好友聚合作物数量
- **使用物品 / 礼包**：`farm_bag_use` 返回 `rewards` + `summary`（「获得 金币×100、点券×10」），
  ItemNotify 捕获优先、为空回退 `UseReply.items`/`land_reward`；背包页展示明细并写入运行日志
- **出售**：`farm_bag_sell` 返回 `sold`/`gained` + `summary`（「出售 白萝卜×20，获得 金币×340」）
- 验证：`RUSTFLAGS=-D warnings cargo check --workspace --all-targets` 0 错 0 警；
  item_capture 4 用例（聚合/命名/退订）通过；`pnpm -C desktop-ui typecheck`/`build` 通过
- 能力状态：面板操作反馈增强；与 bot 面板契约兼容（新增字段，旧字段不变）

### 2026-09-01 — 好友列表去掉逐行「宠物待确认」（用户反馈）

- 现象：好友列表几乎每行都挂「宠物待确认」灰标
- 原因：宠物状态按天缓存，只有巡查访问过该好友或每日 pet-sync 补齐后才有结论，
  其余全是 `unknown`；bot web 对 unknown 同样显示「宠物待确认」（rust 对齐了这一行为）
- 优化（纯 UI，后端不变）：`unknown` 不再显示徽标（护主犬 / 宠物名徽标保留，
  「无宠物」本就不显示）；工具栏改为一条聚合提示
  「宠物状态 N/M 已确认，其余由每日同步自动补齐」（全部确认后隐藏）
- 验证：`pnpm -C desktop-ui typecheck` / `build` 通过
- 能力状态：面板交互优化；`petState` 契约不变

### 2026-09-02 — 修复大号登录初期「转圈→掉线」（socket 发送机制三连修）

- 现象：刚登录点菜单（尤其好友列表，大号必现）转圈后掉线
- 根因链：
  1. 好友 GetAll 无失败单飞（bot `allFriendsRequests` 存 in-flight promise）：rust 只有 800ms
     成功缓存，20s 超时后 tick/面板立刻重发 → 巨型回包在链路上排队叠加
  2. `last_rx_ms` 只在完整消息解析后更新：单个几 MB 回包下载+解密期间入站静默虚高、
     心跳回包被压在后面 → 3 次 miss + 静默>30s 误杀活连接
  3. rust 无 bot 请求班次：登录自动化 burst 与面板点击挤同一 FIFO 5 槽
- **修复**：
  1. `get_all_game_friends` 失败进入 30s 冷却（`FRIEND_LIST_FAIL_COOLDOWN_MS`），冷却期内
     回退陈旧缓存；rpc_gate 串行 + 成功缓存构成完整单飞语义（对齐 bot）
  2. 心跳判死加 pending 保护：`heartbeat_should_force_disconnect`（miss 达标 + 静默超阈值 +
     **无在途请求**）；保护窗封顶 120s（防持续发请求的僵尸连接永不判死）。所有业务 RPC
     20s 超时，真断线 pending 20s 内归零、下一拍照常判死
  3. 前台保留槽：`rpc_slots` 拆共享 4 + `fg_rpc_slot` 保留 1（对齐 bot「非前台业务 ≤ 总预算-1」）；
     `background_scope`（task-local）在调度器两个回调执行点与 pet_sync 循环标记后台，
     桌面 IPC 链路缺省前台
- 验证：`RUSTFLAGS=-D warnings cargo check --workspace --all-targets` 0 错 0 警；
  新增 5 用例（前台保留槽饱和/后台不得占用、task-local 缺省前台+scope 翻转、判死三条件
  与 120s 封顶、列表失败冷却、冷却回退陈旧缓存）全过；`cargo test -p qq-farm-app` 15/15
- 能力状态：连接稳定性对齐 bot 单飞/前台保护语义；实机待验（大号登录连点菜单+好友列表）

### 2026-09-02 — 修复小红花操作被 ACL 拦截（漏声明 IPC 白名单）

- 现象：捐赠爱心报 `Command activity_donate_charity_love not allowed by ACL`
  （视图正常——数据走 `activity_snapshot`；四个操作/查询命令全部被拦）
- 根因：新增 4 条小红花 IPC 只注册了 `generate_handler!`，漏了
  `permissions/desktop.toml` 的 ACL 白名单
- 修复：补声明 `activity_get_charity` / `activity_claim_charity_seeds` /
  `activity_donate_charity_love` / `activity_claim_charity_daily_gift`；
  新增防回归单测（`qq-farm-desktop` 内比对 handler 注册与 ACL 白名单，
  漏声明直接测试失败）
- 验证：`cargo test -p qq-farm-desktop acl` 通过；
  `RUSTFLAGS="-D warnings" cargo check -p qq-farm-desktop --all-targets` 0 错 0 警
- 能力状态：小红花操作链路修复，随 v0.2.11 发布

### 2026-09-10 — 微信快捷登录对齐官方端口与交互（合并子 Tab）

- 对照官方快捷登录实现（`localhost.weixin.qq.com` → 127.0.0.1）：Windows / macOS 均探测
  `14013/14014/14015` + `13013/13014/13015` 共 6 个端口；此前自扩到 14013-14025（含
  14016-14025 观测端口）属过度探测，回退到官方列表
- 交互对齐官方：添加/编辑账号「微信授权」去掉「本机微信 / 扫码」子 Tab。进入即自动探测，
  任一端口命中 → 显示头像 + 昵称 + 绿色「微信快捷登录」按钮 +
  「使用其他头像、昵称或账号」链接（切扫码）；全部失败 → 自动展示二维码并提示
  「未检测到本机微信，请扫码登录」。扫码页保留「重新检测本机微信」入口
- 本机授权失败（如手机端拒绝）不再静默回退：二维码页展示失败原因
- 验证：`RUSTFLAGS="-D warnings" cargo check -p qq-farm-core -p qq-farm-app -p qq-farm-desktop --all-targets`
  与 `vue-tsc --noEmit` 通过
- 能力状态：已登录未锁定的桌面微信在「微信授权」页应直接显示昵称头像并可一键授权；
  未运行桌面微信时自动落到扫码

### 2026-09-10 — 增量同步 bot e44cc12..707a47c（协议 1.13.3.17 / PR #68 / 活动协议修正 / 自动化互斥锁 / NapCat 扫码 / 资源全量覆盖）

- 对照基准：bot `707a47c`（2026-09-09，core 20260908）。此前基准 `e44cc12`（9-1 增量）
  + 小红花进度奖励（`887f0db`）+ 前台请求优先（`d0a6904` 行为对齐）
- **静态资源全量覆盖**（`tools/sync-from-bot.mjs --apply`）：`ItemInfo/Plant/Illustrated/
  MutantEffect` 等配置 JSON 与 60+ 张种子图更新；`mysteryshoppb.proto` 覆盖（activitypb
  的 cabb958 字段 rust 已有，覆盖后与 bot 哈希一致）。脚本 `CONFIG_FILES` 补
  `Illustrated.json`（bot 更新了该文件且 rust 在用，此前一直不在镜像清单）
- **协议版本**：`1.13.3.16_20260826` → `1.13.3.17_20260826`（UPDATED_AT `1788763651029`，
  对齐 bot `cc6a8ab`）
- **PR #68 多季作物阶段识别与补肥**：
  - `game_config.get_plant_grow_phases`：解析官方 `Plant.grow_phases`（名称+时长，
    末冒号分隔）
  - `land_analysis.convert_server_phase_to_client`：phases 按配置后缀对齐（当前下标 =
    grow_phases 总数 − 剩余 phases 数）；成熟判定加 phase==19 / phase_id==19 /
    末配置阶段（盛开）粗状态 2 或 >7；新增 `Unknown` 语义；`PlantPhase::from_i32`
    0/8+ 归 `Unknown`（不再当 Seed）
  - `has_remaining_seasons`（season vs 配置 seasons）；`get_land_lifecycle_state` /
    `classify_harvested_lands_by_map` 对齐：多季或有剩余季 → growing，未知 → unknown
  - `farm scheduler`：收获后 unknown **不再默认铲除**（bot 修复前行为），补拉全量后仍
    未知则跳过并记面板 warn（skip_unknown）
  - `planting.fertilize_by_config_ex`：`multi_season` 此前被忽略，现对齐 bot——有机肥
    目标限定到本次多季地块
  - 新增 10 个回归用例（对齐 bot `farm-multi-season.test.js`）
- **charity 对齐 cc6a8ab**：active 优先取 List 回包 `activity_windows`；进度档与奖励
  种子 claimable 加 `active &&`；DTO 增 `agreementStatus`；settlement 需要
  「个人达标 && 全服达标」（补齐 globalReached/personalReached）；写操作（领种子/捐赠/
  日礼/进度奖励）删除客户端前置校验直接 Operate，快照改由回包 `reply.data` 构造
  （不再发全量 snapshot 请求）；捐赠数回退走 `charity_donate_result.count`
- **公益结算礼包检查移除**：bot 中的 `openCharitySettlementGiftPacksSilently` 使用了
  当前游戏不存在的固定道具 id `101604`；Rust 不再在 farm tick 中查询或记录该礼包，避免
  产生虚假的“打开公益小红花结算礼包失败”日志。
- **背包分类修复**（cc6a8ab）：分类优先用物品元数据 type（17=mutant 新分支 / 6=fruit /
  5=seed，缺失时回退反查植物表）；排序改 fruit → mutant → seed
- **自动化任务全局互斥**（bot `b487b0f`）：新增 `infra/automation_lock`（按账号 FIFO
  队列 + task_local 重入直执行 + running 查询；bot 每账号一进程，rust 单进程多账号，
  故按账号维度建队列，语义一致）。farm tick / friend tick / daily_routines / 神秘商店
  tick / harvest_sell / 登录期背包初始化 / 邀请码 / 登录期化肥礼包 / 施肥立即生效
  全部包进互斥任务
- **启动序列重排**（bot `runStartupSequence`）：登录期领取（daily_routines(true) →
  任务领取）**串行跑完后**才挂 farm/friend 主循环与周期定时器；此前 rust 先挂
  farm ticks 再 fire-and-forget 日更，存在启动期叠跑
- **QQ 扫码登录（NapCat 对接，bot `4ee6894`+`5ddb70b`）**：core 新 `services/qq_login`
  （4 个 NapCat 接口、X-API-Signature、120s 超时、错误码中文映射、任务归一化）；
  `LoginSettings` 持久化（wechatQrLogin/qqQrLogin/napCatEndpoint/napCatSignature，
  store.json `loginSettings`）；app 门面 + desktop 6 条命令 + `generate_handler!` +
  ACL 白名单；desktop-ui 设置页新增「登录设置」页签（开关 + NapCat 地址/签名），
  账号抽屉新增「QQ 扫码」页签（二维码、1.2s 轮询、取消，confirmed 后换 code 走
  platform=qq 保存并自动启动；按登录设置显示页签）。与既有 QQ 小程序 IDE 扫码
  （`qrlogin.rs`）并存，对应 bot 两套方案并存
- **好友页懒加载**（bot `8da9a5a`）：好友列表/黑名单/互动记录按「已加载账号」标记，
  每账号只拉一次；切账号重置标记；WS 触发的列表刷新不再连带重拉黑名单；
  删除好友后强制重拉黑名单；互动记录手动刷新按钮强制重拉
- 验证：`RUSTFLAGS=-D warnings cargo check --workspace --all-targets` 0 错 0 警；
  `cargo fmt --check` 通过；`cargo test -p qq-farm-core --lib` 969/969；
  `cargo test -p qq-farm-desktop`（含 ACL 防回归）通过；
  `desktop-ui` `vue-tsc --noEmit` 与 `vite build --mode prod` 通过
- 环境注：desktop-ui 的 `crypto-es`（887f0db 引入）本机 node_modules 缺装且 pnpm
  shim 损坏，本次已手动补放 `crypto-es@3.1.3` 后构建通过；下次 `pnpm install` 后无感
- 能力状态：业务能力与 bot `707a47c` 对齐；NapCat 需外部 NapCat 服务才能实际使用；
  SYNC.md 已知缺口 L1–L8 实机回归仍待验

### 2026-09-10 — 修复本机微信「检测不到」（微信本地服务按进程过滤连接）

- 现象：账号抽屉「微信授权」始终提示未检测到本机微信（官方网页快捷登录正常）
- 排查（对照官网 qrconnect 抓包 + 本机实测）：
  1. 微信本地服务（`localhost.weixin.qq.com:14013-14015/13013-13015`）对连接做
     **进程过滤**：Edge/WebView2/node 放行；Rust（reqwest-rustls）、curl(schannel)、
     openssl s_client 的 TLS ClientHello 被静默丢弃（连接后服务端 0 字节回包即断）
  2. 排除了 ALPN / TLS 版本 / 密钥交换组 / ClientHello 大小 / ECH GREASE /
     Origin 头等因素：同一份 ClientHello 直连必死、经 node 转发必活；
     curl 改名 node.exe 仍被拦
- **修复**：detect / authorize 两步改由 WebView 直接 fetch（Chromium 网络栈天然
  被放行，与官方页面同路径）；CORS（服务端 ACAO 锁死 open.weixin.qq.com）通过
  主窗口 `additionalBrowserArgs --disable-web-security` 放行（本地工具应用，
  加载内容全部为本地 dist，风险可控）
  - `desktop-ui`：`fetchWxLocalCheckLogin` / `fetchWxLocalAuthorize` 浏览器直连
    （create session 返回的 OAuth 参数 + 端口列表直接可用）；authorize 的 errcode
    （10050/10046/10057）中文映射；探测失败展示具体原因（此前被吞）
  - confirm 换票仍走后端（yybadaccess 请求不受过滤影响）
  - 后端 `local_wechat` 请求头补 Origin/Referer/Sec-Fetch-*（对齐官方页面，保留
    作为非 Windows / 未来解禁后的直连能力）
- 设置页「登录设置」去掉「微信扫码登录」开关（bot 面板概念，rust 桌面版微信
  登录始终可用；LoginSettings 存储字段保留，保存时不动该值）
- 验证：`vue-tsc --noEmit` / `vite build` 通过；实机待验（本机微信已登录未锁定时
  微信授权页应显示头像昵称并可一键授权）

### 2026-09-10 — 微信快捷登录统一使用 Tauri HTTP 插件

- macOS 本机微信在 `127.0.0.1:14013` 正常监听，检测接口返回 `errcode=0` 和
  `authorize_uuid`；响应的 CORS 来源固定为 `https://open.weixin.qq.com`。
  前次修复的 `additionalBrowserArgs` 仅适用于 Windows，无法放行 WKWebView 跨域请求。
- 前端检测和授权统一使用 `@tauri-apps/plugin-http` 的 fetch，底层由插件的 Rust
  reqwest 发起请求；注册 HTTP 插件，ACL 仅允许六个微信端口的 check-login / authorize。
  配置微信 Origin/Referer、自签证书兼容和禁止重定向；移除 `--disable-web-security`。
- 请求超时覆盖连接和响应体，结束后清理定时器；插件字符串错误转换为 Error，保留
  诊断信息。换票和账号保存沿用原流程。
- 新增前端 IPC 回归测试（检测、授权、HTTP 错误、字符串错误、超时取消），以及端口
  权限检查和默认忽略的本机微信连通性测试。后者只检测状态，不确认授权或保存账号。
- 验证：前端 5 项回归测试、类型检查和生产构建通过；Rust 工作区 989 项单元测试
  通过；新增端口权限检查及 macOS 本机微信实测均通过（插件所用 reqwest 客户端收到
  有效授权标识）。`cargo fmt --all --check` / `git diff --check` 通过。
- 平台限制：插件底层仍是原生进程；此前 Windows 环境记录的进程过滤需在该环境
  重新验证，不能由 macOS 连通结果推断 Windows 已解决。

### 2026-09-10 — 增量同步 bot 707a47c..3bb11e2（萌宠成长日记 / 协议 1.14.0.1_20260909 / TSDK QQ 宿主）

- 对照基准：bot `3bb11e2`（2026-09-10，core `20260910`，PR #71 合入）。此前基准
  `707a47c`（同日早间记录）。
- **萌宠成长日记**（全新活动，对齐 bot `bb2f78a` 主适配 + `03c2bef` 护送/锦囊收敛）：
  - proto：新增 `proto/pet-diary.proto`（478 行整份镜像）；`activitypb.proto` 补
    `import "pet-diary.proto";` 与 `ActivityData.pet_treasure_hunt = 115`
  - core：`services/activity_center/pet.rs` —— `GetGroup(2026090100)` 读取分组；
    快照 = 分组 + 拾物小铺目录（op=7）+ 背包余额 + 节令（仅保留与活动窗口重叠项），
    各源容错收集 `warnings`，独立单飞（对齐 bot `pendingRead`，不进活动中心总快照）
  - 写操作 `PetDiaryOperateRequest` 15 个动作（领养/投喂/寻宝/手记领取/锦囊刷新/
    锦囊装备/夺宝/开宝藏/夺宝补偿/领永久比熊/手记已播标记/跳过夺宝动画/种子一键领/
    小铺兑换），门控 1:1 对齐 bot：付费刷新须 `payment=tickets` +
    `expectedPaidRefreshCount` 与服务端计数一致 + 发送前重读点券余额（防官方客户端
    「点券不足自动落钻石」，`allowDiamonds` 一律拒绝）；夺宝前置 op47 好友宝藏
    status==2 且 preview.canStart + 挑战书白名单 80101-03 + 挑战书余额；小铺兑换
    目录重查 + 限购合并校验；任何钻石成本（道具 1004 / id 0 / 负数 /
    `diamond_cost_count>0`）一律拒绝兑换
  - 节令小礼：`SolarTermsService.ClaimSolarTerms`，仅允许与活动窗口重叠且
    `canClaim` 的节令；回包校验 `term_id` + `status==3`
  - 记录读取：互动日志 op=31 / 被夺日志 op=44；好友活动信息 op=47（回包 gid 校验）
  - 数值配置镜像 `assets/activity-data/pet-diary-2026090101.json`（投喂/寻宝消耗
    1028:700、成年阈值 7000、日限投喂 16 / 寻宝 10 / 夺宝 20、锦囊 101-105、刷新
    免费 1 次/日 + 30 点券 ≤3 次/日）与素材映射 `pet-diary-assets.json`；132 张官方
    素材镜像入 `desktop-ui/public/activity-assets/pet-diary/`
  - 目录注册：gameplay `pet` priority 5（活动组 `2026090100-03`），桌面面板入口
    「萌宠日记」；纯面板驱动、无自动化任务（对齐 bot）
  - 宠物表：`pets.rs` 补 90031 比熊（忠心护主 50% + 技能 3001 比熊润田：看护状态
    作物概率变异售价×4；获得方式=萌宠日记培育至成年）
- **协议版本**：`1.13.3.17_20260826` → `1.14.0.1_20260909`
  （`DEFAULT_CLIENT_VERSION_UPDATED_AT` → `1789004223123`，活动协议前提）
- **TSDK 升级**（对齐 bot `b0a4405`）：wasm `v3.9.0.1787640848` →
  `v3.9.0.1788165223`（SHA-256 `a95b1781…b5f99f`）；宿主初始化按账号平台选择：
  QQ（App ID `1112386029`、设备文本 `windows;windows;windows 10.0;0;`、用户目录
  `qqfile://usr/`、debugMode 0），微信维持原宿主；QQ 宿主特征状态归一（数据段
  17288/17352 各 64B，仅 index 1 差 1 时归一为官方值，每次发送 init token 前执行）
- **桌面端**：`activity_get_pet_diary` / `activity_operate_pet_diary` /
  `activity_get_pet_diary_records` / `activity_get_pet_diary_friend` 4 条 IPC（ACL
  两处注册，防回归测试通过）；`pet-diary-view.vue` 紧凑功能视图——养成/寻宝/宝藏/
  锦囊（含付费刷新确认）/夺宝（好友查询→选宝藏→挑战书→战斗结果）/种子日历/小铺/
  手记/节令/双日志全操作可用；不做护送横幅动画等纯视觉复刻（用户决策 2026-09-10，
  符合「业务目标一致」验收口径）
- bot `03c2bef` 回退的 network `sendTail` 发包串行化不在 rust 范围（rust 网关本就
  账号内串行）；bot web 面板视觉件（PetEscortLandscape 等）不移植
- 验证：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets` 0 错 0 警；
  `cargo test --workspace` 全过（core 977 项含 pet 新增 8 项常量/门控/normalize
  测试、desktop 5 项含 ACL 防回归）；`cargo fmt --all --check` 通过；新 tsdk.wasm
  加密/解密往返实测通过；`pnpm typecheck` / `pnpm build` 通过
- 能力状态：矩阵活动中心行补「萌宠成长日记」；实机待验：萌宠活动全链路（活动窗口
  内领取/投喂/寻宝/夺宝/兑换）、QQ 平台账号 TSDK QQ 宿主（并入 L1/L5 待验）

### 2026-09-10 — 热修：萌宠页道具图路径 + 雨落成诗目录状态（实机发现）

- **萌宠页图片全挂**（pet-diary-view 首版笔误连锁）：道具图 `item.image` 是后端
  `/game-config/seed_images_named/...` 相对路径，桌面端必须经 `resolveCatalogImage`
  转成 `farmcfg://localhost/...` 才能在 webview 加载；首版误传 `item.id` 进该函数，
  修类型错误时又简化成直接返回 `item.image` → 萌宠页全部道具/奖励图 404。已改回
  `resolveCatalogImage(item.image)`，其余页面不受影响（它们的转换调用一直正确）
- **雨落成诗目录状态**：目录对齐 bot registry 补 weather 绑定（静态 ID
  `2026070300-05`，priority 80），目录条目继承 List 窗口真实起止时间，活动结束后
  显示「已结束」；前端固定兜底入口同步改用 `weather_snapshot` 的真实窗口时间，
  不再无窗口时恒显「进行中」
- 验证：`cargo test -p qq-farm-core --lib` 977 项全过；`pnpm typecheck` /
  `pnpm build` 通过；构建产物确认 activity chunk 已引用 shared 的 farmcfg 转换，
  dist 内 `/activity-assets/pet-diary/` 素材可访问（1430 服务实测 image/png 200）；
  dev 实机登录链路正常
- **补记（同日二轮热修）**：上条热修只修了萌宠页自身的参数错误；实机复查发现
  「所有图片（含游戏配置页）」在 dev 实例下仍挂——真正全局根因是 tauri-cli dev
  的页面 origin 为 `http://127.0.0.1:1430`，Windows WebView2 下 `farmcfg://`
  形式的自定义协议资源无法从该 origin 加载（安装版 origin `http://tauri.localhost`
  不受影响，因此线上一直正常）。修复：`resolveCatalogImage` Windows 分支统一改用
  WebView2 映射形式 `http://farmcfg.localhost/<rel>`（后端 assets.rs 本就支持该
  host 形式并有单测），macOS 保持 `farmcfg://localhost/`。dev 实机验证图片恢复。
