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
| **qq-farm-rust** | `main`（本提交：bot 10-12 土地详情/升级预测/图鉴购种/预留修复/TSDK 增量） | 2026-10-15 | QQ TSDK wasm 升级 `v3.9.0.1790414929` + 土地加成与升级/解锁详情（landInfo）+ 等级升级时间预测（levelForecast + sessionElapsedSeconds）+ 图鉴 shopSeed 与种子直购（seeds_purchase）+ 多格种子预留 #89 修复（登记表 + 互斥 + 更满优先）；Tauri 与 GPUI 双端同步（实机待验） |
| **qq-farm-bot** | `8303e7d` | 2026-10-12 | QQ TSDK 构建对齐（9b19af9）+ 土地详情（e5f3552）+ 升级预测（62e1880）+ 图鉴种子购买（0f09702）+ 多格预留 #89 修复（1a5c23e） |

文档范围：**业务行为 + 面板 HTTP/Socket 契约**（不含改 Vue 面板本身）。

### 版本语义（避免混用）

| 概念 | 当前值 | 用途 |
|------|--------|------|
| 客户端版本 `FARM_CLIENT_VERSION` | 默认 `1.14.2.15_20260922`（与 bot `config.ts` 默认一致，UPDATED_AT `1790568000000`；QQ/微信统一） | 进游戏网关声明 |
| bot `core` 包版本号 | `20260928` | 原项目发布标签，≠ 客户端版本字符串 |
| TSDK wasm（QQ） | `v3.9.0.1790414929`（161111 字节，SHA-256 `f80c5f34f4e7dc55…`；加载时校验；bot 9b19af9 对齐已核验构建，协议版本仍为 1.14.2.15_20260922） | QQ 账号网关加密 + ACE |
| TSDK wasm（微信） | `v3.9.0.1790237209`（160992 字节，SHA-256 `4bf6aa0ede9677fe…`；加载时校验） | 微信账号网关加密 + ACE（bot 864caf3 新增，`assets/tsdk-wx.wasm`） |
| 青梅活动 ID | 每日 `2026081201` / 酿造 `2026081202` | 活动协议 |
| 公益小红花活动 ID | 活动组 `2026090900` / 活动 `2026090901` | 活动协议 |
| 萌宠成长日记活动 ID | 活动组 `2026090100` / 养成 `2026090101` / 种子赠礼 `2026090102` / 拾物小铺 `2026090103` | 活动协议 |
| 秋日活动 ID | 秋祈良愿 组 `2026092400` / 活动 `2026092401`；快乐不独享 组 `2026092500` / 活动 `2026092501` | 活动协议 |

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
| 连网进游戏 | 齐 | 经 Gateway + TSDK 登录并维持心跳；登录/心跳包字节级对齐官方抓包（QQ 73/27 字节向量）；微信登录分支对齐 bot（device_info 扩展 network/device_id/memory、scene_id 省略、channel `other`）；会话结束输出 connection_summary 汇总诊断（心跳计数/pending/queued/ACE·TSDK 诊断/断开原因），踢下线日志带 reasonCode | `network/*`, `crypto/tsdk.rs` |
| QQ 小程序扫码拿码 | 齐 | 面板可走 QQ 码登录流程 | `services/qrlogin.rs` |
| 微信扫码拿码并启动 | 齐 | 扫码 → 应用宝 login_buffer 落盘 → 换一次性网关 code 启动；掉线/重启可用授权再换码重连；TSDK 按平台选 wasm（QQ `tsdk.wasm` / 微信 `tsdk-wx.wasm`，各自 SHA-256 校验，bot 864caf3 双构建） | `services/wx_login/*`, `routes/wx_login.rs` |
| 本田务农循环 | 齐 | 除草除虫浇水 → 收获 → 铲除 → 种植（含多格 + `bagSeedMultiLandReservationEnabled` 优先多格种子预留空地，默认关；bot #89：按种子登记目标布局、各组整组互斥、优先已预留格数更多的布局，种子可种即清登记）→ 施肥/解锁升级；默认策略/skip_own_weed_bug/smart 秒数对齐 bot | `services/farm/*`, `runtime/worker_loop.rs` |
| 土地详情 / 等级预测 / 图鉴购种 | 齐 | 土地面板 DTO 携带 `landInfo`（当前/下一级加成对比、解锁/升级条件与成本；服务器条件优先、官方 LandLevel(Need).json 回退，bot e5f3552）；状态快照携带 `levelForecast`（按当前作物排期推算升级时间，不足时用会话速率在最后一笔收获后外推；土地快照随 PlantService 写回包/ItemService.Use/土地推送/断线失效，bot 62e1880）与 `sessionElapsedSeconds`；图鉴快照作物条目挂 `shopSeed`（服务器商店失败回落本地目录 `unknownMeta`），`seeds_purchase` 直购（参数校验/解锁卡/限购/金币门控与文案逐条照抄 bot 0f09702） | `farm/land_upgrade.rs`, `services/level_forecast.rs`, `farm/planting.rs`, `services/illustrated.rs` |
| 宠物（护主犬） | 齐 | 上场/收回/喂粮/守护记录 + `ActivateDog` 消耗背包卡片激活图鉴项；快照含 `activatable` 三态（field_6=1 或背包有未锁定同 ID 卡） | `services/pets.rs` |
| 好友帮助 / 偷菜 / 捣乱 | 齐 | 列表、访问、帮助（经验门控）、偷菜（气泡+自巡/空访/一键 Harvest 主地回退/先偷后帮）、静默仅挡好友、黑名单落盘；捣乱按日限/启动筛选/`1001046` 停 | `services/friend/*` |
| 背包展示与操作 | 齐 | 按 UID 堆分行；含 `key`/`uid`/`mutantTypes`/`groupKey`；系统物品分离 | `services/warehouse.rs` |
| 自动/手动出售果实 | 齐 | 自动受 `sell` 开关；`sell_cond` 满足后用 `cond_sells`（活动结束后 / 道具过期后等）；手动预检拒绝不可售 | `warehouse` + `game_config` + `activity_windows` |
| 商城 / 神秘商店 / 月卡 / 钻石 | 齐 | 列表（slot 1 普通 + slot 4 SVIP，非会员禁购 + expectedPrice 防涨价）、购买（回包 success 校验 + 不限购/售罄/广告/分享状态机）、神秘商人并入商城 tab、月卡、充值信息 | `mall`, `mystery_shop`, `monthcard`, `pay`, `commerce` |
| 日常领取 | 齐 | 任务（成长 claim 后刷新 TaskInfo + `currentTask`）、邮件、分享等 | `task`, `email`, `share`, … |
| 活动中心 | 齐 | 千星游记、观星、星砂、节令、青梅（含已领幂等）、鹊桥寄情（筑桥领取 + 赠香囊）、公益小红花（领种子/捐爱心/每日礼包）、萌宠成长日记（养成/寻宝/锦囊/夺宝/种子/小铺/节令/记录）、秋日活动（秋祈良愿抽签/领奖 + 快乐不独享分享/每日/档位/日志） | `activity_center*` |
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

### 2026-09-11 — 热修：普通+有机模式下有机肥永远 0（实机发现；proto 保持 bot 镜像不动）

- **现象**（安装版日志实证）：账号选「普通+有机」后每轮只施 1 块地（普通肥
  1/新种地块），有机肥恒为 0；同日 smart 模式 `巡田施肥完成 有机3/7` 正常，
  02:57 批量种 24 块地 fertilize 245→269（普通 24/24 全成功）排除普通肥链路
- **根因**：`get_organic_fertilizer_targets_from_lands` / `get_fast_mature_lands`
  按 `left_inorc_fert_times <= 0` 排除地块。该字段是植物剩余普通肥额度，
  proto3 零值省略：普通肥一施额度即归 0、服务端停止下发 → prost 解码为 0 →
  Rust 把这些地全部踢出有机肥目标 → both/organic 模式有机肥目标被清空。
  bot 用 `Object.hasOwn`（未下发=仍可施），不受影响
- **官方向量实证**（bot `core/tests/farm-fertilize-proto.test.js` 两只真实抓包，
  protobufjs 复刻解码验证）：有机肥成功回包中 land 的该字段**下发**且=1（施
  有机肥不耗普通额度）；普通肥成功回包中该字段**不下发**（施普通肥把额度
  归 0 后省略）。即服务端**从不显式发 0**，bot 的 hasOwn≤0 分支在真实报文上
  不可达，bot 实际行为 = 所有活着的种植地块均为有机肥目标
- **修复（纯代码侧，proto 保持与 bot 逐字符镜像不动）**：两个函数移除该字段
  过滤，与 bot 实际行为 wire 等价；单测改为「额度 0/额度>0 地块均入选」
- **顺手补齐两处 bot 对齐缺口**：
  - `fertilize_organic_loop` 补单次上限 `min(240, 地块数×20)` 与上限
    面板告警（bot api.ts `MAX_ORGANIC_FERTILIZE_OPERATIONS/ROUNDS`，原实现
    无上限存在长循环风险）
  - `fertilize_by_config_ex` 补齐 bot `runFertilizerByConfig` 的全部面板日志
    （未勾选范围/策略为不施肥/无可施肥地块/拉地失败跳过/普通目标为空/
    已为 X/N 块施普通化肥/0-N 可能化肥不足/有机循环完成共施 X 次/smart 变体），
    此前 rust 完全静默，正是本次问题难以从面板定位的直接原因
- 明确不改：拉地失败 fail-closed（既有决策），仅新增 warn 面板日志提升可见性
- 验证：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets` 0 错
  0 警；`cargo test --workspace -- --test-threads=1` 全过（core 977 项）；
  `cargo fmt --all --check` 通过；`proto/plantpb.proto` 与 bot 文件 diff 一致
  （仅行尾符）；纯 core 侧改动，无 IPC/前端变化
- 能力状态：施肥矩阵行维持「齐」；**实机待验**：both 模式种植后有机肥应出现
  「有机化肥循环施肥完成，共施 X 次」且 X>0，多季补肥同理

### 2026-09-11 — 热修：登录互搏/请求超时集体掉线（安装版日志实证，七项根因）

- **现象**（`AppData\Local\QQFarmRust\logs` 实证）：改名/备注/编辑扫码后同账号
  「已在其他终端登录」互踢；手动登录后点任意菜单请求超时、随后心跳超时掉线；
  09-11 10:17–10:29 三账号集体入站冻结掉线，10:33:39/10:34:06 应用被启动两次
  （两条「发现 N 个已授权微信账号」setup 日志），第二实例错峰重登全部账号
  形成跨进程互踢；09-10 全天为同一循环
- **根因**（全部读码核实）：
  1. 无单实例保护，双击两次 exe = 两进程各自自动登录全部账号互相顶号
  2. `upsert_account` 按"字段是否提供"判断重启，而前端修改时总是回传
     code+platform → 纯改名/备注也触发 `restart_worker`
  3. `restart_worker` 取消即启动：旧 WS 未关就换码重登，被服务端判互踢
     （timing.rs 对被踢重登等 3 分钟的注释早有此教训，restart 无视了它）
  4. worker 启动前奏（TSDK/换码/连接/登录）不检查取消令牌，已停 worker
     继续登录并踢掉新 worker
  5. 注册表按 account_id 无条件摘除：restart 后旧 worker 迟到的退出把新
     worker 注册一并抹掉，has_worker/启动守卫全盲 → 可再 spawn 第二会话
  6. 重连是 detached sleep 任务不可撤销，手动启停后幽灵重连仍按时拉起第二登录
  7. 生命周期 IPC 全是同步命令跑在主线程（日志 `main` 线程「启动 worker」）
- **修复**：desktop 注册 `tauri-plugin-single-instance`（第二实例仅聚焦窗口）；
  `upsert_account` 改语义化 diff（仅 code/platform/qq/uin/avatar 实际变化才
  重启，纯改名只同步运行态显示名）；`restart_worker` 改 async——per-account
  lifecycle 锁串行化 + 等待旧任务退出（10s 上限强杀）+ `WX_RESTART_GRACE_MS`
  宽限后启动；前奏四段 await 全部加取消检查点（`or_cancel`）；WorkerHandle/
  Stopped 事件携带 generation，`release_worker_gen` 与事件桥按世代丢弃过期
  退出（新 worker 注册不再被抹）；重连任务保留 AbortHandle，stop/start/restart
  时 abort，触发时自摘句柄并取 lifecycle 锁；`start/stop/upsert/delete_account`
  IPC 改 async（off 主线程）；前端启动/停止/提交按钮防连点
- 明确不改：被踢 3 分钟/心跳超时 15 分钟重连节奏、`remark_relogin` 语义、
  扫码编辑后前端兜底 `start_account`（现幂等）
- 验证：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets` 0 错
  0 警；`cargo test --workspace` 全过（core 977 项）；`cargo fmt --all --check`
  通过；`corepack pnpm typecheck`+`build` 通过
- **实机待验**（新构建替换安装版后）：任务管理器仅一个 qq-farm-desktop.exe；
  在线账号改名/备注 → 日志无「启动 worker」且不掉线；编辑扫码 → 单次重启
  无互踢；手动登录后各菜单请求正常；双击 exe 两次第二实例仅聚焦窗口。
  另：如再出现无进程内第二登录的「已在其他终端登录」，需查手机端/其他终端
  （游戏本体顶号机制，非本程序问题）

### 2026-09-11 — 二轮排查：重启撞 session 释放窗口修复 + 两类残留问题定位

- **实机复测**（新构建装上后）：14:06:46 旧实例仍在健康务农 → 用户杀进程换新
  构建 → 新实例 13~95 秒内把三账号全部重登 → 全部登录后 ~1 秒入站冻结秒死；
  15 分钟后的自动重连（间隔已过释放窗口）全部健康。证实：**重启后自动登录
  撞服务端旧 session 释放窗口**（同 WX_KICKOUT_RECONNECT_DELAY_MS 的教训，
  但启动路径此前不设防）
- **修复**：新增 `infra/session_liveness.rs` 持久化每账号最后确认在线时刻
  （事件桥 Status(connected) 打点 30s 节流、Stopped 清除）；
  `schedule_wx_authorized_start` 批量启动前按 `boot_delay_ms` 等过 3 分钟
  释放窗口（冷启动不受影响），等待时面板提示「旧会话释放中」
- **可观测性**（定位两类残留问题）：
  - 大号/小小号每次登录 11~14 分钟后被分钟级联饿死（14:37:57/14:39 先后
    冻结）而小号会话一直健康（UI 可用、心跳应答）——外部终端定时抢会话特征，
    待用户排查手机端；ACE AntiData 今日回包恒为空（昨日健康时段 1081 字节）
  - 小号"在线但自动化零动作"：巡查间隔仅 20~25s（秒级），却从登录后第一轮
    tick 起 36 分钟零动作、零超时（会话本身健康，UI 可用）——自动化互斥
    permit 被挂死任务永久持有，且挂死的是独立 tick task，worker 重连/停止
    都无法释放（全局按账号信号量跨 worker 存续）。**修复**：
    `run_exclusive_automation_task` 执行超 10 分钟即取消 future 自愈
    （释放 permit 与锁守卫，排队任务恢复，单轮作废下轮重来），等待 2 分钟
    点名持有者告警；心跳任务三处静默跳过门（phase/gid/在途 Heartbeat）
    升级为 WARN——此前零日志正是"在线但无监控盲区"无法被发现的原因
- 验证：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets` 0 错
  0 警；`cargo test -p qq-farm-core` 978 项全过；`cargo fmt --all --check` 通过
- **实机待验**：重启 app 后日志应出现「旧会话释放中，延迟 N 秒」且首批登录
  不再秒死；小号若再出现零动作，日志会点名卡死的持有者任务

### 2026-09-11 — 三轮排查：找到慢性掉线根因——TSDK 反作弊宿主接口三处未对齐

- **决定性对照**：同机同号，bot（TS）稳定不掉线，rust 慢性掉线（历史日志
  实证 8/23 起每天 13~57 次心跳超时，8/21-22 健康）→ 排除风控/外部终端，
  病灶在 rust 客户端自身协议行为；症状（登录成功 → 0.5~14min 后服务端
  静默断供、TCP 保持）= 服务端 ACE 反作弊隔离区典型表现
- **Explore 逐项 diff**（bot tsdk-runtime.ts vs rust crypto/tsdk.rs）锁定三处：
  1. **init 跳过 `decrypt_all_data()`**：rust 用宿主静态 17 段表手动解密，
     bot 解密 404B 元数据段后调 wasm 自己的 `decrypt_all_data()`（内部
     持有权威段表，且可能做运行态初始化/校验和）→ AntiData 内容与真机
     不一致
  2. **import `c`（captureStackTrace）返回空字符串**：bot 返回真实 Node
     `Error.stack`（"Error: TSDK JavaScript 调用栈
 at ..."），wasm 把它编入
     AntiData 每 5s 上报——空栈 = 非真实 JS 运行时特征
  3. **import `q`（serverTime）不做 ACE 后台校时**：bot GET
     `api.anticheatexpert.com/test` 用 HTTP Date 头校正 wasm 时间槽
- **修复**（全部 1:1 对齐 bot）：init 改为「元数据段 + `decrypt_all_data()`」
  序列（MERGED_DATA_METADATA=(67371008,404)，删除静态 17 段表）；`c` 返回
  Node 风格栈串（容量不足返 0、成功返 byteLength+1，语义同 bot）；`q` 进程
  级偏移实现 ACE 校时（首次触发异步拉取 Date 头，失败静默保持本地钟）；
  顺手对齐 `resolve_data_path` 剥离 `qqfile://usr/` 前缀
- 排除项：TSDK 中途重建链（`WasmReset`/`pending_reset`）——历史日志 8/24
  起 0 次触发，非本因；心跳策略/参数与 bot 全一致（逐项核对无差异）
- 验证：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets` 0 错
  0 警；`cargo test -p qq-farm-core` 979 项全过（含 TSDK 加解密官方向量，
  新 init 序列不破坏密码学）
- **实机验证中**：dev 实例 15:51 起新构建运行，观察三账号能否稳定越过
  历史 1s/11-14min 两个死亡区间
- **四轮补修（帧序）**：15:51 构建实测小号活 15min/小小号活 11min 后仍被
  饿死 → 锁定 Explore diff 的最后一项：**发送帧乱序**。bot 是单线程 drain
  队列，帧严格按 client_seq 递增；rust 的「seq 分配 → 加密 → 入发送通道」
  三步在并发下可交错（通道满载时 `send().await` 必然让出调度），乱序帧或
  加密序≠seq 序都会被服务端丢弃，反复即触发静默断供——登录爆发期并发最高
  最易撞出乱序，与「落地即死/越忙越易死」现象吻合。修复：`Inner` 新增
  `send_order: tokio::sync::Mutex`，`send_rpc` 与 `send_no_reply` 的
  「seq→加密→入队」全部原子化（等回包在锁外），1:1 复刻 bot 单写者语义。
  此修复与 TSDK 三项对齐同批生效待验
- **实机验证结果**（dev 实例 16:15 构建起）：三账号连续在线 27+ 分钟
  **零心跳告警、零掉线、零被踢**，巡查/偷菜/出售全程正常——今天此前
  每一轮（13:26 安装版、15:11 dev、15:51 dev+TSDK 对齐）都在 15 分钟内
  死亡。逐构建对照：15:11（无 TSDK/帧序修复）落地即死；15:51（+TSDK
  对齐）存活 4.5~15 分钟仍死；16:15（+发送顺序锁）27 分钟+ 零告警。
  两个修复孰为决定性无法单独隔离（均 1:1 对齐 bot，都应保留）；更长期
  稳定性（bot 级小时/天）随日常使用继续观察

### 2026-09-11 — 实机反馈三连修：寻宝好友下拉、宠物确认进度口径、移除 NapCat 登录设置

- **萌宠好友夺宝改下拉搜索**（pet-diary-view.vue）：裸 GID 输入框替换为
  好友 NSelect（filterable + tag，`昵称（GID）` 选项，数据走 friend_list
  size=500，对齐七夕/天气页的好友选择交互范式；tag 模式保留手输 GID
  逃生口）。同病复发第二次，此后新页涉及选好友一律用此范式
- **宠物确认进度永远不满**（friends/index.vue）：实测大号 48/67 卡住，
  数据文件证实 19 个未确认全是黑名单/失效 GID 好友——每日宠物同步
  （pet_sync collect_pending_friends）会排除它们，但前端分母用了全部
  好友，口径不一致永远到不了满。修复：分子分母同口径（排除黑名单，
  petSyncScope）。顺带证实机制本身健康：小小号当日全量同步完成
  （last_full_sync_date=2026-09-11，4/4）；小号 0 条是当日自动化卡死的
  后果（同日已修）。`dog_id=0`（好友没上场狗）本就计入 other，非问题
- **移除登录设置（NapCat）页签**（settings/index.vue）：QQ 扫码 NapCat
  方式不好用，删整个页签及相关状态/加载器；后端 store 键与 IPC 命令
  保留——账号抽屉的「QQ 扫码」页签仍按既有 qqQrLogin 开关决定展示
  （默认关即隐藏），零后端风险
- 验证：`corepack pnpm typecheck`+`build` 通过；纯前端改动，Rust 门槛
  不受影响
- **实机待验**：萌宠页好友夺宝下拉可选可搜；好友页宠物进度数值与
  非黑名单好友数对齐且当日能到满；设置页无「登录设置」页签

### 2026-09-11 — 化肥自动购买改事件驱动 + 化肥桶推送刷新（实机需求）

- **需求**：自动购买化肥原为纯定时检查（20/60 分钟一轮），容器在两次
  检查之间耗尽时施肥一直空转到下个周期；且仪表盘化肥桶只靠 30s 轮询/
  手动刷新，施肥后数字不动
- **数据源确认**：化肥容器是背包道具，`count` 字段即剩余秒数
  （warehouse.rs `get_container_hours_from_bag_items`，/3600 得小时），
  一次普通 get_bag 即可读取——施肥前后任意时刻可见
- **后端**（worker_loop）：新增 `maybe_event_fertilizer_buy`（账号级
  3 分钟节流）挂在 farm_tick 尾部与「保存后立即施肥」任务尾部——施肥轮
  结束即检测余量、低于阈值当场购买，下一轮施肥就能用上；购买结果
  （有机 xN/普通 xN + 购后余量）进面板日志；原定时器保留兜底
- **前端**（dashboard）：WS 日志事件 tag 为「施肥/商城」或消息含「化肥」
  时 2s 防抖拉一次背包——化肥桶在施肥/补货后自动更新，不再手动刷；
  卸载时清理定时器
- **补强（同日）**：`fertilize()` 此前把 `FertilizeReply` 整个丢弃——proto 已注明
  `fertilizer`（corepb.Item）的 count 即容器剩余秒数（与 ItemNotify 交叉验证）。
  现在解析回包返回 `Option<剩余秒>`；有机循环与普通分支把余量写进面板日志
  （「…共施 N 次，剩 X.Xh」「已为 X/N 块地施普通化肥…，剩 X.Xh」），
  余量零额外 RPC；事件补购节流降为 60s（余量归零后最多一分钟内补上）
- 验证：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`
  0 错 0 警；`cargo test -p qq-farm-core` 979 全过（scheduler 时序用例
  高负载下偶发，单跑通过）；`corepack pnpm typecheck`+`build` 通过
- **实机待验**：小号有机肥耗尽场景——施肥日志后几秒内出现「已自动补充
  化肥」面板日志，化肥桶数字随之跳动

### 2026-09-11 — 实机反馈三连优化：化肥去间隔 UI、神秘商人标签、萌宠可夺下拉

- **化肥购买去「检测间隔」**（settings）：有机/无机已统一为事件驱动补购
  （施肥后即检 + 定时兜底），设置页再暴露「检测间隔」易被当成无机专属；
  UI 去掉该字段，hint 改为说明事件驱动；后端仍读既有
  `fertilizer_buy_check_interval_minutes` 作兜底
- **神秘商人标签**：上一提交已补 `mystery_shop_watch → 神秘商人` 与防回归
  测试；需前端 rebuild 后生效
- **萌宠夺宝下拉只列可夺好友**（pet-diary-view）：拉好友后限并发探测
  op47，仅保留「护送中 + canStart」好友；排除黑名单；「刷新可夺」
  按钮；查询结果同样只展示可开战宝藏；tag 手输 GID 逃生口保留
- 验证：`corepack pnpm typecheck`+`build` 通过；
  `cargo test -p qq-farm-core every_panel_event_has_frontend_label` 通过
- **实机待验**：设置页无机块无检测间隔；日志 chip 显示「神秘商人」；
  萌宠下拉仅为可夺好友

### 2026-09-11 — 增量同步 bot 3bb11e2..305c300（协议 1.14.0.3_20260909 / 分析页 / 萌宠素材）

- 对照基准：bot `305c300`（2026-09-11，core `20260911`，PR #72 合入）。此前基准
  `3bb11e2`（2026-09-10）。
- **协议版本**：`1.14.0.1_20260909` → `1.14.0.3_20260909`
  （`DEFAULT_CLIENT_VERSION_UPDATED_AT` → `1789111371648`）
- **分析页**（对齐 bot `b6e381b`）：排行与自动选种排除白萝卜种子 `29999`；
  参考等级上限 `200`
- **萌宠日记**（对齐 bot `6cd1e4e` 的 core 契约，不移植奶油纸/SVG 自绘）：
  - 手记 DTO 去掉 `captionImage` / `caption`，只保留 `order / unlocked / claimed /
    animated / photo`
  - 素材映射 `pet-diary-assets.json` 132 → 27 项，官方图改为 WebP；桌面
    `activity-assets/pet-diary/` 同步收敛（手记照片 / 锦囊 / 小铺商品）
  - 手记卡改为「第 N 则 + 原图比例照片 + 领取/未解锁」
- 不移植：拾物小铺关闭按钮 CSS（`e49b7b9`，bot web 专属）；PetMascot /
  PetEscortLandscape / ActivityShell 等纯视觉件
- 验证：`cargo test -p qq-farm-core --lib analytics` 8/8（含排除 29999）；
  `pet` 25/25；`system_config` 12/12；`cargo test -p qq-farm-core --lib` **978/978**；
  `cargo fmt --all --check` 通过；`pnpm typecheck` 通过
- 能力状态：不变；L1–L8 仍待实机勾选

### 2026-09-11 — 去掉手动施肥显示开关；个人页操作不再二次确认

- **设置页**：去掉「显示手动施肥按钮」。桌面版个人农场从未接土地卡手动施肥，
  该开关只写配置、无界面效果。后端 `show_manual_fertilizer` 字段仍保留
  （bot 对齐 / 已存账号配置不破），只是面板不再读写
- **个人页**：农场一键收获/务农/种植/升级/全收，以及背包出售/使用/批量出售，
  去掉 `NPopconfirm`，点按钮即执行
- 验证：`corepack pnpm typecheck` 通过；纯前端改动
- 实机待验：自动控制页无该开关；个人农场/背包操作一击即发

### 2026-09-11 — 新账号默认对齐本机账号 1 的自动化/策略

- 来源：安装版 `store.json` 账号 `1`。自动控制开关原本已与默认一致
  （帮忙/捣乱/经验满关、填充化肥开、智能施肥 360s）
- **策略默认**按账号 1 改：种植 `bag_priority`、兜底 `preferred`、背包优先
  `29003,20129,21380,20108,26032`；静默 01:00–08:30 开启且继续巡田；
  偷菜间隔 60–90 秒
- 仅影响新账号初始值，已存账号不改写
- 验证：`cargo test -p qq-farm-core --lib default_` 30/30；
  `ensure_creates_default` / `get_snapshot_no_id_returns_fallback` /
  `bag_seed_priority_default` / `account_config_default_serde` 通过；
  `corepack pnpm typecheck` 通过
- 实机待验：新建账号开局即背包优先 + 上述间隔/静默

### 2026-09-11 — Release 去掉 Quality checks 门槛

- 发版流水线原先先跑一整轮 Ubuntu fmt/oxfmt/eslint/clippy/test，通过才开始
  打三端包。上次 v0.3.3 被 oxfmt 空格拦住，包没开始编，空耗一轮 runner
- 这些检查本地 skill 门槛已有；打 tag 的目的是出安装包。编包 job 仍带
  `RUSTFLAGS=-D warnings`，编译错误照样会失败
- 改：删除 `quality` job 与 `needs: quality`，push `v*` 直接并行打
  Apple Silicon / Intel / Windows

### 2026-09-11 — 化肥购买去掉周期定时器

- 事件驱动补购（施肥后 / 保存后 / 登录后各检一次）已上线，设置页也已去掉
  「检测间隔」；但 worker 仍按 `fertilizer_buy_check_interval_minutes` 挂
  `fertilizer_buy_check` 周期任务，登录还会打「定时器已启动」
- **改**：删除 `start_fertilizer_buy_timer`；登录与保存配置时 `clear` 掉旧
  任务；登录做一次即时检测。购买成功日志改走 `fertilizer_buy`
- 配置字段 `fertilizer_buy_check_interval_minutes` 仍保留读兼容，运行时不读
- 验证：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets` 0 错 0 警；
  `cargo test -p qq-farm-core --lib` 981/981；`cargo fmt --all --check` 通过；
  `corepack pnpm typecheck` 通过
- **实机待验**：开启自动买肥后不再出现「化肥自动购买检测定时器已启动」；
  施肥耗尽后仍会补购

### 2026-09-11 — 「普通+有机」改全场无机一次 + 有机催熟（有意偏离 bot）

- 用户策略：所有还能施无机肥的地各打 1 次；然后所有未成熟作物循环打有机肥
  直到成熟。原 Both 只打刚种/多季地，且巡田不扫全场
- **选地**：无机 = 未成熟且本季 `ferts_used` 尚无 1011；有机 = 全部未成熟
- **执行**：巡田末尾跑完整 Both（不再 `skip_normal`）；种完/多季补肥不再
  立刻再打，避免同一轮两遍。有机催熟遇单块失败踢掉该地继续，余量 0 停
- 设置页选项改名为「无机一次 + 有机催熟」，并加说明
- 验证：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets` 0 错 0 警；
  `cargo test -p qq-farm-core --lib` 983/983；`cargo fmt --all --check` 通过；
  `corepack pnpm typecheck` 通过
- **实机待验**：Both 下已长着的未成熟地每轮会被无机打到、有机催熟

### 2026-09-11 — 桌面顶栏去掉「更多」

- 「更多」是手机端把搜索/主题/语言/更新收进下拉的入口；桌面这些已是独立图标，
  再留「更多」只剩「关于与更新」一项，看起来像误带了移动端控件
- **改**：桌面隐藏「更多」，改为独立「关于与更新」图标；手机端下拉不变
- 验证：`corepack pnpm typecheck` 通过
- **实机待验**：宽屏顶栏无「更多」，点信息图标仍能打开关于与更新

### 2026-09-11 — 萌宠好友夺宝去掉全量探测（大号掉线根因修复）

- **现象**：大号打开萌宠「好友夺宝」下拉后全部请求超时、账号掉线。根因是
  `pet-diary-view.vue` 在下拉加载时对全量好友（size=500 去黑名单后）以 5
  并发逐个发 op=47 `GetFriendActivityInfo` 探测，几百个请求的突发量触发
  服务端（ACE）静默：所有 RPC stage=pending 无回包，心跳连挂直接下线——
  与 bot `docs/friend-pet-cache.md` 记录的「好友宠物同步早期 ~3 RPC/s 即
  掉线」同形态，这次并发更高、死得更快
- **修法**（对齐官方小程序行为：可夺宝好友由气泡标记，客户端不逐个探测）：
  - 删掉探测循环，下拉直接展示好友列表（复用七夕页范式），选项由共享的
    `activityFriends` computed 生成（`昵称（GID）`，filterable + tag 手输
    GID 逃生口保留）；切到萌宠页时懒加载一次，刷新按钮改发
    `refresh-friends` 强拉
  - op=47 只保留「查询好友宝藏」点击路径：选中单个好友后才发一次请求，
    开战动作后端本就有二次校验，不受影响
  - `activity/index.vue`：`qixiFriends/loadQixiFriends` 泛化为
    `activityFriends/loadActivityFriends`，七夕与萌宠共用，切页即加载
- 验证：`corepack pnpm typecheck` 通过；纯前端改动，op=47 现仅剩单好友
  手动查询一条调用链
- 实机待验：大号打开萌宠页好友夺宝下拉不再触发超时/掉线；选好友→查询
  →开战全流程可用

### 2026-09-11 — Mac 拆架构后「检查更新」失败

- **现象**：v0.3.5 `latest.json` 只有 `windows-x86_64`，Intel/Apple Silicon
  客户端查不到 `darwin-*` 平台，检查更新报错
- **根因**：
  1. `bundle.targets` 只有 `nsis`/`dmg`。DMG 不是 updater 目标，Mac job
     打不出 `.app.tar.gz` + `.sig`（CI 日志：`no updater-enabled targets`）
  2. 三个矩阵 job 都开 `uploadUpdaterJson`（`includeUpdaterJson` 已更名，
     旧输入无效，默认仍为 true）。并行写同一份 `latest.json`，最后写的
     Windows 覆盖掉可能存在的 darwin 键
- **修**：targets 加上 `app`；各 job `uploadUpdaterJson: false`；publish
  全部结束后由 `scripts/build-updater-manifest.py` 读三端 `.sig` 写一份
  清单，缺任一平台直接失败；`SHA256SUMS` 等这份 JSON 上传后再算
- 验证：`python3 scripts/build-updater-manifest.py --self-test` 通过
- **现网 v0.3.5 无法热补**（Release 上没有 Mac `.app.tar.gz.sig`），需再打
  一个 tag 才会恢复 Mac 检查更新

### 2026-09-12 — 删除 4 项废弃功能（用户决策的有意删除）

以下 4 项是用户明确决策从 rust 侧删掉的功能，**从 bot 同步时不要误当缺口补回**：

1. **偷菜「仅活动作物」过滤**（`friend_steal_activity_only`）——功能已从产品删除，
   字段/key 映射/偷菜过滤分支与活动植物标记辅助全部移除
2. **农场社交事件（青蛙）清理管线**——活动已过期；`FarmingRequest` 不再携带
   `social_event_item_ids`（field 5），推送分支与 land_analysis 辅助删除。
   注意：天气活动「雨落成诗」的青蛙瓶逻辑是另一个功能，保留未动；proto/ 是
   bot 镜像，字段留在 proto 里，同步时勿删勿补
3. **离线账号自动删除**（`offline_delete_sec`）——功能取消；仅删自动删除定时器
   与设置项，离线提醒推送本体保留
4. **「一键清理已停止账号」按钮**（clearStopped）——功能取消；仅 desktop-ui，
   后端本就无对应批量命令；通用批量删除不再限制「仅已停止」

### 2026-09-15 — 同步 bot 305c300..1d1edb3（协议 1.14.0.4 / TSDK 升级 / Login 逐字节对齐 / 宠物激活 / 多格预留）

基准推进：bot `305c300`（09-11）→ `1d1edb3`（09-14）。基线间 12 个提交中仅 4 组真实代码变更
（其余为 web 前端样式、行尾归一化、纯文档），全部同步如下：

1. **协议版本升级**（bot `f61b227`）：默认 `1.14.0.3_20260909` → `1.14.0.4_20260911`，
   UPDATED_AT `1789352998016`（`config/system_config.rs`）
2. **TSDK wasm 升级**（bot `9709bcb`）：`assets/tsdk.wasm` → v3.9.0.1789137379
   （161,084 字节，SHA256 `1744e339d43425f9f24834fd49b3239f824f57fe76242d5b3128ac55b3110ac5`）；
   `TSDK_VERSION` 更新，并**新增加载前 SHA256 校验**（对齐 bot `TSDK_SHA256`；
   mergewasm key/元数据/QQ 宿主表 bot 侧未变，无需改动）。
   AntiData 载荷 FNV-1a32 校验和由 wasm 内部计算，宿主侧无需实现，换 wasm 即可
3. **Login/Heartbeat 逐字节对齐**（bot `9709bcb`）：新增 `network/login_body.rs` 纯函数
   `build_login_body`（官方 73 字节定长） / `build_heartbeat_body`（官方 27 字节定长），
   以官方向量做 golden 测试钉死。关键点：官方客户端**显式写出** `sharer_id=0` /
   `share_cfg_id=0` / `field_3=0` / 6 个空 `report_data` 字符串 / 空 `extra`，prost proto3
   语义省略默认值字段产不出该字节流，故手写 wire-format 构造；`Gateway::login` 签名改为
   `(client_version, sys_software)`，worker 不再构造 `DeviceInfo`/`ReportData`
   （device_info 只留 client_version/sys_software，删 `screen_width`）
4. **宠物激活 ActivateDog**（bot `9907ffd`）：`proto/dogpb.proto` 经 sync 工具整份镜像
   （新增 `ActivateDogRequest/Reply`、DogInfo `field_10`）；`services/pets.rs` 新增
   `activate_dog`（前置校验 → RPC → 回读确认 owned）与快照 `activatable` 三态
   （未拥有 &&（field_6=1 || 背包有未锁定同 ID 卡））；DeployDog 错误按 field_6 细分
   「尚未激活/未获得」；全链路接线 app `pet_activate` → desktop IPC `pet_activate`
   （ACL 已加）→ PetPanel 激活按钮/可激活标签（i18n 中英）
5. **多格种子预留**（bot `96fdb39` + `ee4de82`）：新增 `services/farm/layout_reservation.rs`
   （`select_future_layout_reservation`，锚点最小且部分空出的布局，只预留当前空地）；
   配置 `bagSeedMultiLandReservationEnabled`（默认 false，opt-in）三段式接线
   （types/normalize/account_config 快照）；`plant_from_bag_seeds_ex` 仅对显式优先种子、
   size>1、每轮一次触发预留，预留地从本轮可种地扣除且商店 fallback 不占用；
   调度统计改按实际种下数计并新增「预留N」；设置页策略 tab 新增开关（i18n 中英）

**附带修复（预存断裂）**：恢复 `get_settings_panel` / `save_settings` 两个 Tauri 命令
（`474155a` 误删——当时认定无调用方，但设置页 `fetchGetFarmAutomationDetail` /
`fetchModifyFarmAutomation` 实际仍在调用，设置页读写在 HEAD 上是坏的），
`settings_panel` 聚合数据带上新开关字段。

验证：`cargo test --workspace` 1024 通过（含 desktop ACL 防回归）、clippy 干净、
`pnpm typecheck` 通过。待实机回归：登录（新 73 字节 Login 包）、anti_data 上报（新 wasm）、
宠物激活、多格预留种植。
### 2026-09-24 — 增量同步 bot 1d1edb3..9c30b05（协议 1.14.2.11 / 秋日活动 / SVIP 商城 / 施肥语义 / 扫码昵称）

（rebase 到 9-15 的 9-14 批次提交之上；宠物激活、多格种子预留、Login 逐字节对齐、
TSDK 升级在 b686694 已完成，本提交不再重复，配置开关沿用其
`bagSeedMultiLandReservationEnabled` 命名与设置面板开关）

- **协议与版本**：proto 整份重镜像至 bot 9c30b05（activitypb 加 wish_sign=119 /
  share_reward=120 / Operate field 151-157；mallpb 字段重排；新增
  autumn-activities.proto）；版本成对升 `1.14.2.11_20260922` / `1790215551955`；
  tsdk.wasm 升 `v3.9.0.1790160550`（161081B，SHA-256 校验一致）
- **秋日活动**（新增 `activity_center/autumn.rs`）：秋祈良愿（GetGroup field 119 +
  Operate 51/52）+ 快乐不独享（field 120 + Operate 69/70/71/73）；mutation_lock 串行 +
  回包三重校验 + 回读失败降级 refreshRequired；`autumn-20260924.json` include_str!；
  目录绑定 gameplayKey autumnWish/autumnHappy（priority 1/2）；前端 `autumn-view.vue`
  自加载；素材 `public/activity-assets/autumn/`
- **商城 + SVIP**（mall/commerce/qqvip 重写）：MallGoods 新字段、GetMallListBySlotType
  field 2 改 is_manual_open、PurchaseResponse success 校验；SVIP slot 4 目录注入
  membership、非会员禁购、expectedPrice 防涨价（MALL_PRICE_CHANGED）；limit_type=0
  不限购 + slot 1 无 limit 视为可购的 mallAvailability 状态机；促销划线价按服务器时间
  复验；qqvip 字段语义重写 + claimSvipMallFreeGift；前端商城页改「普通 / SVIP /
  神秘商人」三 tab（神秘商人菜单项删除、旧路径重定向 `?tab=mystery`）；非会员
  RefreshVipInfo/GetQQVipRewardsStatus 回 1021001 时按非会员降级，目录不再整体报错
- **施肥语义**（bot a68b187 + d1a7562）：`left_inorc_fert_times` 修正为「本季普通肥
  剩余次数」，普通肥目标改配额 >0 判定；新增巡田补肥 `run_patrol_refill`（multi_season
  开关 + 含普通肥策略时补一轮）；Both「无机一次+有机催熟」偏离保留
- **扫码登录昵称**（bot a989a8e）：微信 confirm 后尽力而为调 `pcyyb_get_user_info`
  （Ual-Access-* 头 + md5(timestamp+nonce)，OAuth 回包解析作 fallback）；
  wx_login_code 返回 nickname 用于账号表单预填（QQ 侧随 NapCat 删除不适用）
- **游戏配置镜像**：ItemInfo/Plant/Illustrated 整份拷 bot 9c30b05；11 张新种子图转
  WebP（1901/1902/20435/2121/26030/101613/204010/401006/6002/90042 新增 + 6001 更新）
- **NTabPane 标签空白修复**：naive-ui 2.45 `normalizeSlots` 把 undefined children 包装
  成空默认插槽，自闭合 `<NTabPane tab="x" />` 的 label 渲染成空注释；改 `#tab` 具名
  插槽（商城三 tab + autumn 日志 tab）
- 验证：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets` 0 错 0 警；
  `cargo test --workspace` 全过；`cargo fmt --all --check`、`corepack pnpm typecheck`、
  `corepack pnpm build` 通过；无账号静态页浏览器实测：商城三 tab 名称渲染 + 点击切换
  正常、菜单无神秘商人项
- **实机待验**：新版本号 + 新 wasm 登录；秋日两个活动；商城 SVIP 浏览/禁购/促销划线价；
  巡田补肥触发


### 2026-09-29 — 增量同步 bot 9c30b05..864caf3（协议 1.14.2.15 / 微信 TSDK 双构建 / 掉线诊断 / 种子图）

bot 本轮 13 个提交**玩法层零变化**（活动/商城/日常自动化均未动）；核心是连接层：
协议升级、微信协议对齐（PR #84）、会话保活诊断（PR #85）、18 张种子图、web 重登 UX（PR #83）。
**重登 UI 不移植（用户决策 2026-09-29）**：rust 后端重登能力本已等价
（upsert 带 id code_changed 必重启 + 不带 id 名字匹配 remark_relogin，均已实现），
桌面端无对应缺陷，bot web 其余改动是修自家 tab 覆盖/表单 bug。

- **协议与版本**：版本成对升 `1.14.2.15_20260922` / `1790568000000`（bot `864caf3`，
  QQ/微信统一版本号；旧存档 updatedAt 更小会正确回落新默认）；握手 ver/heartbeat 挂同一常量链
- **微信 TSDK 双构建**（bot `TSDK_BUILDS`）：新增 `assets/tsdk-wx.wasm`
  （160992B，SHA-256 `4bf6aa0e…`，加载时校验）；`crypto/tsdk.rs` 构建表
  qq(`tsdk.wasm`/`v3.9.0.1790160550`)/wx(`tsdk-wx.wasm`/`v3.9.0.1790237209`) 成组校验，
  Module 缓存按平台分槽；顺带修正 `TSDK_VERSION` 常量停滞 `v3.9.0.1789137379` 的旧账
  （该字符串经 host import `d` 编入 AntiData 特征）；`HostProfile::resolve` 方向反转
  对齐 bot（`wx`/`wechat`→微信宿主，**其余含空/未知→QQ 宿主**，rust 原实现恰好相反）；
  wasm 路径收敛到 `tsdk::resolve_wasm_path(platform)`（`TSDK_WASM_PATH` 覆盖 QQ、
  新增 `TSDK_WX_WASM_PATH` 覆盖微信；删 EngineConfig/WorkerConfig 的单路径字段；
  tauri resources 补 `assets/tsdk-wx.wasm`）；`read_cstring` 默认上限 64KB→1MB
  （对齐 bot readCString；`get_encrypted_init_info` 保持显式 64KB）
- **ACEVM 诊断**（bot `recordUnsupportedAceVm`）：host import `e` 仍返回 0 不 trap，
  新增调用计数/时间/任务原文 SHA-256（64KiB+1 上限只哈希不解码）/读取失败标志，
  once 告警 `event=tsdk_host_limitation`；`TsdkRuntime::diagnostics()` 对齐 bot `getDiagnostics`
- **微信登录分支**（bot `client-profile.ts` + `network.ts`）：`build_login_body_for_platform`
  微信 device_info 追加 network(5)/memory(10)/device_id(13)（有值才写，memory 解析失败按 0
  不写，protobufjs 字段号升序）、scene_id 整字段省略、minigame_channel=`other`；
  QQ 保持 73 字节 golden 不变；单测对齐 bot `wechat-connection.test.js` 断言点
  （wire 层验 field 缺席，prost 表达不了 presence）
- **掉线诊断**（PR #84/85 业务可见面）：KickoutNotify 数字 reason（reasonCode）保留进
  踢下线日志；会话结束输出 `connection_summary` 汇总（platform/version/连接与在线时长/
  心跳 attempts·replies·failures/pending/queued/入站静默/ACE·TSDK 诊断/断开原因，
  tracing 结构化 + 面板 `network` 模块日志各一份）；ACE 统计对齐 bot `getAceDiagnostics`
  （requests/replies/nonempty/failures/lastFailureStage/taskFailures + `antidata_roundtrip`/
  `antidata_received`/`tsdk_task_failed` 事件）；logger 诊断码白名单对齐
  `DIAGNOSTIC_CODE_KEY_RE`（disconnect/close/error/http/status/exit/reason*Code 且值为
  整数才放行）。bot worker→master 的 `account_kicked`/`account_disconnected` IPC 消息与
  `code`→`disconnectCode` 改名不移植（rust 无多进程层，按对齐原则不记缺口）；
  ACE generation 隔离同理（rust 每次 worker 重启全新 AceShared+TSDK 实例）
- **种子图**：18 张新种子图转 WebP 落 `assets/game_config/seed_images_named/seed_images/`
  （1020435/1026030/1040435/1046030/1120435/1126030/201011/202010/203011/204011/204012/
  205010/206010/207011/208011/40435/46030/90035，cwebp -lossless 与存量参数一致，
  目录 988→1006 与 bot 打平）；未跑 images 整目录镜像（防抹掉 webp）
- 验证：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets` 0 错 0 警；
  `cargo test --workspace` 全过（core 1042 项，含新增 wx wasm SHA/体积、双构建字面量、
  平台归一方向、微信 Login wire、reasonCode、诊断码白名单断言）；`cargo fmt --all --check`、
  `corepack pnpm typecheck`、`corepack pnpm build` 通过（前端零改动）
- **实机待验**：QQ 新版本号 1.14.2.15 登录；微信扫码全流程（tsdk-wx.wasm + 微信 Login 体）；
  被踢日志 reasonCode 与 connection_summary 汇总；新种子图在前端正常显示
# 2026-09-30 — GPUI Kit 桌面宿主与全页面交互补齐

新增 `crates/qq-farm-gpui`，固定依赖 GPUI Kit 0.7.0；数据流直连 `qq-farm-app`，使用 `AclPolicy::LocalOwner`。Tauri 保留原有 IPC 契约，并与 GPUI 共用 app 层 `EngineLease`，对同一 `FARM_DATA_DIR` 做进程互斥。

- 十个页面入口接入 app 快照，并按业务拆开布局：总览为账号资产摘要与运行日志；农场按土地、仓库、任务、宠物、图鉴及互动道具分段；好友分好友、黑名单和访客记录；活动按玩法分栏；分析提供排名和种植黑名单；商城与神秘商店使用商品卡片；设置使用分组表单；游戏配置按种子、果实和道具管理。账号页另有专用列表与登录流程。
- 已接入账号 code 新增/编辑、微信授权、启停、删除确认及批量删除；土地状态门控的务农操作；背包筛选、批量出售、宠物与互动道具操作；好友农场详情、互动、黑名单及访客筛选；活动玩法导航、领取/兑换及天气操作；收益排序、种植策略和黑名单；普通/SVIP/神秘商品数量购买；设置数值和开关、离线提醒、系统配置、QQ Bot 绑定；游戏配置增改删和图片路径配置。发布版本检查仅接受含 GPUI 对应产物的 release。
- 游戏配置写入逻辑下沉为具体 Rust 类型的 app 接口；Tauri command 名、参数及返回 JSON 保持原结构。游戏与活动图片按原 URL 约定映射至打包资源目录；包内含 QQ/WX TSDK、游戏配置和活动素材。
- GPUI 独立偏好文件 `gpui-ui.json` 保存主题、界面语言、导航项、侧栏状态和窗口尺寸，不更改业务数据格式。新增 macOS/Linux shell 与 Windows PowerShell 打包脚本、三平台 CI、Release 包上传。
- **验证通过**：本次 `cargo fmt --all -- --check`、`cargo test --workspace`（core 1042、app 16、Tauri 7、GPUI 8、微信 HTTP 集成 1 项通过；另 5 项按项目约定忽略）、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings` 和 `cargo check --workspace --all-targets` 全部通过。历史基础接入阶段也通过前端 `pnpm typecheck`/`pnpm build`、打包脚本语法与 workflow YAML 解析。本机 x86_64 macOS release 构建和 `.app.zip` 打包曾通过，产物为 `dist/qq-farm-gpui-macos-x86_64.app.zip`。
- **实机待验**：尚未在真实窗口检查浅深主题、中文输入、最小尺寸、125%/150% 缩放、关窗到托盘/恢复和弹层焦点；也尚未执行 Windows/Linux CI 与实机打包。微信扫码/快捷授权、在线好友/活动/商店门控及购买需要真实服务环境端到端验收。长列表虚拟化、窄窗口详情视图、完整英文文案和覆盖所有页面的 GPUI 焦点/键盘测试仍待补。
- 页面功能现已覆盖全部入口的主要交互，但仍不能据此声明与 `desktop-ui` 完整等价：需逐项对照 Vue 的边界状态和字段语义，并完成真实后端与三平台窗口验收。

### 2026-09-30 — GPUI 账号页交互补齐

- 账号页改为专用表格，不再复用通用信息行：增加关键字、平台、运行状态、微信授权筛选；按 10 条分页；支持当前页全选、跨页多选、批量删除；行操作包含编辑、启动/停止、删除；显示平台、昵称、运行态、授权态与最近更新时间。
- 新增/编辑改为覆盖式右侧 sheet。Code 流程支持 QQ/微信平台选择与粘贴完整登录链接的平台识别；微信流程先探测本机授权，未检测到时自动提供二维码，扫码后轮询、确认、换码并保存。
- **验证**：该次账号页改动通过专项编译和测试。后续全页面交互与呈现已在上方同日记录；GPUI 专项测试现为 8 项。微信外部授权仍需真实桌面微信/扫码环境端到端验证，所有页面的业务边界也仍需对照 Vue 实机验收，因此当前仍不标记为整套完全等价。

### 2026-09-30 — GPUI 业务模块拆分与跨页交互对照

- 将账号实现拆为 `desktop/accounts/{mod,list,sheet}.rs`，将主窗口侧栏与内容区拆为 `desktop/view/{sidebar,content}.rs`；账号、好友、活动、分析、商城、个人农场、设置、游戏配置仍各自维护在业务模块中。`desktop/mod.rs` 保留共享页面状态、运行时和导航协调。
- 好友页补好友优先级排序、每页 25 条、主列表排除黑名单、黑名单名称回填；好友地块详情展示生涯摘要、作物图、成熟时间、季节和水草虫状态。捣乱、屏蔽/解除屏蔽、删除好友均先确认。七夕页切入时懒加载好友选择，支持选择目标和刷新；香囊数量受库存与活动门控约束。
- 活动页补青梅原料库存选择、数量调整、开始/继续/结算操作门控；星砂兑换增加数量确认（1–99）和后端库存/余额校验。游戏配置补季数、稀有度、道具类型筛选，10/20/50/100 条分页及批量删除确认。
- **验证通过**：`cargo fmt --all -- --check`、`cargo check -p qq-farm-gpui --all-targets`、`cargo test --workspace`（app 16、core 1042、Tauri 7、GPUI 9、微信 HTTP 集成 1 项通过；微信实机用例和 4 项文档测试按项目约定忽略）及 `cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`。
- **尚未完全等价**：好友地块倒计时尚未像 Vue 一样实时刷新，部分宠物徽标/访客记录展示仍较简化；秋日、萌宠日记和天气玩法仍缺 Vue 中的部分带参数交互；配置编辑尚未覆盖各抽屉的完整字段与原生图片选择；分析页多选黑名单、长列表虚拟化、窄窗口详情、完整中英文及真实窗口/三平台验证仍待完成。当前不能宣称所有功能和交互已对齐。

### 2026-09-30 — GPUI UI 代码按页面与交互职责拆分

- 主窗口协调层保留共享状态和初始化；导航、后台事件/刷新、窗口壳、通用数据格式化与资源预览分别放在 `desktop/navigation.rs`、`runtime.rs`、`shell.rs` 和 `helpers.rs`。GPUI 测试移至 `desktop/tests.rs`。
- 通用内容视图只保留页面骨架与列表过滤；设置偏好、设置编辑器、好友详情/确认、宠物记录、账号删除、商城购买确认、游戏配置表单分别移入各业务视图。跨页行操作入口收敛为轻量分发器，具体按钮由账号、好友、农场、活动、商城、分析、设置和游戏配置模块实现；工具栏也由对应页面模块提供。
- 农场操作按种植、仓库、宠物拆入 `desktop/personal/`；账号表单和微信授权拆入 `desktop/accounts/`；活动一般操作和天气操作拆分。秋日快乐活动支持切换“领取我的 / 我领取的”日志页签、请求对应 tab，并展示操作者、快乐值和时间。
- **未对齐项仍保留在上方清单**；该次拆分和秋日日志补充没有把演示状态或通用行按钮算作业务等价，也不代表完成全部 Vue 交互。
- **验证通过**：`cargo fmt --all -- --check`、`cargo check --workspace --all-targets`、`cargo test --workspace`（app 16、core 1042、Tauri 7、GPUI 10；微信 HTTP 集成 1 项通过、1 项需要本机微信环境而忽略；4 项文档测试按仓库约定忽略）、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings` 和 `git diff --check`。同时修复了测试临时文件名碰撞和自动化锁测试共用账号造成的并发竞争。真实窗口、微信授权和其他平台仍须按实机清单验收。

### 2026-09-30 — GPUI 页面映射与大型交互视图按业务职责拆分

- 将原 `pages.rs` 中心文件拆为 `pages/model.rs`、`pages/loader.rs` 和 `pages/mapper/`；总览、农场、好友、活动、分析、商城、设置、游戏配置、账号各自维护快照到展示行的映射，公共实体与字段格式化独立保留。
- 账号列表拆成筛选栏、表格与页面协调器；活动 controls 按萌宠日记、秋日、青梅、七夕、天气和活动目录拆分，付费刷新确认弹层单独管理；好友农场详情/倒计时和弹层、游戏配置表单初始化/保存也分别独立。
- 更新 `docs/ARCHITECTURE.md`，记录 GPUI crate 的业务模块边界。保留 `desktop/mod.rs` 作为共享窗口状态和协调层，不将每个业务页面并入主窗口文件。
- **验证通过**：`cargo fmt --all -- --check`、`cargo test -p qq-farm-gpui --all-targets`（12 项通过）、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings` 和 `git diff --check`。实机/跨平台验证和前文列出的业务等价验收仍未完成。

### 2026-09-30 — GPUI 总览、商城与设置行为复核

- 总览补回 Vue 每 30 秒对在线账号刷新背包与钻石余额的行为；原有操作日志防抖刷新和账号/页面版本丢弃规则保留。
- 商城按当前 `desktop-ui` 合并为普通、SVIP、神秘商人三标签；神秘商人读写同一商城页，购买按服务端单件限购确认，不再显示数量编辑；历史 `mystery-shop` 偏好迁移至商城对应标签。
- 设置策略改为草稿后统一保存；修正 `preferredSeedId` 写入字段，增加可搜索的优选种子、背包种子排序/回退策略/多格预留设置，数字值可直接编辑；背包顺序保留暂时无库存的旧优先项。QQ Bot 绑定扫码后每 2 秒自动轮询，绑定成功、二维码过期或离开设置页时停止轮询。
- **验证通过**：GPUI `cargo test --all-targets` 19 项、GPUI Clippy `-D warnings`、`cargo fmt --all -- --check`、`git diff --check`。
- **尚未完全等价**：仍需对活动规则与参数、好友/访客和宠物细节、配置图片选择等逐项对照；完整英文文案、窄窗口详情布局、真实窗口焦点/缩放、真实后端购买与活动操作及 Windows/Linux 实机仍待验。本条只记录已修复范围，不代表全页面已对齐。

### 2026-09-30 — GPUI 页面按业务职责细化模块

- 总览视图拆到账户、资产、巡查、操作统计、日志列表/筛选和格式化模块；设置拆为策略、草稿、保存、Bot/提醒集成及分区控件；主窗口初始化独立到 `desktop/init.rs`，账号快照读取归入账号模块。
- 活动行操作按七夕好友、萌宠日记、青梅、秋日、节气、星砂、公益和天气拆分；活动后端操作分到各玩法处理器。游戏配置表单按种子、果实、道具和共用字段拆分。
- 本次是结构整理，保留既有页面入口、交互门控和业务调用。同步更新架构树，明确共享窗口协调层与业务模块的边界。
- **验证通过**：`cargo check -p qq-farm-gpui`、`cargo test -p qq-farm-gpui --all-targets`（19 项）、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`、`cargo fmt -p qq-farm-gpui -- --check` 和 `git diff --check`。

### 2026-09-30 — GPUI 页面差异复核与业务行视图拆分

- 按业务拆分通用展示行：`desktop/view/rows/` 下分别维护总览、农场、好友、活动、分析、商城、设置和游戏配置，`rows/mod.rs` 只负责分发；这让页面专属布局不再堆在一个 400 行文件里。crate 当前源文件按页面/交互职责分目录维护，共享窗口状态仍留在协调层。
- 天气活动选择好友后立即读取现场天气；补本账号和所选好友天气状态、按秒递减的剩余时间，并按后端快照展示活动规则。游戏配置的描述、生长阶段和效果说明改用多行编辑器，和 Vue 抽屉的 textarea 字段一致。
- 好友列表补宠物徽标与图片、宠物信息同步进度；访客行补等级；好友农场生涯摘要补收偷比。
- 复核发现此前差异清单有过期项：好友地块倒计时已由 `friends/detail.rs` 实时更新；分析页已支持逐项多选及全部加入/清空；游戏配置已有原生图片选择器；普通列表超过 80 行已虚拟化。卡片网格不属于该虚拟列表覆盖范围，需单独验证长列表体验。
- **验证通过**：`cargo fmt -p qq-farm-gpui`、`cargo check -p qq-farm-gpui`、`cargo test -p qq-farm-gpui --all-targets`（24 项）和 `cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`。
- **仍未宣称完全等价**：需要继续按 Vue 页面逐项核对剩余活动玩法、页面边界门控和字段呈现，并在真实窗口验证窄尺寸、中文输入、焦点恢复、主题/缩放；完整英文界面及 Windows/Linux 实机结果也尚未完成。

### 2026-09-30 — GPUI 壳层与设置按职责继续拆分

- 将运行协调拆为实时事件、页面快照加载、账号启停、更新检查和共享辅助逻辑；内容视图拆出筛选、虚拟/网格行列表及工具栏；背包种子排序单独从种植策略模块移入 `settings/bag_seed_priority.rs`。
- 增补萌宠夺宝好友选择：加载好友列表、支持刷新和按名称/GID筛选选择，同时保留手动输入 GID 查询；好友选项不混入活动展示行。
- 更新 `docs/ARCHITECTURE.md` 的模块树。GPUI crate 目前有 144 个 Rust 源文件，按账号、农场、好友、活动、分析、商城、设置和游戏配置拆分；`desktop/mod.rs` 仍只保留共享窗口状态与装配。
- **验证通过**：`cargo fmt --all -- --check`、`cargo check -p qq-farm-gpui --all-targets`、`cargo test -p qq-farm-gpui --all-targets`（25 项）、GPUI Clippy `-D warnings` 和 `git diff --check`。
- 业务和交互对齐仍需按 `desktop-ui` 完整逐项验收；本次文件拆分与新增好友选择流程不作为全功能已等价的证明。

### 2026-09-30 — GPUI 窗口状态按业务域拆分

- 将原先平铺在 `FarmDesktop` 中的 120 余项状态拆到 `desktop/state/`：账号、总览、农场、好友、活动、分析、商城、设置和游戏配置各有独立状态类型与源文件。各页面交互显式访问所属业务状态，窗口实体和运行时等共享字段仍由 `FarmDesktop` 组合。
- `desktop/mod.rs` 不再声明跨业务的大型状态结构；架构文档中的 GPUI 模块树已更新，标出状态与页面交互的业务边界。
- **验证通过**：`cargo fmt --all -- --check`、`cargo check -p qq-farm-gpui --all-targets`、`cargo test -p qq-farm-gpui --all-targets`（27 项）和 `cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`。
- 本次是代码职责整理，不代表所有页面功能和交互已与 `desktop-ui` 完全等价；端到端对照和桌面实机验证仍按差异清单继续。

### 2026-09-30 — GPUI 天气活动好友选择与扫描状态对齐

- 将天气活动的好友目录从通用行操作改为独立选择区：顶部页面搜索可按好友名称/GID筛选，单击选择后读取现场天气；已选状态展示可采、已采、失效、晴天或读取失败，支持继续使用手动 GID。
- 扫描结果覆盖基础好友资料并在页面刷新期间保留，切换账号时清空；列表按 Vue 上限最多显示 60 位好友。巡查繁忙导致的延迟好友会显示稍后重试提示，好友目录不再重复出现在通用数据行。
- GPUI crate 当前有 161 个 Rust 源文件，根协调状态按业务域拆在 `desktop/state/`，天气好友交互另有 `desktop/activities/weather_friends.rs`。
- **验证通过**：`cargo fmt --all -- --check`、`git diff --check`、`cargo check -p qq-farm-gpui --all-targets`、`cargo test -p qq-farm-gpui --all-targets`（28 项）及 GPUI Clippy `-D warnings`。
- 本条只关闭天气好友选择与扫描状态这一项差异。秋日、萌宠日记其余参数化操作、其他页面字段/门控、真实窗口和跨平台验收仍未完成，不能据此称全功能等价。

### 2026-09-30 — GPUI 业务状态构造归回业务模块

- 将账号、总览、农场、好友、活动、分析、商城、设置、游戏配置的输入控件与默认状态构造移到各自 `desktop/state/*.rs`；`desktop/init.rs` 保留账号选择、共享事件订阅与各业务状态装配，缩至 133 行。
- **验证通过**：`cargo fmt --all -- --check`、`git diff --check`、`cargo check -p qq-farm-gpui --all-targets`、`cargo test -p qq-farm-gpui --all-targets`（28 项）及 GPUI Clippy `-D warnings`。
- 这是状态初始化的职责归属，不表示剩余 Vue 交互差异已经关闭。

### 2026-09-30 — GPUI 萌宠夺宝次数与好友目标门控

- 夺宝按钮现在同时要求快照 `hunt.canPlunder=true`、已选目标仍属于当前输入的好友，并在提交入口再次校验；改选好友后旧目标不会继续可用。无剩余夺宝次数时禁用操作，避免请求一个 Vue 页面会拦截的动作。
- GPUI crate 当前有 161 个 Rust 源文件（约 16.8k 行）；业务模块已拆到账号、个人农场、好友、活动、商城、分析、设置、游戏配置等目录，窗口根模块 67 行、初始化模块 133 行，最大单文件 370 行。
- **验证通过**：`cargo fmt --all`、`git diff --check`、`cargo test -p qq-farm-gpui --all-targets`（29 项）及 GPUI Clippy `-D warnings`。
- 本条只对齐萌宠夺宝的一项提交门控；其他页面和活动交互、真实窗口及跨平台验收仍未全部完成。

### 2026-09-30 — GPUI 秋日祈愿默认选择

- 加载秋日活动时默认选中 Vue 页面首项祈愿；刷新时保留仍有效的用户选择，若该选项已从新快照移除则回退到新的首项。空选项列表会清空旧选择。
- **验证通过**：`cargo fmt --all -- --check`、`git diff --check`、`cargo test -p qq-farm-gpui --all-targets`（30 项）及 GPUI Clippy `-D warnings`。
- 这只消除了祈愿的一次多余点击，完整活动字段、操作和门控仍需要逐项验收。

### 2026-09-30 — GPUI 业务操作按子域拆分

- 活动快照加载移到 `desktop/activities/loading/`，萌宠日记、秋日、天气和七夕各有独立读取模块；活动根模块只声明页面能力，青梅状态操作归入青梅控件模块。去掉重复的萌宠日记加载分支，保留好友列表读取失败时活动快照仍可显示的现有行为。
- 好友动作拆分为确认/常规操作、好友互动与批量偷取、删除三个模块；商城购买执行和购买确认弹层拆分到 `desktop/commerce/`。更新架构树标出实际业务文件边界。GPUI crate 当前为 173 个 Rust 源文件，desktop 下包含账号、活动、好友、个人农场、商城、分析、设置、游戏配置等业务目录。
- 测试临时数据目录改用进程内原子序号，避免并发 UI 测试争用引擎锁；修正星座默认焦点测试样例，使它显式验证 currentDay 回退规则。
- **验证通过**：`cargo fmt --all`、`cargo test -p qq-farm-gpui --all-targets`（34 项）及 `cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`。此结构调整不代表所有 Vue 页面交互已验收等价，剩余对照项仍以上文清单为准。

### 2026-09-30 — 七夕香囊数量跟随余额约束

- GPUI 七夕赠送数量改用带步进控件的数字输入，并按 Vue 规则限制为 `1..=max(1, 香囊余额)`；活动刷新导致余额下降时，输入值同步收敛到新上限。赠送按钮仍要求活动开放、余额充足且选择当前已加载好友。
- **验证通过**：`cargo fmt --all`、`cargo test -p qq-farm-gpui --all-targets`（35 项）和 GPUI Clippy `-D warnings`。其余活动行为及全页面对照仍在进行。

### 2026-09-30 — 公益爱心捐赠增加二次确认

- 公益页拆成 summary、rewards、donation 和 presentation 子模块，活动快照、好友操作和商城购买也分别按玩法/职责拆分；GPUI crate 当前 178 个 Rust 文件，activities/charity.rs 保留 53 行页面组合入口。
- 公益活动从通用行列表改为专用业务视图，呈现爱心余额、累计捐赠、个人结算条件、全服进度与奖励、种子/礼包奖励、阶段奖励。种子、每日礼包和可领取进度奖励在各自卡片操作；“捐赠全部爱心”首次点击进入确认，确认后才调用门面，取消或切换页面/账号会清掉确认状态，并服从 `donateLove.enabled` 门控。奖励图片继续使用本地资源适配器和缺图状态。
- **验证通过**：GPUI UI 测试覆盖确认状态显示与取消返回，`cargo test -p qq-farm-gpui --all-targets`（37 项）及 GPUI Clippy `-D warnings` 通过。仍需继续核活动及其他页面的字段和操作差异。

### 2026-09-30 — GPUI 自动化与微信登录按流程继续拆分

- 自动化设置视图按基础自动化、施肥购买、好友自动化、神秘商人拆到 `desktop/settings/controls/automation/`；微信授权按本机快速授权和扫码登录拆到 `desktop/accounts/wx_login/`。页面入口只负责组合，原字段、门控和业务调用保持不变。
- GPUI crate 当前有 184 个 Rust 文件、约 17.9k 行；这些数字包含页面映射、状态、交互、控件、测试和桌面壳模块，不代表业务等价验收完成。最大文件现为账号表单 Sheet（356 行），后续继续按职责复核，避免用空壳文件增加数量。
- **验证通过**：`cargo check -p qq-farm-gpui --all-targets`、`cargo test -p qq-farm-gpui --all-targets`（37 项）、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings` 和 `cargo fmt --all`。完整 Vue 页面交互与实机验证仍未完成。

### 2026-09-30 — GPUI 账号弹层按登录方式拆分

- 账号弹层拆成共用壳层、Code 凭证表单和微信授权内容；弹层状态、焦点恢复、Code/扫码/本机授权流程保持原有调用关系。
- crate 当前为 186 个 Rust 源文件（约 17.9k 行）；最大文件是账号列表表格（331 行）。业务子模块继续按实际页面职责拆分，文件数量本身不作为功能对齐的验收项。
- **验证通过**：`cargo check -p qq-farm-gpui --all-targets`、`cargo test -p qq-farm-gpui --all-targets`（37 项）、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。其他页面的完整功能等价仍在逐项复核。

### 2026-09-30 — GPUI 分析与配置交互按 Vue 页面语义对齐

- 分析页现在是“种植策略 / 种植黑名单”两个标签。策略页只展示当前设置的策略、最佳作物和五种效率对比，不再把分析页伪装成可直接切换策略的控制台；参考等级使用 1–200 步进输入并随账号初始化/切换同步。黑名单支持按名称/编号搜索、多选、移除、全部加入和清空；“全部加入”保留原列表中当前排名之外的 ID。
- 游戏配置编辑时锁定种子/果实/道具编号；果实新增不显示自动生成的果实编号，编辑时关联植物只读。编号、等级、价格、经验、收获量及持有上限使用有最小值的数字输入，配置保存仍校验名称、编号和关联植物。
- 将原 `desktop/tests.rs` UI 测试按账号、总览、分析、商城、农场、设置、活动拆分，窗口/runtime/隔离目录助手独立放入 `tests/support.rs`；分析页交互和真实配置写入有 GPUI UI 测试覆盖。
- GPUI crate 当前有 211 个 Rust 文件、19,698 行；业务页面实现分布于各自目录，不以文件数量作为等价证明。最大单文件为策略视图（382 行），账号表格为 331 行。配置页新增/编辑流程也有独立 GPUI UI 测试。
- **验证通过**：`cargo fmt --all -- --check`、`cargo test -p qq-farm-gpui --all-targets`（39 项）、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings` 和 `git diff --check`。此处只关闭分析页与配置编辑中记录的差异；其它页面仍需逐项对照，完整 Vue 等价、三平台构建和真实窗口验证尚未完成。

### 2026-09-30 — GPUI 跨业务辅助逻辑归回所属模块

- 删除集中了 303 行跨业务函数的 `desktop/helpers.rs`：活动门控、活动目录分流和活动标题归入 `activities/helpers.rs`；账号记录读取、时间格式及登录链接解析归入 `accounts/helpers.rs`；农场操作门控与背包分类分别归入 `personal/farming.rs` 和 `personal/inventory.rs`；设置 JSON 路径更新归入 `settings/draft.rs`；图片资源展示归入 `view/media.rs`；页面分段配置留在 `navigation.rs`。
- 更新架构目录，页面调用仍通过明确的业务模块导出访问。此整理不改变字段、页面入口、操作门控或业务调用，也不作为全功能等价证明。
- **验证通过**：`cargo test -p qq-farm-gpui --all-targets`（42 项）、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`、`cargo fmt --all` 和 `git diff --check`。完整 Vue 页面交互和真实窗口/跨平台验收仍需继续完成。

### 2026-09-30 — GPUI 秋日玩法改用专用交互视图

- Vue 秋日页的祈愿与快乐值信息此前被压平成通用行。GPUI 现按玩法拆为 `autumn_view/{shared,wish,happy}.rs`：展示活动标题/时间/规则、祈愿选择和剩余次数、待领奖励与连续奖励、快乐值进度/分享奖励文案/里程碑/双标签互动记录。
- 抽签选择、领取门控、每日领取、分享、档位领取和记录切换继续调用原有活动 action；自定义视图不再同时渲染重复的通用行或通用规则块。
- **验证通过**：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo test --workspace`（含 GPUI 44 项）、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check`、`pnpm -C desktop-ui typecheck` 和 `pnpm -C desktop-ui build`。完整活动集及其它页面的差异仍需逐项核对；真实窗口、跨平台和线上服务验证未完成。

### 2026-09-30 — GPUI 普通与 SVIP 商城商品卡片对齐

- 普通/SVIP 商城从通用行卡片改为独立网格卡片：读取奖励图片、会员身份与剩余天数、钻石余额、刷新倒计时、搜索商品名称/ID，并展示原价、现价、折扣、限购进度和不可购买原因。
- 按 `purchaseStatus` 使用 Vue 对应的禁用文案，包括免费售罄商品的“奖励已领取”；购买确认补上价格、原价和折扣信息。神秘商人继续走独立商品流程，购买仍调用原 app 门面并使用服务端价格校验。
- **验证通过**：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo test --workspace`（含 GPUI 45 项；本机微信端到端测试按要求忽略）、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。`pnpm -C desktop-ui typecheck` 与 `pnpm -C desktop-ui build` 在这次仅改 GPUI 的商城提交前已通过。真实游戏商城数据和线上购买仍需实机验收。

### 2026-09-30 — GPUI 七夕鹊桥改为专用业务视图

- 七夕页由通用行列表改为 `activities/qixi_view/`：余额摘要、鹊桥阶段进度/状态/成本/奖励、好友搜索与选择、香囊数量调整及活动规则均按 Vue 页面字段呈现；选中好友、搜索框和数量状态均归属活动状态模块。
- 好友搜索支持昵称与 GID；赠送入口要求好友来自当前已加载列表、后端允许赠送、余额已知且数量不超过香囊余额。刷新好友、领取鹊桥奖励和赠送仍调用原有 app 门面。
- **验证通过**：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo test --workspace`（GPUI 46 项、core 1042 项、app 16 项、Tauri 7 项；微信本机端到端用例按要求忽略）、GPUI Clippy `-D warnings`、格式检查和 `git diff --check`。真实活动数据与线上赠送仍需实机验收；其余页面差异、真实窗口和三平台验收继续按清单逐项处理。

### 2026-09-30 — GPUI 青梅酿造按 Vue 流程拆为专用视图

- 新增 `activities/qingmei_view/`，拆出活动摘要/每日种子、原料选择、分轮报价与结算操作。库存卡、全选、各原料独立数量、已选汇总和规则都不再挤进通用行；开始后只呈现报价轮次、单价/总价及继续/结算门控。
- 新选原料默认使用 Vue 页面相同的库存数量，全选保留有效的已有数量；数量步进不再错误地把库存上限截断到 99。开始酿造前仍按当前快照验证原料 UID 与持有数量。
- **验证通过**：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo test --workspace`（GPUI 47 项、core 1042 项、app 16 项、Tauri 7 项；微信本机端到端用例按要求忽略）、GPUI Clippy `-D warnings`、格式检查和 `git diff --check`。真实青梅活动操作仍需服务端实机验收；其它页面差异、真实窗口和三平台验收仍继续按清单逐项处理。

### 2026-09-30 — GPUI 账号最近在线字段语义对齐

- 账号表最近在线显示现在遵循 Vue API 适配顺序：`lastOnlineAt`、`updatedAt`、`updated_at`；支持数字与数字字符串，并在值为空或无效时显示占位符。此前 GPUI 只读取更新时间字段，存在两个字段同时出现时显示不一致的问题。
- crate 当前有 230 个 Rust 源文件、约 22.3k 行，按账号、农场、好友、活动、商城、分析、设置、游戏配置、页面映射、运行时和测试职责组织；`desktop/mod.rs` 约 78 行。文件数用于描述结构，不作为页面等价证明；完整模块边界见 `docs/ARCHITECTURE.md`。
- **验证通过**：`cargo test -p qq-farm-gpui --all-targets`（48 项）及 GPUI Clippy `-D warnings`。仍需继续逐页对照 Vue 字段、交互与业务门控；当前不能宣称全功能完全等价。

### 2026-09-30 — GPUI 好友卡片展开、访客时间与一键偷取门控对齐

- 好友主列表现在点击好友卡片即可在卡片内展开/收起农场详情，替代原先独立“查看农场”按钮和列表末尾详情面板；详情按 Vue 的 `visibleLands` 规则隐藏合并土地的被占格，并补齐土地状态、土质、季数、作物进度、倒计时和加载失败重试。详情请求使用独立加载状态，账号/页面切换或折叠后丢弃旧结果。
- 访客记录时间按 Vue 显示“刚刚 / N 分钟前 / 今天 HH:mm / MM-DD HH:mm”。“一键偷取”按钮状态与目标列表共用 Vue 顺序：可偷数量、待帮忙数量、等级降序，GID 升序；黑名单好友不再进入目标列表。
- `docs/ARCHITECTURE.md` 已补好友详情/辅助逻辑的文件边界；新增好友卡片内联展开、合并地块和批量目标排序测试。
- **验证通过**：`cargo fmt --all`、`cargo check --workspace --all-targets`、`cargo test -p qq-farm-gpui --all-targets`（52 项）、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings` 和 `git diff --check`。其它页面交互、真实窗口和跨平台验收仍需继续，不能据此认定全功能已等价。

### 2026-09-30 — GPUI 好友操作状态与互动道具流程继续对齐

- 好友偷取/帮助/捣乱的忙碌状态按好友和操作区分，不再锁整页；成功与无可操作结果有短时反馈。操作成功后立即更新好友可操作数量，展开中的农场同步重新读取；读取农场详情也会按可见土地统计更新列表状态。切换账号、页面或好友子页时清掉旧任务状态并丢弃过期响应。
- 一键偷取继续排除黑名单并按 Vue 的优先级顺序执行，汇报成功/跳过数量及果实汇总。好友互动道具调整为先选择道具、填写好友 GID 和地块编号、再批量提交；展示道具描述、库存、可出售数量和空状态提示，刷新及提交采用独立状态。修正互动道具列表之前未挂载行操作区的问题。
- 好友代码进一步拆分为 `actions.rs`、`blacklist.rs`、`delete.rs`、`detail.rs`、`feedback.rs`、`interaction.rs`、`controls.rs`、`row_actions.rs` 和 `overlays.rs`，分别承担操作、反馈、详情、控件与呈现职责。GPUI crate 当前 234 个 Rust 源文件、约 23.5k 行，按业务域组织；最大源文件为 382 行策略视图，文件数量不作为等价证明。
- **验证通过**：`cargo fmt --all`、`cargo test -p qq-farm-gpui --all-targets`（59 项）、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings` 和 `git diff --check`。其它页面仍需逐项对照；完整业务验收、真实窗口和三平台验收尚未完成。

### 2026-10-01 — GPUI 星座与节气页面拆分并补齐详情操作

- 将星座和节气从通用活动行切换为各自业务视图。星座页按顺序选择节点、查看当前状态和奖励，规则采用显式展开，点亮操作放在所选节点详情中并沿用后端门控；节气页左侧选择节气、右侧显示标题、英文名、说明、奖励和领取状态，领取只针对当前选择项。
- 账号快照刷新时保留有效节气选择；原选择失效后优先落到服务端当前节气，再回退到首项。奖励卡片复用活动资源图片适配器和缺图状态。对应逻辑分别位于 `activities/constellation.rs`、`activities/constellation/view.rs`、`activities/solar_terms.rs`、`activities/solar_terms/view.rs` 与 `activities/rewards.rs`，没有把玩法实现加回主窗口协调文件。
- 赛季战令也从通用行/顶栏操作改为 `activities/season_pass/view.rs` 专用页面：显示旅行积分进度、等级奖励与已领/可领/未解锁状态，规则显式展开，“领取全部”按 `claimPass` 门控。
- **验证通过**：GPUI crate 测试 65 项；GPUI Clippy `-D warnings`、`cargo fmt --all -- --check`、`git diff --check` 和 `cargo check --workspace --all-targets`。本机实窗、其他平台及其它未覆盖 Vue 差异仍待验，当前仍不声明全功能完全等价。

### 2026-10-01 — GPUI 账号生命周期与业务模块边界

- 基准：rust `a176032c` + 工作区改动。
- 账号启停实现从表单模块拆至 `desktop/accounts/lifecycle.rs`：同一账号的重复启停会被拒绝，不同账号分别记录进行中状态，列表只禁用正在处理的行；页面刷新仍由页面级加载状态统一阻止操作。
- GPUI 页面按业务目录组织。当前 `src/` 有 241 个 Rust 文件，业务视图、状态、数据映射、交互和测试分层；`main.rs` 负责桌面启动，`desktop/mod.rs` 负责模块组合。文件数仅说明代码结构，不代表业务等价验收完成。
- **验证通过**：`cargo fmt --all -- --check`、`git diff --check`、`cargo test -p qq-farm-gpui --all-targets`（66 项）及 `cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`。本记录只覆盖账号行级启停和模块边界；全页面业务差异、真实窗口及三平台验收仍需继续核对。

### 2026-10-01 — GPUI 中英文导航和共用工具栏

- 对照原设置页中英文切换行为，修复此前只切换侧栏主页面标题的问题。页面分段导航现按页面/分段键返回中英文标题；账号占位、添加账号、刷新、更新检查、务农和设置编辑等共用文案也随语言切换。未知语言值继续回退简体中文。
- 新增英文环境下的 GPUI 窗口测试，检查主导航、分段导航、共用工具栏和添加账号入口；测试覆盖中英文选择器回退。此项只覆盖当前明确接入翻译选择器的共用界面，账号表格、业务详情/门控状态及大部分页面控件仍待翻译与逐页验收，不能据此宣称完整中英文支持。
- **验证通过**：`cargo test -p qq-farm-gpui --all-targets`（68 项）；本次最终格式、Clippy 与 workspace 检查仍待完成。

### 2026-10-01 — GPUI 日常礼包与成长任务卡片

- Vue 的个人农场“日常任务”区由 `DailyOverview` 和成长任务卡组成，只展示每日礼包状态与成长任务进度。GPUI 之前将 `/dailyGifts` 整份嵌套对象压成通用行，未显示礼包状态、会员/月卡未开通说明、完成时间和成长任务状态。
- 新增 `desktop/personal/daily_gifts.rs` 专用只读视图：按六类礼包显示对应图标、状态、订阅门控和进度/完成时间；成长任务显示全局完成数、每项完成态和进度。保留搜索过滤，不添加 Vue 原页面没有的领取操作。
- 新增 GPUI 窗口测试检查礼包卡片与成长任务行，单测覆盖会员/月卡状态文案及图标。
- **验证通过**：个人页针对性窗口测试通过；本次合并验证 `cargo test -p qq-farm-gpui --all-targets`（71 项）、`cargo fmt --all -- --check`、GPUI Clippy `-D warnings`、`cargo check --workspace --all-targets` 和 `git diff --check` 均通过。该修复只关闭个人页这一项差异，其它页面及真实窗口/跨平台验收仍继续逐项核对。

### 2026-10-01 — GPUI 设置页导航对齐 Vue 标签

- Vue 设置页默认显示“种植策略”，标签顺序为种植策略、自动化、离线提醒、系统配置。GPUI 此前默认落在自动化，且额外提供了没有对应页面控件的“好友规则”空分段；好友黑名单实际属于 Vue 好友页。
- 移除空分段并按 Vue 顺序排列现有设置分段，设置页初始分段现为种植策略；保留 GPUI 独有的“界面偏好”用于主题和语言选项。
- 回归测试核对分段键、顺序、默认页及英文标题。
- **验证通过**：当前 GPUI 全量测试 71 项、`cargo fmt --all -- --check`、GPUI Clippy `-D warnings`、`cargo check --workspace --all-targets` 与 `git diff --check` 均通过。其它页面字段/交互、完整双语、真实窗口和三平台验收仍未完成。

### 2026-10-01 — GPUI 我的农场土地视图对齐 Vue

- 基准：rust `a176032` + 工作区改动。
- 原 GPUI 土地页把地块压成通用行，缺少 Vue 的生涯统计、四类地块计数、4 列网格、合并地块覆盖关系、作物图、成熟进度、变异/紫晶加成、互动道具、季数与养护状态。新增 `desktop/personal/lands/`，拆成网格规则、摘要、生涯、地块卡片和时钟模块；工具栏务农操作也改为忽略合并作物覆盖的隐藏地块。
- 倒计时按读取时长递减并归零，页面活动期间每 60 秒仅刷新土地数据；请求校验页面版本和账号页面状态，加载、空态、刷新失败均有呈现。该改动仍未覆盖其他土地业务或全应用对齐。
- 新增 GPUI 窗口测试检查生涯、汇总和合并地块展示；单测覆盖合并网格/占位、操作门控和倒计时。
- **验证通过**：`cargo test --workspace`（GPUI 78 项、core 1042 项、app 16 项、desktop 7 项及 1 项可用的本机微信集成测试；另有 1 项需本机微信的集成测试和 4 项 doctest 按仓库原配置忽略）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `-D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。未做本机真实窗口复核；其它页面、全功能验收及 Windows/Linux 实机仍待完成。


### 2026-10-05 — GPUI 业务状态归属、个人页交互与资源适配

- 基准：rust `a176032` + 工作区已有改动；沿用当前 `desktop-ui` 可见行为。未修改 bot 镜像资源或既有 Tauri 图片协议。
- **业务边界**：移除全局 `desktop/state/` 业务文件分类；账号、总览、好友、活动、分析、商城、设置和游戏配置分别在自己目录维护 `state.rs`、输入创建和订阅。`desktop/state.rs` 只组合窗口和业务状态。农场状态继续分为土地、仓库、互动道具、图鉴、宠物五个子状态；仓库的控件、操作、反馈和刷新均在 `personal/bag/`。原 `personal/inventory.rs`、`personal/pets.rs` 与混合的 `personal/view.rs` 已移除，好友确认视图归到 `friends/confirmations.rs`。事件桥和窗口观察分别归入 `desktop/runtime/events.rs`、`window.rs`，渲染不再保存窗口偏好。
- **仓库**：默认果实分类，隐藏 Vue 的八类系统物品；类别计数、售价/货币、条件出售、图片和数量保持 DTO 含义。批量模式支持整卡点击和复选框，复选框不会触发第二次卡片切换；全选和出售数量仅计算当前分类及搜索结果。没有原始库存或没有选中可售物品时不发请求；成功显示后端 `summary` 并退出批量模式，失败保留选择。出售/使用各自按物品阻止重复提交，工具使用与批量出售分别维护状态。刷新和操作响应带账号及世代校验，页面离开使旧结果失效。
- **互动道具**：使用 GPUI Kit 选择器，默认第一个可用物品，并在刷新后保留有效选择；展示库存、可出售条件数量和说明。流程为选择道具、输入地块、批量使用，支持英文/中文逗号和空格分隔，提交前校验地块编号；后端反馈短时显示。
- **图鉴**：新增作物/变异切换、等级与进度、当前加成、星级和解锁状态；使用按窗口宽度组织的虚拟网格，滚动由虚拟列表拥有。
- **宠物**：读取后端 `owned`、`active`、`activatable`，修正此前错误的激活/上场门控；补充守护剩余/最大时间、礼包数量、狗粮库存和可编辑数量（1–99）。守护记录使用 GPUI Kit 对话框，时间戳按秒处理。Escape 只关闭最上层对话框，由组件恢复此前焦点。
- **加载与状态**：农场各接口独立收集成功/错误结果，一项失败不会丢失其它快照；无账号、离线和当前分段失败均显示对应状态并隐藏不可用业务控件。修复战令快照存在时误覆盖其它活动分段的渲染条件。
- **主题与资源**：增加产品浅深主题，使用系统字体、灰阶表面、蓝色强调和统一圆角；切换外观后保留产品令牌。资源适配器支持 `/game-config/`、开发及包内 `/activity-assets/`、HTTP(S) 外链和 `data:` 图像（base64/百分号编码），本地路径阻止越界；加载失败统一缺图呈现，活动视图复用同一适配器。修正包内 TSDK 位于资源根目录而源码位于 `assets/` 的查找差异。
- **验证结果**：本轮完整 `cargo test --workspace` 通过（GPUI 101、core 1042、app 16、desktop 7 项及 1 项本机微信集成测试；另 1 项本机微信集成和 4 项 doctest 按原配置忽略）。此前 `corepack pnpm typecheck && corepack pnpm build` 通过，本轮没有修改 Vue。最终新增分析等级初始化回归及格式/Clippy 严格检查结果见后续补充；不能将测试窗口等同于本机实窗。

#### GPUI 验收清单（继续实施，全功能等价尚未验收）

| 页面/能力 | 本轮与已有自动覆盖 | 剩余验收 |
| --- | --- | --- |
| 总览 | 状态/资产/计划专用视图、实时日志筛选回归；筛选项、事件名、卡片标题、计时状态和数字字符串快照均有中英文窗口/单测覆盖 | 真实在线/掉线/重连状态、长日志与滚动实窗验收 |
| 我的农场 | 六个分段按需加载；切换分段恢复 Vue 初次挂载默认状态；互动道具与宠物操作按各自数据范围刷新；土地合并网格/倒计时、仓库分类/选择/操作门控、只读礼包任务（状态文案优先级及成长任务状态色按 Vue）、图鉴切换、宠物门控/数量/记录、互动道具校验；礼包和成长任务所有可见字段已与 Vue 源码逐项对照 | 真实收获/种植/清理/升级、出售/使用/宠物操作、真实任务服务数据组合及授权失效 |
| 好友 | 展开/折叠与地块倒计时、状态/排序/分页（含页码直跳）、偷取/帮助/捣乱确认、黑名单/删除、访客筛选和互动道具入口均有实现；访客动作类型按 Vue `Number()` 转换，好友/访客头像复用图片协议适配；互动道具成功后仅重载道具清单；范围标签按 Vue 的搜索结果显示起止范围，并使用原始好友数作为总数；好友金币零值和宠物徽标门控按 Vue 模板显示；主要控件、确认、反馈和好友农场详情支持中英文，英文操作有窗口回归 | 真实服务数据下的操作成功/失败、窄窗口详情和弹层关闭后的焦点恢复实机验收 |
| 活动 | Vue 支持的玩法键（千星游记四分段、七夕、青梅、天气、公益、萌宠、秋日祈愿/快乐）均分流到专用视图；不支持的目录项禁用；活动目录支持进入详情和返回，战令分段隔离；萌宠日志折叠默认值、昵称和数字 GID 前缀建议项有窗口回归 | 全玩法逐字段与操作码验收、活动未开放/余额不足/规则/奖励实测；早期记录中的“玩法仍用通用行”已过期，当前代码没有已知支持玩法依赖通用行呈现 |
| 收益分析 | 策略对比与可搜索多选黑名单；黑名单即时写入/不重载、全部加入/清空/逐项移除和成功反馈；当前作物摘要的策略标签、作物等级/季数信息不重复显示，按 Vue 字段组织；五种指标、参考等级门控、缺失/未知等级、无效指标、并列排序、黑名单名称/编号/缺失种子回退均已与 Vue 源码逐项对照；关键数据接受数字字符串 | 真实账号配置读写、等级同步及在线状态验收 |
| 商城/神秘商店 | 普通/SVIP 商品卡、数量/确认、星砂分类与神秘入口；神秘商店库存/折扣/原价/余额/活动时间、余额/库存门控和购买确认有窗口回归；商城状态、固定说明和购买确认支持中英文 | 真实后端余额、购买成功/失败结果，无账号/离线状态的实数据验收 |
| 设置 | Vue 标签/默认页、自动化开关与施肥配置、种植/回退策略选项对照；指定种子搜索选择、已选值恢复和基于等级/收益排序的策略选种预览有窗口/单测；自动化、提醒和运行环境配置可写入共享 store 并在新快照中读回；无账号时账号专属页签提示选账号，全局提醒和系统配置仍可加载与保存；安静时段 `continue_farm` 按 Vue API 适配为 `continueFarm`；静态业务标签、表单说明和校验前缀按中英文显示 | 真实账号在线设置保存读回、各门控/阈值边界、提醒实际发送/QQ Bot 绑定/授权过期实测；服务端动态错误详情保持原文 |
| 游戏配置 | 种子搜索 name/seedId、果实搜索 name/id/plantId、道具搜索 name/id 与 Vue 字段范围一致；筛选、分页、单行及批量删除确认、原生图片选择均有实现；种子/果实/道具表单字段齐全，果实收获数量可编辑；各类型表单和共享数据目录读回有窗口回归 | 真实用户数据下增改删、图片上传后的应用重启互读、错误恢复实测 |
| 账号 | 账号行启停去重、按 Vue 的名称/QQ/备注字段搜索、筛选/表单、登录输入解析与在线时间回归；粘贴登录 URL 后即时识别平台并显示 `platform/os/ver` 中英文提示；账号页字段、验证、微信登录状态、错误提示、删除确认和页面摘要支持中英文；单项/批量删除共用唯一确认框并有窗口回归 | Code/扫码/本机授权真实闭环、真实后端成功/失败反馈、多账号长列表和实窗焦点验收 |
| 桌面壳 | 共享锁与重复启动唤起、侧栏宽度拖动持久化、应用菜单与托盘菜单共用 Action、全局快捷键、关窗进托盘/无托盘退出、退出引擎停机和订阅清理均有实现及相关窗口/单元覆盖 | macOS 原生菜单和托盘实机检查；Linux GTK 托盘线程与 Windows/Linux 打包需在对应 runner 验证；引擎停机、锁释放和恢复窗口的实机生命周期验收 |
| 全局 | 导航及共用工具栏、应用/托盘菜单、共用图片错误状态、页面摘要、总览、好友和账号页业务文案中英文回归；后台任务异常前缀在账号启停、账号删除、通用页面加载和商城购买等路径按语言显示；业务目录归属重构 | 活动、农场、分析、商城、游戏配置等动态业务文案完整双语复核；浅深主题、中文输入、1100×720、125%/150% 缩放、长名称和弹层实窗 |
| 交付 | 三平台 CI 与打包脚本已有 | CI 实际运行结果及 macOS/Windows/Linux 分别记录实机验证；本轮未完成三平台验收 |

### 2026-10-07 — GPUI 活动目录与玩法详情交互对齐

- 活动页现在先显示 Vue 风格的活动目录卡，标出玩法支持状态和起止时间，并按进行中、未开始、已结束排序；选择受支持活动后进入对应详情，提供返回目录入口。千星游记的旅程、星座、星砂商店和节气改为详情内标签，避免把玩法标签误当成活动目录。
- 活动详情按 `detailTarget` 选择玩法分段；无目标的千星游记默认进入旅程。游戏数据缺少目录记录时保留 Vue 页面可进入的天气/已知活动入口。该实现独立在 `desktop/activities/directory_view.rs`，目录快照映射、加载和具体玩法仍分属各自模块。
- Escape 快捷键继续让 GPUI Kit 弹层先关闭最上层 sheet/dialog，活动返回和紧凑详情键盘操作有窗口回归覆盖。修复好友安静时段测试并发修改共享配置导致的用例争用。
- GPUI crate 现有 294 个 Rust 文件、约 30.8k 行；按账号、农场、好友、活动、分析、商城、设置、游戏配置、运行时、映射和测试拆分。窗口组合入口不承载业务实现。最大文件是 567 行的活动 UI 测试；生产代码中较大的页面模块仍独立于各业务目录。文件拆分只说明代码组织，不证明页面等价完成。
- **验证通过**：`cargo test --workspace -- --test-threads=1`（GPUI 109、core 1044、app 16、desktop 7；本机微信集成用例 1 项通过、另 1 项按需忽略；4 项 doctest 按仓库配置忽略）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check`、`pnpm -C desktop-ui typecheck` 和 `pnpm -C desktop-ui build`。真实窗口、所有后端操作及 Windows/Linux 实机仍待验，当前不能宣称全功能已等价。

### 2026-10-07 — GPUI 活动路由按 Vue gameplayKey 优先级分流

- 对照 `desktop-ui/src/views/farm/activity/index.vue` 的 `resolveGameplay/openActivity` 修正目录目标优先级：非空 `gameplayKey` 决定玩法，千星游记再依据 `detailTarget` 选择内部分段；只有玩法键缺失时才按目标回退。未知玩法键不再被冲突目标、普通 ID 或名称误识别为可用玩法。
- `autumnWish` 与 `autumnHappy` 共用 Rust 活动分段，但目录入口分别打开祈愿/快乐模式；快乐模式的目录首次加载完成后读取当前“领取我的/我领取的”日志。目录同状态条目按开始时间升序，日期文案对齐 Vue 的本地日期和单边起止状态。
- 游戏配置对照确认三种表单字段、搜索/筛选/分页、批量删除和原生图片选择均已有实现；果实关联植物与字段有窗口测试。本轮把同步表改为区分代码实现和需真实数据往返/重启验证的事项，不把未实测描述成缺失实现。
- **验证通过**：GPUI 全量 113 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check`。全功能真实后端操作、实窗和三平台验收仍待完成。

### 2026-10-07 — GPUI 种植策略选择交互对齐

- 对照 `desktop-ui/src/views/farm/settings/index.vue` 的 `strategyOptions`、`bagFallbackStrategyOptions` 和 `preferredSeedSelectOptions`，补上此前缺失的“等级优先”；主策略和背包回退策略改为 GPUI Kit 选择器，回退策略选项与 Vue 的六项一致。
- 指定种子不再依赖顶部页面搜索加前 12 个按钮，改为独立可搜索选择器；显示等级与名称、保留“自动选择”，并按 Vue 的 locked/soldOut 状态禁用不可选项。加载设置快照后恢复策略、回退策略和种子选中值，选择仍写入未保存草稿。
- **验证通过**：策略选项和种子 DTO 映射单测；窗口测试核对已选项恢复，并通过下方向键 + Enter 选择另一种子且写入草稿；该项完成时 GPUI 全量 116 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`、格式检查和 `git diff --check` 均通过。
- 真实账号保存后读回、所有阈值和门控边界、离线提醒实际发送/QQ Bot 绑定，以及实窗和三平台验收仍待完成；本次不据此宣称设置页或整个客户端已完全等价。

### 2026-10-07 — GPUI 收益分析并列排名顺序对齐

- 对照 Vue `getStrategyBestPlant` 的稳定降序排序，修正 GPUI 策略比较在指标相同时选择最后一项的问题；现在保留分析快照中的首个同分作物。缺失指标继续按 Vue 的 `-1` 回退值参与比较。
- 增加同分保序和缺失指标回退测试，避免排名相同或字段为空时分析卡片显示不同作物。
- **验证通过**：GPUI 全量 118 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`、格式检查和 `git diff --check`。完整真实数据排序、策略写入和其它页面验收仍继续逐项进行。

### 2026-10-07 — GPUI 种植黑名单多选交互对齐

- 对照 Vue 分析页的 `NSelect multiple filterable clearable`，将常驻复选框列表改为 GPUI Kit 可搜索多选组合框；选项按等级、作物名和 ID 标注，已有但不在当前排名中的黑名单 ID 仍可显示和移除。
- 每次选择即时调用 app 门面保存账号黑名单，并更新当前分析快照与已选项列表，不再重载整页；“全部加入”“清空”和逐项移除也统一走同一写入路径，刷新/切换账号后从后端快照恢复选中值。
- **验证通过**：选项映射、未知 ID 保留、组合框配置值恢复，以及多选变更事件写入本机 app 配置且不改变页面请求世代的回归；后续仍需在真实窗口检查弹层列表的键鼠操作，并完成真实账号数据读回及剩余页面对照。

### 2026-10-07 — GPUI 神秘商店商品卡与购买门控对齐

- 对照 `desktop-ui/src/views/farm/mystery-shop/panel.vue` 补齐专属商品卡：展示库存、货币与余额、折扣/原价、起止时间；商人不活跃时显示空状态，不暴露商品购买按钮。
- 购买按钮保持 Vue 的“购买”标签，并在库存不足、已知余额不足或刷新期间禁用；确认弹层标题和购买摘要对齐神秘商店流程，取消或门控失败不会发起购买。
- **验证通过**：窗口回归覆盖商品/刷新入口、购买确认、余额不足时点击无效，以及商人停用后隐藏购买按钮。真实后端余额与购买结果仍待实测；好友和其他页面继续逐项对照，当前仍不能宣称全功能等价。

### 2026-10-07 — GPUI 好友访客记录数值转换对齐

- 对照 Vue 对访客 `actionType`、GID、等级和服务器时间的 `Number(...)` 转换，GPUI 访客类型筛选与字段映射现在也接受数字字符串；此前字符串类型的记录会在筛选时消失，或缺少等级/时间字段。
- **验证通过**：过滤器覆盖数字、数字字符串、空白数字字符串及非法值；访客映射回归使用字符串 GID/等级/动作类型/毫秒时间戳，确认动作标签与字段保留。好友真账号操作和窄窗口交互仍待实机验收。
- 本轮最终回归：`cargo test -p qq-farm-gpui --all-targets -- --test-threads=1`（123 项）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check` 均通过。未据此宣称所有页面已完全等价。

### 2026-10-07 — GPUI 好友标签按需加载与页面组合按业务拆分

- 好友页的访客记录和互动道具改为在各自标签打开时请求，匹配 Vue 的懒加载行为；好友列表和黑名单仍先加载。标签请求失败只影响当前分段，清空该分段的旧数据并显示错误提示，重试成功后清除错误；账号/页面切换仍通过版本校验丢弃过期结果。
- 页面组合层进一步按业务拆分至 `desktop/view/content/personal.rs`、`activities.rs`、`commerce.rs`，可见状态集中在 `state.rs`；`content.rs` 保留共享布局、工具栏、页面导航、加载/错误/空态装配。具体业务视图仍归属 `desktop/personal/`、`activities/`、`friends/`、`commerce/` 等目录。
- GPUI crate 当前 298 个 Rust 文件、约 31k 行，账号、农场、好友、活动、分析、商城、设置、游戏配置、运行时、资源、数据映射和测试均按职责分层。入口文件只负责启动和组合，不承载整套业务实现；文件数仍不作为功能等价证明。
- **验证通过**：GPUI 全量测试 127 项；`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真账号操作、真实窗口和 Windows/Linux 实机验收仍待完成。直接对 workspace 依赖运行 Clippy 会遇到未修改的 `qq-farm-core` 两条 lint，因此新 crate 使用 `--no-deps` 验证。

### 2026-10-07 — GPUI 活动默认路由与目录状态时钟对齐

- 对照 Vue 活动页默认的活动目录，修正 GPUI 从侧栏进入、启动恢复和切换账号后的默认分段；原先空分段列表会回退到通用 `all`，绕开目录卡片和活动详情导航。
- 活动目录可见期间每秒触发状态重算，使“即将开始/进行中/已结束”和排序在起止边界更新；离开活动页或切换到玩法详情时使计时任务失效，返回目录后重新启动。
- 增加窗口回归，验证侧栏进入活动页直接呈现目录卡；增加起止边界测试。该修复不替代剩余活动字段、在线操作与实机验收。
- **验证通过**：`cargo test -p qq-farm-gpui --all-targets -- --test-threads=1`（129 项）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy --no-deps -p qq-farm-gpui --all-targets -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实游戏数据及实窗验收继续进行。

### 2026-10-07 — GPUI 业务页面文件边界与主题预览修正

- crate 已按账号、总览、农场、好友、活动、分析、商城、设置和游戏配置划分业务目录；本轮继续把复合视图拆到具体职责：活动目录的模型/时钟/导航/视图、每日礼包状态标签、土地卡片的作物/效果/状态徽标、商城与神秘商店独立视图、收益策略的指标/作物/比较卡。结构导航集中维护在 `docs/ARCHITECTURE.md`，GPUI crate 当前有 311 个 Rust 源文件；不为增加数字而拆空模块。
- 修正预览主题被主机系统外观覆盖的问题：每个截图场景把 `ThemeMode` 映射到对应 UI 偏好。重新生成的账号、农场和活动目录浅深主题图像哈希均不同；目视检查账号深色图确认深色表面和文字已生效。此项只修正预览初始化，不代表本机窗口/其它系统主题已实机验收。
- **验证通过**：`cargo test -p qq-farm-gpui --all-targets -- --test-threads=1`（129 项）、`cargo check --workspace --all-targets`、`cargo clippy -p qq-farm-gpui --features ui-preview --no-deps -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check` 和 18 张 GPUI 预览图生成。业务操作等价、真实窗口以及 Windows/Linux 验收仍按清单逐项完成；当前不能宣称全功能完全等价。

### 2026-10-07 — GPUI 设置自动化保存后的化肥检查对齐

- 对照 Vue 设置页 `saveAutomation`：保存自动化设置成功后，如果启用了普通或有机化肥购买，立即调用 app 门面执行一次化肥检查。检查结果既读取外层调用错误，也读取业务返回 JSON 的 `error` 字段；存在错误时显示 Vue 对应的警告文案，否则显示保存成功反馈。
- 新逻辑落在 `desktop/settings/save.rs`，状态与提示仍由设置业务模块维护；增加购买开关门控和错误字段解析单测。GPUI crate 仍按业务目录分层，当前 311 个 Rust 文件；单次业务修复触及文件少不意味着整套业务挤在单文件。
- **验证通过**：GPUI 全量测试 131 项；该次窗口测试因 GPUI 确定性调度器不支持由 Tokio 外部线程驱动而移除，改为纯分支/返回值测试。真实账户自动化保存和实际购买结果仍待在线验收，不据此宣称设置页或全客户端已完全等价。

### 2026-10-07 — GPUI 微信授权后启动兜底对齐

- 对照 Vue 的本机微信快捷授权和扫码授权保存流程，在 GPUI 两条授权路径的账号 upsert 成功后都显式调用 `start_account`。core 对已运行 worker 的启动调用是幂等的；这补上 app 层新增账号隐式启动失败未返回错误，以及编辑账号后凭证未变化、原 worker 已停止时缺少启动兜底的情况。
- 新账号从 app 返回账号清单中按 code/platform 定位；编辑时优先使用原账号 ID。启动失败时关闭已成功保存的表单并在账号页显示警告；凭证写入失败仍保留表单并显示原错误。账号 sheet 补回 Vue 中可见的备注文本框；现有 Vue IPC 适配器也未把备注传给 app，所以与 Vue 一样，该输入目前不持久化。
- **验证通过**：账号 ID 解析和账号 sheet 键盘回归；GPUI 全量测试 132 项；`cargo check --workspace --all-targets`、GPUI Clippy（`--all-targets --no-deps -D warnings`）、`cargo fmt --all -- --check` 和 `git diff --check`。微信真实登录、启动成功/失败反馈仍需在线后端验收。

### 2026-10-07 — GPUI 总览实时日志跟随行为对齐

- Vue 总览会在用户停留在日志底部时跟随新日志；用户向上查看历史记录后暂停自动滚动，返回底部后恢复。GPUI 增加对应滚动状态、页面首次载入/清空后的底部定位，并在追加实时事件时仅于底部附近跟随；原有 300 条显示上限和展开历史日志行为保留。
- 增加滚动位置判定单测，覆盖向上查看时暂停、向下回到底部时恢复和无溢出内容三种状态。真实窗口内的触控板/滚轮手感仍需实机验收。
- **验证通过**：`cargo test -p qq-farm-gpui --all-targets -- --test-threads=1`（133 项）、`cargo check --workspace --all-targets`、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。总览在线长日志及滚轮/触控板仍未完成真实窗口验收，当前不能据此宣称全客户端完全等价。

### 2026-10-07 — GPUI 活动操作反馈与忙碌状态对齐

- 对照 Vue 活动页的 `pendingKey` 和操作成功处理：活动操作现在按按钮/目标显示忙碌态，不再锁住整个玩法页；萌宠日记保留 Vue 的单操作互斥规则，同时允许继续展开和收起页面分区。离开账号、页面或玩法时清理旧操作令牌，迟到结果不会写入新视图。
- 成功后读取后端返回的活动快照并合并到现有页面数据，保留萌宠日记等其它业务快照；秋日操作直接更新对应玩法数据，秋日日志继续按所选标签更新。操作结果现在显示奖励/成功反馈；公益星级领取等“无可领取内容”结果使用信息态反馈。快照缺失时保留刷新兜底。
- 增加回归覆盖：公益捐赠确认在请求忙碌时不可重复提交；萌宠日记操作进行中仍可交互；活动结果消息、无奖励信息态、快照合并及跨页数据保留。
- **验证通过**：GPUI 全量测试 135 项、工作区 `cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、格式检查和 `git diff --check`。活动全玩法字段/操作码与真实后端结果仍需逐项验收，当前不据此宣称全功能等价。

### 2026-10-07 — GPUI 天气与商城请求状态按操作隔离

- 天气瓶操作改为按操作和目标维护忙碌状态：召唤、兑换、各研究节点及不同好友扫描互不锁定；同一操作/目标仍防止重复提交。页面或账号切换会清空旧令牌，迟到结果继续由请求版本丢弃。
- 商城购买只让确认按钮进入加载态，确认弹层保持打开直到后端成功；失败时保留待购商品和弹层以便修正/重试，成功后才关闭弹层、显示短暂反馈并刷新。购买数量继续限制在 Vue 的 1–99 范围。
- GPUI crate 现有 312 个 Rust 文件、约 32k 行。`desktop/` 下按账号、活动、分析、商城、总览、好友、游戏配置、农场、运行时、设置、视图和测试分目录；活动 94 个文件、农场 39 个。文件量和拆分只描述代码组织，不证明业务等价。
- **验证通过**：GPUI 全量测试 137 项、`cargo check --workspace --all-targets`、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check`。真实天气/商城后端成功失败、实窗体验和 Windows/Linux 验收仍待完成，当前不能宣称全功能完全等价。

### 2026-10-07 — GPUI 账号表格列配置与固定区对齐

- 对照 `desktop-ui/src/components/advanced/table-column-setting.vue` 补回账号表格列管理：可单独或全量显隐、调整顺序、循环设为固定左/右或取消固定。固定列由独立左右区域承载，中间未固定列单独横向滚动；列状态保存在账号页状态，与 Vue 的页面会话状态一致。
- 列设置弹层支持 Escape 收起，账号页切换时清除打开状态；中英文覆盖列标题、设置入口、固定动作和键盘辅助标签。
- **验证通过**：GPUI 全量测试 140 项；账号窗口回归覆盖显隐、上移、固定左侧、全选/取消全选与 Escape；`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy、格式和 diff 检查均通过。固定列在真实窗口的横向滚动、窄宽度与不同缩放仍需实窗测量。

### 2026-10-08 — GPUI 导航按职责拆分与账号分页行为对齐

- 在已有账号、农场、好友、活动、商城等业务目录基础上，将原 336 行导航聚合文件拆为 `desktop/navigation/page.rs`、`section.rs`、`account.rs`、`preferences.rs` 和 `sections.rs`；窗口层仍只编排页面生命周期，业务页面状态和具体操作继续留在各自目录。GPUI crate 当前 319 个 Rust 源文件，避免把“文件数量”当成业务等价证明。
- 对照 `desktop-ui/src/hooks/common/table.ts` 的分页配置，账号页补齐 10/15/20/25/30 条每页选项，并支持键盘上下选择与 Enter 确认；选项变更回到第一页。分页呈现在表格裁剪区域之外，菜单可以正常展开。
- **验证通过**：GPUI 全量 141 项测试（含账号分页窗口回归）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all --check` 和 `git diff --check`。其它页面仍需逐项做 Vue 可见行为与门控核对，账号真实后端操作、真实窗口布局和跨平台实机验收仍待完成；当前不能宣称全客户端完全等价。

### 2026-10-08 — GPUI 活动参数和交互回归按玩法拆分

- 活动操作公共入口只保留请求编排、版本校验和结果处理；青梅配方库存校验、七夕好友/香囊数量校验、秋日祈愿选项校验分别归入 `desktop/activities/actions/inputs/{qingmei,qixi,autumn}.rs`，仍向已有玩法 handler 发送相同参数。
- 原活动窗口测试按目录、秋日、萌宠、青梅、七夕、赛季/节气/星座拆成 `desktop/tests/activities/` 六个模块；农场测试也按礼包、农场主体、仓库、互动道具、图鉴和宠物拆入 `desktop/tests/personal/`。后续新增交互可直接落在对应玩法文件，不再继续堆入横向总文件。结构索引同步更新到 `docs/ARCHITECTURE.md`。
- 当前 crate 有 335 个 Rust 源文件，最大的业务交互测试文件从 620 行拆到不超过 157 行的玩法模块；全量测试 144 项通过，农场测试拆分后的 11 项定向回归通过。`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy、格式检查和 `git diff --check` 均通过。拆分只说明代码边界更清楚，真实后端、实窗和跨平台验收仍待完成，不能据此宣称全功能等价。

### 2026-10-08 — GPUI 好友搜索与同步进度对齐

- 好友和黑名单搜索现在只按昵称/GID 匹配，并去除关键词首尾空格，避免误搜金币或宠物字段；区间提示始终显示，区间按过滤后的正常好友计算、总数仍按好友快照计算，中文和英文文案对齐 Vue。宠物同步进度也会按数值规范化黑名单 GID，避免字符串 GID 被错误计入分母。
- 增加好友页窗口回归和范围/搜索/宠物同步单测，覆盖名称/GID 搜索、其它字段不匹配、黑名单字符串 GID 排除及含黑名单的总数文案。
- **验证通过**：GPUI 全量 148 项测试，`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check`、`git diff --check`。好友真账号数据及真实窗口缩放、键鼠仍待验；这只关闭好友页这几项差异，不代表所有页面已完全等价。

### 2026-10-08 — GPUI 业务目录说明与长列表网格虚拟化

- 当前 GPUI 源码不是单文件实现：334 个 Rust 文件分布在账号、农场、好友、活动、分析、商城、设置、游戏配置、运行时、页面映射和交互测试目录；最大生产文件为账号表格 375 行。`docs/ARCHITECTURE.md` 维护了按业务目录的职责索引，便于从页面入口定位实现。
- 新增 `desktop/view/grid.rs` 统一响应窗口宽度计算网格列数及卡片行分组；商城/配置业务网格和超过 80 项的仓库列表使用虚拟列表。仓库筛选只生成一次源数据索引，滚动时按行创建卡片，原有搜索、分类、隐藏系统物品与操作编号保持不变。
- **验证通过**：GPUI 全量 151 项测试、120 项仓库卡片的窗口虚拟滚动回归、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。这轮处理代码职责可发现性与长列表渲染，不代表各页面业务已全部等价；真实后端、实窗和 Windows/Linux 验收仍继续逐项完成。

### 2026-10-08 — GPUI 活动目录缺少玩法键时按 Vue 回退

- 对照 `desktop-ui/src/views/farm/activity/index.vue::resolveGameplay/displayActivities`，补上只有 `detailTarget`、没有 `gameplayKey` 的兼容分支：已知目标进入对应分段，非空未知目标回退到千星游记；明确未知 `gameplayKey` 仍禁用。千星兜底现在按 Vue 的 season/shop/constellation 标题与 solar terms 列表判定，并固定回到 travel；青梅要求 `known && active`，七夕/公益兜底和已有目录项去重也采用 Vue 相同字段。
- 新增回归覆盖未知详情目标、未知玩法键、千星兜底目标、节气列表可用条件，以及七夕/青梅/公益目录兜底门控。活动玩法真实字段/操作码及状态门控仍待在线验收，这些路由修复不代表活动页或全客户端已完全等价。
- **验证通过**：GPUI 全量 154 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。未改 Vue，实窗、真实后端和 Windows/Linux 验收仍按页面清单继续。

### 2026-10-08 — GPUI 账号页双语和删除确认交互对齐

- 账号页筛选器、分页、列设置、表格状态、表单字段、Code/微信登录 sheet、验证提示、扫码/本机授权状态、常见错误和删除确认支持中英文；切换语言会同步更新页面搜索、账号名称、登录 code 和备注占位符。共享工具栏按页面语言显示摘要，动态后端摘要原样保留。
- 删除确认框现在从账号页分支实际挂载，单项和批量删除使用同一确认框，避免列表内提示与通用提示叠加或批量操作没有确认弹层。账号文案映射放在独立的 `desktop/accounts/messages.rs`，页面摘要映射放在页面模型。
- 当前 crate 有 338 个 Rust 源文件，按账号、农场、好友、活动、分析、商城、设置、游戏配置、运行时和页面映射等业务边界组织；`git status` 将整个未跟踪 crate 折叠显示为一个目录，不代表只有一个源文件。
- **验证通过**：GPUI 全量 158 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。Code/扫码/本机授权真实闭环、真实后端操作以及其他页面双语、实窗和跨平台验收仍未完成；本条不表示全功能已经等价。

### 2026-10-08 — GPUI 活动路由归入目录业务模块

- 活动目录的玩法映射和秋日模式选择从通用 `activities/helpers.rs` 移到 `activities/directory/routing.rs`；`activities.rs` 等入口文件继续只做模块组合和对外导出，具体页面和操作留在各自业务目录。
- GPUI 状态现在保留最近选择的千星游记分段。目录卡目标缺失或未知时沿用该分段，`travel`、`constellation`、`shop`、`solar` 等有效目标仍按 Vue 映射覆盖；窗口回归验证记忆分段和明确目标两条点击流程。
- GPUI crate 当前有 339 个 Rust 源文件，按账号、农场、好友、活动、分析、商城、设置、游戏配置、运行时、映射和测试分目录。Git 对整个新增未跟踪 crate 只显示一个目录项，这是状态展示折叠，不是源码合并。
- **验证通过**：GPUI 全量 160 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。这只验证本轮活动目录路由；真实后端业务、其它页面逐项核对、实窗及 Windows/Linux 验收仍未完成，不据此宣称全功能已经等价。

### 2026-10-08 — GPUI 活动卡片和详情导航交互对齐

- 对照 Vue 活动目录的语义按钮，将仅“查看详情”可点的 GPUI 卡片改为整张可点击、可聚焦的按钮；不支持的玩法保持禁用。窗口回归从卡片右下方空白处触发，确认整卡命中后进入相同玩法。
- 千星详情页按 Vue 计算展示名称、四个分段标签、可领取提示点、商店余额和剩余时间；倒计时在活动详情打开时也持续更新。切换详情分段不再清除进行中的操作标记、成功反馈或变更页面版本，因此不会提前解锁重复提交，也不会丢弃仍有效的响应。
- GPUI crate 当前有 340 个 Rust 源文件，按账号、农场、好友、活动、分析、商城、设置、游戏配置、运行时、数据映射和测试分目录；本 crate 最长生产文件为账号表格 426 行。`git status` 把未跟踪 crate 折叠为一个目录项，只是状态展示方式。
- **验证通过**：`cargo test --workspace -- --test-threads=1`（GPUI 164 项、core 1044 项及其余 workspace 测试）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check`、`corepack pnpm@12.3.4 -C desktop-ui typecheck` 和 `corepack pnpm@12.3.4 -C desktop-ui build`。活动真账号流程、其他页面完整逐项对照、真实窗口与 Windows/Linux 验收仍未完成；当前不能宣称全功能已等价。

### 2026-10-08 — GPUI 总览双语与业务源码收录修复

- 总览页补齐账号/资产/运行计划状态、日志事件名、筛选项、操作统计及清空/展开文案的中英文；页面语言切换同步更新日志搜索占位符。等级、资产和操作计数同时接受 JSON 数字与数字字符串，避免字符串快照被显示为 0。
- GPUI crate 现有 340 个 Rust 源文件，按账号、总览、农场、好友、活动、分析、商城、设置、游戏配置、运行时、页面映射和测试拆分；账号页又分成列、筛选、表格、分页、行操作、表单、授权和删除模块。此前通用 `.gitignore` 的 `logs/` 规则会隐藏 `desktop/dashboard/logs/` 中的两个 Rust 文件，现已加精确例外，crate 源码均能由 Git 正常枚举。
- **验证通过**：GPUI 全量 170 项测试、GPUI Clippy `--all-targets --no-deps -D warnings`、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`、`corepack pnpm@12.3.4 -C desktop-ui typecheck` 和 `corepack pnpm@12.3.4 -C desktop-ui build`。真实在线/掉线/重连、长日志实窗滚动、其他页面完整双语和跨平台实机仍待验；本轮不代表全客户端功能已经等价。

### 2026-10-08 — GPUI 好友页主要操作双语

- 好友页的刷新、访客筛选、分页、批量偷取、互动道具表单、行操作、删除/屏蔽确认、反馈状态、好友农场详情和土地状态按界面语言显示；切换语言时同步更新好友 GID 与地块输入提示。服务端返回的动态消息继续原样展示。
- 新增英文窗口回归和状态/奖励数量格式单测，覆盖导航分段、刷新、分页、偷取操作、好友输入占位符及好友农场状态文案。好友的真实服务端成功/失败和窄窗口详情仍待实机验收。
- **验证通过**：好友页窗口回归 8 项、GPUI 全量 172 项测试、GPUI Clippy `--all-targets --no-deps -D warnings`、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check` 和 `git diff --check`。分析、商城、设置、游戏配置与活动/农场页面的完整双语、实机交互和跨平台验收仍在逐项继续；这轮不代表全客户端已完全等价。

### 2026-10-08 — GPUI 收益分析语义、数值和双语回归

- 对照 Vue 分析页后修正黑名单说明：它控制自动偷菜时跳过的作物，不影响账号自己的种植。黑名单选项、移除行、账号等级、种子 ID、等级和比较指标均兼容 JSON 数字字符串；策略对比恢复当前策略、五项比较说明与单位，并提供中英文页面文案和等级占位符。
- 黑名单逐项移除、全部加入/清空和多选仍即时写入本地账号配置而不触发整页刷新；操作完成后显示短暂的成功状态。新增窗口回归覆盖英文控件、字符串 ID 选中/移除、真实配置更新及策略比较区显示。
- **验证通过**：分析页定向窗口测试 4 项、GPUI 全量测试 176 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。排名字段完整对照、真实账号配置读回、在线/离线及完整分析页实窗验收仍待完成；活动、商城、设置、配置和农场等页面也仍有逐项核对及完整双语工作，本轮不代表全客户端已完全等价。

### 2026-10-08 — GPUI 商城和购买确认双语回归

- 游戏商城、神秘商人、商品状态、库存/余额/限购/原价说明、无账号与空状态、数量输入、兑换/购买确认和服务端门控说明现在随中英文切换；服务器返回的商品、货币、折扣与不可用原因保持原文。
- 新增英文窗口回归，验证普通商品与已领取状态、神秘商店刷新/购买操作、购买确认/取消按钮和数量提示。现有购买门控、数量区间和失败后保留确认弹层语义不变。
- **验证通过**：商城定向窗口测试 6 项、GPUI 全量测试 177 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。其余页面的完整中英文、真实后端购买成功/失败和跨平台验收仍未完成；当前不能宣称全功能已完全对齐。

### 2026-10-08 — GPUI 游戏配置抽屉与表单边界对齐

- 游戏配置的新增/编辑表单改用 GPUI Kit Sheet，对齐 Vue 的 480px 侧边抽屉；使用 Sheet 自带滚动区，避免嵌套滚动。字段宽度按抽屉内容区收敛，果实植物选择器改为输入后显示最多 8 个匹配结果，不再默认横排首批植物。
- 新建种子的土地等级默认值改为 Vue 的 1；果实编辑产量按 Vue 的表单模型固定为 200。种子/果实的数值写入补上非负校验，避免文本输入绕开 Vue 数字控件的下限。
- **验证通过**：游戏配置窗口回归覆盖 Sheet 打开/关闭、种子默认值与果实新增/编辑状态；GPUI 全量 180 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings` 和 `cargo fmt --all -- --check`。真实图片上传、配置重启读回和其他页面的逐项门控仍待实机/真后端验收，本条不代表全客户端已经完全等价。

### 2026-10-08 — GPUI 系统配置表单补齐

- 对照 Vue 设置页补齐运行环境字段：设备预设、网关 URL、时区、QQ/微信平台、Windows/iOS/Android 系统、客户端版本、系统软件、设备 ID、内存、网络和 User-Agent。编辑字段进入系统配置草稿；保存时将设备信息中的系统和客户端版本同步到后端要求的顶层字段。
- 首次未保存系统配置时向 GPUI 页面提供 core 默认值；设备预设应用时同步设备字段和顶层字段。系统配置表单放在 `desktop/settings/system_config.rs`，与设置草稿、保存和其它设置页职责分开。
- 新增窗口回归验证文本编辑、平台/时区/系统选择及设备预设对草稿和输入状态的更新，并确认系统配置不再在专用表单之外重复显示为通用行。**验证通过**：GPUI 全量 182 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实数据目录的保存/重启读回、其它页面逐项门控、实窗与 Windows/Linux 验收仍待完成，当前不能宣称全功能已完全等价。

### 2026-10-08 — GPUI 源码结构核对与设置双语补齐

- 核对新 crate 的实际源码目录：当前 341 个 Rust 文件，按账号、总览、农场、好友、活动、分析、商城、设置和游戏配置等业务模块组织；`desktop/activities.rs` 等根文件只装配子模块。Git 状态将尚未跟踪的 `crates/qq-farm-gpui/` 折叠成一个目录项，完整模块树见 `docs/ARCHITECTURE.md`。
- 补齐设置页有机/普通化肥库存阈值及缺名种子的英文标签，新增选择器和设置控件双语回归。窗口测试期间锁定进程级 `FARM_DATA_DIR`，修复并行测试之间共享环境变量导致的数据目录串扰。
- **验证通过**：GPUI 全量 186 项测试、设置英文控件新增定向窗口测试 1 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。其它页面剩余文案和业务差异、真实后端/窗口及 Windows/Linux 实机验收仍在继续；当前不能宣称全功能完全等价。

### 2026-10-08 — GPUI 七夕阶段与赠送交互补齐英文状态

- 七夕页的阶段进度、阶段状态、奖励消耗、好友列表空态/搜索、收礼对象、余额提示、数量上限和刷新/赠送控件现在跟随界面语言；原有账号、库存、活动开放状态和赠送数量门控保持不变。
- 七夕窗口回归在已有搜索、选择好友和数量归一化流程后切换英文，核对领奖、赠送、全部数量和刷新按钮；活动全组窗口回归覆盖目录、秋日、萌宠、青梅、七夕、战令/节气/星座。
- **验证通过**：活动定向窗口回归 18 项、`cargo fmt --all`。`cargo test -p qq-farm-gpui --all-targets -- --test-threads=1` 全量 190 项通过，`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy、`cargo fmt --all -- --check` 和 `git diff --check` 通过；其它活动玩法的文案/后端门控、真实窗口及跨平台验收仍待完成，这不表示全客户端已完全等价。

### 2026-10-08 — GPUI 账号表格身份和页内序号对齐

- 对照 Vue 账号表格的 `row-key: row.id` 与每页从 1 开始的序号，GPUI 行勾选框及编辑/启停/删除控件改用账号 ID 作为稳定控件身份，过滤或分页后不会把位置索引当成账号身份；序号改为页内序号。
- 账号窗口回归验证稳定账号控件可选择并打开同一条删除确认；**验证通过**：账号页窗口测试 8 项、`cargo fmt --all -- --check`、GPUI Clippy。此前本轮 GPUI 全量 190 项及 workspace `cargo check` 通过；该表格微调后重新跑了账号专项回归和 Clippy。

### 2026-10-08 — GPUI 个人农场操作状态双语补齐

- 对照个人农场、仓库、宠物和互动道具视图，补齐务农工具栏操作名、批量出售校验、出售/使用默认结果、宠物动作结果、地块季数、互动道具缺名回退值及后台任务失败前缀的英文显示。保留 Vue 返回的动态商品名、描述和服务端消息原文。
- 窗口回归检查英文农场操作按钮、互动道具批量使用按钮和地块编号占位符；已有宠物与仓库窗口用例继续覆盖门控、数量、筛选及选择状态。
- **验证通过**：个人页定向窗口测试 12 项、GPUI 全量 `cargo test -p qq-farm-gpui --all-targets -- --test-threads=1`（190 项）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。这轮没有执行真实账号操作或 macOS/Windows/Linux 实机验收，也没有完成所有个人页动态文案翻译；不能据此宣称全功能已完全等价。

### 2026-10-08 — GPUI 设置页背包种子拖拽排序对齐

- 对照 Vue 设置页的 `dragstart`、`dragover`、`drop`，种子优先级卡片现在支持拖放排序；排序写入设置草稿，按 Vue 在移除源项后修正目标索引的规则插入。保留上移/下移按钮，并在中英文提示中说明两种操作方式；卡片补齐种植面积、库存、等级和种子 ID，隐藏或离线种子仍保留在配置顺序中。加载、错误、空列表状态互斥，读取失败时隐藏旧卡片。
- 将背包种子优先级控件从策略表单拆到 `desktop/settings/controls/planting/bag_seed_priority.rs`；顺序计算和草稿写入仍位于 `desktop/settings/bag_seed_priority.rs`。窗口回归验证从首行拖到第三行后草稿变为 `[20, 10, 30]`，并覆盖加载/错误/空状态互斥；单测核对中英文卡片数据字段。当前 GPUI crate 有 342 个 Rust 源文件，业务目录索引见 `docs/ARCHITECTURE.md`；Git 状态把尚未跟踪的新 crate 折叠成一个目录项。
- **验证通过**：GPUI 全量 194 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实账号设置保存/重启读回、其余页面交互差异、实窗与 Windows/Linux 实机验证仍待完成；本项修复不代表全客户端已经完全等价。

### 2026-10-08 — GPUI 公益活动状态与门控文案对齐

- 对照 Vue 的公益活动视图和中英文 locale，公益页按摘要、奖励、捐赠、展示辅助和窗口回归分在 `desktop/activities/charity/` 与 `desktop/tests/activities/charity.rs`。补齐全服奖励解锁爱心阈值、里程碑已达成/目标文案、领取中/已领取/不可领取状态，以及摘要、卡片和捐赠确认的英文文案；奖励缺名时遵循 Vue 的数字名称回退规则。
- Vue 在领取任一个人里程碑时会锁住整组奖励按钮；GPUI 之前只显示当前目标的加载状态。本次按整个公益进度领取状态统一显示“领取中”并禁用各档按钮，防止视觉状态和可操作状态不一致。窗口回归覆盖英文按钮、后台动作门控、请求中的多档同步状态与重复点击保护。
- GPUI 当前共有 343 个 Rust 源文件，测试和业务代码沿页面及能力分目录；公益业务页面入口组合 `charity/` 下各职责模块，页面窗口测试与其它活动用例并列。
- **验证通过**：GPUI 全量 197 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。公益真实后端流程及实窗仍待验；其它页面和全客户端也仍有逐项对照工作，当前不能宣称所有功能已经完全等价。

### 2026-10-08 — GPUI 秋日祈愿和快乐活动双语补齐

- 对照秋日玩法两种页面状态，把页面文案放在 `activities/autumn_view/{shared,wish,happy}.rs`：模式标题回退、活动状态、祈愿天数/次数、抽签与领取状态、快乐值统计、里程碑状态、分享与每日领取、日志标签和加载/空状态均支持中英文。后端动态活动名、选择项、规则和好友昵称仍显示服务端原文。
- 模式切换控件在 `activities/autumn_controls.rs` 使用相同本地化标签；缺少玩法快照时的刷新状态也按当前语言显示。活动窗口用例切换到英文，验证祈愿和快乐两种玩法的按钮状态及日志筛选标签。
- **验证通过**：秋日专项测试 7 项、GPUI 全量 197 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。秋日真账号操作、活动状态实窗和其它页面逐项对照仍在继续，当前不宣称全功能完全等价。

### 2026-10-08 — GPUI 游戏配置搜索字段范围对齐

- 对照 `game-config/index.vue::filterSeeds/filterFruits/filterItems`，原 GPUI 搜索会扫描 JSON 全字段，把等级、描述等 Vue 不搜索的字段也纳入结果。现在种子仅搜索名称/种子编号，果实仅搜索名称/果实编号/关联植物编号，道具仅搜索名称/道具编号；列表内容和分页计数复用同一过滤规则。
- 将规则拆到 `desktop/game_config/search.rs`，新增窗口回归验证等级值不再误命中、有效编号仍会命中，并给目录卡片设置稳定测试身份。
- **验证通过**：配置搜索单测与窗口回归各 1 项；之后 GPUI 全量测试 200 项通过，GPUI `RUSTFLAGS="-D warnings" cargo check -p qq-farm-gpui --all-targets`、Clippy、格式和 diff 检查通过。真实配置文件保存/重启读回、图片读写及其它页面业务验收仍未完成。

### 2026-10-08 — GPUI 好友农场合并地块布局对齐

- 对照 `desktop-ui/src/views/farm/friends/index.vue` 与共享 `land-display.ts`，好友详情此前按普通换行卡片排列，合并作物会丢失 4 列农场中的编号位置和跨格尺寸。现在本人农场与好友页共用 `desktop/land_grid.rs` 的地块过滤、编号及占地计算；好友地块按 4 列坐标渲染，合并占地正确跨行/列，并隐藏从属格。未识别状态、空作物名和空阶段也回退为 Vue 的“生长中”和 `-`。
- 窗口回归直接检查合并地块尺寸、后续地块编号位置和从属格隐藏；好友详情现有状态/倒计时展示保持原有行为。
- **验证通过**：GPUI 全量 200 项测试、好友详情定向窗口回归、`RUSTFLAGS="-D warnings" cargo check -p qq-farm-gpui --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。其余好友字段、真实后端和实机窗口仍需继续逐项对照；本项不代表全客户端已完全等价。

### 2026-10-08 — GPUI 游戏配置表单字段与顺序对齐

- 对照三种 Vue 配置抽屉，种子页将生长阶段模板选择放到阶段文本编辑器之前；果实页移除 Vue 表单没有的“每株收获数量”控件，同时保留 Vue 新增/编辑请求所用的固定默认值 `200`。
- 窗口回归检查新增果实抽屉不显示该额外字段，并继续验证编辑状态和默认 payload。该调整不影响种子表单的收获数量字段。
- **验证通过**：GPUI 全量 200 项测试、`RUSTFLAGS="-D warnings" cargo check -p qq-farm-gpui --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。其它配置字段与原生图片选择、保存/重启读回以及所有页面剩余交互仍须继续对照；当前不据此宣称全客户端等价。

### 2026-10-08 — GPUI 游戏配置双语与业务文案拆分

- 将游戏配置表单字段的中英文映射放入 `desktop/game_config/labels.rs`；筛选、分页、表单选项、增删确认、字段占位符和本地校验错误都跟随语言设置，切换语言时同步更新输入占位符。服务端的动态物品/植物名称和错误详情仍保留原文。
- 新增英文窗口回归，覆盖种子季数筛选、分页/新增操作、表单保存操作和生长阶段占位符；同时保留种子/果实/道具表单字段与 Vue 的增改删语义。
- GPUI crate 当前有 351 个 Rust 源文件，按业务能力分目录；`desktop/*.rs` 是小型模块入口，页面实现位于各自的 `desktop/{accounts,personal,friends,activities,analytics,commerce,settings,game_config}/` 子树。详细职责索引见 `docs/ARCHITECTURE.md`。
- **验证通过**：GPUI 全量 202 项测试、`RUSTFLAGS="-D warnings" cargo check -p qq-farm-gpui --all-targets`、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实图片上传/配置读回、完整业务中英文、真实窗口及 Windows/Linux 验收仍待继续；当前不宣称全客户端已完全等价。

### 2026-10-08 — GPUI 天气活动双语和反馈状态回归

- 天气活动的摘要、研究、物资、任务、好友可用状态、倒计时和操作反馈现在按界面语言显示；后端提供的动态名称、门控原因和规则继续显示原文。账号/页面切换后过期请求丢弃、天气好友扫描详情及操作门控保持原有语义。
- 天气窗口回归覆盖英文控件、服务端原文门控提示、好友详情返回时搜索焦点恢复，以及快照合并后不整页重载。
- **验证通过**：`cargo test -p qq-farm-gpui --all-targets -- --test-threads=1`（203 项）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实天气后端操作、全客户端剩余业务/文案差异、真实窗口与 Windows/Linux 验收仍未完成；当前不能宣称所有页面完全等价。

### 2026-10-08 — GPUI 活动反馈与配置图片资源读回

- 萌宠日记的成功提示、好友 GID 校验、夺宝门控和异步任务失败提示按中英文显示；青梅、节气、星座、七夕和秋日玩法的列表选择动作补齐英文已选/选择文案。通用内容区的加载、空状态、好友分区错误和发布页下载操作也跟随界面语言。
- 配置图片表单上传后生成的 `/api/config/images/{name}` URL 之前无法由 GPUI 解析；现在通过 app 门面读取共享数据目录的覆盖图片，并限制为单个 PNG/JPG/WebP 文件名。窗口回归走真实的 app 保存接口，再按原 Vue URL 解析并检查读回字节。
- GPUI crate 现在有 352 个 Rust 源文件，按账号、总览、农场、好友、活动、分析、商城、设置、配置、运行时和测试拆分；模块目录索引见 `docs/ARCHITECTURE.md`。
- **验证通过**：`cargo test --workspace -- --test-threads=1`（GPUI 207 项；app/core 与其余 workspace 测试均通过）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实账号在线/过期授权、配置保存后的进程重启读回、各页面真后端错误与成功流程、macOS 实窗缩放/输入/托盘、Windows/Linux 编译打包及实机验收仍待完成；本轮不表示全客户端已经完全等价。

### 2026-10-08 — GPUI 设置写入读回与无账号门控对齐

- 设置快照加载时将后端免打扰字段 `continue_farm` 适配为 Vue 表单使用的 `continueFarm`，避免值为 `false` 的已保存设置在重新打开页面后回退成 `true`。
- 无账号时仅种植策略和自动化显示“请选择账号”；离线提醒与系统运行环境仍可加载、编辑和保存。窗口回归分别将自动化选项、离线提醒和运行环境写入共享配置，再通过新的 app 配置快照读回；自动化用例还验证重新加载后页面能恢复免打扰开关状态。
- **验证通过**：`cargo test -p qq-farm-gpui --all-targets -- --test-threads=1`（213 项）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。该轮保存读回使用本机 app store 和测试账号配置；真实在线账号、提醒投递及授权服务仍在设置验收清单中，不能由本地持久化测试替代。

### 2026-10-08 — GPUI 好友零值金币展示与范围回归

- 对照 Vue 的最终模板绑定确认：范围标签的起止位置使用过滤后的好友数，但“共 N 名”直接读取原始 `friends.length`，其中会包含黑名单好友。GPUI 保留这一可见行为，并增加窗口断言避免以后只看同名 computed 就误改模板实际语义。
- Vue 正常好友行始终显示金币；`formatFriendGold(null/undefined)` 会显示 `0`。GPUI 现在正常好友行显示零值，黑名单行遵循外层等级/金币真值门控；好友宠物徽标也要求 Vue 所需的 `pet` 对象存在。范围标签和页码按钮提供可访问名称。
- 好友分页新增五页槽和直接跳页，覆盖第 1/2 页切换及返回上一页；原始好友总数与过滤后起止范围仍分别遵循 Vue 模板绑定。
- **验证通过**：`cargo test -p qq-farm-gpui --all-targets -- --test-threads=1`（216 项）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。Linux 与 Windows 交叉目标检查已尝试，但本机分别缺少 `x86_64-linux-gnu-gcc` 和 Windows MSVC SDK 头文件，未能到达项目源码编译；需由相应 CI runner 验证。真实服务操作与窗口实机验收仍待完成。

### 2026-10-08 — GPUI 活动目录兜底字段遵循 Vue 绑定

- 对照 `activity/index.vue::displayActivities`，修复七夕、青梅和公益目录兜底卡的字段优先级：七夕/公益使用 `groupId || activityId`，青梅使用 `activityId || brewActivityId`；名称分别遵循 `name || title` 或青梅专用 `name`。七夕仅有 `groupId` 时也会像 Vue 一样创建卡片。
- 目录默认名称改为 Vue locale 中的“鹊桥寄情”“青梅”等实际标签；天气兜底使用 Vue 固定 ID `2026070300` 和本地化标签，不再取快照里的动态名称。千星兜底 ID 也改为遵循 JavaScript 真值回退。
- 新增冲突字段兜底单测及 Vue 全部 8 个 `gameplayKey` 的路由表回归。所有已支持玩法已有专用视图；同步矩阵中早期“仍有玩法用通用行”的记录已更正，待完成项集中在字段/门控逐项验收与真实后端结果。
- **验证通过**：GPUI 全量测试 217 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实服务端活动数据和窗口实机仍待验。

### 2026-10-08 — GPUI 设置回归按业务拆分与钉钉字段对齐

- 将原先集中在 `desktop/tests/settings.rs` 的设置回归拆为导航、离线提醒、系统配置、自动化、种植和 QQ Bot 六个模块，单个测试文件控制在 170 行以内；业务实现继续位于 `desktop/settings/` 各能力目录。
- 钉钉提醒表单现在和 Vue 一样分别显示 Webhook、Access Token、加签密钥标签及具体占位说明，并提供匹配的无障碍名称；初始化和中英文切换时都使用同一套文案。
- GPUI crate 当前有 358 个 Rust 源文件，按页面和业务能力拆分；新 crate 在 Git 中仍是未跟踪目录时，普通 `git status --short` 会折叠显示为一条目录项，完整结构见 `docs/ARCHITECTURE.md`。
- **验证通过**：GPUI 全量 218 项测试、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。本次只完成一项设置差异与测试组织整理；其余页面仍在逐项对照，不能据此宣称全功能完全等价。

### 2026-10-08 — GPUI 分析卡片数值空值遵循 Vue 转换

- 对照收益分析页 `metricDisplay`：显式 `null` 经 JavaScript `Number(null)` 显示为 `0`，缺失字段经 `Number(undefined)` 显示为 `-`。GPUI 比较卡片之前把两者都显示为 `-`；现在区分处理，数字字符串与非法值行为继续遵循页面语义。
- **验证通过**：新增空值/缺字段回归，GPUI 全量 219 项测试、GPUI Clippy `--all-targets --no-deps -D warnings` 和 `cargo fmt --all -- --check`。排名全部字段、真实账号读写和线上状态仍需继续核对；本条不代表分析页或全客户端全部等价。

### 2026-10-08 — GPUI 账号授权状态遵循 Vue 优先级和提示

- 将账号授权状态判定拆到 `desktop/accounts/auth_status.rs`：未授权优先于重扫建议；授权状态的三种标签、警示颜色和重扫原因提示与 Vue 一致。Vue 的授权筛选仍只有“已授权/未授权”两项，因此 GPUI 保留原两类筛选。
- 该 crate 当前 361 个 Rust 源文件；账号、农场、好友、活动、分析、商城、设置和配置等分别位于 `desktop/` 的业务子目录，窗口入口只负责组合。普通 `git status --short` 将未跟踪的整个 crate 折叠为一个目录项；使用 `git status --short --untracked-files=all` 可查看内部文件。
- **验证通过**：GPUI 全量 222 项测试、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。全客户端业务等价和实机验收仍未完成。

### 2026-10-08 — GPUI 账号搜索字段范围遵循 Vue

- 对照 `toAccountRecord` 和账号列表 `fetchGetFarmAccountList`，关键词只匹配账号名、QQ 号、备注（备注为空时回退到账号名），不匹配运行态昵称或 UIN，也不裁掉用户输入中的空白。账号名称列同样只显示 Vue 绑定的 `name` 字段；最近在线为空时使用 Vue 的 `-` 占位符；英文未授权筛选标签使用 `Unauthorized`。
- 将字段匹配拆到 `desktop/accounts/search.rs`，新增单元和窗口回归，确认昵称/UIN 不误命中、备注命中、空白按原样搜索。
- **验证通过**：账号搜索字段单测与窗口回归通过，时间占位符与英文授权筛选标签有回归覆盖；GPUI 全量 226 项测试、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。全客户端其它页面字段和交互差异、真实后端与实机验收仍需继续核查。

### 2026-10-08 — GPUI 分析页当前策略摘要遵循 Vue

- 对照 analytics 页 `strategyLabelMap` 和当前作物卡，修正“指定种子”显示为 Vue 设置项的“优先种植种子 / Preferred Seed”；作物行只保留 Vue 展示的等级和季数，不再重复显示策略名或参考等级。
- **验证通过**：分析页相关单测和窗口回归 12 项、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。完整排名排序数据、在线设置读回、其他页面差异和实机验收仍需继续核对。

### 2026-10-08 — GPUI 好友访客记录按业务拆分并对齐字段

- 将访客记录映射和行视图分别拆到 `pages/mapper/friends/visitors.rs` 与 `desktop/view/rows/friends/visitors.rs`；好友主行只处理好友和黑名单。访客卡按 Vue 展示头像回退、昵称/GID、动作标签、等级、动作详情及独立右侧时间，不再把字段拼到通用详情文本。
- 访客相对时间按中英文显示“刚刚/Just now”“分钟/min ago”“今天/Today”；日期时间仍采用本机日期格式。更新 `docs/ARCHITECTURE.md` 的模块树，并将访客窗口回归从好友大测试文件移到 `desktop/tests/friends/visitors.rs`。
- GPUI crate 当前 364 个 Rust 源文件，按账号、农场、好友、活动、分析、商城、设置、游戏配置及交互测试等业务能力分目录；源文件数用于定位职责，不以新增空模块增加数量。
- **验证通过**：访客相关 7 项测试、好友相关 28 项测试、GPUI 全量 229 项测试；Vue 日期分隔符微调后再次通过访客 7 项测试、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。其它页面业务差异和实机验收仍继续逐页核对，当前不宣称全客户端已完全等价。

### 2026-10-08 — GPUI 分析页整数型浮点显示遵循 Vue

- Vue 将数字 `12.0` 插值为 `12`，而 Rust JSON 数字原样格式化会保留 `.0`。分析页当前策略摘要现在将 JSON 数值转成数字字符串后显示，同时保持字符串字段的原始文本。
- **验证通过**：分析策略数值模块 6 项测试、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。分析页完整字段/排序、真实账号读写及全客户端剩余页面仍需继续验收。

### 2026-10-09 — GPUI 分析页等级格式与黑名单计数补齐

- Vue 的 `formatLv` 对数值 `12.0` 显示 `12`，GPUI 现在对整数型 JSON 浮点等级作相同格式化，同时保留字符串字段原文。黑名单分段入口现在显示 Vue 页签中的数量徽标，移除最后一项后即时隐藏，并同步更新无障碍名称。
- **验证通过**：分析页定向窗口/映射测试 12 项、GPUI 全量 229 项测试、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。排名全部字段/排序、真实账号配置读写、全客户端剩余页面及真实窗口/跨平台验收继续进行。

### 2026-10-09 — GPUI 好友确认框与取消交互对齐

- 好友捣乱、加入/移出黑名单和删除改用 GPUI Kit AlertDialog，确认继续调用既有好友操作，取消按钮和 Escape 只关闭对话框，不触发业务写入；中英文标题、说明和操作标签随界面语言切换。
- crate 目前有 364 个 Rust 源文件，账号、农场、好友、活动、分析、商城、设置和游戏配置等分别位于 `desktop/` 业务目录；主入口及 `desktop/mod.rs` 负责组合。未跟踪的新 crate 在普通 `git status --short` 中会折叠成一个目录项，模块索引见 `docs/ARCHITECTURE.md`。
- **验证通过**：好友取消与 Escape 窗口回归、GPUI 全量 231 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。其它页面字段/门控、真实服务操作和三平台实机验收仍继续逐页对照，本项不表示全客户端已完全等价。

### 2026-10-09 — GPUI 分析等级筛选遵循 Vue 小数和清空语义

- 对照 `desktop-ui/src/views/farm/analytics/index.vue::plantableRankings`，修正参考等级输入：账号等级同步时使用账号等级；用户输入小数时保留小数门槛；清空或无效值按 Vue 的 `Number(value || 0)` 回退为 0。原有 1–200 输入约束继续由分析状态执行。
- 新增窗口回归覆盖默认账号等级、输入 `12.5` 后作物候选变化、清空后只保留等级未知作物；分析语义单测覆盖范围边界和数字格式。
- **验证通过**：GPUI 全量 `cargo test -p qq-farm-gpui --all-targets -- --test-threads=1`（234 项）、分析页定向回归（5 项）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。分析页完整排名字段/排序、真实账号读写及全客户端其它页面仍在验收清单中。

### 2026-10-09 — GPUI 活动奖励回执和操作校验按语言显示

- 英文活动奖励回执现在使用英文物品回退名、逗号分隔和英文冒号；天气活动奖励列表也按语言切换分隔符。青梅酿造、七夕赠礼和秋日祈愿的提交校验错误使用当前界面语言，并保留库存、好友列表及活动选项校验。
- 对照代码结构，GPUI crate 维持单客户端 crate、内部按业务能力拆分；当前 `src/` 有 364 个 Rust 文件，业务入口位于 `desktop/accounts/`、`personal/`、`friends/`、`activities/`、`analytics/`、`commerce/`、`settings/`、`game_config/`。未跟踪文件在普通 Git 状态输出中折叠为 crate 目录，完整模块索引见 `docs/ARCHITECTURE.md`。
- **验证通过**：活动模块定向测试 56 项，覆盖本次双语回执与输入校验，以及已有活动视图和门控回归。全客户端未完成页面字段、交互与实时后端验收，本项不表示所有功能已经完全等价。

### 2026-10-09 — GPUI 设置错误状态按当前语言显示

- 设置字段失效、QQ Bot 凭据缺失、测试通知默认失败、绑定/打开链接错误及 Tokio 后台任务失败的固定说明现在生成中英文文案，并按所选语言使用对应标点；服务端返回的具体错误内容继续原样保留。
- **验证通过**：设置页定向测试 28 项、错误前缀中英回归 1 项、GPUI 全量测试 236 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy、`cargo fmt --all -- --check` 和 `git diff --check`。其它页面文案和实机服务状态仍需继续对照。

### 2026-10-09 — GPUI 萌宠好友夺宝加载与刷新时机对齐

- 好友名单刷新和单个好友宝藏查询使用各自的忙碌状态与请求代次，不再把局部查询提升为整页加载；切账号或离开玩法会使旧请求失效。Vue 夺宝操作结束后始终重新查询当前好友，GPUI 现在在成功和失败结果后都刷新宝藏详情，并保留操作错误提示。
- 窗口回归验证好友查询/刷新进行中仍不触发整页加载、重复点击不会启动新请求，其他玩法操作仍留在页面。**验证通过**：宠物日记窗口专项回归 1 项、GPUI 全量测试 236 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy、`cargo fmt --all -- --check` 和 `git diff --check`。
- 真实活动账号的宝藏刷新结果和网络失败提示仍需端到端验收；这只关闭好友夺宝的加载与刷新差异，不代表所有活动或全客户端已完全等价。

### 2026-10-09 — GPUI 每日礼包状态颜色遵循 Vue

- 每日礼包状态徽标现在按 Vue 的优先级决定颜色：未开通为默认灰色，今日完成为成功色，已启用待执行为信息色，未开启为默认灰色。状态文字与颜色共用同一状态分类，避免订阅异常数据被误标成完成。
- **验证通过**：状态优先级单测覆盖未开通、完成、等待、关闭四类；每日礼包状态和窗口测试 3 项、GPUI 全量测试 237 项、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check` 通过。工作区及实机验收仍继续进行。

### 2026-10-09 — GPUI 设置页背包加载错误按语言显示

- 设置页表单静态文案、策略选项和输入错误已经接入双语映射；补齐 Tokio 任务异常时“读取背包种子失败”的英文前缀，后端返回的动态错误详情继续原样显示。验收矩阵对应改为真实账号/通知/门控实测仍待完成，不再把已本地化的静态标签列为缺口。
- **验证通过**：设置页定向回归 28 项、通用动态错误前缀中英文回归、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。本轮更改仅补任务异常的静态前缀，之前 GPUI 全量 237 项测试已通过。

### 2026-10-09 — GPUI 跨页面后台任务错误按当前语言显示

- 账号启停无账号提示、账号批量删除/启停异常、通用页面加载的 Tokio 任务异常和商城购买异常现在使用中英文文案及语言对应标点；后端业务错误继续保留原文。其它好友和个人农场操作已有同类双语分支，暂无回退到中文的共用任务前缀。
- **验证通过**：GPUI 全量测试 237 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。

### 2026-10-09 — GPUI 账号授权标签浅色主题对比度

- 账号未授权/建议重扫标签在普通浅色表面上改用警示文字颜色；此前误用了警示背景上的前景颜色，在浅色主题中接近白色、难以辨认。该视觉问题通过浅色和深色 GPU 预览检查。
- **验证通过**：GPU 预览重新渲染并检查账号浅/深主题；账号窗口回归 9 项、GPUI Clippy、格式检查和 `git diff --check` 通过。预览用于代码层视觉检查，真实窗口实机验收仍未完成。

### 2026-10-09 — GPUI 萌宠日志与好友筛选交互遵循 Vue

- 对照 `pet-diary-view.vue` 的 `NCollapse default-expanded-keys`，萌宠养成、寻宝、魅力、对战、种子、商店、故事和节气默认展开；互动/被夺日志没有列入默认展开键。GPUI 之前额外默认展开日志，导致首次进入页面时内容密度和滚动位置与 Vue 不同。
- Vue 的好友可搜索选择器支持按昵称或 GID 前缀查找。GPUI 用输入框呈现建议项时，原逻辑会把数字前缀当作完整 GID 并隐藏结果；现在只有完全匹配好友 GID 才隐藏建议。
- 移除日志默认展开状态；窗口回归确认日志初始折叠，并验证日志可展开/折叠、昵称搜索、数字 GID 前缀及完整 GID 选择。活动逐页验收继续进行，本项不代表全客户端已完全等价。
- **验证通过**：萌宠日记对应 GPUI 窗口测试通过；全量 GPUI 与 workspace 检查仍需在后续整体验收中持续验证。

### 2026-10-09 — GPUI 成长任务进行中状态色遵循 Vue

- Vue 的成长任务汇总和单项进行中标签均使用信息色，已完成项使用成功色。GPUI 原先将单项进行中状态显示为灰色，汇总也没有使用对应状态色；现改为使用 GPUI Kit 的信息色/成功色主题令牌。
- **验证通过**：GPUI 全量 237 项测试通过；整体验收及真实窗口主题复核继续进行。

### 2026-10-09 — GPUI 游戏配置主键与表格列设置对齐

- Vue 种子表使用 `seedId` 做行键、选择键和删除参数。GPUI 原先优先使用可选 `plantId`，可能把关联植物编号传给删除接口；现逐行删除和批量选择/删除统一使用 `seedId`，仅在兼容旧快照时回退到 `id`。同时保留缺少可操作主键的目录行，但禁用其选择，不再吞掉可搜索记录。
- 补齐 Vue `TableColumnSetting` 在游戏配置页的可见行为：各分类可显隐字段、调整列顺序、轮换左/右固定状态及全选。固定列显示在独立侧栏，中心列可横向滚动；切换分类时按 Vue 行为重置列、筛选、关键词和行选择。离开该页会清除页面本地过滤、分页、表单及列状态，再次进入恢复 Vue 的默认值。状态、设置面板、字段目录和固定表格继续拆在 `game_config/` 业务目录内。
- 窗口回归覆盖种子同时带 `seedId`/`plantId` 时按 `seedId` 删除、无主键行保留但不可选、列显隐/顺序/固定/全选、分类与页面切换重置、三类表格字段和窄窗口横向滚动。
- **验证通过**：GPUI 全量 246 项测试、游戏配置专项 13 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--no-deps --all-targets -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实配置数据写入/重启互读、实际图片加载及三平台实机操作仍需继续验收；本次只关闭游戏配置已确认的差异，不代表全客户端完成等价验收。

### 2026-10-09 — GPUI 果实配置补齐每株收获数量

- 对照 Vue `fruit-operate-drawer.vue`，果实表单之前漏渲染 `fruitCount`（每株收获数量），尽管编辑状态和 app 写入接口已有该字段，用户无法修改它。现在在“持有上限”后加入数字输入，保持 Vue 字段顺序和默认值 200。
- 更新窗口回归检查新增/编辑表单均显示该字段，并通过 GPUI 表单将收获数设为 7、重新加载共享游戏配置后读回 7。全配置页字段仍需结合真实用户数据和应用重启继续验收。
- **验证通过**：游戏配置定向 10 项测试通过；GPUI 全量与 workspace 严格检查在整体验收中继续执行。

### 2026-10-09 — GPUI 萌宠物品文案遵循 Vue itemText

- 萌宠奖励、日志与操作回执现在按 Vue `itemText` 保留数字型非空名称，并在数量缺失时继续显示 `×` 后缀；此前 GPUI 会把数字型名称替换成道具编号，且省略空数量后缀。
- **验证通过**：新增数字名称/缺少数量回归和成功回执中英文测试；萌宠日记相关 4 项测试通过。其他玩法的动态物品文案仍需按各自 Vue 模板逐项核对。

### 2026-10-09 — GPUI 游戏配置目录表格与选择交互对齐

- 游戏配置目录此前用通用卡片显示少量字段，和 Vue 的种子、果实、道具三套表格列不等价。现在按分类显示 Vue 对应的字段、图片/名称、价格文案与行操作，加入当前页全选和逐行复选框；原有分页、筛选、编辑、删除确认及批量删除继续复用，长目录启用虚拟列表，窄窗口支持横向滚动。
- 新增 GPUI 窗口回归覆盖三套表头和行字段、种子选择状态及操作列存在。**验证通过**：`game_config_catalog_uses_vue_columns_and_row_selection` 1 项；全量测试、Clippy 和严格 workspace 检查仍在本轮继续执行。
- 种子等页面数据的真实用户配置读写、图片缺失/上传、最小窗口实机滚动及跨客户端重启互读仍需端到端验收；本条不表示全客户端已完全等价。

### 2026-10-09 — GPUI 业务模块目录与好友申请门槛交互复核

- GPUI crate 当前 `src/` 包含 370 个 Rust 文件，页面代码按 `accounts/`、`personal/`、`friends/`、`activities/`、`analytics/`、`commerce/`、`settings/`、`game_config/` 等业务目录组织。普通 `git status --short` 将未跟踪的新 crate 折叠为 `?? crates/qq-farm-gpui/`，完整文件结构仍在 crate 内；架构索引见 `docs/ARCHITECTURE.md`。
- 对照 Vue 设置页，修正好友自动通过申请的门控：总开关关闭时，最低等级、自身等级要求和收取/偷取门槛仍显示但禁用；启用收取/偷取门槛后，即使总开关关闭，具体阈值仍保持显示且禁用。好友批量偷取也继续沿当前搜索结果执行，与 Vue 的 `normalFriends` 范围一致。
- **验证通过**：GPUI 全量测试 249 项、好友偷取范围和好友申请门控定向回归 3 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--no-deps --all-targets -D warnings`、`cargo fmt --all -- --check`、`git diff --check`、`pnpm --dir desktop-ui typecheck` 和 `pnpm --dir desktop-ui build`。工作区检查包含上游 `block` 依赖的 future-incompatibility 提示。真实账号逐项业务验收、真实窗口视觉与缩放、托盘恢复以及 Windows/Linux 实机验证仍待完成；本条不代表全客户端已完全等价。

### 2026-10-09 — GPUI 宠物页技能说明与操作门控继续对齐

- 对照 `PetPanel.vue` 补回技能说明的 12 字预览、悬停全文提示和完整无障碍标签。宠物激活期间只锁定重复激活；刷新、守护日志、上场、收回、领取及有库存的狗粮操作仍可使用，激活成功后加载状态持续到刷新结果返回。
- 宠物信息不可用时仍显示独立的狗粮区域和空状态，与 Vue 的分区渲染一致；新增窗口回归覆盖该状态、激活期间刷新可继续操作，以及技能预览长度。
- **验证通过**：GPUI 全量 251 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--no-deps --all-targets -D warnings`、`cargo fmt --all -- --check`。实际账号操作、真实窗口悬停/加载反馈及跨平台桌面验收仍待完成；本条不代表全客户端已完全等价。

### 2026-10-09 — GPUI 果实新增 ID 遵循 Vue 与 app 门面语义

- Vue 新增果实时不要求用户提供隐藏 ID，app 门面也会从所选植物配置推导果实 ID；GPUI 保存时曾把 ID 当成所有类型都必填，导致新增果实被空字段拦截。现在只有编辑果实或新增种子/道具才要求正数 ID；新增果实保持为空并交由 app 从关联植物推导。
- 更新窗口级持久化回归：新增果实时不填写 ID，只选择关联植物并保存，随后从共享配置重新加载确认写入成功。
- **验证通过**：果实与道具配置持久化专项回归通过。真实用户配置读写、应用重启互读、图片上传和恢复仍列入端到端待验；此修复不代表游戏配置或全客户端已完整等价。

### 2026-10-09 — GPUI 分析黑名单保存与农场子页失败恢复继续对齐

- 分析页黑名单改为调用 `qq_farm_app::settings::save_settings`，和 Vue `fetchModifyFarmAutomation` 走相同设置快照门面；生产 UI 将保存放到共享 Tokio runtime，保存中禁用重复修改，成功后更新视图并显示确认，失败保留原选择并显示错误。切换账号或离开分析页会让旧结果失效。GPUI 测试模式在当前线程同步执行该本地配置保存，避免唤醒 GPUI 测试调度器的跨线程竞争；公共完成/失败状态更新逻辑共用。
- 农场页刷新现在按 `sectionErrors` 保留失败子页最近一次成功快照；同一账号子页恢复后用新数据替换，离线快照仍清空旧数据，对齐 Vue `TaskPanel` 请求失败时保留原 `dailyGifts` 的行为，并扩展到其它独立农场能力。
- **验证通过**：分析黑名单窗口回归覆盖多选保存/配置版本递增、移除反馈及 app 保存失败回退；失败子页的保留、恢复与离线清理单测通过。GPUI 全量 253 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check` 通过。生产 Tokio 窗口时序、真实服务账号和实机运行仍待验；这两项修复不代表全客户端已完全等价。

### 2026-10-09 — GPUI 账号与游戏配置操作反馈、分页删除语义继续对齐

- 账号列表批量删除现在只删除当前页选中的账号；确认数量和目标在打开确认框时一并固定，跨页遗留勾选仍随 Vue 行为在操作后清空。账号分页刷新使用当前页大小计算页数，避免切换每页条数后用固定 10 条错误钳制页码。
- 账号新增、编辑、启停和删除成功后显示短暂成功反馈，对齐 Vue 全局消息；游戏配置新增、编辑、单项删除和批量删除也显示对应成功反馈，批量部分失败仍优先给出错误明细。
- **验证通过**：账号定向 22 项测试、游戏配置定向 18 项测试；GPUI 全量 256 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--no-deps --all-targets -D warnings`、`cargo fmt --all -- --check`、`git diff --check`。提示显示由窗口测试覆盖；真实后端账号生命周期、配置用户数据和所有页面的完整操作仍需继续实测，本条不表示全客户端已完全等价。

### 2026-10-09 — GPUI 设置保存副作用与成功反馈继续对齐

- 种植策略和提醒页保存时不再意外触发自动买肥检查；该检查只在保存自动化分段且 Vue 的自动买肥开关启用时执行。设置保存提示移到设置页共用区域，策略页保存后也能看见提示，不再局限于自动化页。
- 账号批量删除范围/确认目标、游戏配置三类字段与表格操作、设置写入副作用均有定向回归；其它业务页既有操作反馈与门控由分业务窗口测试覆盖。GPUI crate 当前有 370 个 Rust 文件并按业务目录拆分；该结构和自动测试仍不能替代真实账号及实机操作验收。
- **验证通过**：GPUI 全量 258 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--no-deps --all-targets -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实在线/离线状态、账号授权、活动与购买门控、窄窗口/缩放/焦点、托盘退出恢复及 Windows/Linux 实机验收仍待完成；当前不宣称所有功能已经完全等价。

### 2026-10-09 — GPUI 务农操作反馈与按业务拆分结构复核

- 农场操作按钮使用地块页的局部忙碌状态，禁用重复操作且不再触发整页加载；成功/失败结果以短暂提示显示在页面内容顶部，避免长地块列表把反馈挤出首屏。成熟时间和作物名称门控按 Vue 的 JavaScript 数值/真值转换处理，避免数字字符串和空名称导致错误显示收获操作。
- 用户提出代码文件集中问题后复核 crate：`src/` 有 370 个 Rust 文件；账号、农场、好友、活动、商城、分析、设置和游戏配置分别位于独立业务目录，活动按具体玩法继续细分。已有模块索引见 `docs/ARCHITECTURE.md` 的“GPUI crate 内部结构”；新增页面继续沿该边界维护，不为增加文件数而拆散单一职责。
- **验证通过**：GPUI 全量 261 项测试、`cargo check --workspace --all-targets`、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。带依赖递归的严格 Clippy 仍会报告 `qq-farm-core/src/services/commerce/service.rs` 既有的 `useless_conversion` 与 `manual_ok_err` 两项；当前改动不在该文件。真实账号务农结果、反馈时序、窗口尺寸/缩放和三平台托盘仍需实机验收，本条不表示全客户端已经完全等价。

### 2026-10-09 — GPUI 宠物刷新范围与好友头像缺图表现对齐

- 对照 Vue `PetPanel.vue`，宠物手动刷新、上场、收回和使用狗粮只重新读取宠物信息与礼包状态；激活和领取礼包才通知农场父页刷新全部子面板。GPUI 之前对所有宠物操作都刷新整页，现在按 Vue 缩小刷新范围，并在异步合并时保留失败接口对应的旧快照、不覆盖其它农场数据。账号、页面或请求版本变化后会丢弃过期结果。
- 好友列表及访客记录的头像图片加载失败时，改为显示昵称首字母占位；缺少 URL 时直接显示相同占位，匹配 Vue 的图片失败兜底。
- 两项实现分别落在 `personal/pets/refresh.rs` 和 `view/rows/friends/avatar.rs`，沿用按业务拆分的目录；GPUI 源码现在有 372 个 Rust 文件，索引已同步到 `docs/ARCHITECTURE.md`。
- **验证通过**：`cargo test --workspace -- --test-threads=1`（GPUI 262 项，全 workspace 测试完成）、宠物/好友访客定向回归、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check`、`git diff --check`、`pnpm --dir desktop-ui typecheck` 和 `pnpm --dir desktop-ui build`。真实账号接口失败/成功时序、真实窗口中的网络头像加载及三平台桌面验收仍待实机完成；本项仅关闭上述两项差异，不代表全客户端已完全等价。

### 2026-10-09 — GPUI 应用菜单和托盘菜单跟随界面语言

- 应用菜单、托盘菜单及支持 tooltip 的平台托盘提示现在跟随保存的界面语言；运行中切换中英文会即时更新应用菜单和托盘菜单。Linux 的托盘菜单更新通过 GTK 主线程轮询轻量语言消息完成，不跨线程移动 GTK 所有权对象。
- **验证通过**：GPUI 全量 264 项测试、应用/托盘中英文标签专项回归、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、格式检查和 `git diff --check`。本机 Linux 交叉检查先因缺少 `x86_64-linux-gnu-gcc` 失败；绕过 C 编译器检查后仍因没有 GTK/GLib Linux sysroot 与交叉 pkg-config 配置而无法到达本 crate 的 Linux 条件代码，需由 Linux CI runner 验证。菜单实机显示和 macOS/Windows/Linux 托盘生命周期仍待各平台验收。

### 2026-10-09 — GPUI 共用图片错误状态遵循应用语言

- 应用级 `ActiveLanguage` 在启动和运行时语言切换时同步更新；共享图片适配器根据该状态显示“缺图”或 “Image unavailable”，因此活动、商城、背包、好友农场、游戏配置和分析页的缺图回退统一跟随中英文。
- **验证通过**：GPUI 全量 266 项测试、活动语言状态注入和共用缺图文案专项回归、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。多页面真实网络图片失败显示及跨平台窗口实机复核仍待完成；动态活动/农场/分析/商城/配置文案仍需完整逐项双语审查。

### 2026-10-09 — GPUI 游戏配置表格按职责和配置类型拆分

- 游戏配置的表格骨架保留在 `game_config/table.rs`；表头、行、通用单元格、共享格式化及种子/果实/道具各自字段渲染拆入 `game_config/table/`，编辑与删除动作仍由 `row_actions.rs` 管理。当前 GPUI crate 有 379 个 Rust 源文件，页面能力按业务目录组织，三类游戏配置在表格渲染层也各有对应模块。
- **验证通过**：GPUI 全部测试、严格 warning 检查和格式检查；游戏配置单元格的中英文格式化与道具类型目录回退有专项回归。此项只调整代码边界，不代表真实账号、三平台窗口及所有页面交互已完成实机验收。

### 2026-10-09 — GPUI 每日任务和仓库长文本表现继续对齐

- 成长任务单项状态由彩色文字改为与 Vue `NTag` 对应的成功/信息色标签，礼包卡片和成长任务汇总也统一使用轻量状态标签；任务描述添加单行截断，进度保持次要字号且不换行。仓库商品名按 Vue 截断显示，并提供悬停全文和完整无障碍名称。
- **验证通过**：每日任务与仓库窗口回归、GPUI 全量 267 项测试、`cargo fmt --all -- --check`；严格编译及 Clippy 检查随后执行。页面视觉和真实窗口悬停仍需实机验收，其它页面继续逐项对照。

### 2026-10-09 — GPUI 商城价格样式和业务模块结构复核

- 对照 Vue 商城卡片、神秘商人和购买确认弹窗，商品原价现在与当前价同行并显示删除线，商品名限制两行；神秘商店的折扣原价也单独显示删除线。商城商品按钮铺满卡片，搜索框增加清空操作，并移除 Vue 页面没有的重复“商品”标题。普通/SVIP 目录按当前账号分别缓存，已加载标签之间切换不重复请求；未命中会先清空旧目录，切换账号不会复用他人数据。离开商城页时清除本地缓存。
- 再次核对源码结构：GPUI crate 当前有 379 个 Rust 源文件，按账号、总览、农场、好友、活动、分析、商城、设置、游戏配置等业务目录拆分；商城卡片、神秘商店与确认弹窗各自位于 `desktop/commerce/` 子模块。架构索引见 `docs/ARCHITECTURE.md`。普通 `git status --short` 将整个未跟踪 crate 折叠为一个目录项，使用 `--untracked-files=all` 可展开内部文件。
- **验证通过**：GPUI 全量 268 项测试、商城定向 10 项、`RUSTFLAGS="-D warnings" cargo check -p qq-farm-gpui --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实价格接口、实窗视觉与三平台验收仍待完成；本次局部对齐不表示全客户端已经完整等价。

### 2026-10-09 — GPUI 总览无账号状态仍保留日志与统计

- 对照 `dashboard/index.vue`，无账号时仍显示运行日志、关键词/事件筛选、清理操作、下次巡查和今日统计；此前 GPUI 把这些区域随账号卡片一起隐藏。日志关键词现在可清空，且无账号时清理日志仍可用；账号头像加载失败时显示昵称首字母，离线统计卡也补齐无账号插画和状态说明。日志空状态文案同步使用 Vue 的“暂无实时事件”。
- 新增窗口回归覆盖无账号下各面板可见、清空按钮和搜索框清除按钮行为。当前最终验证通过：GPUI 全量 271 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。`block v0.1.6` future-incompatibility 提示来自上游依赖。
- 本修复只关闭总览已确认的交互差异；账号、农场、好友、活动、分析、商城、设置和游戏配置的完整逐项验收，以及真实窗口和真实账号测试仍在进行，不代表所有页面已经完全等价。

### 2026-10-09 — GPUI 账号页监听跨账号状态变更

- Vue 账号页会在 `account_status` 和 `status:update` 中授权失效时刷新整张账号表。GPUI 之前只更新侧栏账号切换器，且按当前所选账号过滤实时事件，导致其它账号状态改变时表格会保留旧数据。现在账号页会放行这两类列表级事件并重载快照；其它页面仍过滤非当前账号事件。
- 新增事件匹配和跨账号过滤回归，覆盖账号生命周期、授权失效、健康状态及无关日志。**验证通过**：GPUI 全量 271 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。此项只修复账号列表实时刷新差异，不代表全页面已完成等价验收。

### 2026-10-09 — GPUI 总览升级预估保留经验小数速率

- Vue 使用完整的每小时经验速率计算升级时间；GPUI 之前先把速率取整，速率小于 1 时会漏掉升级预估，并在当前经验超过本级目标时隐藏对应计算结果。现拆出独立格式函数，保留小数速率、取整显示分钟并覆盖等级进度边界。
- 新增低速率、超过目标、分钟展示及无经验状态回归；全量与严格检查在本轮最终验证后更新。

### 2026-10-09 — GPUI 总览连接日志括号原因格式化

- Vue 会把 `(source=..., phase=...)` 整体转为可读的连接原因和阶段；GPUI 之前只解析空格分隔的普通键值，括号内的来源会残留英文。现在同时解析括号形式，并保留中文/英文事件和阶段名称。日志关键词输入的占位文案也同步为 Vue 的“关键词/Keyword”。
- 解析同时允许括号、逗号、等号和值周围出现空白，与 Vue 正则行为一致；新增中英文、大小写混合和空白变体回归。
- **验证通过**：GPUI 全量 274 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。账号、农场、好友、活动、分析、商城、设置及配置页的真实后端逐项验收、实机窗口与跨平台托盘/build 验证仍未完成；当前不能据此宣称所有功能已完全等价。

### 2026-10-09 — GPUI 设置页施肥与好友申请规则提示对齐

- 逐项比对 `settings/index.vue` 的说明文案时补齐了普通+有机策略的巡田顺序、自动买肥检查时机/有机优先规则、好友自动通过门槛、背包种子排序、施肥地块范围与智能施肥时机，以及离线提醒和 QQ Bot 绑定说明。提示按 Vue 的状态条件显示并提供中英文；好友说明在自动通过开关关闭时仍保留。
- GPUI 窗口回归覆盖新增说明的显示条件、中文与英文、QQ Bot 等待绑定及二维码两种分支。**验证通过**：GPUI 全量 275 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实账号阈值保存/巡查结果与跨平台窗口仍需实机验收；其它设置字段和门控继续逐项对照。
- 结构复核：crate 当前有 380 个 Rust 源文件、约 45.7k 行；`main.rs` 为 73 行，账号、农场、好友、活动、分析、商城、设置、配置、运行时和测试按业务目录拆分，模块索引见 `docs/ARCHITECTURE.md`。Git 默认短状态会将整个未跟踪 crate 折叠成一行；目录结构与代码拆分不作为功能等价证明。

### 2026-10-09 — GPUI 设置页种植策略选种预览对齐 Vue

- 对照 Vue `strategyPreviewLabel` 的 watcher 和策略表单，发现 GPUI 只提供策略/种子选择器，没有显示预览。现在主策略和背包回退策略都展示预计种子；等级策略排除锁定/售罄种子后按等级降序、ID 升序选择，收益策略按对应 analytics 排名选择第一个可用种子。无可用/无匹配状态按 Vue 显示，指定种子分支继续显示可搜索选择器。
- 排名查询放在 Tokio 后台任务，策略或账号变化会使旧结果失效；预览位于 `desktop/settings/strategy_preview.rs`，窗口控件与状态仍归设置业务模块。
- **验证通过**：GPUI 全量 279 项测试、策略预览专项 4 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check`。实账号策略预览与真实在线排名数据仍需端到端验收；其它页面逐项对照继续进行，不能据此宣称全客户端已完全等价。

### 2026-10-09 — GPUI 账号列表顺序遵循 Vue 返回顺序

- 对照 `fetchGetFarmAccountList`，Vue 会按 app 返回顺序做筛选和分页；GPUI 之前按账号 ID 排序，导入的旧账号或非连续 ID 会改变行顺序。移除额外排序，筛选后保留源顺序。
- 回归覆盖混合数字/非数字 ID 的筛选结果顺序。**验证通过**：账号定向 24 项、GPUI 全量 280 项、全 workspace 测试（app 16、core 1044、GPUI 280）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check`、`git diff --check`、Vue `pnpm --dir desktop-ui typecheck` 和 `pnpm --dir desktop-ui build`。实际账号数据仍需实机核对；其余页面及真实窗口验收继续进行，本项不代表全客户端已经完全等价。

### 2026-10-09 — GPUI 好友实时事件对齐并复核按业务拆分

- 好友页现在对齐 Vue 的 `friend_plant` 即时状态补丁、成功偷菜互动与访问/偷菜日志触发的 800ms 合并刷新；刷新读取当前账号好友列表与黑名单，并同步重载展开中的好友地块。payload 账号 ID 优先级、数字 ID 比较、乐观清零、当前语言状态标签均有回归覆盖。账号或顶层页面切换会使旧的延迟刷新失效；事件刷新失败时保留上次好友数据。
- 好友实时事件解析与刷新放在 `desktop/friends/realtime.rs`，事件桥只分发到好友业务模块。`src/` 现有 381 个 Rust 文件，账号、农场、好友、活动、分析、商城、设置和配置各有业务目录；`desktop/mod.rs` 为窗口组合入口。默认 `git status` 将未跟踪的整个 GPUI crate 折叠成一个目录项，完整文件列表可用 `git status --short --untracked-files=all` 查看。
- **验证通过**：GPUI 全量 285 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。本轮未连接真实账号验证推送与接口时序；账号、农场、活动、商城等全页面操作及三平台实机验收仍在继续，本项不表示全客户端已完全等价。

### 2026-10-09 — GPUI 神秘商店独立页面入口与操作补齐

- 页面目录之前遗漏 `MysteryShop`，且切换时强制并入商城神秘商人标签；这与 GPUI 目标导航不一致。现在页面清单含十个业务入口，神秘商店可独立打开并保存导航状态，同时保留商城内神秘商人标签。独立页接入共享商品面板、刷新、折扣/库存/余额门控与限购购买确认。
- 页面目录和桌面业务源码已按 `accounts`、`personal`、`friends`、`activities`、`commerce`、`settings`、`game_config` 等业务目录拆分；当前 crate 有 381 个 Rust 文件。普通 Git 状态将未跟踪 crate 折叠成单个目录项，模块索引见 `docs/ARCHITECTURE.md`。
- **验证通过**：GPUI 全量 286 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实商店账号、跨平台窗口及剩余业务页面逐项验收仍需继续；此项不表示全客户端已完全等价。

### 2026-10-09 — GPUI 正式构建与 macOS 资源包核验

- 首次正式构建发现 10 个个人农场视图从 `gpui_kit::test` 导入 `TestSupportExt`，而 `test` 模块只在 `test-support` feature 下开放。改为使用 GPUI Kit 顶层重导出的 `TestSupportExt`：测试 feature 开启时仍可观察 UI，正式构建则按库实现原样返回元素。
- **验证通过**：`cargo build --release -p qq-farm-gpui`、GPUI 全量 286 项测试、GPUI Clippy `--all-targets --no-deps -- -D warnings`、格式检查和 `git diff --check`。本机打包产物为 `dist/qq-farm-gpui-macos-x86_64.app.zip`；`lipo` 确认为 x86_64，包内 `Info.plist` 通过 `plutil -lint`，且检查到客户端、TSDK/WASM、游戏配置与活动图片资源。
- 这仅验证当前 Intel Mac 的 release 编译与包结构；没有替代真实窗口启动、账号登录、Apple Silicon 实机、Windows/Linux 构建与托盘生命周期验收，客户端完整功能等价仍在逐项核对。

### 2026-10-09 — Linux GPUI CI 补齐托盘系统依赖

- 对照 `tray-icon 0.24.2` 的 Linux 默认 gtk/libxdo 后端，GPUI 验证 workflow 与 release workflow 原先未安装 XDo 和 AppIndicator 的开发包，原生 Ubuntu runner 会在构建托盘依赖时失败。现两个 workflow 均安装 `libxdo-dev` 与 `libayatana-appindicator3-dev`。
- Ubuntu Noble 官方包索引确认这两个开发包可用。**本地验证**：workflow 中 Linux apt 安装项已更新，格式与工作区检查仍通过；当前宿主是 macOS，实际 Linux 编译需由 workflow 的 Ubuntu runner 完成。

### 2026-10-09 — GPUI 账号多条件筛选与 Vue 的应用时机回归

- 将账号页的真实控件流程与 `account-search.vue`、`fetchGetFarmAccountList` 对照：平台、运行状态、微信授权条件均先写入草稿，点“搜索”后一起应用；“重置”只清空表单草稿，列表保持最近一次已应用的结果，直到再次搜索。新增窗口回归组合验证三种筛选，并覆盖重置后的旧结果保留和清除条件后恢复全列表。
- **验证通过**：GPUI 全量 `cargo test -p qq-farm-gpui --bin qq-farm-gpui -- --test-threads=1`（287 项）、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实账号筛选数据与远端生命周期操作仍需实机验收；本条仅证明该筛选交互，不表示账号页或全客户端已完全等价。

### 2026-10-09 — GPUI 好友互动更新后立即重排卡片

- Vue 的 `sortedFriends` 会在偷取/帮忙计数、好友地块状态变化后重新排序；GPUI 此前只在重新映射整页数据时排序，导致局部成功操作、详情回填和实时种植事件更新后仍沿用旧顺序。现在三条更新路径都会调用好友页共享排序器，保持互动次数、等级和好友 ID 的排序规则一致。
- 回归覆盖操作成功、打开好友详情以及实时种植事件更新后的卡片顺序，并按好友 ID 验证更新后的状态。代码仍按业务目录组织；默认 `git status --short` 会把尚未跟踪的 `crates/qq-farm-gpui/` 整体显示成一项，查看完整文件清单需加 `--untracked-files=all`。
- **验证通过**：GPUI 全量 288 项测试。严格编译、Clippy、格式检查与 `git diff --check` 在本条追加后复验。真实好友数据及实机窗口排序仍待实账号验收；此项只关闭一个局部排序差异，不代表全客户端已完全等价。

### 2026-10-09 — GPUI 天气活动重新进入时重建页面状态

- Vue 离开天气详情会卸载 `WeatherView`，再次进入时重新加载天气和好友现场数据，清空扫描缓存、选中项、搜索、提示、错误和规则展开状态。GPUI 之前复用窗口状态，只清了搜索和选中好友，旧扫描缓存会让后续选择跳过扫描，旧快照/提示也可能在重载期间继续显示。现在进入天气详情时统一重置这些状态并移除旧天气快照，再开始加载；账号切换复用同一重置逻辑。
- Vue 的天气页刷新只重新加载天气快照和天气好友。GPUI 原先通过整页活动加载刷新，随后再二次加载天气数据；现在刷新直接走天气专用加载器，保留已加载的天气页状态并避免重复请求。天气好友刷新失败时，GPUI 也保留上次成功列表，匹配 Vue 的加载失败语义。
- 增加回归验证，覆盖扫描缓存、延后好友、提示、错误、规则弹层、搜索值和过期快照清理。
- **验证通过**：GPUI 全量 292 项测试、`RUSTFLAGS="-D warnings" cargo check -p qq-farm-gpui --all-targets`、GPUI Clippy `--all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。此项关闭天气页生命周期差异；真实天气服务及实窗加载/失败体验仍待验收，不能据此宣称所有页面已完全等价。

### 2026-10-09 — GPUI 活动目录卡片和窄窗口账号操作布局修正

- 用 GPUI 原生渲染预览复核账号浅色/深色/最小窗口、农场、活动目录、天气及 125%/150% 字体缩放。活动目录卡片之前把多段文字作为按钮的并排子项，长状态提示会挤压活动名称和日期；现在统一在卡片内纵向排列并截断超长标题，操作提示在底部完整显示。
- 账号表在 1100×720 时需要横向滚动，行操作列原先也会随中间数据列一起移出视口。现在默认将行操作固定在右侧，中间列仍可横向滚动；新增最小窗口布局回归。
- **验证通过**：活动目录布局和 1100×720 账号表专项回归；GPUI 全量 293 项测试、`RUSTFLAGS="-D warnings" cargo check -p qq-farm-gpui --all-targets`、GPUI Clippy `--all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check` 及 `cargo run -p qq-farm-gpui --features ui-preview -- --render-preview /tmp/qq-farm-gpui-preview-audit`。该预览覆盖选定状态与窗口，不替代所有页面实机交互验收；真实账号、Windows/Linux 窗口与托盘仍待验证，客户端整体业务等价仍在继续对照。

### 2026-10-09 — GPUI 账号筛选折叠时保留未提交草稿

- 对照 Vue `account-search.vue` 和本地 Naive UI `NCollapse` 实现：筛选项默认收起与 GPUI 一致；但 Vue 表单数据由父组件持有，折叠并重开后仍保留未提交的平台/运行状态/授权草稿。GPUI 之前在每次展开时用已应用条件覆盖草稿。现在折叠只隐藏表单，不改筛选状态；“重置只清草稿、点击搜索才更新列表”语义保留。
- **验证通过**：新增窗口回归覆盖选择平台、折叠重开、应用搜索、重置、再次折叠重开及提交重置结果；GPUI 全量 294 项测试通过。严格编译、Clippy、格式及差异检查在本条后复验。真实账号筛选仍待在线环境验收，其它页面继续逐项对照。

### 2026-10-09 — GPUI 总览头像回退到账号资料

- 对照 `desktop-ui/src/views/farm/dashboard/index.vue`：总览头像先显示实时状态头像，缺失时回退到账号资料头像，图片失败后再显示昵称首字母。GPUI 之前只读取实时状态头像；现在账号目录同步保留头像 URL，并按相同优先级选择来源，`//` 和 `data:image/` 地址也可交给图片控件处理。
- **验证通过**：账号目录头像提取与回退优先级单测、GPUI 全量测试 296 项、`RUSTFLAGS="-D warnings" cargo check -p qq-farm-gpui --all-targets`、GPUI Clippy、格式检查和 `git diff --check`。真实头像加载/失败回退仍需实窗检查；本项只关闭总览头像来源差异，不代表全客户端等价完成。

### 2026-10-09 — GPUI 设置保存范围和未提交草稿遵循 Vue

- Vue 的种植策略和自动化使用独立保存按钮，只提交各自分组字段；GPUI 之前提交整份共享设置快照，并在成功后重载页面，可能把隐藏分组的未提交修改一起写入或丢弃。现在按当前分组投影 Vue 保存字段，成功后保留其他分组草稿；自动化肥料检查仍只在保存自动化且开启自动购买时运行。
- **验证通过**：新增窗口回归在策略保存后确认共享配置的自动化值未变、页面里的未提交自动化草稿仍在；快照投影单测确认分组字段互不串写。GPUI 全量测试、严格编译、Clippy、格式及差异检查在本条后复验。真实账号和运行中 worker 的配置热重载仍待端到端验收；本修复只关闭设置保存范围差异，不代表全客户端完成等价。

### 2026-10-09 — GPUI 活动奖励反馈和星砂兑换回执对齐 Vue

- 活动操作响应未携带奖励列表时，战令领取和节气领取现在从当前快照回退奖励；星砂兑换确认保留商品原始奖励资料，并按购买数量生成回执。反馈继续尊重服务端消息、Vue 的奖励名称/数量显示和无可领取状态；活动分组标题也按所选语言显示。
- **验证通过**：GPUI 全量 302 项测试、`cargo test --workspace`、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实账号兑换与服务端响应仍待端到端核验；其它页面、实机窗口和跨平台待验项继续保留，不能据此宣称全客户端已完全等价。

### 2026-10-09 — GPUI 账号表固定操作列与主表顶边对齐

- 原生预览发现右侧固定操作列被垂直居中，导致操作表头和账号行落在数据行下方。账号表现在将固定列对齐到表格起点；最小窗口回归比较固定列和主表的纵向坐标，避免这一视觉错位回归。
- **验证通过**：GPUI 全量 302 项测试（含账号交互组 15 项）、原生预览 18 个场景、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -- -D warnings`、格式检查和 `git diff --check`。预览只覆盖现有固定场景；其它页面的完整布局、操作与真实窗口体验仍需逐项核对，不能据此宣称全客户端完全等价。

### 2026-10-09 — GPUI 活动规则入口恢复 Vue 弹窗交互

- 对照 Vue 的 `ActivityRulesDialog`，旅行战令和观星规则按钮应打开居中模态窗，而不是把规则插入页面并切换成“收起规则”。GPUI 现保持按钮原标签和页面布局，规则在独立对话框内显示，支持 Escape、遮罩和关闭按钮；规则内容只在对应详情或慈善页面展示，不再泄漏到活动目录、商城或宠物日记。
- 新增 GPUI 窗口回归覆盖旅行战令/星座规则打开、无内嵌规则、Escape 关闭和按钮状态不变。
- **验证通过**：GPUI 全量 303 项测试、活动专项 5 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check` 及 `corepack pnpm@12.3.4 --dir desktop-ui typecheck/build`。这些自动化用例只关闭该项交互差异；真实窗口视觉/焦点与账号后端、跨平台验收仍待完成，不能据此宣称全客户端已完全等价。

### 2026-10-09 — GPUI 萌宠夺宝查询状态遵循 Vue

- 同一好友再次查询时，Vue 会在请求完成前继续显示上次宝藏和挑战书；GPUI 之前在发请求时立即清空，造成内容闪断。现在同 GID 查询保留已加载结果，换目标时仍隐藏/清除旧目标，失败后清掉过期结果并将错误显示在夺宝分区，不再作为整页错误；切回已加载好友时保留其查询结果。
- 窗口回归覆盖切回好友后恢复挑战书列表及好友错误只显示在局部区域；纯状态回归覆盖同目标缓存保留和失败清理。
- **验证通过**：GPUI 全量 305 项测试、萌宠日记专项 6 项、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实好友服务返回、查询失败和战斗操作仍待在线账号验收；全页面差异与跨平台实机验收继续进行，不据此宣称全客户端完全等价。

### 2026-10-09 — GPUI 分析操作按子业务拆分并补齐缺失名称回退

- 收益分析之前把黑名单持久化和优先种子操作留在 `desktop/analytics.rs`。现在该文件只声明业务模块，黑名单保存/反馈与优先种子操作分别归入 `desktop/analytics/blacklist.rs`、`preferred_seed.rs`；页面视图和策略排名继续各自留在 `analytics/view/`。`docs/ARCHITECTURE.md` 已同步更新该结构。
- 逐项核对 Vue 的分析字段和排序时发现，黑名单作物名称为空时 Vue 会回退到“种子 ID”，GPUI 原先渲染为空白；现在中英文均走同一回退。策略对比的五个指标字段、参考等级过滤、并列时保留源顺序和缺失指标回退也与 Vue 实现逐项核过。
- **验证通过**：GPUI 全量 307 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。Rust 对 `block v0.1.6` 仍给出上游 future-incompatibility 提示。真实账号设置读写、在线排名结果和窗口实机仍待验；这次只关闭已确认的空名称显示差异，不表示分析页或全客户端的真实后端验收已完成。

### 2026-10-09 — GPUI 好友互动状态遵循 Vue 真值语义

- 对照好友页的 `friendStatusText`/`hasActionStatus`，Vue 先按 JavaScript 真值决定是否显示原始互动计数；GPUI 此前将计数转整数并要求大于零，字符串 `"0"` 和负值等数据会显示不同。现在状态文案及状态强调按 Vue 真值/字符串转换，实际操作按钮仍按数值大于零门控。
- 在 `desktop/friends/helpers.rs` 增加原始计数的中英文回归，包括数字零、字符串零、负值、空值和布尔值。**验证通过**：好友专项 39 项、`RUSTFLAGS="-D warnings" cargo check -p qq-farm-gpui --all-targets`、GPUI Clippy `--all-targets --no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。
- 此项只关闭一个异常数据形态下的显示差异；常规线上计数及全部好友操作仍待真实账号复核，所有页面、实窗和跨平台验收继续进行。

### 2026-10-09 — GPUI 账号删除确认范围对齐 Vue

- 对照 `desktop-ui/src/views/farm/account/index.vue`：Vue 单行删除才打开确认气泡，批量删除直接处理当前页已选行。GPUI 之前给两种操作都加了自制遮罩确认框，且从账号页和全局内容层重复挂载。现在单行删除使用 GPUI Kit AlertDialog，文案跟随中英文；Escape/取消清除待删状态并恢复打开前的焦点。批量删除直接调用当前页选中 ID，不再出现额外确认。
- 窗口回归覆盖英文确认与 Escape 后焦点恢复、中文取消、确认删除回调，以及跨页勾选时只处理当前页且不弹确认。**验证通过**：`cargo test -p qq-farm-gpui accounts -- --test-threads=1`（28 项）、`RUSTFLAGS="-D warnings" cargo check -p qq-farm-gpui --all-targets`、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。测试使用隔离数据目录；真实账号删除和运行中 worker 停止仍需在线实机复核，其它页面继续逐项对照。

### 2026-10-09 — GPUI 游戏配置删除流程按 Vue 拆分

- 删除入口与呈现分别放入 `desktop/game_config/delete.rs`、`delete_view.rs`，表格勾选逻辑放入 `table/selection.rs`；`desktop/game_config.rs` 只组合子模块。Vue 单行删除才显示行锚定确认；GPUI 现用 Kit Popover 紧贴删除按钮，取消、Escape 和确认都会关闭气泡。两端批量删除都直接逐项执行并清空选择，至少一项成功后刷新目录。
- **验证通过**：游戏配置专项 23 项测试，覆盖单行取消/Escape/真实配置覆盖删除、批量立即执行及清空选择。真实账号之外的远端删除与三平台实窗仍待验；其它业务页继续逐项对照，不据此宣称全客户端完成等价。

### 2026-10-09 — GPUI 离线提醒保存保留其他设置草稿

- Vue 保存离线提醒只应用该接口返回的数据，不重载整个设置页。GPUI 原先保存后刷新完整设置快照，会覆盖其它分组尚未保存的草稿；现在保存只更新并持久化离线提醒数据，保留其它草稿。新增窗口回归确认请求版本不变、未提交的系统配置草稿仍在，且提醒设置确实落盘并可重新读取。
- 补充架构说明：`desktop/activities.rs`、`friends.rs`、`settings.rs` 等是模块入口和导出层，实际页面状态、视图、操作按业务放在同名目录；内容路由器只负责组合页面区块。
- **验证通过**：GPUI 设置专项 44 项测试、`RUSTFLAGS="-D warnings" cargo check -p qq-farm-gpui --all-targets`、GPUI Clippy `--all-targets --no-deps -- -D warnings`、格式检查和 `git diff --check`。Rust 仍提示上游 `block v0.1.6` 的 future-incompatibility；其它页面、真实账号及跨平台实机验收仍需继续，不能据此宣称全客户端已完全等价。

### 2026-10-09 — GPUI 农场提示和游戏配置批量删除确认遵循 Vue

- 更正上一条游戏配置删除记录：Vue 的 `TableHeaderOperation` 用 `NPopconfirm` 包住批量删除，因此 GPUI 批量删除也必须先确认。现在取消会保留选择，确认后才删除；窗口回归实际验证取消不改数据、确认后移除隔离配置记录。
- 农场地块的变异说明、互动道具 ID 回退、无名称互动和待清理提示补齐 Vue 对应的悬停信息与徽标条件；长名称及天气任务/召唤原因恢复截断提示和 tooltip。
- 目录结构复核：GPUI 当前有 389 个 Rust 文件，按 `desktop/accounts`、`personal`、`friends`、`activities`、`settings`、`game_config` 等业务目录组织；`desktop/mod.rs` 等是模块声明/导出层，不是把页面实现集中在一个文件。生产 Rust 单文件最大约 433 行。目录结构不代表全部功能已等价。
- **验证通过**：GPUI 全量 315 项测试、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI Clippy `--all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实账号/后端操作、实窗视觉与 Windows/Linux 实机验收仍待完成；本条不表示全客户端已完全等价。

### 2026-10-09 — GPUI 删除确认交互与账号测试模块校正

- 更正上方“GPUI 账号删除确认范围对齐 Vue”记录：Vue 的账号单行删除和批量删除均通过 `NPopconfirm` 确认；此前记录的“AlertDialog”“批量删除直接执行”和“焦点恢复”不准确。GPUI 现在两种删除都使用锚定触发按钮的 Kit Popover：取消/Escape 不执行删除，确认才执行；批量删除仍只作用于当前页选中行。好友捣乱、拉黑、删除和移出黑名单也与 Vue 的确认气泡统一使用 Popover。
- 将原 637 行账号交互测试拆成 `tests/accounts/{navigation,filters,search,actions,presentation,table}.rs`，父模块只保留共享窗口和弹层等待辅助；在架构文档列出该测试布局。当前 GPUI crate 共 394 个 Rust 源文件，按业务目录组织。默认 `git status --short` 会把整个未跟踪 crate 折叠成一行；展开清单使用 `--untracked-files=all`。
- **验证通过**：账号相关 32 项测试、账号删除确认 3 项测试、好友确认 2 项测试及 GPUI 全量 318 项测试；`cargo fmt --all -- --check`、`git diff --check` 和普通 `cargo clippy -p qq-farm-gpui --all-targets` 通过。严格 `-D warnings` Clippy 被 `qq-farm-core/src/services/commerce/service.rs` 两条既有告警挡住（冗余 `i64::from`、手写 `Result::ok`）；本轮未改该业务文件。真实后端账号与好友操作、焦点实机行为及 Windows/Linux 实机仍待验，不能据此宣称所有功能已完全对齐。

### 2026-10-09 — GPUI 分析页黑名单写入遵循 Vue 专用命令

- 对照 `desktop-ui/src/service/api/farm.ts` 与 `commands/farm.rs`：Vue 分析页调用 `farm_set_plant_blacklist`，直接保存当前账号名单；GPUI 此前借用 `settings::save_settings`，会额外重载 worker 并递增运行时配置版本。现 app 门面返回具体 `Vec<i64>`，Tauri 命令仍序列化为同一 JSON 数组，GPUI 改用专用写入；不会触发整份设置保存副作用。
- 黑名单窗口回归现在验证：选择器更改不刷新页面请求、不递增运行时配置版本、当前账号名单正确持久化且其他账号保持不变；空账号仍显示写入错误并保留原选择。
- **验证通过**：分析页专项 19 项、GPUI 全量 318 项、`cargo check --workspace --all-targets`、`cargo test --workspace`、涉及 crate 的严格 Clippy `--no-deps -D warnings`、格式检查和 `git diff --check`。该项关闭一个设置写入副作用差异；真实运行中账号的窃取策略热更新仍待在线验收，其它页面及 Windows/Linux 实机仍未全验，不表示全客户端已完成对齐。

### 2026-10-09 — GPUI QQ Bot 绑定完成后保留其它设置草稿

- 对照 Vue 设置页：QQ Bot 绑定成功/解绑只刷新离线提醒和绑定状态；系统配置保存只提交，不重载整页；重置只更新系统配置表单。GPUI 原先这些路径刷新整页设置快照，会覆盖尚未保存的种植策略、自动化、提醒或系统配置草稿。现在绑定、解绑和系统配置保存/重置只更新对应分组，保留其它草稿；重置会同步默认值到可编辑输入框。
- 新增窗口状态回归验证绑定数据局部合并后其它分组草稿不变、页面请求版本不变；系统配置保存/重置使用同一请求版本并保留其它草稿。**验证通过**：相关专项测试、GPUI 全量 319 项、workspace 严格编译、涉及 crate 的 Clippy `--no-deps -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实 QQ 扫码绑定和所有页面真实后端/跨平台实机验收仍待完成，不能据此宣称全客户端已完全对齐。

### 2026-10-09 — GPUI 事件与设置实现继续按职责归档

- 复核 `qq-farm-gpui` 源码后确认客户端不是单文件实现：拆分前有 394 个 Rust 源文件；新增两个职责模块后共 396 个。账号、农场、好友、活动、分析、商城、设置和游戏配置各有业务目录；活动再按玩法拆分，测试也按页面分组。把实时事件的 app 总线订阅、账号隔离过滤从窗口监听器中移到 `runtime/events/{subscription,account_filter}.rs`；把系统配置保存和重置集中到 `settings/system_config.rs`，分组策略/自动化保存留在 `settings/save.rs`。同步更新架构目录图。
- **验证通过**：GPUI 全量测试 319 项通过、`cargo fmt --all`、GPUI 严格 Clippy、`git diff --check`。这是无行为变化的结构整理；仍需按既有清单完成真实后端操作、窗口实机和 Windows/Linux 验收。

### 2026-10-09 — GPUI 设置与活动目录继续按职责拆分

- `desktop/settings.rs`、`desktop/friends.rs` 等根文件只声明/导出模块；页面实现位于同名业务目录。将运行环境表单渲染移至 `settings/system_config/view.rs`，保存、重置和设备预设留在 `settings/system_config/mod.rs`；活动目录模型测试移至 `activities/directory/model/tests.rs`，数据整理文件保留生产逻辑。GPUI 源码现有 398 个 Rust 文件，业务目录索引见 `docs/ARCHITECTURE.md`。
- **验证通过**：GPUI 全量 320 项测试、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。本次只有模块归档，没有改业务行为；目录结构与单测通过不等于所有页面已完成真实后端及各平台实机验收。

### 2026-10-09 — GPUI 离线提醒测试请求使用独立状态

- 对照 Vue 设置页的 `offlineTesting` / `offlineSaving`：测试通知期间 Vue 只显示测试按钮 loading、禁用提醒保存；GPUI 原先复用窗口级 `loading`，导致其它设置表单一并锁定。现在由 `SettingsState::offline_test_loading` 管理该请求，重复测试和保存会被拦截，但提醒字段、服务选择和其它设置保持可操作；成功、业务失败和传输失败都清除该状态。
- 新增窗口回归检查测试中可编辑提醒表单且页面未进入全局 loading，新增完成处理回归覆盖失败反馈和状态复位。
- **验证通过**：离线提醒专项 5 项、GPUI 全量 322 项、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。真实通知服务仍需在线账号验证；这项关闭测试进行中锁住整页设置的交互差异，不代表全客户端已完成等价验收。

### 2026-10-09 — GPUI 设置分组保存状态与 Vue 一致

- 对照 `desktop-ui/src/views/farm/settings/index.vue` 的 `strategySaving` / `automationSaving`：GPUI 原先共用窗口级 `loading`，切换设置分组时旧请求完成还可能留下全页锁定。现在种植策略和自动化分别持有保存状态，只 loading 当前分组的保存按钮；字段与另一分组仍可编辑，切换账号/页面后旧结果不覆盖当前提示，但会清理原分组的 pending 状态。状态和视图继续分别放在 `settings/state.rs`、`settings/save.rs`、`settings/controls/automation/`、共用工具栏及 `tests/settings/automation.rs`，不把业务重新塞回单个文件。
- **验证通过**：定向 stale completion 回归、GPUI 全量 323 项、GPUI 严格 Clippy `--all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。当前 crate 为 398 个 Rust 源文件，页面、能力、视图和测试按业务目录拆分（目录索引见 `docs/ARCHITECTURE.md`）。真实账号操作和 Windows/Linux 实机验收仍待完成；自动化回归通过不能证明全客户端已完成真实后端/平台等价验收。

### 2026-10-09 — GPUI 成长任务进度遵循 Vue 原始值语义

- 继续核对 Vue `TaskPanel.vue` 与 GPUI 每日礼包/成长任务。Vue 用 JavaScript 真值判断是否显示进度：字符串 `"0"` 是已提供值，因此 `progress="0"` 且 `totalProgress="0"` 时展示 `进度：0/0`。GPUI 原先先转整数、再把两个零当成空值，错误显示成“进行中”。现按 Vue 真值判断是否为空，数字字符串仍可显示为数值进度；修复在 `personal/daily_gifts/labels.rs`，视图仍在该业务目录。
- **验证通过**：窗口回归覆盖数字字符串 `0/0` 的实际标签，真值单测覆盖缺失值、数字/布尔零、字符串零和空字符串；GPUI 全量 324 项、GPUI 严格 Clippy、macOS `cargo build --release -p qq-farm-gpui`、`cargo fmt --all -- --check` 和 `git diff --check`。本机尝试 Windows 交叉 `cargo check`，但 C 依赖编译缺少 Windows SDK 头文件（`windows.h`、标准库头文件），所以 Windows 仍需由 workflow 原生 runner 验证；这是本机工具链限制，不作为 Windows 源码通过的证据。这关闭任务进度显示的一个输入形态差异；农场真实服务数据、其余页面和跨平台实机验收继续进行。

### 2026-10-09 — GPUI 果实配置植物选择器遵循 Vue 目录字段

- 对照 `desktop-ui/src/views/farm/game-config/modules/fruit-operate-drawer.vue`：Vue 植物选项标签包含名称、种子编号和植物编号，搜索也可按种子编号查找。GPUI 原来只匹配植物名/编号且标签不显示种子号；现在列表、选中项及中英文标签都展示两类编号，搜索兼容 `seedId`/`seed_id` 与 `plantId`/`id`。代码按业务分别位于 `desktop/game_config/form.rs` 的字段解析、`form_view/fruit.rs` 的果实表单视图和 `tests/game_config/forms.rs` 的窗口回归。
- **验证通过**：GPUI 全量 325 项测试、选择器字段单测与真实窗口表单回归、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。测试及编译仅验证本地目录输入和选择回填；线上目录和配置写入仍需真实账号核验，其余页面与跨平台实机验收继续进行，不据此宣称全客户端已完全等价。

### 2026-10-09 — GPUI 分页页码与 Naive UI 可直跳行为对齐

- 对照本地 Naive UI 2.45.3 分页实现：账号和游戏配置桌面分页使用 `pageSlot: 9`，好友分页使用 `pageSlot: 5`。GPUI 账号/配置原先只有上一页、下一页，好友仅连续显示五个页码且没有首尾省略跳转。现新增共享 `desktop/view/pagination.rs`，按 Naive UI 的首尾页、中间页窗和省略跳转规则生成页码；账号、好友、游戏配置三处均可直达可见页，并可点省略号跳到相邻隐藏页。架构文档已登记共享分页模块。
- 窗口回归覆盖账号与游戏配置页码直跳、折叠页段跳转和记录范围切换；好友现有窗口回归覆盖页码直选；纯函数回归覆盖 `pageSlot` 为 5/9 时的首尾、中间与跳转目标。
- **验证通过**：GPUI 全量 327 项测试、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。这是三个列表的本地分页交互回归，不代表其余页面或真实后端、平台实机验收已完成。

### 2026-10-10 — GPUI 游戏配置搜索草稿与提交时机对齐 Vue

- 对照 `desktop-ui/src/views/farm/game-config/modules/game-config-search.vue`：Vue 搜索面板默认折叠，关键词与分类筛选先留在表单草稿，点击“搜索”才筛选目录；“重置”只清空草稿，不立即更改当前结果。GPUI 原先复用全局即时搜索且筛选常驻，操作时序不同。现新增 `desktop/game_config/search_panel.rs`，区分草稿与已应用条件，并隐藏该页通用即时搜索。
- 窗口回归验证默认折叠、输入草稿不改变结果、提交后过滤，以及重置只清表单并由再次搜索更新结果；英文筛选表单回归已适配折叠面板。
- **验证通过**：GPUI 全量 328 项测试、GPUI 严格 Clippy `--all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。这是游戏配置搜索时序的自动化回归，不表示其它筛选选项、真实配置写入/图片流程或全客户端已完成端到端等价验收。

### 2026-10-10 — GPUI 账号、配置和总览筛选按业务归档并对齐下拉交互

- 账号筛选的渠道、运行状态、微信授权状态，以及游戏配置的季数、品质、道具类型都使用单选下拉控件，并保留 Vue 的草稿/搜索/重置时机。总览日志的模块、事件、等级筛选统一为下拉菜单；各项实现仍在 `desktop/accounts/`、`desktop/game_config/`、`desktop/dashboard/logs/` 对应业务目录，窗口回归按对应页面维护。
- crate 实际不是单文件：当前 `src/` 有 402 个 Rust 文件，页面能力位于各自业务目录；共享 `desktop/view/` 只承担窗口骨架和跨业务展示组件。新增 `crates/qq-farm-gpui/README.md` 作为快速目录索引，完整模块图仍见 `docs/ARCHITECTURE.md`。
- **验证通过**：GPUI 全量 330 项测试、严格 Clippy `cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check`。日志菜单窗口回归也验证了连续切换筛选时弹层关闭和选择状态。真实后端、实窗与 Windows/Linux 仍待验；测试通过和目录拆分都不代表所有页面已完成端到端等价。

### 2026-10-10 — GPUI 设置页呈现与预览资源复核

- 设置快照不再被通用列表重复渲染为原始 JSON 路径；设置页只显示按业务分组的表单，分组无通用列表时也不再额外出现空态刷新卡片。新增窗口回归确认这两种重复内容不会出现。
- 神秘商店库存标记改用可读的主题主色；活动目录时间戳换算补齐 Vue `Number` 对数字、数字字符串、布尔值、空字符串及秒/毫秒输入的语义。预览夹具改用仓库真实游戏图片和有效种子字段，覆盖总览、农场、商城、神秘商店及配置列表。
- 结构核对：`crates/qq-farm-gpui/src` 当前有 402 个 Rust 文件，账号、农场、好友、活动、分析、商城、设置、游戏配置分别位于业务目录，活动玩法、表单、表格、运行时与页面测试继续下分子模块。`desktop/*.rs` 多为模块入口，具体页面实现不集中在一个文件；目录索引见 [`crates/qq-farm-gpui/README.md`](../crates/qq-farm-gpui/README.md)，拓扑见 [`docs/ARCHITECTURE.md`](ARCHITECTURE.md)。
- **验证通过**：`cargo test -p qq-farm-gpui --features ui-preview -- --test-threads=1`（332 项）、严格 GPUI Clippy、workspace 严格编译、Rust 格式与差异检查、Vue typecheck/build；生成并复核了 32 张 GPUI 页面/主题预览图。真实账号在线操作、实窗输入/缩放、托盘生命周期及 Windows/Linux 原生运行和打包仍待验；本条不表示所有功能已完成端到端等价验收。

### 2026-10-10 — GPUI 秋日祈愿选择恢复 Vue 单选交互

- 对照 `desktop-ui/src/views/farm/activity/autumn-view.vue`：Vue 使用禁用态单选组选择祈愿奖项；GPUI 原先用普通按钮，缺少单选语义和方向键选择。现改为带 `RadioButton` 可访问角色的单选组，支持鼠标聚焦、方向键循环选择、Home/End、焦点跟随和不可祈愿时禁用；回归验证选中状态、焦点移动、循环边界与禁用门控。
- 复核用户提到的文件布局：GPUI crate 当前有 402 个 Rust 源文件，按账号、农场、好友、活动、分析、商城、设置、游戏配置等业务分目录，页面聚合入口只负责组合与分发；crate README 已列出业务路径。默认 `git status --short` 将未跟踪的新 crate 折叠为单行，展开状态会列出其内部文件。
- **验证通过**：祈愿交互专项 2 项、GPUI 全量 331 项、启用 `ui-preview` 的全量 332 项、GPUI 严格 Clippy、workspace 严格编译、格式检查和 `git diff --check`。隔离数据目录的 macOS 进程启动成功；当前桌面环境拒绝截图，因此未把窗口视觉复核记作通过。真实账号操作、完整实窗输入/缩放、托盘生命周期及 Windows/Linux 运行和打包仍待验；本条不表示全客户端已完成端到端等价验收。

### 2026-10-10 — GPUI 账号备注接入共享持久化

- Vue 表单和 API DTO 包含 `remark`，app 的 `UpsertAccountRequest` 接收可选备注，`AccountRecord` 以默认空值兼容旧 JSON；GPUI 新增、更新、普通 Code、本机微信和扫码微信保存都会写入备注。Vue 的 Tauri API 适配器当时尚未转发该字段，后续修正在本文“GPUI 与 Vue 账号保存字段对齐”记录。编辑旧账号且响应缺少备注时，表单按 Vue 适配器回退到账号名。IPC 命令名未变，新增字段可省略。
- 窗口级持久化回归验证旧数据回填、保存后内存与 `accounts.json` 读回；core 单测验证旧账号 JSON 缺少新字段仍可加载。
- 账号表单回归单独归档在 `desktop/tests/accounts/forms.rs`；当前 GPUI crate 有 403 个 Rust 源文件，页面与测试仍按业务目录维护。
- **验证通过**：workspace 全量测试（app 16、core 1045、GPUI 332、desktop 及按原配置忽略的 doctest 均通过）、GPUI 预览模式全量 333 项、app/core/GPUI 严格 Clippy、workspace 严格编译、Rust 格式检查、Vue typecheck/build 和 `git diff --check`。真实账号操作、实窗和 Windows/Linux 实机验收仍待完成，不据此宣称全客户端完全等价。

### 2026-10-10 — GPUI 收益分析、地块操作和仓库交互源码复核

- 逐项对照 Vue 的收益分析策略指标与排名顺序、参考等级筛选、黑名单候选和增删保存；对照个人农场的合并地块、成熟倒计时、操作按钮门控，以及仓库分类、系统道具隐藏、批量出售选择和道具使用流程。当前代码在这些检查点保持对应语义，本轮没有发现需要修改的业务差异。
- 源码结构复核：`crates/qq-farm-gpui/src` 有 403 个 Rust 文件；账号、活动、好友、分析、商城、设置、游戏配置、个人农场及各自测试按业务目录维护。根模块只负责注册/组合；默认 Git 状态把未跟踪 crate 折叠成目录项。快速索引见 `crates/qq-farm-gpui/README.md`。
- **验证通过**：个人农场定向测试 39 项、收益分析定向测试 19 项。此源码与自动化验证不覆盖真实账号接口时序、真实窗口键鼠/缩放、托盘生命周期及 Windows/Linux 实机运行；完整端到端等价仍未完成。

### 2026-10-10 — GPUI 好友并发删除清理各行状态

- 复核好友页删除回调发现全局请求代次会相互覆盖：删除不同好友并发执行时，后启动的请求会令先启动的回调直接返回，首行一直显示 loading。现在完成回调按好友 GID 与该行请求票据匹配并单独清理；页面/账号失效仍通过清空待处理映射让旧回调退出。
- **验证通过**：并发删除与过期票据纯逻辑回归 2 项、好友窗口测试 15 项。自动化未连接真实账号执行游戏内删除；服务端删除及黑名单联动仍需实账号验收。

### 2026-10-10 — GPUI 星砂商店目录字段与余额回退对齐 Vue

- 对照 `desktop-ui/src/views/farm/activity/index.vue`：Vue 的商店分类允许字符串及 `{ id, name }`，商品分类按 `categoryId || categoryName || category` 对比分类 ID；余额依次读取 `balance`、首个 `currencies[].balance`、`currency.balance/count`。GPUI 先前只识别字符串分类、`category` 和顶层余额。现在补齐字段优先级、Vue 字符串转换、说明文案与余额回退，筛选后商品操作控件也使用原目录索引保持稳定。继续核对后还修正缺省 `exchangeable` 门控、兑换状态文案和商品名称回退顺序。
- **验证通过**：GPUI 全量 340 项测试，含对象分类/分类 ID/备用字段、商品门控窗口回归和余额/名称回退单测；严格 GPUI Clippy、格式检查及 `git diff --check` 通过。未连接真实活动商店账号验证服务端商品目录和兑换结果；该项源码及自动化回归不代表全客户端已完成实机端到端等价。

### 2026-10-10 — GPUI 交互测试按业务工作流拆分

- 好友窗口回归拆到 `tests/friends/` 的展示、操作、加载、实时事件和访客记录；商城拆到普通商城、购买确认、星砂商店和神秘商店；游戏配置目录拆到搜索、表格、删除和页面生命周期。生产代码仍按账号、农场、好友、活动、分析、商城、设置及游戏配置分目录，索引同步在 crate README 与架构文档。
- **验证通过**：GPUI 全量 341 项测试、`cargo test -p qq-farm-gpui --no-run`、严格 GPUI Clippy、`cargo fmt --all -- --check` 和 `git diff --check`。这是无行为变化的测试结构整理；不改变全客户端业务等价和真实后端、窗口及跨平台待验状态。

### 2026-10-10 — GPUI 好友与天气好友区按业务职责拆分

- 好友页辅助逻辑拆成搜索排序、状态文案、快照更新和独立测试；天气活动好友区拆成数据映射、好友列表与现场详情，页面入口只负责宽/窄窗口组合。更新 README 与架构模块图，明确根级 `friends.rs`、`activities.rs` 是模块入口，完整实现位于同名目录。
- 同轮继续核对个人农场离线态：Vue 显示“账号未运行”空状态并隐藏分段面板，GPUI 也隐藏农场业务区并提示启动账号；已有窗口回归覆盖离线时不显示业务行和操作，本轮未发现交互差异。
- 这次是文件组织调整，没有更改业务判断或界面交互；`src/` 现有 423 个 Rust 文件。由于整个新 crate 尚未跟踪，默认 `git status --short` 会把 crate 内部文件折叠为一个目录项；用 `git status --short --untracked-files=all` 可展开查看。
- **验证通过**：`cargo check -p qq-farm-gpui --all-targets`、好友辅助规则 8 项、GPUI 全量 341 项、严格 Clippy `cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。这是文件组织调整，不代表剩余页面、真实账号服务或其他平台实机验收已全部完成。

### 2026-10-10 — GPUI 总览刷新逻辑归入业务子模块

- 总览入口 `desktop/dashboard.rs` 原本还包含在线计时与状态/资产刷新。现拆到 `desktop/dashboard/clock.rs`、`refresh.rs`，入口只保留总览状态、子模块声明及页面组合；同步更新 crate README 与架构图。
- 源码核对时 `src/` 共 425 个 Rust 文件；业务页按目录维护。Git 对未跟踪 crate 的默认折叠显示已在 README 解释，并提供展开命令。
- **验证通过**：`cargo check -p qq-farm-gpui --all-targets`、GPUI 全量 341 项测试、严格 GPUI Clippy、`cargo fmt --all -- --check` 和 `git diff --check`。此模块整理未改页面行为，也不代表全部页面已完成真实后端和跨平台验收。

### 2026-10-10 — GPUI 好友刷新范围及展开详情遵循 Vue

- 对照 `desktop-ui/src/views/farm/friends/index.vue`：好友列表刷新应清缓存并重载当前展开好友的农场详情；访客记录与互动道具页的手动刷新只应读取各自数据。GPUI 原来在访客/道具分段触发整页刷新，额外重载好友数据；列表刷新后也保留旧的展开详情。现按当前分段显示对应刷新入口、分别读取记录/道具；好友列表刷新完成后重载展开中或正在打开的详情。请求继续受账号与页面版本约束。
- `friends/` 与 `runtime/` 按页面状态、按钮、请求和结果处理职责拆分；未将实现塞回公共 shell。
- `src/` 当前共 426 个 Rust 文件；账号、农场、好友、活动、分析、商城、设置、游戏配置与各页测试按业务目录维护，模块索引见 crate README 和架构图。
- **验证通过**：新刷新范围窗口回归、详情刷新目标回归、GPUI `ui-preview` 全量 344 项测试、`cargo clippy -p qq-farm-gpui --all-targets --no-deps -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`。好友服务真实账号、各平台窗口输入/缩放及 Windows/Linux 原生运行仍待验；此项关闭一组已定位的交互差异，不代表全客户端已完成端到端等价验收。

### 2026-10-10 — GPUI 账号和务农操作入口按 Vue 收窄

- 对照 Vue 页面确认账号启停只在账号表格行内提供，GPUI 顶栏额外的“启动/停止账号”会重复并绕开行级操作；现已移除，账号表仍保留编辑、启停、删除，页面工具保留新增、批量删除和刷新。Vue 的“一键务农”只属于“我的农场”土地面板且按可操作地块显示，GPUI 此前把整轮务农按钮挂在总览和全部个人农场分段的通用顶栏；现移除通用入口，地块分段继续按 Vue 门控显示对应操作。
- **验证通过**：GPUI `ui-preview` 全量 345 项测试（含账号顶栏/行操作与总览、土地分段按钮范围回归）、严格 Clippy、`cargo check -p qq-farm-gpui --all-targets`、格式检查和 `git diff --check`。源码现有 425 个 Rust 文件，页面按业务目录拆分。真实账号服务调用、全窗口视觉与键鼠验收、托盘生命周期及 Windows/Linux 实机仍待完成；本条只关闭已定位的多余全局操作入口，不表示全客户端已完成端到端等价验收。

### 2026-10-10 — GPUI 账号页工具栏操作层级

- Vue 只在账号页表格工具栏提供“添加账号”；GPUI 原先还在全局侧栏和空结果状态提供重复入口。现移除两个重复项，新增账号只从账号页表格工具栏触发，空结果保持纯空状态。该按钮使用轻量样式，“保存设置”仍保留主操作强调。
- **验证通过**：GPUI `ui-preview` 全量 345 项测试；窗口回归确认账号页从表格工具栏打开表单、搜索无匹配时仍保留工具栏入口且空态不添加额外按钮、其它页面不出现全局添加入口。账号页浅色与最小窗口预览复核仍适用；本项只收敛入口范围，不表示全客户端已完成等价验收。

### 2026-10-10 — 共享桌面启动锁确认唤起请求

- `EngineLease::wake_existing_owner` 此前写出令牌后立即返回 owner，未等待锁持有方确认；启动竞争时调用方可能把尚未接收的请求当成已唤起。现改为客户端半关闭写入流并等待短时应答；服务端只在令牌正确且唤起事件成功入队后回复 `ok`，否则回复 `no`。
- 扩展共享锁回归：无效令牌收到 `no` 且不会产生唤起事件，正常跨宿主请求收到确认、进入唤起队列，并在释放后允许重新获取。
- **验证通过**：`cargo test -p qq-farm-app runtime_lease -- --test-threads=1`（3 项）、`cargo test -p qq-farm-app -- --test-threads=1`（20 项）、GPUI 与 Tauri `--all-targets` 编译检查。另有子进程强制退出测试验证操作系统释放目录锁；GPUI/Tauri 两端真实窗口唤起及托盘生命周期仍待实机验证，不能据此宣称全部平台和页面已完成等价验收。

### 2026-10-10 — GPUI 神秘商店导航收敛到商城标签

- 对照 Vue 路由：`/farm/mystery-shop` 隐藏于侧栏并重定向到商城的“神秘商人”标签。GPUI 之前同时暴露独立侧栏页和商城标签，形成重复入口；现移除独立页，商品加载、摘要、详情和购买统一由商城分段处理。旧版 `mystery-shop` 导航偏好会迁移为商城的神秘商人标签并持久化为 `mall`。移除重复页面模块后，GPUI 当前有 425 个 Rust 源文件，业务仍分目录维护。
- **验证通过**：旧偏好迁移、神秘商店商城映射和购买流程定向测试 4 项，英文动态摘要测试 1 项；启用 `ui-preview` 的 GPUI 全量测试 346 项、`cargo fmt --all`。真实商城后端购买、窗口实机和 Windows/Linux 实机仍待验；此项不表示全客户端已完成端到端等价。

### 2026-10-10 — GPUI 秋日玩法详情交互按 Vue 路由

- 对照 `autumn-view.vue`：Vue 在活动详情头部提供当前玩法刷新；玩法切换通过返回活动目录后选择对应活动。GPUI 原先成功加载时没有本地刷新，还在工具栏增加模式切换，并一次请求 wish/happy 两份快照，导致单玩法失败会被另一份成功掩盖。现补回当前玩法刷新、移除额外模式切换、仅读取目录选中的玩法，并保留另一个玩法已加载的快照；进入详情时先设置玩法再启动请求。
- **验证通过**：GPUI 秋日定向测试 8 项，覆盖目录选中模式、模式请求键、详情刷新和两种模式无额外切换入口；本次完整 `ui-preview` 测试 349 项全通过，workspace `RUSTFLAGS="-D warnings" cargo check --workspace --all-targets` 与 GPUI 严格 Clippy 通过。真实活动服务的响应时序和操作仍需在线账号验收。

### 2026-10-10 — GPUI 萌宠日记快照刷新入口

- 对照 `pet-diary-view.vue`：Vue 在萌宠日记摘要卡标题栏提供“刷新”，GPUI 原先没有该入口。现增加标题栏刷新，并只重新读取萌宠日记快照；保留已加载的好友列表，好友列表仍由单独的“刷新好友”操作更新。
- **验证通过**：萌宠日记窗口专项回归覆盖刷新按钮、快照失败时隐藏旧活动区并保留重试、操作后仍留在当前玩法，以及既有好友筛选/操作门控；本次 GPUI `ui-preview` 全量测试 349 项通过，workspace 严格编译与 GPUI 严格 Clippy 通过。真实活动和好友服务响应仍需在线账号验收。

### 2026-10-10 — GPUI 天气详情刷新与快照失败状态对齐

- 对照 `weather-view.vue`：Vue 天气卡标题栏可同时刷新天气快照与好友目录；GPUI 原先只能用全局工具栏刷新。现补天气卡内刷新入口，沿用同一快照/好友联合加载，并在请求期间禁用重复提交。快照请求失败时清除旧快照、隐藏旧活动操作区，在活动卡内显示错误和重试入口；好友目录独立失败仍保留上一份好友列表。
- 新增窗口回归覆盖卡片刷新、失败状态隐藏旧操作、重试后清除错误和保留当前活动路由；没有改动天气业务 API 或好友操作门控。
- **验证通过**：GPUI `ui-preview` 全量 349 项、workspace `RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、GPUI 严格 Clippy。真实天气服务响应、实窗键鼠与跨平台运行仍需验收；这些自动化结果不表示全客户端所有功能已完成真实端到端等价验证。

### 2026-10-10 — GPUI 公益捐赠确认操作对齐 Vue

- 对照 `charity-view.vue`：进入确认态时 Vue 同时显示并排的“取消 / 确认捐赠”按钮，并把主操作按钮改成确认状态。GPUI 原先只有纵向取消和主按钮；现补齐并排的确认入口，两处确认都调用相同的受门控操作，等待期间禁止重复提交。没有改变捐赠二次确认和 `donateLove.enabled` 门控。
- 同轮对照 Vue 收益分析：策略使用的五个指标字段、参考等级过滤、未知等级处理、并列首项保留，以及黑名单的候选项/缺失种子回退一致；这部分暂未发现需要改动的源码差异，真实账号读写仍待验。
- 当前 `crates/qq-farm-gpui/src` 有 424 个 Rust 文件，按账号、农场、好友、活动、分析、商城、设置和游戏配置分目录；`git status --short` 把未跟踪的新 crate 折叠为一个目录项，模块索引在 crate README。
- **验证通过**：公益捐赠窗口回归覆盖确认态双按钮、主按钮文案、请求中重复点击保护和取消；GPUI `ui-preview` 全量 349 项、严格 GPUI Clippy、`cargo fmt --all -- --check` 和 `git diff --check` 均通过。真实公益服务操作、实窗及 Windows/Linux 验收仍待完成，不据此宣称全客户端所有功能已完成端到端等价。

### 2026-10-10 — GPUI 游戏配置编辑字段与 Vue 回填语义对齐

- 对照 Vue 的游戏配置抽屉，修正道具类型选项显示为“编号 - 名称”，并保留当前目录中已不存在的旧类型值；类型仍从目录选项中搜索选择。种子编辑字段改为遵循 Vue 的 `||` 默认回填，包括等级、季数、收获数量及缺失的生长阶段配置；新增表单仍保留 Vue 默认模板。
- 新增窗口回归覆盖可搜索类型的键盘选择、旧类型编辑回填，以及种子字段为 `0`/缺失时的 Vue 回退。GPUI 代码继续按业务目录维护；当前 `src/` 有 424 个 Rust 文件。
- **验证通过**：游戏配置定向回归 24 项、GPUI `ui-preview` 全量 353 项、workspace 测试（app 20 项、core 1045 项、GPUI 默认配置 352 项）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、严格 GPUI Clippy、格式检查、`git diff --check`、Vue typecheck/build，以及 32 个浅深色、最小窗口和放大比例预览场景。在线真实账号操作、Windows/Linux 原生窗口、托盘及跨平台安装包仍待实机验收；自动化通过不等于所有功能已完成端到端验证。

### 2026-10-10 — GPUI 页面搜索快捷键聚焦当前业务筛选

- 对照计划中的统一搜索动作检查快捷键：账号页隐藏了通用工具栏搜索，但快捷键之前仍聚焦那个隐藏输入；分析页策略分段也会聚焦不可见的通用搜索框；账号和游戏配置的专属搜索面板默认折叠。现在快捷键会打开对应折叠面板并聚焦其真实输入，分析页按分段聚焦参考等级或黑名单选择器。
- 新增窗口级回归覆盖账号筛选和游戏配置筛选的展开/聚焦，以及分析策略的参考等级聚焦。
- **验证通过**：3 项 `search_shortcut` 窗口回归。此项修复统一搜索动作的焦点路由，不表示剩余页面的真实服务和跨平台实机验收已完成。

### 2026-10-10 — GPUI 个人农场刷新限定在当前分段

- 对照 `desktop-ui/src/views/farm/personal/`：Vue 的分段默认按需创建，初始只加载当前可见页。GPUI 原先进入个人农场时会并发读取六个分段；现在进入新分段时只请求该页，重置本地选项和输入以匹配面板重新挂载。互动道具手动刷新和使用成功后只重新读取互动道具；宠物刷新、激活和礼包领取只重取宠物信息与礼包状态。结果继续按账号和页面版本校验。
- 互动道具刷新期间显示加载状态并阻止重复刷新；宠物激活后，分段刷新完成时会正确结束激活状态。
- **验证通过**：个人农场定向回归 41 项；完整 GPUI `ui-preview` 测试 359 项；严格 Clippy、默认配置全目标编译、`cargo fmt --all -- --check` 和 `git diff --check` 均通过。跨目标检查没有完成：Windows MSVC 目标在依赖 C 编译时缺少 MSVC CRT 头文件；Linux 目标缺少交叉 GCC 与 GLib/GObject sysroot/pkg-config。仍未连接真实 QQ 账号，未完成三平台实机窗口、托盘和在线业务操作验收；此项只关闭已确认的个人农场交互差异，不表示全部页面已经端到端对齐。

### 2026-10-10 — GPUI 账号登录链接即时识别提示

- 对照账号表单 `onCodeInput`：粘贴登录链接时 Vue 会立即识别平台并显示 URL 中的 `platform/os/ver`。GPUI 之前只在提交时解析平台，没有呈现这条即时提示；现在输入变化会同步平台，登录表单实时显示本地化提示。手动输入 Code 时隐藏提示，编辑表单程序回填不触发平台覆盖。
- 新增 GPUI 窗口回归覆盖 QQ/微信平台解析、中文与英文提示、手动 Code 清除提示，以及辅助测试标签。
- **验证通过**：账号页窗口测试 22 项、URL 提示专项 2 项、GPUI 严格 Clippy、`cargo check -p qq-farm-gpui --all-targets`、格式检查和 `git diff --check`。未完成真实微信/QQ 授权闭环、真实服务错误时序及多平台实机验收；此项只关闭一个已定位的账号表单交互差异，不表示整个客户端全功能对齐。

### 2026-10-10 — GPUI 好友访客与互动道具行为继续对齐 Vue

- 对照 `desktop-ui/src/views/farm/friends/index.vue` 和 `personal/InteractionItemsPanel.vue`：访客动作筛选与标签按 Vue `Number()` 兼容数字字符串、小数形式、常见进制字符串和单值数组；好友及访客头像、好友宠物图标交由共享图片适配器解析，兼容协议相对 URL、data URL、本地图片协议和旧 `farmcfg://localhost/` 地址。互动道具使用成功后只重载道具清单，不额外重载好友和黑名单数据；若旧手动刷新仍在途，操作后的刷新会取代旧请求，避免留下过时库存。
- 新增访客数值筛选、头像图片源、互动道具专属刷新范围回归；好友访客的字段与时间窗口用例仍通过。
- 结构核对：`crates/qq-farm-gpui/src` 有 427 个 Rust 源文件；账号、农场、好友、活动、分析、商城、设置、游戏配置和测试分别放在业务目录，模块索引见 crate README。`git status --short` 对未跟踪的新 crate 会折叠显示，需加 `--untracked-files=all` 展开。
- **验证通过**：GPUI `ui-preview` 全量 364 项测试、访客专项 4 项、头像专项 1 项、互动道具专属刷新窗口回归 1 项；`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、严格 GPUI Clippy、`cargo fmt --all -- --check`、`git diff --check`，并重新渲染 32 个浅深主题/最小窗口/缩放预览场景。此处只修复对照时发现的好友页行为差异；真实好友服务操作和跨平台实机验收仍待完成，不据此宣称所有页面已全功能等价。

### 2026-10-10 — GPUI 业务模块与农场/分析字段源码复核

- 复核 crate 结构：`src/` 有 427 个 Rust 源文件、约 51.5k 行；账号、农场、好友、活动、分析、商城、设置、游戏配置和测试按业务目录组织，活动玩法、表单、表格、运行时和测试继续按职责拆分。根级页面文件是 Rust 模块入口；默认 `git status --short` 将未跟踪 crate 折叠为一个目录项，查看明细用 `git status --short --untracked-files=all -- crates/qq-farm-gpui`。
- 逐项对照个人农场的 `DailyOverview.vue` / `TaskPanel.vue` 与 GPUI：礼包 key/图标/label、会员与月卡门控、状态优先级、次数/完成时间；成长任务汇总、描述、完成状态和进度判断均有对应实现。本次未发现代码级字段或状态差异，真实接口返回组合与授权失效仍待在线验收。
- 逐项对照收益分析 `index.vue` 与 GPUI：五项排名指标、当前策略与优先种子/背包策略、参考等级过滤、缺失或无效值、并列项顺序、作物摘要字段、黑名单候选和缺失种子回退均已核对。本次未发现代码级排名字段或筛选差异，真实账号设置读写及等级同步仍待在线验收。
- **验证通过**：分析页定向测试 20 项、每日礼包/成长任务定向测试 4 项；未因本次源码复核改变业务实现。源码对照和窗口测试不替代真实服务数据与三平台实窗验收。

### 2026-10-10 — GPUI 七夕好友刷新与赠送交互对齐 Vue

- 对照 `qixi-view.vue` 和活动父页：好友搜索仅匹配昵称/名称；进入页面读取缓存，手动刷新清好友缓存后重载；好友刷新只显示局部 loading，不锁住赠送和鹊桥领取；规则默认展开并可折叠。GPUI 现把七夕好友请求、过期请求代次和规则展开状态独立维护，并补齐这些交互。
- 字段语义同步修正：赠送数量对小数截断并按库存限制；阶段状态优先级与中英文文案、缺省阶段号、数字/字符串奖励数量、空计数回退、数字道具名、好友名称/头像回退遵循 Vue 真值规则。相关实现分布在 `activities/loading/qixi.rs`、`qixi_view/{gift,stages,shared,summary}.rs` 和 `activities/actions/inputs/qixi.rs`，仍按活动业务职责拆分。
- **验证通过**：七夕定向测试 11 项、GPUI `ui-preview` 全量 370 项、严格 GPUI Clippy、Rust 格式检查及 `git diff --check`。crate 仍有 427 个 Rust 源文件，账号、农场、好友、活动、分析、商城、设置和游戏配置分别按业务目录维护；默认 Git 状态会把整个未跟踪 crate 折叠为目录项。
- 自动化只证明源码和预览交互；七夕真实好友接口时序及真实账号送礼/领桥、实窗输入与其他平台验收仍待完成。本条不代表所有页面已完成端到端等价。

### 2026-10-10 — GPUI 总览日志规则与 Vue 一致

- 对照 `desktop-ui/src/views/farm/dashboard/log-events.ts`：事件标签是否显示应将标签和本地化后的事件文案比较；GPUI 原来误用原始事件 key。另将 `source=` / `phase=` 的提取改为支持字段紧邻标点，并让事件 token 仅在与 Vue 正则一致的 ASCII 单词边界替换，避免误改较长标识符。
- 新增日志字段标点边界回归，并保留 Vue 的后续事件 token 转换语义。模块仍位于 `desktop/dashboard/`，未把日志行为并入窗口或公共页面文件。
- **验证通过**：总览定向测试 17 项、GPUI `ui-preview` 全量测试 371 项、严格 GPUI Clippy、`cargo fmt --all -- --check` 和 `git diff --check`。真实运行日志组合及 Windows/Linux 实窗仍待验；自动化源码对照不等于所有页面已完成端到端等价。

### 2026-10-10 — GPUI 免打扰时间字段及开关门控对齐 Vue

- 对照设置页免打扰时段：起止时间已使用 GPUI Kit 的分段时间字段，采用 24 小时制和分钟精度，键盘按小时/分钟编辑；时间随设置快照同步，并写回现有设置草稿字段。
- 窗口回归曾发现禁用条件传反：免打扰开启时字段不可编辑。现改为开关关闭时禁用，并覆盖开启后键盘修改、草稿更新及关闭后值不再变化。
- **验证通过**：设置页窗口测试 24 项、GPUI `ui-preview` 全量 372 项、严格 GPUI Clippy、`cargo fmt --all -- --check` 和 `git diff --check`；另重新生成 32 个浅深主题/最小窗口/缩放预览，并检查设置页浅深主题及天气页最小窗口。在线账号实际保存、真实设置加载时序与跨平台窗口仍待验；本条只修复该设置交互差异，不表示所有页面已完成端到端等价。

### 2026-10-10 — GPUI 萌宠好友选择器与 Vue 下拉交互对齐

- 将萌宠好友候选项从输入框下方的内联按钮改为锚定下拉列表：打开时可浏览全部好友，输入昵称或 GID 后筛选，长列表可滚动；原有直接输入任意 GID 的查询路径保留。选择好友会清除旧的夺宝目标并关闭列表；Escape 通过共享 `CancelOverlay` Action 关闭列表并把焦点还给 GID 输入框，切换账号或页面时会清理弹层状态。
- 将好友选择控件拆到 `desktop/activities/pet_diary_view/friend_picker.rs`，萌宠战斗/目标处理仍在 `battle.rs`。crate 当前有 429 个 Rust 源文件，按业务域和工作流组织；Git 对未跟踪 crate 的状态折叠与源文件拆分无关。
- 新增窗口回归覆盖空搜索展开、昵称/GID 筛选、选择写回、Escape 关闭与焦点恢复及英文控件文案。
- **验证通过**：萌宠好友选择相关窗口测试通过；GPUI `ui-preview` 全量 373 项测试、严格 GPUI Clippy、`cargo fmt --all -- --check` 和 `git diff --check` 均通过。完整活动字段/操作码、真实后端数据与跨平台实窗验收仍继续；本条仅关闭好友选择控件的交互差异，不代表全客户端已完成端到端等价。

### 2026-10-10 — GPUI 萌宠好友选择器收敛为单个可搜索控件

- 后续交互复核发现上条实现仍是独立输入框加好友弹层，与 Vue 的单个 `NSelect`（可搜索、可清空、tag 模式）操作路径不同。现在使用单个 GPUI Kit `Combobox`：可按好友昵称或 GID 搜索，从候选项选择；没有匹配项时可把输入值作为自定义 GID 继续查询。未知的已选 GID 在好友目录刷新后仍保留。
- 好友标签按 Vue `nickname || 未命名` 语义生成；键盘 Escape 关闭候选菜单并恢复选择器焦点。选择器实现独立位于 `desktop/activities/pet_diary_view/friend_picker.rs`，操作和加载仍在各自活动模块。
- **验证通过**：萌宠日记相关窗口/逻辑回归、GPUI `ui-preview` 全量 377 项、严格 GPUI Clippy。自动化覆盖搜索候选、自定义 GID、Esc 关闭及业务门控；未完成真实账号服务操作和 Windows/Linux 实窗验收，不能据此宣称所有页面已完成端到端等价。

### 2026-10-10 — GPUI 同客户端重复启动确认唤起

- 隔离数据目录的 Mac 原生窗口双进程检查发现，第二次启动偶尔在 350ms 确认期限内未收到唤起应答，随后错误提示“目录已占用”。共享锁的本机 TCP 握手现统一使用 2 秒期限；成功唤起同一 GPUI 实例时，重复进程正常退出且不再打印互斥错误；另一客户端仍展示冲突提示。
- **验证通过**：共享锁专项 3 项（同宿主连续唤起、无效令牌拒绝、子进程异常退出释放），在隔离 `FARM_DATA_DIR` 下先启动 Mac GPUI 窗口、再启动第二个 GPUI 进程，第二进程收到确认并正常退出；未使用已有 Tauri 数据目录或启动游戏账号。此项只关闭单实例重复启动边界，不替代托盘实际点击恢复及其它平台生命周期验收。

### 2026-10-10 — GPUI 业务目录结构与当前工作区验收

- 复核 `crates/qq-farm-gpui/src` 共 429 个 Rust 文件（约 52.4k 行），账号、个人农场、好友、活动、分析、商城、设置和游戏配置均按业务目录组织；页面入口模块负责组合，测试按页面和工作流拆分。目录索引见 crate README 和本文件前述架构说明。新 crate 尚未跟踪时，默认 `git status --short` 会折叠为 `?? crates/qq-farm-gpui/`；展开可用 `git status --short --untracked-files=all -- crates/qq-farm-gpui/src`。
- **验证通过**：`cargo test --workspace -- --test-threads=1`、GPUI `ui-preview` 全量 377 项、app/GPUI 严格 Clippy、Rust 格式和差异检查、Vue typecheck/build；macOS `cargo build --release -p qq-farm-gpui` 与 `scripts/package-gpui.sh release` 成功，检查了 `.app.zip` 中的可执行文件、资源和应用清单。真实账号在线操作、完整实窗键鼠与缩放、托盘点击恢复和 Windows/Linux 原生运行及打包仍待验；本条不表示端到端全功能已完全等价。

### 2026-10-10 — GPUI 英文页面视觉复核与天气提示本地化

- 为总览、农场、好友、分析、普通/神秘商城、设置、游戏配置、活动目录/详情和天气补充英文视觉场景；现在 13 个主要页面都能用 English 偏好渲染，另保留浅深主题、最小窗口及 125%/150% 尺寸场景。
- 逐页检查发现天气服务返回的“金币不足”“雷电徽章不足”等门控原因、活动通用规则标题以及预览专用副标题仍显示中文。现在这些界面文案按语言本地化，预览数据的游戏内账号/作物/活动名称保留服务端原文；新增中英文单测和英文天气窗口断言。
- **验证通过**：全量 GPUI `ui-preview` 382 项全部通过、英文场景覆盖与天气中英文窗口回归、i18n 单测、严格 GPUI Clippy、`cargo fmt --all -- --check` 和 `git diff --check`；重渲染 45 个预览场景并查看英文天气、活动和总览页面。首次运行发现一条旧断言仍期待英文界面显示中文余额原因；修正断言后重新运行全量测试通过。
- 视觉预览和自动化仍不覆盖真实游戏服务器的全部文案，也不替代真实账号读写、实窗中文输入、托盘恢复及 Windows/Linux 原生运行验收。

### 2026-10-10 — GPUI CI 与 Release 原生覆盖 macOS 双架构

- 平台矩阵复核发现常规 GPUI CI 只用了 `macos-latest`，当前对应 ARM；Release 却在同一个 ARM runner 上交叉构建 Intel 包。现将 CI 与发布矩阵拆成 `macos-15` ARM 和 `macos-15-intel` Intel 原生任务；Actions 产物名增加架构，避免两个 macOS 任务冲突。Windows/Linux 任务与产物也显式标注 x86_64。
- **验证通过**：用 Ruby YAML 解析器读取 GPUI 与 Release 两个 workflow，并断言 CI 产物唯一、Release 的 macOS runner/目标架构相配；在本机 Intel Mac 安装 ARM Rust 标准库后，`cargo check -p qq-farm-gpui --target aarch64-apple-darwin` 和 ARM 目标严格 Clippy 均通过；GPUI `ui-preview` 全量 383 项、更新器产物名映射单测（覆盖 5 个受支持/不支持目标）、严格 Clippy、格式与 diff 检查通过。
- macOS ARM 交叉检查只证明源码能为该目标编译，不代表 ARM 实机启动；Windows/Linux runner、原生打包、托盘和实机交互仍待相应 GitHub Actions 与设备验证。

### 2026-10-10 — GPUI 恢复神秘商店独立页面入口

- 对照用户确认的目标侧栏和页面范围，发现 GPUI 曾把 `mystery-shop` 持久化页面状态折叠到商城的神秘商人标签，且回归测试断言侧栏不应有独立入口。现新增独立 `Page::MysteryShop`、中文/英文标题和 `Store` 图标；从侧栏进入后直接加载神秘商店快照，不显示商城分段标签或商品搜索，并把选择写入独立页面偏好。商城里的神秘商人标签继续保留，两种入口共用同一数据加载、视图和购买确认流程。
- 参考 `desktop-ui` 当前把 `/farm/mystery-shop` 隐藏并重定向至商城标签；GPUI 此处按用户提供的 GPUI 目标截图及原实施范围保留可见独立入口，同时保留原商城标签，不改变业务接口和购买门控。
- **验证通过**：独立页路由、初始化、侧栏点击、无分段标签、页面映射和购买确认回归；GPUI `ui-preview` 全量 385 项、`cargo check -p qq-farm-gpui --all-targets`、严格 Clippy、格式和 diff 检查通过。重新渲染 45 个预览场景并查看独立页浅色预览，确认导航选中、页面标题和商品卡正确，独立页没有商城分段或搜索框。真实账号购买及跨平台窗口/托盘仍未验收。

### 2026-10-10 — GPUI 账号侧栏改为常驻账号列表

- 对照用户提供的账号页参考图，发现 GPUI 仍用下拉框切换账号，侧栏也没有新增入口。现在展开侧栏常驻显示当前账号和可选账号行，选中态用单选圆点表示；顶部 `−` 打开账号管理、`+` 打开新增表单，底部保留“添加账号”。折叠时收起列表但保留导航与紧凑新增入口。
- 账号行直接调用既有 `set_account` 生命周期，保留账号变更后的旧请求失效、页面快照刷新和偏好保存；移除仅为下拉框存在的第二份 `SelectState`，避免列表选中状态与表单状态分叉。
- **验证通过**：窗口回归覆盖多个账号行、点选切换、管理入口显示和侧栏新增表单打开；折叠/展开及侧栏宽度回归通过；全量 GPUI `ui-preview` 测试 385 项全部通过，`cargo check -p qq-farm-gpui --all-targets`、严格 Clippy、格式和 diff 检查通过。重新渲染 45 个浅深主题/英文/最小窗口/缩放预览场景，并检查账号页浅色和深色预览；账号登录及真实生命周期服务、Windows/Linux 实机窗口和托盘仍待验收。

### 2026-10-10 — GPUI 刷新处理按业务目录拆分

- Git 默认状态会把整个未跟踪的新 crate 折叠显示成 `?? crates/qq-farm-gpui/`；展开后 `src/` 有 433 个 Rust 文件。账号、个人农场、好友、活动、分析、商城、设置、游戏配置和页面测试均按业务目录组织，目录索引见 crate README。
- 将 `desktop/runtime/loading.rs` 从 411 行的跨页面刷新实现收敛到 178 行的请求和结果分发；好友/农场快照合并、账号选择回收、设置草稿同步、活动默认选项和总览资产基线分别放回 `desktop/{friends,personal,accounts,settings,activities,dashboard}/`。
- 修正果实编辑窗口回归测试的异步时序：选择器确认由 GPUI 延迟派发，测试现在等待事件队列完成后再检查关联植物和名称，避免在确认回调执行前读取空状态。
- **验证通过**：GPUI `ui-preview` 全量 385 项测试通过；刷新相关测试 16 项通过；`cargo check -p qq-farm-gpui --features ui-preview --all-targets`、严格 Clippy、Rust 格式和 `git diff --check` 通过。真实账号操作、Windows/Linux 实窗和托盘恢复仍待验收；本条只记录模块拆分和测试时序修复，不代表所有功能已完成端到端等价。

### 2026-10-10 — GPUI 与 Vue 账号保存字段对齐

- 复核确认账号备注字段虽在 Vue 表单和 API 类型中存在，Tauri API 适配器之前却没有把它传给 app；现在新增和编辑都转发 `remark`，GPUI 与 Vue 共用后端备注持久化语义。旧数据没有备注时，GPUI 继续按 Vue 适配器回填账号名。
- 本机微信快捷授权沿用 Vue 的 IPC 行为：确认结果只使用登录 code，空白名称交给 app 生成默认账号名；扫码登录仍按 Vue 回填二维码登录昵称。
- **验证通过**：GPUI `ui-preview` 全量 385 项测试；本机微信名称语义修正后，账号专项 22 项、GPUI 预览全目标检查和严格 Clippy、Rust 格式与 diff 检查通过；Vue typecheck/build 通过。真实账号授权与跨平台实窗仍待验收。本项源码对照和自动化不代表全客户端已完成端到端等价。

### 2026-10-10 — GPUI 青梅原料选择与操作中门控对齐 Vue

- 对照 Vue 青梅酿造交互：库存 UID 在选择和提交时统一转成字符串，兼容数字 UID；开始、继续、结算或领取操作在途时，锁定原料选择、数量和全选控件，避免配方在提交期间改变。实现分别位于 `activities/actions/inputs/qingmei.rs` 和 `activities/qingmei_view/ingredients.rs`。
- 新增数字 UID 提交回归，以及请求 pending 时点击选择、全选和数量控件不会改变配方的窗口测试。业务模块继续按活动目录拆分；当前 crate 有 433 个 Rust 源文件。默认 Git 状态折叠整个未跟踪 crate 时，可使用 `--untracked-files=all` 查看明细。
- **验证通过**：青梅专项 4 项测试、GPUI `ui-preview` 严格 Clippy、格式和差异检查。此项关闭两个已定位的青梅交互差异；未连接真实账号验证青梅服务时序，也不代表所有页面已完成端到端等价。

### 2026-10-10 — GPUI 好友一键偷取按好友逐项回写

- 对照 `desktop-ui/src/views/farm/friends/index.vue` 的 `stealAllFriends` / `runFriendOp`：Vue 对每位好友顺序请求，响应后立即更新可偷状态，并在详情展开时重载对应农场。GPUI 原来等整批请求结束才统一更新。现在每位好友的请求完成后立即回写列表、更新排序并刷新当前好友详情；请求期间对应行显示操作中，账号切换或离开页面仍由批次代次拦截旧 UI 更新，最终汇总提示保持原有时机。
- 新增回归覆盖首个好友完成后立即更新列表、后续好友尚未完成时的中间状态、继续处理下一位，以及当前好友行操作在批处理中保持锁定。
- **验证通过**：GPUI `ui-preview` 全量 387 项测试通过；行锁测试加入后，批量偷取专项 3 项通过；GPUI 严格 Clippy、Rust 格式与差异检查通过；macOS x86_64 Release 构建和 `.app.zip` 直接打包成功，检查了 Mach-O 架构、bundle identifier、Info.plist 和 ZIP 资源。修复了打包脚本缺少执行位的问题。真实 QQ 好友网络请求与展开详情在途时序仍需在线账号验收；本项继续关闭已发现的逐页交互差异，不代表全客户端已完成真实后端和跨平台端到端验收。

### 2026-10-10 — GPUI 本机微信授权头像与 Vue 对齐

- 对照 `desktop-ui/src/views/farm/account/modules/account-operate-drawer.vue`：本机微信检测成功后 Vue 会显示 `headimgurl`，GPUI 状态原本已接收该 URL，但授权面板始终只显示“微”。现在通过共享图片资源适配器显示检测到的头像；没有头像地址时显示原有文字占位，加载失败仍走缺图回退。
- 同轮核对账号筛选/分页、行启停删除、增改凭证、二维码和本机授权流程，以及商城搜索、商品门控、数量上限、确认与购买后刷新；源码层面除头像展示外未发现新的操作路径差异。真实登录与购买服务返回时序尚未在线验收。
- 新增窗口回归覆盖检测到头像与无头像时的两种呈现。**验证通过**：GPUI `ui-preview` 全量 389 项测试，严格 Clippy，`cargo fmt --all -- --check` 和 `git diff --check`。该结果只验证这一账号页差异和自动化覆盖范围，不表示其他页面或跨平台实机功能已全部对齐。

### 2026-10-10 — GPUI 设置表单字段与操作源码复核

- 对照 `desktop-ui/src/views/farm/settings/index.vue` 的种植策略、自动化、离线提醒和运行环境四组表单：种植时间间隔/免打扰/延迟、背包种子回退、27 个自动化选项及其数值门槛、提醒服务选项与 QQ Bot 绑定和钉钉测试、运行环境设备预设/时区/平台/OS/设备字段，在 GPUI 的设置业务模块中均有对应状态和操作。策略与自动化保存只提交当前分组；化肥购买开关保存后保留一次检查；提醒及系统配置操作不会覆盖其他未保存草稿。
- 本轮源码对照未发现设置表单字段或操作门控缺项；验证范围不包括真实提醒发送、Bot 在线绑定、服务端配置回读和各平台实窗输入，因此不能据此宣称全客户端已全部对齐。

### 2026-10-10 — GPUI 总览日志筛选字段对齐 Vue

- 对照 `desktop-ui/src/views/farm/dashboard/index.vue` 与 `dashboard/log-events.ts` 的 `filteredLogs`：Vue 关键词只搜索消息、模块标签和事件文案，不搜索时间；模块筛选使用已经本地化的日志消息。GPUI 原先把时间拼入关键词搜索，并用未翻译的原始消息筛模块，可能多匹配或漏掉 `source=harvest_crop` 这类日志。现已修正两个筛选范围。
- 新增 `docs/GPUI_PARITY_CHECKLIST.md`，按页面列出 Vue 对照源、GPUI 业务目录、关键行为和回归测试位置，并明确真实账号、原生窗口及三平台验收仍未完成；crate README 增加该清单入口。
- **验证通过**：日志筛选定向回归 5 项、GPUI `ui-preview` 全量测试 391 项、严格 GPUI Clippy、Rust 格式检查、`git diff --check`，以及 macOS x86_64 `cargo build --release -p qq-farm-gpui`。未执行真实账号操作；本条只关闭总览日志筛选差异，不代表全客户端已完成端到端等价验收。

### 2026-10-10 — GPUI 事件队列积压恢复回归与好友页复核

- 将 Tokio 广播接收结果转换抽成 `receive_desktop_event`：常规事件继续转成 GPUI 实时事件，订阅落后时发出 `DesktopEvent::Refresh`，关闭时停止转发。用容量为 1 的真实广播队列制造 lag，验证先触发快照刷新、再继续收到最新事件；同时覆盖关闭分支。
- 再次对照好友页 Vue 交互的搜索、排序、分页、黑名单、详情、逐项/批量偷取、访客筛选和互动道具入口，并核对星砂兑换数量沿用 1–99 的共享购买确认流程；本轮源码核对未发现新的行为缺口。
- **验证通过**：GPUI `ui-preview` 全量 393 项、严格 Clippy、Rust 格式和差异检查。尝试 Linux x86_64 与 Windows MSVC 交叉检查时，分别因本机没有 Linux 目标 GCC/GTK/sysroot 工具链、Windows `llvm-rc`/MSVC SDK 而无法完成；仓库已有原生平台 CI runner，但当前工作树尚无其运行结果。真实账号、原生三平台窗口和托盘验收仍未完成，不能据此宣称全客户端已经全部对齐。

### 2026-10-10 — GPUI 文件结构说明与每日任务源码复核

- 用户反馈新客户端“文件少、业务挤在一起”后检查 Git 展示：新 crate 仍是未跟踪目录，`git diff --stat` 默认不计入它，`git status --short` 也会把整棵目录折叠成一行。实际 `src/` 有 433 个 Rust 文件；活动、个人农场、账号、好友、设置等各自在独立业务目录，页面入口文件只声明和组合模块。已在 crate README 中注明这两种 Git 展示行为及展开文件清单的命令。
- 对照 Vue `TaskPanel.vue` 和 `DailyOverview.vue`，复核每日礼包状态优先级、会员/月卡未开通提示、成长任务完成态、JavaScript falsy 进度、时间及空态；GPUI 只读视图行为一致，本轮未发现需要改动的业务差异。
- **验证通过**：GPUI `ui-preview` 全量 393 项测试、严格 Clippy、`cargo fmt --all -- --check` 与 `git diff --check`。真实账号服务操作、原生窗口和三平台运行验收仍未完成；本次源码复核不代表全客户端已通过端到端等价验收。

### 2026-10-10 — Rust workspace 与 Vue 客户端全量检查

- 为继续推进完整对齐，本机重新运行整个 Rust workspace 测试：app 20 项、core 1,045 项、GPUI 默认特性 391 项全部通过，文档测试无失败；`cargo clippy --workspace --all-targets --no-deps -- -D warnings` 通过。
- Vue 对照客户端 `pnpm run typecheck` 与 `pnpm run build` 通过。该检查证明当前两端源码和构建正常，不替代账号侧真实后端操作、原生窗口键鼠验收及 Windows/Linux 原生 runner 结果。

### 2026-10-10 — 明确覆盖 Tauri/GPUI 共享引擎锁

- 为补足跨客户端验收证据，在 app 层增加 Tauri 与 GPUI owner 轮换测试：Tauri 持锁时 GPUI 无法取得同一数据目录且唤起查询返回 `tauri`；释放后 GPUI 可取得锁，Tauri 再次申请则被拒绝并识别 `gpui`。测试使用隔离临时目录。
- **验证通过**：`cargo test -p qq-farm-app -- --test-threads=1` 的 21 项测试、app 严格 Clippy、Rust 格式和差异检查。此前 workspace 全量测试的 core 1,045 项与 GPUI 默认 391 项也已通过；真实双应用窗口互斥与唤起仍需原生桌面进程验收。

### 2026-10-10 — 补充账号切换代次与平台交叉检查证据

- 侧栏账号切换窗口回归现在断言选择另一账号后 `request_revision` 递增；受控旧账号请求最后返回并验证 UI 丢弃旧结果的完整异步场景仍待补。
- 当前 Mac 上重新验证 `aarch64-apple-darwin` 的 GPUI `cargo check` 和严格 Clippy 均通过。Linux target check 被缺少 GTK/GIO 交叉 `pkg-config` sysroot 与 `x86_64-linux-gnu-gcc` 阻断；Windows MSVC check 被缺少 `llvm-rc`、MSVC SDK/CRT 阻断。现有 x86_64 `.app.zip` 内含 Mach-O x86_64 可执行文件、bundle identifier `com.qqfarm.gpui` 和应用资源；这些结果不代替目标平台原生 runner。
- **验证通过**：账号侧栏定向窗口测试、aarch64 check/Clippy、GPUI `.app.zip` 结构检查。Linux/Windows 原生 runner、真实双应用窗口唤起和托盘交互仍未完成。

### 2026-10-10 — 验证切号后迟到的页面结果会被丢弃

- 将页面请求结果提交逻辑提取到 `desktop/runtime/loading.rs::apply_page_load_result`，异步加载回调与窗口回归共用这一生产路径。账号导航窗口测试先执行真实 `set_account` 生命周期，再交付旧账号的页面快照，确认快照不会覆盖新账号页面，当前加载标记也不会被旧结果清除。
- `docs/GPUI_PARITY_CHECKLIST.md` 已关闭“旧账号请求最后返回”这一代码级回归项。该测试控制的是结果提交时序，不替代真实 QQ 网络请求或原生双窗口验收。
- **验证通过**：`cargo test -p qq-farm-gpui --features ui-preview switching_accounts_discards_a_late_page_response -- --test-threads=1`、`cargo fmt --all -- --check` 与 `git diff --check`。随后继续执行 GPUI 严格 Clippy。

### 2026-10-10 — macOS 打包同时输出可直接打开的应用包

- 发布复核发现 `scripts/package-gpui.sh` 原先只输出 `.app.zip`，没有按计划把可直接打开的架构独立 `.app` 写入 `dist/`。现在 macOS 打包会同时生成 `QQ Farm-<架构>.app` 和 `qq-farm-gpui-macos-<架构>.app.zip`；README 与发布检查清单已同步产物约定。
- **验证通过**：当前 x86_64 Mac `cargo build --release -p qq-farm-gpui`、打包脚本、`bash -n`、bundle `Info.plist` 校验、Mach-O 架构检查、游戏/活动/TSDK 资源检查；`.app` 与 ZIP 内可执行文件 SHA-256 一致。该结果只验当前 Mac 产物，不替代 ARM、Windows/Linux 原生构建和窗口验收。

### 2026-10-12 — GPUI 萌宠日记日志字段、页签缓存与快照回退对齐 Vue

- 对照 `desktop-ui/src/views/farm/activity/pet-diary-view.vue`：Vue 的被夺日志只显示时间、玩家等级、胜负标签、损失道具和“移花接木假宝藏”；GPUI 还额外渲染了挑战书和注入道具两行，现已按 Vue 可见字段移除。互动/被夺两类日志在 Vue 中各自缓存，切换页签展示该页签自己的数组；GPUI 原先共用一个记录数组，切换后会短暂显示另一类日志的数据，现在两类日志分别缓存、切换页签只读取对应缓存；日志行补上完整无障碍名称（含消耗/获得/损失明细）。
- 同轮修正的可见差异：宝藏未知状态按 Vue 显示 `状态 N`（英文 `Status N`），不再显示“未知状态”；拾物小铺的“剩 N”标记按 Vue 真值判断，值为 `null`/空串/数字 0 时不显示、字符串 `"0"` 仍显示；顶部资产按 Vue 的 `name || id` 和 `count ?? 未知` 显示；种子赠礼和成长手记为空的占位文案是 GPUI 额外添加的，Vue 对应区段没有空态文案，现已移除。
- 操作成功但后端快照刷新失败时（回包 `snapshot` 为空并带 `refreshError`），Vue 保留上一份快照并只提示刷新失败；GPUI 原先整段重新拉取萌宠日记，现改为保留当前快照并显示同样的刷新警告。
- 新增窗口回归覆盖被夺日志字段集合（带挑战书/注入道具数据仍只显示 Vue 字段）、页签缓存切换不串数据、互动日志消耗/获得文案；新增单测覆盖未知宝藏状态、剩余数量真值和资产回退文案。萌宠记录读取服务、真实账号操作与跨平台实窗仍未验收。
- 新增萌宠日记浅色/深色/英文预览场景和展示快照（含日志页签、资产回退、宝藏状态、小铺剩量），预览渲染补齐 arrange 后的渲染帧和等待，滚动场景中后加载的图片能正常呈现；本轮重新渲染 48 张预览图并检查萌宠日记三种场景。
- **验证通过**：GPUI `ui-preview` 全量 399 项测试、严格 GPUI Clippy（含 `--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check` 和 `git diff --check`；`cargo run -p qq-farm-gpui --features ui-preview -- --render-preview <目录>`（配合 `QQ_FARM_ASSET_DIR` 指向仓库根）生成 48 张预览图。本条只关闭上述已定位的日志、展示与刷新回退差异，不代表全客户端已完成端到端等价。

### 2026-10-12 — GPUI 千星游记三个页签文案与详情刷新对齐 Vue

- 对照 `desktop-ui/src/views/farm/activity/index.vue` 及其 zh/en locale 修正千星游记页签：
  - 游记：积分标题改用 Vue `travelScore`（游记积分 / Travel Score）、领取按钮改用 `claimAll`（一键领取 / Claim All）、提示改用 `travelTip`、规则按钮改用 `viewRules`（活动说明 / Event info）；进度按 Vue `Number(progress ?? 0)` 与 `progressMax || '--'` 显示（缺失按 0，上限为 0/缺失显示 `--`）；节点只保留 Vue 的已领取/可领取标签，移除额外的“未解锁”标签；战令为空时保留 Vue 的完整卡面和禁用的“一键领取”按钮，不再显示带刷新按钮的简化空态。
  - 观星：锁定状态改用 Vue `locked`（未开启）、可点亮英文改用 `Lightable`、点亮按钮改用 `lightConstellation`（点亮观星 / Light Constellation）、规则按钮改用 `viewRules`；移除 Vue 模板没有的 `explain` 额外说明行。
  - 节令：领取按钮标签按 Vue `solarButtonLabel`（领取 / Claim、未开始 / Not started、已领取 / Claimed、暂不可领取 / Unavailable）；移除 Vue 没有的状态行；空态去掉额外刷新按钮。
- 活动详情头部新增 Vue 的全局刷新入口（`activity-detail-refresh`，加载中锁定），覆盖全部玩法详情；季节和节令空态里的重复刷新按钮随之移除。
- 新增季节进度显示单测、星座状态文案单测、空战令页签保留“一键领取”和头部刷新的窗口回归；更新季节/星座/节令窗口用例的文案断言并覆盖头部刷新点击。
- **验证通过**：GPUI `ui-preview` 全量 402 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 48 张预览图并检查千星游记详情与天气详情的头部刷新布局。真实活动服务的领取/点亮/兑换时序及跨平台实窗仍未验收，本条不代表全客户端已完成端到端等价。

### 2026-10-12 — GPUI 活动目录卡片与星砂商店文案对齐 Vue

- 活动目录卡片按 Vue `page.farm.activity` locale 修正：状态标签改为 进行中/未开始/已结束（英文 Active/Upcoming/Ended）；只有开始时间时显示 `<开始> 未开始 / Upcoming`；时间缺失显示 活动时间待定 / Schedule TBD；不支持玩法的操作提示改为 Vue `unsupported`（暂未支持详情 / Details not supported）。空目录去掉额外的刷新按钮，与 Vue 空态一致（刷新走页面级入口）。
- 星砂商店页签：首个分类按钮改为 Vue `allCategories`（全部 / All）；标题兜底英文改为 `Star Sand Shop`；移除面板内重复的“星砂余额”行 —— Vue 只在页面头部显示一次余额，GPUI 详情头部已有 `activity-shop-balance`，余额回退实现合并为 `activities/shop.rs` 中的单一函数供头部复用。
- 更新目录与星砂用例断言（含英文状态文案、时间窗口、暂未支持详情、全部分类按钮）。
- **验证通过**：GPUI `ui-preview` 全量 402 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 48 张预览图并检查活动目录场景的状态、时间与不支持提示。真实活动目录数据和新窗口实机仍未验收，本条不代表全客户端已完成端到端等价。

### 2026-10-12 — GPUI 公益页规则区与每日礼包状态对齐 Vue

- 对照 `desktop-ui/src/views/farm/activity/charity-view.vue`：Vue 在公益页底部渲染可折叠规则区（默认展开，标题为 `rules.title` 或「活动说明 / Event info」，正文来自 `rules.paragraphs` + `rules.tips.txt` + `rules.lines`），GPUI 之前完全没有规则内容或入口。现在新增同结构折叠卡（`charity-rules-toggle` / `charity-rules-body`，默认展开，进入公益页时重置为展开，对应 Vue 组件重建语义）。
- 每日公益礼包：Vue 在奖励行内、已领取时显示「今日已领取」标签，且奖励名后不显示数量；GPUI 补上该标签并去掉数量后缀。全服共同目标奖励同样不显示数量（Vue 只显示名称与解锁门槛），GPUI 改为无数量版本；`reward_line_plain` 作为共享的“仅名称”奖励行。
- 新增窗口回归覆盖公益规则默认展开、折叠与再展开、每日礼包已领取标签；新增公益浅色/深色/英文预览场景并逐张检查。
- **验证通过**：GPUI `ui-preview` 全量 403 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 51 张预览图（含公益三种场景）。真实公益服务操作与跨平台实窗仍未验收，本条不代表全客户端已完成端到端等价。

### 2026-10-12 — GPUI 七夕页余额、阶段与赠礼文案对齐 Vue

- 余额卡按 Vue `qixi-view.vue` locale 修正：鹊羽 / 鹊羽香囊 / 获赠香囊 / 已赠好友（英文 Magpie feather / Feather sachet / Received sachets / Friends gifted）；获赠香囊按 Vue 使用固定文案，不再回退到物品名。页面内重复的标题/状态/时间头部被移除（Vue 的七夕视图没有该块，标题与剩余时间已由活动详情头部显示）。
- 阶段区：待领取（原「可领取」）、空态「暂无鹊桥阶段」、领取按钮进行中显示「领取中… / Claiming…」、消耗行英文改用 `Requires`、羽毛回退改「鹊羽 / Magpie feather」；阶段状态标签按 Vue 的 tag 类型着色（待领取=警示色、当前阶段=黄色）。
- 赠礼区：区块标题「赠送香囊 / Gift sachet」、刷新按钮「刷新列表 / Refresh List」、空态「未找到可赠送好友」、选中文案「赠送给 / Gift to」、未选「选择一位好友 / Pick a friend」、香囊行「可用 {n} / Available {n}」、赠送按钮进行中显示「领取中… / Claiming…」；移除 Vue 没有的「已选择」行内文字。
- 新增/更新回归：阶段待领取文案、余额与已赠好友标签（含获赠香囊固定文案）、赠礼与鹊桥按钮进行中标签、刷新列表文案；新增七夕浅色/深色/英文预览场景并逐张检查。
- **验证通过**：GPUI `ui-preview` 全量 404 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 54 张预览图（含七夕三种场景）。真实七夕服务操作与跨平台实窗仍未验收，本条不代表全客户端已完成端到端等价。

### 2026-10-12 — GPUI 非活动模块卡片视觉统一

- 用户指示活动页暂缓（多数活动已过期），先统一其他模块的视觉。本轮核对非活动模块（总览、我的农场、好友、收益分析、商城、设置、游戏配置、账号）的卡片样式：多数页面已是「圆角 + 描边 + group_box 填充」，但总览（账号/资产/物资/日志/巡查/今日统计）、设置（种植策略各分组、自动化、离线提醒、系统配置、界面偏好）、我的农场每日礼包、账号页本机微信授权共 16 处卡片只有填充没有描边，导致同一应用内卡片边界不一致。
- 现在这些卡片统一补上主题描边（`border_1` + `border_color(theme.border)`），与好友、分析、商城、仓库等页面使用同一套卡片处理；重新渲染 58 张预览图并检查总览与设置的浅色/深色场景。
- **验证通过**：GPUI `ui-preview` 全量 404 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。下一轮继续其他模块（总览、分析、好友等）的行为逐项对齐；真实账号与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 总览页卡片结构与文案对齐 Vue

- 继续非活动模块对齐，本轮深度对照 `desktop-ui/src/views/farm/dashboard/index.vue`：
  - 账号卡：右上角改为 Vue 的 `Lv.N` 信息标签，移除姓名下方重复的等级；在线/离线指示移到资产卡底部（Vue 的位置），使用状态圆点 + 在线/离线文案 + 运行时长。
  - 资产卡：金豆改为 Vue 的「金豆豆 / Gold Beans」；点券、钻石、金豆在离线时按 Vue 显示 0 / -；底部去掉 GPUI 额外的「土地 N · 好友 N」字段。
  - 物资卡：改为 Vue 的两段结构「化肥容器（普通 / 有机）」和「收藏点（普通 / 典藏）」，不再使用「普通化肥 / 稀有采集点」等自造别名。
  - 巡查卡：行标签改为 Vue 的「下次农场巡查 / 下次帮助 / 下次偷菜」（英文 Next farm check / Next help / Next steal）；离线文案改为「账号未登录」、倒计时结束文案改为「巡查中…」。
  - 账号卡效率行改「效率：N/时 / Rate: N/h」；日志工具条按钮改「清空 / Clear」，展开后的收起按钮改「收起 / Collapse」，与 Vue locale 一致。
- 新增/更新回归：巡查倒计时中英文文案断言、日志清空按钮断言。
- **验证通过**：GPUI `ui-preview` 全量 404 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 58 张预览图并检查总览中英文场景。真实账号操作与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 好友与个人农场的分段标签、操作文案对齐 Vue

- 好友页分段按 Vue 改为「好友列表 / 互动道具 / 好友黑名单 / 最近访客」（含顺序），黑名单分段按 Vue 显示数量徽标（`friends-blacklist-tab-count`，为空时隐藏）；个人农场分段改为 Vue 的「我的背包 / 我的任务」，英文 My Bag / My Tasks / Illustrated / Interaction。
- 好友操作文案对齐 Vue locale：刷新「刷新列表 / Refresh List」、批量「全部偷取 / Steal all」、行内「一键偷取 / One-click steal」「一键务农 / One-click Help」「捣乱 / Make trouble」「加入黑名单 / Add to Blacklist」「移出黑名单 / Remove from Blacklist」；删除与移出黑名单触发按钮改用危险色强调；无操作状态改「无操作 / No action」；宠物同步提示改为 Vue 原文「宠物状态 N/M 已确认，其余由每日同步自动补齐」；黑名单说明改为 Vue 原文（去掉“· N 位好友”后缀）；访客筛选改「全部 / 偷菜 / 帮忙 / 捣乱」。
- 新增黑名单徽标窗口回归；更新好友与设置页相关断言。
- **验证通过**：GPUI `ui-preview` 全量 405 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 58 张预览图并检查好友与个人农场分段。真实好友服务操作与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 收益分析页细节对齐 Vue

- 策略页：当前策略标题改为 Vue 的中粗体；背包种子优先提示框改为 Vue 的警示色底与文字（原来是无色底）；其余策略对比卡、参考等级、单位与可种植提示复核后与 Vue 一致。
- 黑名单页：补上 Vue 的「搜索并添加作物 / Search and add crops」小节标题；移除 GPUI 额外添加的「已选 N 项」行；选中行底色改为与卡片区分的前景底色（Vue 中行与卡片底色不同，GPUI 原来行与卡片同为 group_box 难以区分）；「清空黑名单」「移出黑名单」按钮按 Vue 的 error 样式改用危险色。
- 新增收益分析黑名单预览场景（中英文），重新渲染 60 张预览图并检查策略页与黑名单页。
- **验证通过**：GPUI `ui-preview` 全量 405 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。真实账号分析与黑名单写入、跨平台实窗仍未验收。

### 2026-10-12 — GPUI 商城与神秘商店细节对齐 Vue

- 复核商城与神秘商店的商品卡、状态标签（已拥有/奖励已领取/已售罄/游戏内广告与分享领取/需要 SVIP/暂不可购买/购买）、购买确认弹窗，以及神秘商店的价格、折扣、余额、活动时间与购买门控，与 Vue 保持一致。
- 商城「神秘商人」标签内不再重复显示面板标题，只保留右对齐的刷新按钮（Vue 的面板在该标签内没有标题）；独立「神秘商店」页面保留标题。
- 修复「库存 N」标签被卡片拉伸成整行的问题：Vue 中是内联小标签，现在与商城会员/钻石标签一样按内容宽度显示。
- **验证通过**：GPUI `ui-preview` 全量 405 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 60 张预览图并检查商城与神秘商店场景。真实账号购买与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 设置页自动化与策略字段文案对齐 Vue

- 自动化分组按 Vue locale 对齐全部开关与字段标签：自动种植收获 / 推送触发巡田 / 自动升级土地 / 自动做任务 / 自动卖果实 / 自动好友互动 / 巡田时跳过一键务农；自动填充化肥 / 自动购买有机化肥 / 自动购买无机化肥 / 多季补肥；施肥范围（紫金土地 / 金土地 / 黑土地 / 红土地 / 普通土地）、施肥策略、快成熟判定秒数、购买数量、触发阈值 (小时)；自动偷菜 / 自动帮忙 / 自动捣乱 / 经验满不帮忙 / 好友申请自动通过 / 最低等级 / 不低于自己等级 / 收偷比过滤 / 收获份数 / 偷菜份数；神秘商人自动购买 / 神秘商人到货通知 / 购买成功通知 / 允许用金币购买 / 允许用点券购买 / 允许用金豆豆购买 / 允许用钻石购买；保存按钮改为「保存自动控制 / Save Automation」；英文标签同步为 Vue en locale。
- 策略页字段文案对齐：巡田间隔（秒）—— GPUI 之前误标为「分钟」而字段值与后端均为秒；农场巡查最小/最大、帮助巡查最小/最大、偷菜巡查最小/最大；好友静默时段 / 启用静默时段 / 开始 / 结束 / 静默时继续农场巡查；种植与偷菜延迟设置 / 种植顺序随机 / 种植延迟 (秒) / 偷菜延迟 (秒)。
- 新增设置自动化预览场景（中英文），重新渲染 62 张预览图并检查策略与自动化两个页签。
- **验证通过**：GPUI `ui-preview` 全量 405 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。真实设置保存与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 游戏配置页字段与列标题对齐 Vue

- 表格列标题改为 Vue locale：种子 / 种子ID / 等级 / 季节 / 生长时间 / 收获数 / 经验 / 价格 / 操作；果实表：果实 / 果实ID / 植物 / 稀有度 / 价格 / 操作；道具表：道具 / 物品ID / 类型 / 稀有度 / 价格 / 操作（英文 Seed ID / Level / Seasons / Grow Time / Harvest / Exp / Plant / Type / Operate）。
- 搜索筛选标签改为 季节 / 稀有度 / 类型（原为季数、品质、道具类型）。
- 表单字段标签与占位对齐：种子ID、阶段模板、生长阶段、等级、季节、收获数、价格、货币、经验、占地、稀有度、堆叠上限、可使用、交互类型、类型、植物、资源名、描述、效果描述；尺寸选项改为 Vue 的 1×1（普通作物）/ 2×2（占地4格）/ 3×3（占地9格）。
- 编辑弹窗：标题改为「录入种子 / 录入果实 / 录入道具」与「编辑种子 / 编辑果实 / 编辑道具」，底部按钮改为「取消 / 确认」；工具栏「新增配置」改为「新增」、「批量删除（N）」改为「批量删除」，与 Vue 表格工具栏一致。
- **验证通过**：GPUI `ui-preview` 全量 405 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 62 张预览图并检查种子表格。真实配置写入与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 账号页字段与工具栏对齐 Vue

- 表格列标题改为 Vue locale：序号 / 账号备注 / 平台 / 运行状态 / 授权状态 / 最近在线 / 操作（英文 Index / Display Name / Platform / Run Status / Authorization / Last Online / Operate）；其中「账号名称」改为 Vue 的「账号备注」，「微信授权」列改为「授权状态」。
- 搜索筛选标签「微信授权」改为「授权状态」（英文 Run Status / Authorization），与表格列一致。
- 账号表单：字段标签改为 Vue 的「账号备注」（原「账号名称」）与「备注」（原「账号备注」）；占位改为 Vue 的「留空默认「账号ID」」「可粘贴裸 Code，或完整 URL（含 platform/os/ver/code）」「备注」；Code 字段标签改为「Code / 登录 URL」；标题「添加账号」改为「新增账号」，底部保存按钮改为「确认 / Confirm」。
- 表格工具栏：「添加账号」改为「新增账号 / Add Account」，「删除所选 (N)」改为 Vue 的「批量删除 / Batch Delete」。
- 更新账号相关回归断言；布局测试改为断言表单按钮标签，避免依赖面板滑入动画未完成时的瞬时可见性。
- **验证通过**：GPUI `ui-preview` 全量 405 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 62 张预览图并检查账号表格与新增账号表单。真实登录授权与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 共享搜索占位按页面对齐 Vue

- 共享工具栏搜索框的占位文案改为按页面取 Vue 对应搜索框的文案：商城「搜索商品名称 / ID」（Vue mall 页内联占位）、好友「搜索昵称 / GID」（`page.farm.friends.searchPlaceholder`）、游戏配置搜索面板「请输入关键词搜索」（`common.keywordSearch`）；其余页面保留「搜索当前页面 / Search this page」。之前所有页面共用「搜索当前页面」，与 Vue 不一致。
- 同步时机：启动时按存储的所选页面初始化；侧边栏切换页面、账号管理入口、命令面板翻页与「打开设置」后立即同步；切换语言时按当前页面重取文案；预览场景种子同步，保证截图与实际一致。
- 新增回归：`page_search_placeholder_tracks_page_and_locale` 依次切到 商城/好友/游戏配置 断言中文占位，再切英文后断言英文占位。
- **验证通过**：GPUI `ui-preview` 全量 406 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 62 张预览图并检查商城、好友与游戏配置场景。真实账号与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 我的背包工具条与卡片细节对齐 Vue

- 继续非活动模块逐项对照 `desktop-ui/src/views/farm/personal/BagPanel.vue`：
  - 分类按钮「工具」改为 Vue 的 `bagCatTool`「道具 / Tool」，英文种子「Seeds」改回 Vue 的 `Seed`。
  - 批量模式文案按 Vue：批量开关显示「批量出售 / Batch Sell」，激活后为 `common.cancel`「取消 / Cancel」（原为“取消批量出售”）；「全选可售」改为 `common.selectAll`「全选 / Select All」；提交按钮由“出售所选 (N) / Sell selected (N)”改为 `personal.batchSell`「批量出售 (N) / Batch Sell (N)」，并从主色改为 Vue 的 error 危险色。
  - 空态改为 Vue `common.noData`「无数据 / No Data」（搜索无结果时保留“没有匹配的物品 / No matching items”）；卡片数量由「×N」改为 Vue 的「xN」；出售条件不再单独占一行，改为按 Vue 挂在出售状态文字上的悬停提示。
- 新增背包预览场景（`bag-light`/`bag-dark`/`bag-english`，共 65 张预览图），配套 bag 快照夹具（可出售果实、条件出售果实、种子、可用道具），补齐该页此前无截图可核对的缺口。
- 回归更新与新增：分类英文名、批量按钮文案（中英文）断言；新增空态「无数据 / No Data / No matching items」回归。
- **验证通过**：GPUI `ui-preview` 全量 407 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 65 张预览图并检查背包浅色/深色/英文场景。真实账号与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 我的任务页每日礼包卡片布局对齐 Vue

- 对照 `desktop-ui/src/views/farm/personal/DailyOverview.vue` 与 `TaskPanel.vue` 复核「我的任务」分段：
  - 每日礼包卡片的图标改为 Vue 的 32×32 圆角底色方块（原为无底色的裸 emoji）；
  - 卡片头部改为 Vue 的「图标 + （名称/状态标签）同行」布局，状态标签跟名称列内联显示并按内容宽度收窄（此前是整行独立标签，改为列内布局后曾被拉伸成长条，已修复）。中英文状态文案此前已与 locale 一致（今日已完成 / Done today、等待执行 / Waiting、未开启 / Disabled、未开通 / Not activated、完成时间 / Done at）；
  - 成长任务面板标题、汇总徽标（进度：N/M 与今日已完成）、行内「已完成/进行中」标签与进度文案复核后与 Vue 一致；本轮未发现其他字段差异。
- 新增「我的任务」预览场景 `daily-gifts-light` / `daily-gifts-dark` / `daily-gifts-english` 与快照夹具（6 种礼包状态 + 3 个成长任务），预览图从 65 张增至 68 张，补齐该分段此前无截图可核对的缺口。
- **验证通过**：GPUI `ui-preview` 全量 407 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 68 张预览图并检查我的任务浅色/深色/英文场景。真实账号与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 图鉴页等级标签、未点亮文案与 buff 徽标对齐 Vue

- 对照 `desktop-ui/src/views/farm/personal/IllustratedPanel.vue` 复核「图鉴」分段：
  - 等级从进度行里的「Lv.N」文字改为 Vue 卡片右上角的等级徽标（`illustrated.level` =「等级 N / Lv N」，信息色圆角标签），放在图鉴页签行右端；进度行只保留进度条与 `progress / nextLevelProgress` 文本；
  - 卡片悬停说明里的锁定文案由「未解锁 / Locked」改为 Vue locale 的「未点亮 / Not Lit」，奖励数量由「×N」改为 Vue 的「xN」；
  - 加成 buff 由无底色文字改为 Vue 警示色 NTag 样式的圆角徽标（如「变异几率 +15%」「变异收获 +3」），并补无障碍名称。
- 新增「图鉴」预览场景 `illustrated-light` / `illustrated-dark` / `illustrated-english` / `illustrated-mutants-light` 与快照夹具（作物/变异两册、解锁与未点亮条目、星级、加成 buff），预览图从 68 张增至 72 张。
- 回归更新：等级徽标中英文（等级 2 / Lv 1）、进度文本（40 / 80）、buff 徽标文本断言。
- **验证通过**：GPUI `ui-preview` 全量 407 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 72 张预览图并检查图鉴浅色/深色/英文/变异册场景。真实账号与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 特殊互动道具面板对齐 Vue

- 对照 `desktop-ui/src/views/farm/personal/InteractionItemsPanel.vue`（self 模式）逐项对齐「互动道具」分段：
  - 面板标题改为 Vue 的「特殊互动道具 / Interaction Items」，刷新按钮改为 `pet.refresh`「刷新 / Refresh」；
  - 道具列表改为 Vue 的 NList 样式有边框圆角容器（行间分隔线，最后一行不再重复描边）；数量改为 Vue 的「xN」小标签；可出售改 `interaction.sellableCount` 警示色徽标「可出售: N / Sellable: N」；描述改为 Vue 的 12px 次要色并限制 360px 截断；
  - 控制行改为 Vue 的「选择器 + 地块输入 + 使用按钮」单行无标签布局：选择器修正为固定宽度容器（组件根使用 `size_full` 会在换行行内被撑满，导致控件各占一行），选项文案改为「名称 xN」；地块占位改为 `interaction.landIdsPlaceholder`「例如： 1,2,3 / e.g. 1,2,3」；按钮改为 `interaction.use`「使用 / Use」。
  - 未输入地块时的提示改用 Vue 的占位文案；成功回退文案英文改为 locale 的「Batch use done」。
- 新增「互动道具」预览场景 `interaction-light` / `interaction-dark` / `interaction-english` 与快照夹具，预览图从 72 张增至 75 张。
- 回归更新：可出售徽标中英文、使用按钮「Use」、地块占位与校验提示断言。
- **验证通过**：GPUI `ui-preview` 全量 407 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 75 张预览图并检查互动道具浅色/深色/英文场景。真实账号与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 宠物分段文案与标签对齐 Vue

- 对照 `desktop-ui/src/views/farm/personal/PetPanel.vue` 与 `pet` locale 逐项对齐「宠物」分段：
  - 标题改为 `pet.title`「我的宠物 / My Pets」；待领礼包改为警告色圆角徽标「待领礼包: N / Pending Gifts: N」，领取按钮改为 `giftClaim`「领取 / Claim」，守护记录/刷新按钮改为 `logTitle`「守护记录 / Guard Logs」与 `refresh`「刷新 / Refresh」。
  - 摘要改为 Vue NDescriptions 的「标签 + 值同行」布局，标签改用 `protectRemaining`「守护剩余 / Guard Left」等 locale 文案。
  - 犬列表改为 Vue NList 描边圆角容器（末行不重复描边）；状态、可激活、稀有度改为 Vue NTag 样式的圆角徽标，状态文案改为 `onDuty`「看家中 / On Duty」、`owned`「已获得 / Owned」、`locked`「未获得 / Locked」、`activatable`「可激活 / Activatable」；技能预览改为 12px；激活/上场/收回按钮改用 Vue 的 warning/primary/warning 层级；「收回」按钮恢复为内容宽度（此前被纵向布局拉伸为整行）。
  - 狗粮行：名称改为 120px 截断，库存/时长改为「库存: N / Stock: N」与「+N天 / +Nd」徽标，按钮改为 `foodUse`「喂食 / Feed」；空态英文改为 `foodEmpty`「No dog food」。
  - 守护记录弹窗标题改为「守护记录 / Guard Logs」，列标题改为「被偷数量 / Stolen」「守护金币 / Guarded Gold」；操作成功提示改用 locale 原文（宠物激活成功/宠物上场成功/宠物已收回/喂食成功/已领取礼包: N）。
- 新增「宠物」预览场景 `pets-light` / `pets-dark` / `pets-english` 与快照夹具（值守犬、可上场犬、可激活犬、未获得犬与两种狗粮），预览图从 75 张增至 78 张。
- 回归更新：头部按钮中英文断言。
- **验证通过**：GPUI `ui-preview` 全量 407 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 78 张预览图并检查宠物浅色/深色/英文场景。真实账号与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 好友页互动道具分段对齐 Vue

- 好友页的「互动道具」分段此前是自定义行内「选择/已选择」按钮 + “已选道具”文字提示，与 Vue（好友页复用 `InteractionItemsPanel mode="friend"`：列表 + 选择器/GID/地块/使用 控制行）结构不一致。本轮改为：
  - 复用个人页的互动道具选择器组件，新增 `friends.interaction_picker`，并在分页/刷新加载后同步（选中项失效时回退到第一项）；下拉确认事件回写 `selected_interaction_item_id`；
  - 控制行改为 Vue 的「选择器 + 好友 GID + 好友地块编号 + 使用」布局，按钮文案改为 `interaction.use`「使用 / Use」（原“对好友使用”），删除自造的“已选道具”文字提示；
  - 列表行改为 Vue 的「名称 + xN 标签 + 可出售: N 警示标签 + 右侧描述截断」样式，去掉自造的行内选择按钮与头像。
- 新增「好友互动道具」预览场景 `friends-interaction-light` / `friends-interaction-dark` / `friends-interaction-english` 与快照夹具，预览图从 78 张增至 81 张。
- 回归更新：默认选中第一项、picker 确认事件回写选中项、行/控件存在性断言。
- **验证通过**：GPUI `ui-preview` 全量 407 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 81 张预览图并检查好友互动道具浅色/深色/英文场景。真实好友服务操作与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 设置页下线提醒与运行环境分段对齐 Vue

- 对照 `desktop-ui/src/views/farm/settings/index.vue` 的 offline/system 两个标签页逐项对齐：
  - 分段标题改用 Vue tab 文案：`offlineReminder`「下线提醒 / Offline Alert」（原“离线提醒”）、`systemConfig`「运行环境 / Runtime」（原“系统配置”）。
  - 下线提醒：卡片去掉与分段重复的标题，首行改为 Vue 的「机器人类型 + 选择器（flex-1 宽度）+ 官网按钮（仅 QQ Bot 可用）」；QQ Bot 分支改为 Vue 的 AppID / AppSecret 独立字段、完整 `qqBotHint`、「绑定状态」徽标 + 昵称，按钮改为「绑定 QQ 通知 / 打开机器人 / 解绑」；页脚加分隔线并用 `testOffline`「测试通知」、`saveOffline`「保存下线提醒」；机器人类型下拉选项改为 Vue 的「不推送 / QQ 官方机器人 / 钉钉机器人 / 微信机器人（敬请期待）」。
  - 运行环境：字段标签对齐 Vue locale（服务器地址 / 平台 / 操作系统 / 内存 (MB) / 重置为默认）；时区由按钮组改为 Vue 的下拉选择器（新增 `settings.time_zone`，随草稿与语言同步，确认后写回 `systemConfig/timeZone`）。
- 新增预览场景 `settings-offline-light/dark/english`、`settings-system-light/dark/english`、`settings-ui-light/english`，设置夹具补充 `systemConfig`（含 deviceInfo）与设备预设数据，预览图从 81 张增至 89 张。
- 回归更新：机器人类型可访问名、测试/保存按钮英文、QQ Bot 完整提示文案；系统配置测试改为通过选择器确认事件切换时区。
- **验证通过**：GPUI `ui-preview` 全量 407 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 89 张预览图并检查下线提醒、运行环境与界面偏好的浅色/深色/英文场景。真实提醒推送与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 商城 SVIP 与神秘商人标签预览补齐

- 商城页此前只有「普通商城」有预览图，本轮补上另外两个标签的可视验证：
  - 新增 `mall-svip-light` / `mall-svip-dark` / `mall-svip-english` 场景与 SVIP 目录夹具（SVIP 会员条、购买/需要 SVIP/奖励已领取/已拥有四种商品卡状态），预览中确认会员条、钻石余额、刷新倒计时与四态商品卡与普通商城一致。
  - 新增 `mall-mystery-light` / `mall-mystery-english` 场景（页内「神秘商人」标签复用神秘商人面板，不带重复标题、仅保留右对齐刷新按钮），与独立「神秘商店」页面区分。
- 预览图从 89 张增至 94 张；本轮未发现需要修改的代码差异。
- **验证通过**：GPUI `ui-preview` 全量 407 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。真实购买与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 预览补齐最后三个深色变体

- 补上此前只有浅色/英文截图的深色场景：`analytics-blacklist-dark`、`settings-automation-dark`、`settings-ui-dark`，使收益分析黑名单、自动化、界面偏好在浅色/深色/英文三种核对条件下都有预览。
- 逐张核对三个深色场景：卡片描边、危险色按钮、开关轨道/滑块、选中态主色在深色主题下对比度与浅色一致，本轮未发现需要修改的代码差异。
- 预览图从 94 张增至 97 张，覆盖总览、我的农场（六分段）、好友（四分段）、收益分析（两分段）、商城（三标签）、神秘商店、设置（五分段）、游戏配置、账号与各活动页的浅色/深色/英文组合。
- **验证通过**：GPUI `ui-preview` 全量 407 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。真实账号与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 好友列表行内容对齐 Vue

- 对照 `desktop-ui/src/views/farm/friends/index.vue` 的好友卡片行，修正折叠态三处内容差异：
  - `(GID)` 从徽标行的第一个徽标改回 Vue 的「昵称 (GID)」同一行内联显示；
  - 金币徽标由中性底色改为 Vue 的琥珀色提示样式（`amber` 底 + 琥珀文字），浅深主题跟随主题 token；
  - 有可执行互动的好友状态行（如“偷 2 水 1”）改为 Vue 的中粗体强调。
- 逐张核对好友列表浅色/深色/英文预览，其余内容（头像、Lv 徽标、宠物徽标、操作按钮、展开详情与地块网格）与 Vue 一致。
- **验证通过**：GPUI `ui-preview` 全量 407 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 97 张预览图并检查好友列表场景。真实好友服务操作与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 好友黑名单与最近访客行对齐 Vue（含漏挂按钮修复）

- 对照 `desktop-ui/src/views/farm/friends/index.vue` 的黑名单/访客模板逐项核对：
  - 黑名单行：Lv 与金币由徽标改回 Vue 的纯灰文本、去掉宠物徽标与「已加入黑名单」状态行（Vue 该行只有头像、昵称 (GID)、Lv/金币和移出按钮）；
  - **修复真实缺口**：黑名单行的「移出黑名单」按钮此前从未渲染——行动作挂载条件只覆盖好友行与互动道具行，`append_friend_row_actions` 中的 unblacklist 分支成了死代码。现在黑名单行同样挂载动作区（含确认弹窗），并新增回归 `blacklist_rows_keep_the_unblock_action`；
  - 访客行：头像对齐方式由垂直居中改为 Vue 的顶部对齐（`items-start`）；其余字段（行动作彩色徽标、Lv、GID、详情、右侧相对时间）复核一致。
- 新增预览场景 `friends-blacklist-light/english` 与 `friends-visitors-light/english`（含黑名单夹具与三条偷取/帮忙/捣乱访客记录），预览图从 97 张增至 101 张。
- **验证通过**：GPUI `ui-preview` 全量 408 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 101 张预览图并检查黑名单与访客场景。真实好友服务操作与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 账号表格行操作按钮配色对齐 Vue

- 对照 `desktop-ui/src/views/farm/account/index.vue` 的 operate 列：Vue 的行操作为彩色 ghost 按钮（编辑 primary、启动 success、停止 warning、删除 error），GPUI 此前全部是灰色 ghost 文本按钮。现改为对应的彩色描边样式（variant + outline），并保留启停的在途 loading/禁用与删除确认。
- 复核同行其余单元格：序号/账号备注/平台、运行状态（运行中绿 / 已停止灰，与 Vue 的运行/停止标签一致）、授权状态（已授权绿 / 未授权与建议重扫为警告色并带悬停说明）、最近在线时间格式，均与 Vue 保持一致；Vue 表中的“错误(2)”状态经适配后与 GPUI 一样归一为已停止，无需额外状态。
- **验证通过**：GPUI `ui-preview` 全量 408 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 101 张预览图并检查账号表格场景。真实登录授权与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 游戏配置表格与工具栏按钮配色对齐 Vue

- 对照 `desktop-ui/src/views/farm/game-config/index.vue` 与共享组件 `table-header-operation.vue`：
  - 行操作 `renderOperate` 为「编辑 primary ghost + 删除 error ghost」，GPUI 此前是灰色文本按钮，现改为对应的彩色描边样式（编辑 primary+outline、删除 danger+outline），删除确认文案与 Vue `common.confirmDelete`「确认删除吗？」一致；
  - 表格工具栏按共享组件对齐：新增（primary ghost → primary+outline）、批量删除（error ghost → danger+outline，保留选中为空时禁用）。
- 同步对齐账号页工具栏：新增账号（primary+outline）与批量删除（danger+outline），与账号表行的彩色操作按钮保持同一视觉层级。
- **验证通过**：GPUI `ui-preview` 全量 408 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 101 张预览图并检查游戏配置与账号表格场景。真实配置写入与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 我的农场与好友工具栏按钮对齐 Vue

- 「我的农场」操作按钮此前是灰色文本且文案与 Vue locale 不一致，本轮修正：
  - 文案对齐 `page.farm.personal.op`：`clear` 由「清理 / Care」改为「一键务农 / Maintain」、`upgrade` 由「升级 / Upgrade」改为「升级土地 / Upgrade Lands」、`all` 由「一键务农 / Farm all」改为「一键全收 / Run All」（`harvest`/`plant` 原本一致）；
  - 按 Vue 的 `operateOptions` 为每个操作上色（收获 success、一键务农 info、种植 primary、升级土地 warning、一键全收 error），使用彩色描边样式，与页面卡片语言一致。
- 好友页「全部偷取」按 Vue 的 `type="primary" ghost` 改为蓝色描边（原先为灰色文本）；刷新列表等 Vue 默认样式按钮继续沿用应用内灰字次按钮，保持工具栏层级一致。
- **验证通过**：GPUI `ui-preview` 全量 408 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 101 张预览图并检查我的农场与好友场景。真实农场操作与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 好友分段空态文案对齐 Vue

- 好友页三个分段此前共用通用空态文案「这里还没有可显示的内容」，与 Vue 的独立空态不一致。现按 Vue locale 对齐：好友列表「无数据 / No Data」（`common.noData`）、好友黑名单「暂无黑名单好友 / No blacklisted friends」（`blacklistEmpty`）、最近访客「暂无访客记录 / No visitor records」（`visitorsEmpty`）；其余页面继续使用通用空态并保留刷新入口。
- 空态标签补充可访问名（`empty-state-label`），新增回归 `friend_section_empties_follow_vue_copy` 断言三个分段的中文空态与英文访客空态文案。
- 本轮改动仅影响空数据场景，现有预览场景都有数据，无需重新渲染截图。
- **验证通过**：GPUI `ui-preview` 全量 408 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。真实好友服务与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 账号与游戏配置表格空态对齐 Vue

- Vue 的账号表与游戏配置表都使用 NDataTable 内建空态（`common.noData`「无数据 / No Data」）。GPUI 之前始终显示筛选态文案（“没有符合条件的账号 / 没有符合条件的配置”）。现改为：无任何筛选（关键词/平台/运行状态/授权状态，或关键词/季节/稀有度/类型）时显示「无数据 / No Data」；有筛选时保留更具体的“没有符合条件…”提示。
- 两个空态容器补充 `accounts-empty` / `game-config-empty` 可访问名，新增回归 `account_table_empty_copy_follows_vue_no_data_and_filters` 与 `game_config_empty_copy_follows_vue_no_data_and_filters`，断言无筛选、有筛选与英文三种状态。
- 本轮改动仅影响空数据场景，现有预览场景都有数据，无需重新渲染截图。
- **验证通过**：GPUI `ui-preview` 全量 411 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。真实账号与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 商城普通标签补充刷新按钮并对最近改动做深色复查

- 对照 `views/farm/game-mall/index.vue`：Vue 的商城卡片右上角（header-extra）在会员/钻石标签之后还有一个「刷新」按钮（`loadMall`），GPUI 的普通/SVIP 标签此前缺少该入口。现于商城面板头部右侧补上「刷新」按钮（ghost、加载时禁用，触发现有页面刷新链路），与神秘商人面板的刷新入口一致。
- 回归更新：商城目录测试新增刷新按钮存在性与可用性断言。
- 深色复查：重新渲染 101 张预览并抽查最近改动涉及的深色场景（我的农场彩色操作按钮、好友行、账号/游戏配置彩色工具栏），描边与文字对比度与浅色一致，未发现需要修改的问题。
- **验证通过**：GPUI `ui-preview` 全量 411 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。真实购买与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 下拉筛选触发样式向 Vue 选择框对齐

- Vue 的日志筛选与分页大小都使用 NSelect（带边框和下拉箭头）。GPUI 的三项日志筛选（模块/事件/等级）与账号表「每页」选择此前是无边框文本按钮，现改为默认描边样式并显示下拉箭头（`dropdown_caret`），在保持原有 popover 菜单交互的同时与 Vue 的框式选择框外观一致。
- 日志面板「显示更早的 N 条日志」从灰色文本按钮改为主色描边（对应 Vue 的 quaternary primary），「收起」保持次要样式。
- **验证通过**：GPUI `ui-preview` 全量 411 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 101 张预览图并检查总览与账号场景。真实账号与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 游戏配置果实/道具页签预览补齐并加分段覆盖回归

- 游戏配置页的「果实」「道具」页签此前只有种子有截图。本轮补充夹具（草莓/蓝莓果实、化肥/礼包道具与 itemTypes 目录）与四个场景 `game-config-fruits-light/english`、`game-config-items-light/english`，预览图从 101 张增至 105 张；截图中核对了两类表格的列（果实ID/植物/稀有度/价格、物品ID/类型/稀有度/价格）与行操作按钮。
- 预览场景的 section 映射从 `seed` 内联 match 提取为 `section_for_scene`，新增回归 `preview_scenes_cover_every_visible_page_section_in_english`：凡渲染分段页签的页面（排除账号/总览这类隐藏页签的页面），每个分段都必须有英文预览场景，防止后续新增分段时漏图。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 105 张预览图并检查果实与道具页签。真实配置写入与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 深色矩阵补齐并升级为逐分段全条件覆盖

- 逐张抽查最近改动页面的深色截图（总览、设置、游戏配置、账号、好友、商城），描边、彩色按钮、选中态与切换控件对比度均与浅色一致，未发现需要修改的问题。
- 补齐缺失的深色场景 `friends-blacklist-dark`、`friends-visitors-dark`、`mall-mystery-dark`、`game-config-fruits-dark`、`game-config-items-dark`，预览图从 105 张增至 110 张。
- 覆盖回归由「每个可见分段有英文图」升级为「每个可见分段同时具备浅色、深色、英文三张图」（`preview_scenes_cover_every_visible_page_section_in_all_conditions`），排除不渲染分段页签的账号/总览页；日后新增分段或缺失主题截图会直接测试失败。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。真实账号与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 缩放与最小宽度样例复核

- 抽查 110 张预览中的缩放与窄窗样例：`account-sheet-125`、`account-sheet-150`（新增账号面板在 125%/150% 字体缩放下字段、分段控件与底部按钮完整无裁切）、`accounts-minimum`（1100×720 最小窗口下表格按预期横向滚动，工具栏与分页完整）、`weather-125/150/minimum` 一并确认布局稳定。
- 本轮为验收复核，无代码改动；树状态与上一轮 412 项测试通过时一致，另执行 `cargo fmt --all -- --check` 与 `git diff --check` 确认干净。
- 说明：截图只能证明静态布局在缩放/窄窗下不溢出；真实窗口的键盘导航、输入法与 DPI 行为仍按清单在实机验收。

### 2026-10-12 — GPUI 商城卡片字段与购买按钮状态对齐 Vue

- 逐字段核对 `game-mall/index.vue` 商品卡与 GPUI 卡片：名称（两行截断）、原价（删除线灰色，仅非免费时显示）、现价（绿色）、折扣（红色）、限购 b/m、不可购买原因（红色、仅非 available 时显示）、奖励图 64px 与 🛒 兜底均一致；GPUI 的“原价”前缀只用于无障碍名称，可见内容仍是纯数字，与 Vue 相同。
- 购买按钮状态改为与 Vue 一致：可购买时为实心主色；不可购买时保持默认描边样式并禁用（原先 GPUI 用灰色 ghost，Vue 使用 default 风格的禁用按钮）。神秘商店的购买按钮同步调整：Vue 是 `type="primary" :disabled`，GPUI 现在始终主色、仅在不可购买时禁用（原先禁用态会变灰 ghost）。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 110 张预览图并检查商城与神秘商店场景。真实购买与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 我的农场地块卡片细节对齐 Vue

- 对照 `personal/FarmPanel.vue` 的地块卡与摘要标签逐项修正：
  - 状态标签对齐 `landStatusTag` locale：合种占用（原「已合并」）、空闲（原「空地」）、枯死（原「已枯萎」）、可偷（原「可偷取」）；可收获/生长中/未解锁保持不变，好友农场浮层地块标签同步「空闲/枯死」。
  - 摘要标签改为 Vue 的 `summary*` 文案并补冒号：可收: N / 生长: N / 空闲: N / 枯萎: N（原为可收获/生长中/空地/枯萎且无冒号）。
  - 合种占地徽标改为 Vue 的 `plantSizeBadge`「合种 NxN」（原为裸「N×N」）；季节徽标改为「第x/y季」（去掉多余空格）。
  - 变异徽标改为 Vue 的琥珀色提示样式（原为普通底色），紫晶共鸣徽标改为 Vue 的 error 红色（原 info 蓝色），英文文案改为 `purpleCrystalBadge` 原文「Purple crystal +N%」。
  - 浇水/除草/除虫徽标改为 Vue 的纯 emoji 标签（💧/🌿/🐛），文字仅保留在无障碍名称里。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 110 张预览图并检查我的农场场景。真实农场操作与跨平台实窗仍未验收。

### 2026-10-12 — GPUI 好友展开地块卡复用个人地块组件

- 好友展开详情里的地块卡此前是独立实现：状态为灰色文本、缺水/草/虫显示为“水 · 草”文字、倒计时带「剩余」前缀且进度条宽度固定、季节显示「第 1 / 2 季」。现改为复用个人农场地块组件（把 `land_status_label`/`land_status_color`/`soil_label`/`care_flags`/`render_countdown`/`land_badge_with_color` 提升为 desktop 可见）：
  - 顶部状态改为彩色徽标（生长中绿、可收获琥珀、空闲灰、可偷信息色等）；
  - 倒计时与进度条使用与个人地块一致的渲染（橙色时间 + 自适应宽度进度条，结束时显示阶段名）；
  - 缺水/草/虫改为与 Vue 相同的彩色 emoji 徽标（💧 info、🌿 success、🐛 error，文字仅保留无障碍名）；
  - 季节徽标改为「第x/y季」（仅在总季数大于 1 时显示）；土壤标签统一使用个人地块的文案。
- 预览夹具中好友地块的非法状态 `ready` 修正为 `harvestable`，预览里显示真实「可收获」状态；浮层地块标签回归同步更新（未知状态回退为原样显示，与 Vue 一致）。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 110 张预览图并检查好友展开详情。真实好友服务与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 生涯统计条配色对齐 Vue

- Vue 我的农场的生涯统计条使用淡紫底（`bg-violet-50`，深色 `violet-900/20`），GPUI 此前是中性灰底。现改用主题的 `magenta`（默认主题映射到 purple 系）以 12% 透明度着色，浅色呈淡紫、深色呈暗紫，与 Vue 的紫罗兰色调一致；好友展开详情里的生涯条在 Vue 中本来就是白/深灰底，保持中性不变。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`；重新渲染 110 张预览图并检查我的农场浅色/深色场景。真实农场操作与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 文案全量比对与离线提醒测试结果对齐 Vue

- 用脚本把 GPUI 非活动模块里的 355 条中文 i18n 文案与 Vue 语言包比对：139 条不在语言包中，逐条抽查确认全部属于「Vue 模板硬编码、GPUI 翻译为 i18n」（如 单季/双季、帮务农、免费、条件出售）或 GPUI 专属界面文案（工具栏、加载中、错误提示、通用空态），未发现遗漏或错译。
- 比对中发现「测试通知」的结果文案与 Vue 不一致，按 Vue locale 对齐：成功改为 `testOfflineSuccess`「测试消息发送成功 / Test message sent」（原“测试通知已发送。”）；失败改为 Vue 的 `testOfflineFail: msg` 形式「测试失败: {msg}」（消息缺失回退 `unknown`，原为直接显示 msg 或“测试通知未发送。”）。同步更新回归断言并移除失效的英文映射。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。本轮只影响设置页运行时提示文案，预览场景不含该状态，无需重新渲染截图。真实提醒推送与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 搜索面板按钮顺序与样式对齐 Vue

- 对照 `game-config-search.vue` 与 `account-search.vue`：两处都是「重置（默认样式）+ 搜索（primary ghost）」且右对齐。GPUI 此前是「搜索（实心主色）+ 重置（灰色）」，现改为：搜索改用主色描边（primary + outline），游戏配置面板的重置按钮移到搜索之前，与 Vue 的按钮顺序和层级一致；账号面板顺序原本已一致，只调整搜索按钮样式。
- 本轮只影响折叠搜索面板展开后的按钮，当前预览场景均收起该面板，无需重新渲染截图。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。真实账号查询与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 购买确认弹窗标题与按钮对齐 Vue

- 对照商城与神秘商店的确认弹窗：Vue 两处弹窗标题都是「确认购买」、确认按钮文本都是「购买」。GPUI 普通商城此前标题为「购买确认」、确认按钮为「确认购买」，现分别改为「确认购买 / Confirm purchase」与「购买 / Buy」；神秘商店标题原本一致，活动兑换（星砂）保留「兑换确认 / 确认兑换」。
- 回归更新：商城/神秘商店测试中的确认按钮英文断言改为「Buy」。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。本轮只影响弹窗文案，预览场景不含弹窗状态，无需重新渲染截图。真实购买与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 设置保存提示文案对齐 Vue

- 对照设置页的保存成功提示与 Vue locale：
  - 策略保存提示由「种植策略已保存。」改为 `saveStrategySuccess`「策略设置已保存 / Strategy settings saved」；
  - 自动化保存提示由「自动化设置已保存。」改为 `saveAutomationSuccess`「自动控制已保存 / Automation settings saved」（去掉句号）；
  - 下线提醒保存提示由「离线提醒设置已保存。」改为 `saveOfflineSuccess`「下线提醒已保存 / Offline alert saved」。
  - 运行环境保存/重置提示原本已与 Vue 一致；账号删除确认、好友三类确认、游戏配置删除确认经复核也全部与 Vue locale 相同。
- 同步更新回归夹具里的旧提示文本并移除失效英文映射。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。本轮只影响运行时提示文案，预览场景不含这些状态，无需重新渲染截图。真实设置保存与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 账号保存/启停提示对齐 Vue

- 账号新增/编辑/删除与启停的成功提示改为 Vue `common` locale 原文：英文由 “Account added / Account updated / Account deleted / Updated successfully” 统一为 “Add Success / Update Success / Delete Success”（中文仍为 添加成功 / 更新成功 / 删除成功，原本一致）。
- 自动启动失败的附加提示按 Vue 抽屉文案结构对齐：「更新成功，自动启动失败，请手动重新登录：{error}」与「添加成功，自动启动失败，请手动点击启动：{error}」（Vue 为提示 + 固定后缀；GPUI 额外保留具体错误详情，便于排查）；表单保存与微信扫码两条路径（`accounts/form.rs`、`accounts/wx_login/qr.rs`）同步修改。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。本轮只影响运行时提示文案，预览场景不含这些状态，无需重新渲染截图。真实登录授权与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 好友操作反馈文案对齐 Vue

- 逐条对照 `friends/index.vue` 的 message 文案：
  - 单项操作空结果提示「当前没有可偷取的好友作物。」改为 Vue `stealAllEmpty`「当前没有可偷好友 / No friends with stealable crops」；
  - 单项操作成功兜底英文由 “Action completed” 改为 `opSuccess` 原文 “Operation succeeded”（中文「操作成功」原本一致）；
  - 批量偷取汇总：英文由 “Steal all complete: … succeeded, … skipped” 改为 `stealAllDone` 原文 “Steal all done: {ok} ok, {skip} skipped”，奖励明细与 Vue 相同用「、」连接、「：」追加（Vue 在中英文下都用全角分隔），中文格式原本一致；
  - 删除好友成功英文由弯引号改为 Vue `deleteSuccess` 的直引号 “Deleted friend \"{name}\"”（中文一致）。
  - 黑名单成功/失败、opFailed 兜底经复核与 Vue 相同（失败透传后端 error）。
- 同步更新批量偷取汇总单元测试的英文期望。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。本轮只影响运行时提示文案，预览场景不含这些状态，无需重新渲染截图。真实好友服务与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 农场操作与购买反馈文案对齐 Vue

- 我的农场操作成功提示由「操作成功 / Operation completed」改为 Vue `operateSuccess` 原文「操作已提交 / Operation submitted」（失败路径继续透传后端错误，与 Vue `error.message || operateFailed` 一致）。
- 复核商城/神秘商店购买反馈与收益分析相关提示：购买成功「购买成功 / Purchase successful」（Vue 为硬编码「购买成功」）、星砂兑换反馈、背包出售/使用汇总等均已在此前轮次对齐，本轮未发现新的差异。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。本轮只影响运行时提示文案，预览场景不含这些状态，无需重新渲染截图。真实农场操作与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 背包反馈与收益分析黑名单提示复核

- 背包出售/使用/批量出售的后端汇总缺失时的英文兜底改为 Vue locale 原文：`Sold successfully`、`Used successfully`、`Batch sell succeeded`（原为 "Item sold/Item used/Items sold"；中文出售成功/使用成功/批量出售成功原本一致）。
- 收益分析黑名单：成功提示「黑名单已更新 / Blacklist updated」与 Vue `blacklistUpdated` 一致；Vue 失败时静默，GPUI 额外展示带错误详情的“黑名单更新失败”，属于更利于排查的有意增强，保留。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。本轮只影响运行时提示文案，预览场景不含这些状态，无需重新渲染截图。真实背包操作与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 游戏配置与好友互动道具结果提示对齐 Vue

- 游戏配置保存：Vue 三个抽屉都用 `common.updateSuccess`「更新成功 / Update Success」（新增与编辑相同），GPUI 此前新增显示「添加成功 / Configuration added」，现统一为「更新成功 / Update Success」并移除不再使用的 `adding` 变量；删除与批量删除的成功提示英文由 “Configuration(s) deleted” 统一为 `common.deleteSuccess` 原文 “Delete Success”（中文「删除成功」原本一致）。
- 好友互动道具批量使用的成功兜底英文由 “Batch use complete” 改为 Vue `useSuccess` 原文 “Batch use done”（中文「批量使用完成」原本一致，与个人页保持一致）。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。本轮只影响运行时提示文案，预览场景不含这些状态，无需重新渲染截图。真实配置写入与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 微信快捷登录状态与入口对齐 Vue

- 对照 `account-operate-drawer.vue` 的本机微信流程：
  - 就绪提示由「检测到本机微信，可以开始授权」改为 Vue 原文「本机微信已就绪，请点击授权」（昵称分支 `${nickname} · 请在电脑微信中确认` 原本一致）；
  - 检测/授权失败回退扫码的提示改为 Vue 的结构：「未检测到本机微信：{原因}。请扫码登录」；授权失败时若存在错误信息则与 Vue 一样直接展示错误原文，为空时回退「本机微信授权失败，请扫码登录」；
  - 「微信快捷登录」按钮按 Vue 的 `type="success"` 改为绿色成功样式（原为主色）；登录方式分段「输入 Code」按 Vue 原文改为「输入 code」。
- 同步更新账号消息英文本地化的精确键与前缀规则（移除已废弃的旧前缀，新增「未检测到本机微信：」前缀与授权失败回退键）。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。本轮只影响运行时提示与弹窗按钮，预览场景不含这些状态，无需重新渲染截图。真实微信授权与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 账号保存状态文案对齐 Vue

- 微信/Code 保存流程的两条状态文案对齐 Vue 抽屉原文：「正在保存并连接账号…」改为「正在保存账号…」（英文 “Saving the account…”）；保存失败状态「保存失败」改为「保存账号失败」（英文 “Could not save the account”）。
- 同步更新账号消息本地化的两条映射键；扫码流程其余状态（等待微信扫码、已扫码请在手机上确认、二维码已失效/获取失败等）经复核与 Vue 相同。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。本轮只影响运行时状态文案，预览场景不含这些状态，无需重新渲染截图。真实登录授权与跨平台实窗仍未验收。

### 2026-10-13 — 对齐清单追加本机核验记录

- 在 `docs/GPUI_PARITY_CHECKLIST.md` 追加“2026-10-13 本机追加核验”小节，固化当前证据：非活动模块多轮逐项对齐、110 张预览矩阵与逐分段浅色/深色/英文覆盖回归、缩放/最小窗口抽查、355 条文案与 Vue 语言包的全量比对结论，以及 412 项测试 + 严格 Clippy + 格式/差异检查的验证门槛结果。
- 同时重申表格中的未勾选项（真实账号逐页操作、真实窗口交互与缩放、双进程唤起/托盘、三平台原生 runner）仍依赖本机之外的环境，不能由本轮证据替代。
- **验证**：本轮为文档更新，未改动代码；树状态与上一轮 412 项测试通过时一致，另执行 `cargo fmt --all -- --check` 与 `git diff --check` 确认干净。

### 2026-10-13 — macOS aarch64 release 包本机交叉构建补齐

- 在本机（x86_64 macOS）交叉构建 aarch64 release 客户端：`cargo build --release -p qq-farm-gpui --target aarch64-apple-darwin`（8 分钟），随后执行 `TARGET=aarch64-apple-darwin bash scripts/package-gpui.sh release` 生成 `dist/QQ Farm-aarch64.app` 与 `dist/qq-farm-gpui-macos-aarch64.app.zip`（约 31 MB）。
- 产物核验：`file`/`lipo -info` 确认 Mach-O arm64；`plutil` 确认 `CFBundleIdentifier=com.qqfarm.gpui`、版本 0.3.0、最低系统 12.0、高分辨率支持；包内包含 `AppIcon.icns`、`assets/game_config`（含种子图）、`activity-assets`（autumn、pet-diary）与 `tsdk.wasm`/`tsdk-wx.wasm`；zip 共 1074 个条目，`.app` 与 `.zip` 内可执行文件 SHA-256 一致（`15e95f63…bcb0322`）。
- 对照清单中“macOS x86_64/aarch64 release 构建与打包”一项已补充 aarch64 结果，并注明仍未在 ARM 实机运行——三平台原生 runner 与实机窗口验收保持未完成。
- **验证**：本轮为发布产物与文档更新，未改动源码；`cargo fmt --all -- --check` 与 `git diff --check` 通过，源码测试状态与上一轮 412 项全绿一致。

### 2026-10-13 — GPUI 主窗口标题与跨目标检查记录

- GPUI 主窗口补充标题：`WindowOptions.titlebar` 设为 `QQ Farm`（与 Tauri 版 `tauri.conf.json` 的窗口标题一致；窗口此前没有标题）。预览窗口保持无标题，不影响截图。
- 重新尝试 Windows 目标检查：`cargo check -p qq-farm-app --target x86_64-pc-windows-msvc` 依旧在 `zstd-sys` 的 C 编译阶段失败（`cc --target=x86_64-pc-windows-msvc` 需要 MSVC/llvm-rc 工具链），与清单中“缺 llvm-rc 和 MSVC SDK”的结论一致；Linux 目标同样需要目标 GCC/GTK sysroot，本机不安装新工具链时无法完成。原生平台编译与实窗验收继续由仓库 `gpui.yml` 的 macOS/Windows/Linux runner 承担。
- **验证通过**：GPUI `ui-preview` 全量 412 项测试、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。窗口标题的真实显示效果待实窗验收。

### 2026-10-13 — macOS release 应用启动冒烟

- 用隔离数据目录（`FARM_DATA_DIR=/tmp/qq-farm-gpui-smoke.*`）启动 x86_64 release 包内的可执行文件 `dist/QQ Farm-x86_64.app/Contents/MacOS/qq-farm-gpui`：进程正常拉起并稳定存活 12 秒，标准输出/错误无任何报错或 panic 文本，随后可被正常终止；临时数据目录独立于用户真实 `data/`，未触碰真实账号或顶号。
- 证据范围说明：本轮只证明 release 应用能启动、驻留并干净退出，**未断言窗口可见性、键鼠交互、主题切换与缩放**——这些仍属清单中的实窗验收项；隔离目录保留在 `/tmp` 供后续排查。
- **验证**：本轮未改动源码；`cargo fmt --all -- --check` 与 `git diff --check` 通过，源码测试状态与 412 项全绿一致。

### 2026-10-13 — 工作区级验证门槛执行

- 按仓库规范的完整门槛补跑工作区范围（此前多轮仅跑 GPUI 包）：
  - `RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`：通过（32 秒，0 警告）；
  - `cargo test --workspace -- --test-threads=1`：全部通过（退出码 0，覆盖 qq-farm-core、qq-farm-app、qq-farm-desktop、qq-farm-gpui 及 doc-tests；GPUI 的 `ui-preview` 全量 412 项在此之前单独运行同样全绿）。
- 说明：工作区运行中的 GPUI 测试未启用 `ui-preview` 特性，属于常规单测集合；预览截图与特性相关回归仍以 `--features ui-preview` 的运行为准。
- **验证**：本轮未改动源码；`cargo fmt --all -- --check` 与 `git diff --check` 通过。

### 2026-10-13 — GPUI 游戏配置表格外壳对齐账号页

- 对照 Vue：游戏配置和账号管理在 desktop-ui 里都用同一套表格外壳（`NCard` 头部操作：新增/批量删除/刷新/列设置 + `NDataTable` 底部分页）。GPUI 此前把游戏配置的分页（10/20/50/100 药丸按钮）和列设置/新增/批量删除挤在表格上方工具条，而账号页是底部「每页选择 + 页码」分页、列设置跟在搜索行，两页既不一致也不符合 Vue 结构。现统一：
  - 新增共享分页页脚 `desktop/view/pagination.rs::render_table_pagination`（左侧范围/已选、每页选择、上一页/页码/省略跳转/下一页只有这一份实现），账号与游戏配置共用；账号页原来的 `accounts/pagination.rs` 删除。游戏配置保留 Vue 的每页 `[10,20,50,100]`，账号保留全局默认 `[10,15,20,25,30]`。
  - 游戏配置的「新增 / 批量删除 / 清除选择」移到页面工具栏，与账号页的新增账号/批量删除同位置；表格上方的整条工具条删除。
  - 「列设置」移到搜索折叠行右侧，与账号页一致；折叠的「搜索」触发按钮由居中改为左对齐。
- 好友展开地块卡的作物名按 Vue 的 `text-center` 改为居中，与个人地块卡保持一致。
- **本轮发现但未修复（已记入对照清单）**：账号与游戏配置表格的行数超过可视高度时会被外框 `overflow_hidden` 直接裁剪且无法滚动。探针证据：1440×900 窗口中游戏配置默认每页 10 行时最后一行底部在 920.5px，超出 900px 视口；账号页同理。尝试过在表格内部加纵向滚动（`overflow_y_scrollbar` / `overflow_y_scroll`），在嵌套 `gpui_component` 滚动容器下子元素被判定不可见（`visible=false`），方案已回退；需要先确定冻结表头 + 固定列的滚动实现方案再动手。
- **验证通过**：GPUI `ui-preview` 全量 **413 项测试**（新增 `game_config_footer_uses_the_shared_table_pagination`，覆盖每页选择、页码指示与翻页行为）、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵重渲染 110 张并抽查游戏配置（三个页签 + 英文）、账号与好友展开地块。真实账号操作、真实窗口滚动交互与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 账号/游戏配置表格改为冻结表头 + 可滚动表体

- 修复上一条记录的表格裁剪缺陷（行数超过可视高度时被 `overflow_hidden` 裁掉、无法滚动；1440×900 默认每页 10 行时最后一行底部 920.5px 超出 900px 视口）。两张表改为 desktop-ui（Naive UI `flex-height` 表格）同款的「冻结表头 + 可滚动表体 + 固定列」结构：
  - 表头独立成行固定不动；表体是 `overflow_y_scroll` 原生滚动区。固定列泳道与中间列的表体共享同一个 `ScrollHandle`（`window.use_keyed_state` 按表名持有），任一泳道滚动时三条泳道按同一偏移绘制、行基线不漂移；表体的纵向滚动条由 gpui-component 的 `Scrollbar` 覆盖层托管在最右侧泳道（无右侧固定列时托管在中间列）。
  - 表头与表体的中间列共享同一个横向 `ScrollHandle`，横向滚动时列标题与数据同步；固定列不参与横向滚动（冻结列语义）。
  - 百分比高度链（`h_full`/`size_full`）补齐 `min_h_0`，避免表体按内容高度把整页撑高、把分页页脚挤出视口；替换掉上一轮失败方案中的嵌套 gpui-component `Scrollable`（该结构下行的可见性判定失效）。
  - 游戏配置每页超过 80 行的 `v_virtual_list` 分支移除：分页大小上限 100、行数有界，普通渲染后所有泳道才能共享同一滚动句柄（这也是替换自由滚动方案的先决条件）。
- 新增回归：`accounts/table.rs::account_table_scrolls_the_body_and_keeps_the_header_fixed` 与 `game_config/catalog/table.rs::game_config_table_scrolls_the_body_and_keeps_the_header_fixed`（断言：滚动前最后一行在框外、滚动后进入可视表体、表头 y 不变、固定列动作/单元格与中间行滚动前后基线一致）；列固定用例补跨泳道基线断言。
- **验证通过**：GPUI `ui-preview` 全量 **415 项测试**、`cargo test --workspace -- --test-threads=1`（工作区全部通过，退出码 0）、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵重渲染 110 张，抽查账号（常规 + 1100×720 最小窗口的横向滚动与固定操作列）和游戏配置（三个页签 + 英文）均为冻结表头、页脚可见。真实窗口的滚轮/触控板手感、拖动滚动条与跨平台实窗仍未验收。

### 2026-10-13 — GPUI 预览矩阵补齐最小窗口与表格缩放场景

- 预览矩阵从 110 张扩到 **119 张**：
  - 新增 1100×720 最小窗口场景：总览、我的农场、好友、收益分析、商城、设置、游戏配置（此前只有账号、账号面板、天气三种最小窗口样例）。逐张检查：看板日志工具栏换行、好友行五个动作按钮、农场四列地块网格、商城卡片、设置卡片换行、游戏配置表格的横向溢出（列可横向滚动到达）均正常，未发现裁剪或溢出缺陷。
  - 新增 150% 字体缩放场景：账号（浅色）、游戏配置（深色）。游戏配置在 150% 下无异常；账号表格「最近在线」列在 150% 下按固定列宽被硬截断——gpui 的 `truncate` 省略号在该单元格未生效（同一写法在侧栏标题能渲染省略号），本轮为该单元格补充悬停提示以提供完整时间回退（`account-last-online-{id}`），并把该限制记录在此。应用目前没有用户可调的字体缩放入口，150% 属设计指南要求的缩放压力测试；若将来引入缩放入口，列宽模型（固定像素）需要改为 rem 方案。
- **验证通过**：GPUI `ui-preview` 全量 **415 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；119 张预览全量重渲染并抽查上述新增场景与既有账号/游戏配置页面。

### 2026-10-13 — GPUI 空态与未运行态文案按 Vue 语言包逐条对齐

- 先做了一轮控件级系统审计：提取 desktop-ui 农场各页面用到的 NaiveUI 组件（NSpin/NEmpty/NProgress/NTimePicker/NRadioGroup/NList 等）逐项核对 GPUI 对应物，确认没有整类缺失的控件；随后逐条比对空态文案，修正 5 处与 Vue 语言包不一致的文案：
  - 我的农场（未选择账号）：`personal.selectAccount`「请选择账号 / Select account」（原「请先选择账号 / Select an account first」；页面级状态与地块区两处同步）；
  - 我的农场（账号未运行）：`personal.notRunning`「账号未运行，请先启动账号 / Account is not running. Please start it first.」（原「当前账号未运行… / Start this account to view its farm」）；
  - 我的农场地块无数据：`common.noData`「无数据 / No Data」（原「当前没有可显示的地块 / No lands to show」；带搜索词时的「没有匹配的地块」保留为 GPUI 筛选态）；
  - 好友展开地块无数据：`friends.noLands`「无土地数据 / No land data」（原「没有可显示的地块 / No lands to display」）；
  - 收益分析（未选择账号）：`selectAccount`「请选择账号 / Select account」（原「请选择账号后查看收益分析 / Select an account to view planting analytics」）；看板同状态的英文由 "Select an account" 收敛为 Vue 原文 "Select account"。
- 复核无差异：成长任务空态（`growthEmpty`）、分析黑名单空态（`analytics.blacklistEmpty`）、宠物/图鉴/互动道具空态、每日礼包两态（`giftNeedLogin`/`giftEmpty`）、神秘商店两态、好友三段空态、账号与游戏配置表空态。
- 回归：`personal_offline_state_hides_business_actions_and_rows` 增加未运行/未选择账号两种文案断言（空态 div 补 `aria_label`）；新增 `friend_lands_empty_copy_follows_vue_no_land_data`（好友地块空态 div 补 id + `aria_label`）。
- 说明：这些状态不在现有预览快照里（快照都带账号和数据），本轮无需重渲染截图；文案由单测断言锁定。
- **验证通过**：GPUI `ui-preview` 全量 **416 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`。

### 2026-10-13 — GPUI debug 构建运行时冒烟（隔离数据目录）

- 用隔离数据目录（`FARM_DATA_DIR=$(mktemp -d /tmp/qq-farm-gpui-debug-smoke*)`）运行 **debug** 构建（启用 debug 断言）17 秒：进程在 5s / 17s 两次检查均存活，收到 SIGTERM 后干净退出；stdout/stderr 全程为空——没有 gpui debug 断言、重复 element id、资源缺失、panic 或布局告警输出。
- 与 2026-10-13 的 release 冒烟（12 秒，仅验证启动驻留）相比，本次覆盖 debug 断言路径，并实际渲染了空数据目录下的「无账号/空态」页面（本轮多轮文案对齐的目标状态之一）。
- 证据范围：只证明 debug 构建可启动、驻留、干净退出且无运行时告警；**未**断言窗口可见性、键鼠交互、主题切换与缩放。窗口在桌面短暂出现，属本机自建实例与隔离数据，未触碰真实 `data/`。
- **验证**：本轮未改源码；`cargo fmt --all -- --check` 与 `git diff --check` 通过；`ui-preview` 全量 416 项测试与当前源码状态一致（本会话内最近一次运行即绿）。

### 2026-10-13 — GPUI 满页 100 行回归与反馈/外观/文案/无障碍核查

- 新增回归 `game_config_full_page_renders_scrolls_and_selects_all_rows`（游戏配置每页 100 行、101 条种子）：首行可见；末行初始位于可视表体下方；滚动后进入可视表体且位于分页页脚上方；表头「全选本页」一次选中 100 条（`selected_ids.len() == 100`）。该用例锁定上一轮「移除 >80 行虚拟列表、改用共享滚动句柄」后的最大页容量路径。
- 同轮核查结论（均为已实现，无需改动）：
  - 反馈提示生命周期：账号、好友、游戏配置、设置、商城、农场、背包、宠物、互动道具的成功/错误提示均为「4 秒自动消失（分析黑名单 3 秒）+ 代次校验防止旧定时器清掉新提示」，与 Vue 消息弹层自动消失一致；
  - 「跟随系统」外观：窗口观察系统外观变化，偏好为 system 时实时切换深浅主题（`desktop/runtime/window.rs`）；
  - 文案反向扫描（Vue→GPUI）：非活动页面使用的 414 个语言包键在 GPUI 中 **0 缺失**；41 条模板硬编码逐条核对后全部可解释（Vue 事件格式化器专用文案、微信登录流程内部状态、时区白名单在 core 常量、省略号排版归一化），未发现缺口；
  - 无障碍：脚本扫描全部 `Button` / `Checkbox` 构造，**0 处**缺少 label 或 accessibility_label。
- 说明：为定位 150% 字体缩放下账号表格「最近在线」的省略号行为差异，本轮尝试了一个无头渲染探针，探针代码在编译期触发 rustc proc-macro 栈溢出（SIGBUS），已回退；该限制继续按要求记录在对照清单（应用暂无缩放入口，属设计指南要求的压力测试项）。
- **验证通过**：GPUI `ui-preview` 全量 **417 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`。本轮无生产代码改动，预览矩阵无需重渲染。

### 2026-10-13 — GPUI 设置·种植策略页签改为顶部对齐的行布局

- 现象：设置页「种植策略」页签里，种植策略下拉/预览块与巡田间隔、好友静默时段、种植与偷菜延迟三张卡片共处一个 `flex_wrap` 行，行内默认垂直居中；三张卡片高度不一，导致较矮的卡片和左侧表单块"浮"在最高卡片的半腰，下方留出明显空洞（1440 宽下尤为明显）。
- 修改：`desktop/view/controls.rs` 的共享控件行在**设置页**改为 `items_start`（顶部对齐）。其它页面工具栏保持默认居中，避免按钮与文本行的基线被打乱（好友/农场等工具栏混排按钮与说明文本）。
- 效果（119 张预览重渲染抽查）：1440 宽下策略页签成为一条顶部对齐的行——[种植策略+预览][巡田间隔][好友静默时段][种植与偷菜延迟]；1100×720 下换行为两行且同样顶部对齐；深色主题一致；自动化/下线提醒/运行环境/界面偏好页签未受影响（自动化页签的外框此前已存在，非本轮引入）。
- **验证通过**：GPUI `ui-preview` 全量 **417 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵重渲染 119 张并抽查设置页五个页签的浅色/深色/最小窗口样例。

### 2026-10-13 — GPUI 策略页签对齐回归与自动化字段复核

- 为上一轮的设置·种植策略页签顶部对齐补回归：给种植策略标签、巡田间隔卡片、好友静默时段卡片、种植与偷菜延迟卡片补测试 id（`planting-strategy-label` / `settings-intervals-card` / `settings-quiet-hours-card` / `settings-plant-delay-card`，仅 cfg(test) 注册），新增 `strategy_sections_top_align_with_the_strategy_controls`：1440 宽下三张卡片的顶边与「种植策略」标签顶边误差 < 1px，锁定"同一阅读行"布局，防止以后回退成垂直居中。
- 复核设置·自动化页签字段：对照 Vue 模板的自动化 pane 键位清单（基础自动化 7 项、施肥与购买含有机/无机购买数量与触发阈值 4 个数值项、施肥范围/策略、快成熟判定秒数、自动好友互动 8 项、神秘商人 7 项、保存自动控制），GPUI 均有对应实现（购买数量/阈值在对应开关开启时显示，与预览快照中开关关闭时不可见一致），未发现缺项。
- **验证通过**：GPUI `ui-preview` 全量 **418 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`。本轮新增的测试 id 不改变渲染结果，预览矩阵无需重渲染。

### 2026-10-13 — GPUI 预览矩阵补钉钉提醒渠道（119→122 张）

- 设置·下线提醒的钉钉渠道（Webhook/Access Token/加签密钥）此前只有代码实现、没有预览覆盖。新增 fixture 变体 `settings_dingtalk_snapshot()` 与三个场景 `settings-dingtalk-light/dark/english`，并在预览 seed 的设置分支里同步 provider 选择与输入框取值（此前该分支的 draft 只注入 qq_bot 快照）。
- 渲染核验（浅色截图）：机器人类型显示「钉钉机器人」；Webhook 地址已填、Access Token 空（占位符「与 Webhook 地址二选一」）、加签密钥以密码掩码显示并可切换明文；底部说明、测试通知（端点已填→可用）、保存按钮均正常；「官网」按钮按 Vue 逻辑在非 QQ 机器人渠道禁用。
- 对照结论（无需改代码）：顶部提示 `offlineHint`、渠道选项文案（不推送/QQ 官方机器人/钉钉机器人/微信机器人（敬请期待）及其英文）与 Vue 语言包逐字一致；渠道下拉的禁用位（微信机器人禁用）一致；`offlineHint` 在 Vue 中同样是 QQ 机器人措辞并作用于所有渠道，GPUI 保持一致（非本轮引入）。
- **验证通过**：GPUI `ui-preview` 全量 **418 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 122 张全量重渲染并抽查新增的三个钉钉场景。

### 2026-10-13 — GPUI 预览矩阵补 QQ 机器人已绑定状态（122→125 张）

- 下线提醒的 QQ 机器人「已绑定」状态此前只有测试断言、没有预览覆盖。新增 fixture 变体 `settings_qq_bot_bound_snapshot()`（`qqBotBinding.bound=true` + 昵称「农场通知助手」+ 邀请链接）与三个场景 `settings-offline-bound-light/dark/english`；预览 seed 相应注入已绑定快照与凭据输入值。
- 渲染核验（浅色截图）：机器人类型「QQ 官方机器人」、官网按钮可用；绑定状态显示绿色「已绑定」标签 + 昵称；「绑定 QQ 通知」保持可用（与 Vue 只在凭据缺失或轮询中禁用的门控一致）、「打开机器人」与「解绑」按邀请链接/绑定态启用；「测试通知」按绑定态启用；保存按钮正常。
- **验证通过**：GPUI `ui-preview` 全量 **418 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 125 张全量重渲染并抽查新增场景。

### 2026-10-13 — GPUI 商城空态与神秘商店非活跃分支（预览 125→129）

- 新增分支场景：`mall-empty-light/dark`（普通商城空目录）与 `mystery-shop-inactive-light/dark`（无活跃神秘商人），补上这两个从未被预览覆盖的状态。
- 新场景暴露一处真实不一致：商城空态文案「暂无商品 / 没有匹配的商品」是左对齐的，而应用内其它空态（背包、宠物、图鉴、互动道具、游戏配置、黑名单、神秘商店、通用空态）都居中（Vue 的 NEmpty 也居中）——已加 `text_center()` 修复；同轮脚本扫描全部空态样式，另把好友展开地块的「无土地数据」（py_4、此前同样左对齐）一并居中，非活动模块空态现已全部居中一致（活动模块本轮不动）。
- 渲染核验：商城空态在内容区居中显示；神秘商店非活跃分支显示标题 + 刷新 + 居中「当前没有活跃的神秘商人」。
- **验证通过**：GPUI `ui-preview` 全量 **418 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 129 张全量重渲染并抽查新增场景。

### 2026-10-13 — GPUI 空数据分支预览与游戏配置重复空态修复（预览 129→134）

- 新增五个空数据分支场景：`bag-empty-light`、`daily-gifts-empty-light`、`pets-empty-light`、`friends-empty-light`、`game-config-empty-light`（分别使用空背包、空礼包/任务、无宠物快照、空好友三列表、空配置目录的 fixture 变体）。
- 新场景发现并修复一处真实缺陷：游戏配置目录为空时，页面会**同时**渲染通用空态（「这里还没有可显示的内容 + 刷新」）和表格自身的表内空态（表头 + 无数据 + 页脚「0–0 条，共 0 条 · 已选 0 条」），出现双空态。Vue 只有表内空态（NDataTable 空数据渲染），现已在通用空态条件中排除 `Page::GameConfig`（与 Dashboard/Settings/Analytics 的既有排除一致），修复后该页只保留表格外壳与表内空态。
- 渲染核验：空背包显示分类工具条 + 居中「无数据」；空礼包/任务显示两张卡片的对应空态（含成长任务进度徽标）；无宠物快照显示标题 + 守护记录/刷新 + 居中「暂无宠物信息」+ 狗粮区「暂无狗粮」（fixture 同步把顶层 `petGiftStatus.pending` 归零，避免空态页出现「待领礼包」）；空好友显示居中「无数据」+ 刷新。
- **验证通过**：GPUI `ui-preview` 全量 **418 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 134 张全量重渲染并抽查五个新增空态场景。

### 2026-10-13 — GPUI 未选账号分支（预览 134→139）与好友页账号门控修复

- 新增五个分支场景：`accounts-empty-light`、`dashboard-no-account-light`、`analytics-no-account-light`、`mall-no-account-light`、`friends-no-account-light`（预览 seed 支持 `-no-account` 场景清空所选账号）。
- **修复真实缺口**：好友页在未选择账号时仍会渲染好友列表与行内动作（一键偷取/捣乱/删除好友等可点），而 Vue 在该状态下用 `NEmpty`「请选择账号」隐藏整个页签区域。现按 Vue 语义处理：未选账号时隐藏分段标签、页面控件与行内容，居中显示「请选择账号 / Select account」+ 刷新（`view/content.rs` 的 `friends_without_account` 门控）。
- 回归：新增 `friends_page_requires_an_account_before_showing_rows`（好友卡片与分段标签不存在、空态文案为「请选择账号」）；两个既有好友用例（互动道具选择器、访客行）补上账号选择——它们原本依赖"无账号仍渲染"的旧行为，与 Vue 不符。
- 复核其它页面的未选账号状态：收益分析＝居中「请选择账号」；看板＝账号卡位置显示「请选择账号」，日志/倒计时/统计卡片保留（与 Vue 结构一致）；商城＝页签保留 + 居中「请先选择农场账号」（Vue 原文）；账号列表为空时＝表头 + 居中「无数据」+ 页脚「0–0 条，共 0 条 · 已选 0 条」。
- **验证通过**：GPUI `ui-preview` 全量 **419 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 139 张全量重渲染并抽查新增场景。

### 2026-10-13 — 看板未选账号结构核对（一次回退纠错）

- 起因：为核对「未选账号」状态，本轮先按截取的 Vue 片段判断"Vue 隐藏整个看板网格"，于是把 GPUI 看板的日志面板 + 巡查/统计列也随账号网格一起隐藏，并新增了对应测试。完整重读 `dashboard/index.vue` 后确认判断有误：`NEmpty`/`<NSpin v-else>` 只包住**账号/资产/进货卡网格**，运行日志卡片与右侧巡查/统计列在 `v-else` 之外，无账号时仍然显示（与 2026-10-09 的同步条目一致）。该改动已回退，并删除了基于错误结论新增的测试；`dashboard_keeps_logs_and_status_cards_available_without_an_account` 继续作为该结构的回归。
- 保留的成果：`dashboard-no-account-light` 预览场景（未选账号时显示「请选择账号」占位 + 日志/巡查/统计仍然可见）纳入矩阵，作为该状态的视觉基线。
- 教训（供后续轮次参考）：判断"应该隐藏/显示"之前必须完整读取 Vue 模板的分组边界（v-if/v-else 的包裹范围），不要按片段推断；本轮好友页的同类判断已完整核对（好友页 `NEmpty` + `NTabs v-else` 包住整个页签区域，GPUI 的隐藏修复成立）。
- **验证通过**：GPUI `ui-preview` 全量 **419 项测试**（回退后与上一轮一致）、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览 139 张全量重渲染并确认 `dashboard-no-account-light` 恢复为 Vue 对齐结构。

### 2026-10-13 — 未选账号边界全量核对 + 更新提示/加载状态预览（139→141）

- 承接上一轮教训，逐页完整核对「未选账号」在 Vue 模板里的 v-if/v-else 边界与 GPUI 的对应关系：看板＝仅账号/资产/进货网格被替换为 NEmpty，日志与右侧巡查/统计列保留（GPUI 一致）；好友＝`NEmpty` + `NTabs v-else` 包住整个页签区域（上一轮 GPUI 修复成立）；我的农场＝页签区域整体替换为 selectAccount/notRunning 文案（一致）；收益分析＝整页空态（一致）；商城＝页签保留、空态在面板内（一致）；神秘商店＝面板内空态（一致）；设置＝页签保留、账号相关面板内空态（一致）；游戏配置与账号＝无账号门控（一致）。未发现新的边界差异。
- 新增两个全局状态预览场景：`friends-update-light`（面板内「发现新版本 + 打开发布页下载」横幅）与 `friends-loading-light`（工具栏下方的「正在加载…」提示，列表按 Vue 的 NSpin 语义保留可见）。两者渲染正常。
- **验证通过**：GPUI `ui-preview` 全量 **419 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 141 张全量重渲染并抽查两个新增场景。

### 2026-10-13 — 商城/神秘商店/分析的未选账号与空态回归固化

- 前几轮新增的分支状态此前主要靠预览背书，本轮补测试锁定（同时给相关元素补 id + `aria_label`，兼顾无障碍）：
  - `mall_empty_and_no_account_states_follow_vue_copy`：商城空目录显示「暂无商品」；未选账号显示「请先选择农场账号」。
  - `mystery_shop_empty_state_follows_vue_copy`：无活跃商人时「当前没有活跃的神秘商人」；未选账号时「请先选择农场账号」。
  - `analytics_without_an_account_uses_vue_select_account_copy`：未选账号「请选择账号」。
- 生产代码仅新增 id/`aria_label` 与一个不改变布局的包装 div（商城未选账号分支返回类型要求）；重渲染后抽查 `mall-no-account-light`、`mall-empty-light` 与之前外观一致。
- **验证通过**：GPUI `ui-preview` 全量 **422 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 141 张全量重渲染并抽查受影响场景。

### 2026-10-13 — 更新提示与加载提示补回归（测试 422→423）

- 上一轮新增的 `friends-update` / `friends-loading` 两个预览状态尚无测试锁定：给面板内的「更新提示」行与「正在加载…」提示补 id + `aria_label` + `test_support`，新增 `page_level_states_show_update_notice_and_loading_text`——断言更新提示文案、`open-release`（打开发布页下载）按钮、加载文案；清空 `release_url` 后按钮消失、`loading=false` 后加载提示消失。
- 生产代码仅新增 id/`aria_label`（不改变视觉）；预览 141 张重渲染并确认 `friends-update-light` 外观不变（加载提示同批断言覆盖）。
- **验证通过**：GPUI `ui-preview` 全量 **423 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 141 张重渲染。

### 2026-10-13 — GPUI 近似文案扫描：设置/好友/分析对齐 Vue 语言包

- 用「近似文案」脚本（difflib 相似度）把 desktop 非活动模块的 389 条 i18n 文案与 Vue 中文语言包逐条比对，筛出与语言包高度相似但不相等的候选，再逐条人工核对（排除 Vue 模板硬编码与 GPUI 专属文案），确认并修正 7 处：
  - 设置·种植策略页（背包种子优先块）：面板标题「背包种子顺序」→ Vue `bagSeedOrder`「背包种子优先顺序 / Bag Seed Priority Order」；回退策略字段标签「背包种子顺序：回退策略」→ Vue `bagSeedFallback`「第二优先策略 / Secondary Strategy」（含无障碍名）；重置按钮「按回退策略重排」→ Vue `bagSeedOrderReset`「重置顺序 / Reset Order」。
  - 设置·种植策略页：多格种子预留从「多格种子预留：开启/关闭」文字切换按钮改为 Vue 的「标签 + Switch」形态——复用现有 `append_setting_switch`，标签用 Vue `bagSeedMultiLandReservation`「多格种子预留空地 / Reserve Lands for Multi-Land Seeds」，与自动化页其它开关的控件类型和样式一致。
  - 设置·QQ Bot 绑定凭据缺失提示「请先填写 QQ Bot AppID 和 AppSecret。」→ Vue `qqBotCredentialsMissing`「请先填写 AppID 和 AppSecret。 / Enter AppID and AppSecret first.」。
  - 好友·互动道具的地块编号输入占位符「好友地块编号，例如 1,2,3」→ Vue 复用的 `landIdsPlaceholder`「例如： 1,2,3 / e.g. 1,2,3」（与个人页原本的占位符一致）。
  - 收益分析·黑名单选择器占位符「搜索并选择种子」→ Vue `blacklistSelectPlaceholder`「搜索并选择作物（可多选） / Search and select crops」；其搜索占位符「输入种子名称或编号」按同页词汇统一为「输入作物名称或编号」。
  - 复核后保留（非差异）：Vue 模板硬编码的「搜索商品名称 / ID」「所有模块/事件/等级」「帮务农」等已一致；地块养护徽标 GPUI 与 Vue 同用 💧🌿🐛（「待浇水」等仅作无障碍文本）；黑名单缺名回退「种子ID:{id}」与 Vue 模板字面量一致。
- 回归：`planting_selectors_restore_vue_strategy_and_seed_choices` 增加中英双语断言（新开关标签、重置按钮）；预览矩阵按生产代码变化重渲染 119 张，抽查好友互动道具（新占位符）与收益分析黑名单（新占位符）。
- **验证通过**：GPUI `ui-preview` 全量 **416 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`。

### 2026-10-13 — GPUI 错误态覆盖：页面级错误横幅与分节错误（预览 141→147）

- 覆盖此前只有代码路径、没有预览与回归的错误状态（活动页按最新指示暂停，不涉及）：
  - 页面级错误横幅（`view/content.rs` 的 `page-error`）：补 `aria_label` + `test_support`；新增回归 `page_load_failure_shows_the_shared_error_banner_instead_of_the_empty_state`——好友页首次加载失败时显示共享错误横幅、通用空态被横幅取代，错误清除后恢复「无数据」空态。
  - 好友分节错误（`friend-section-error`，来自 `recordsError` / `interactionItemsError` 通道）：补 `aria_label`；既有用例 `visitor_service_failure_clears_old_rows_and_shows_a_retryable_error` 增补断言（横幅文案「加载失败: 访客服务暂不可用」、横幅取代「暂无访客记录」空态）。
  - 个人分节错误（`personal-section-error` + `personal-section-retry`）：由裸红字改为与应用内更新提示、页面级错误同一形态的横幅行（消息居左、胶囊「重试」居右，共享圆角 + `group_box` 底色），补 `aria_label`；新增回归 `personal_section_failure_shows_a_banner_and_retry_restarts_the_load`（横幅文案、错误态不显示通用空态、非加载态点击重试触发页面刷新、加载中点击重试被忽略——该按钮不向测试框架暴露禁用标志，改用行为断言）。
- 预览矩阵 141→147：新增 `friends-error-light/dark`（页面级错误横幅）、`friends-records-error-light/dark`（访客分节错误；失败时列表按生产路径 `records = []` 清空，不展示旧行）、`farm-section-error-light/dark`（个人分节错误横幅 + 重试）。渲染核验：三种状态浅/深主题下横幅、文案与重试按钮正常，好友工具栏与分节标签、个人工具栏在失败时保留（页面壳不被错误清空）。
- 说明：`recordsError`/`interactionItemsError`/`sectionErrors` 是 GPUI 桌面端自身的分节错误通道（Vue 端对应行为是 toast + 空态），本轮只统一 GPUI 内部呈现与无障碍名，不改动两端既有语义；表格中「真实账号逐页错误行为」仍需实机验收。
- **验证通过**：GPUI `ui-preview` 全量 **425 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 147 张全量重渲染并抽查三个新增错误态场景（浅/深）。

### 2026-10-13 — GPUI 弹层视觉基线：购买确认 / 守护记录 / 游戏配置编辑（预览 147→156）

- 补齐四类此前零预览覆盖的弹层与抽屉（均为非活动模块），每类浅色 + 深色，购买确认另含英文样例：
  - 商城购买确认（`commerce/confirmation.rs` 的窗口级 dialog）：`mall-confirm-light/dark/english`——商品名、划线原价、绿色现价、红色折扣、数量步进器（1–99）、“服务端会再次检查余额和限购”提示、取消/购买（primary）。对照 Vue `game-mall/index.vue` 的 NModal：标题、正负按钮、划线原价与数量范围一致（Vue 该弹窗文案为模板硬编码中文，GPUI 做了本地化，属既有差异类别）。
  - 神秘商店购买确认：`mystery-confirm-light/dark`。**顺带修复一处真实差异**：通用商品名回退「所选商品」此前也会出现在神秘商店弹窗标题下（core `MysteryNpcDto` 无 name 字段，生产必现），而 Vue 正文只有一句「确认花费 X 货币购买 Y×N？」。现只在非神秘商店路径渲染该行；摘要句补 id/`aria_label`/测试钩子，回归 `mystery_shop_remains_available_as_a_mall_tab` 增加摘要文案断言（「确认花费 30 金币 购买 神秘种子 ×8？」）。
  - 游戏配置编辑抽屉（与账号表单同款 sheet）：`game-config-editor-light/dark`——编辑态种子记录回填（编号锁定不可改、名称、等级/收获数/价格/经验数值步进、季节/货币/占地分段选择、阶段模板、生长阶段文本域、取消/确认）。对照 Vue `game-config/modules/seed-operate-drawer.vue`：标题（编辑种子）与页脚（取消 + primary 确认，带提交中 loading）一致。
  - 宠物守护记录弹窗（窗口级 dialog）：`pets-logs-light/dark`——四列表格（好友/时间/被偷数量/守护金币）+ 关闭。核查时间列：Vue 用 `toLocaleString()`（zh-CN 呈现为 `2026/9/21 22:13:20`），GPUI 固定 `%Y/%m/%d %H:%M:%S`，形态一致，不改为短横线（此前对 dayjs 格式的怀疑已澄清：`dayjs().format('YYYY-MM-DD…')` 只在神秘商店的活动起止时间上使用）。
- 渲染核验：9 张新场景抽查——浅色商城/神秘商店/编辑器/守护记录与英文购买确认、深色商城/神秘商店/编辑器/守护记录；弹窗遮罩、深浅主题对比、按钮层级与页脚位置均正常。
- **验证通过**：GPUI `ui-preview` 全量 **425 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 156 张全量重渲染。

### 2026-10-13 — GPUI Popover 确认基线（账号/游戏配置/好友删除）与好友确认联动缺陷修复（预览 156→163）

- 三类 Popover 确认纳入预览（此前只有行为测试、无视觉基线）：`account-delete-confirm-light/dark`、`game-config-delete-confirm-light/dark`、`friends-delete-confirm-light/dark` 与 `friends-delete-confirm-english`。预览 harness 在 `arrange` 阶段用点击编排打开：账号表格行「删除」（`account-delete-1`）、游戏配置行动作（`("config-delete", 0)`）、好友行「删除好友」（`("friend-delete", 0)`）。
- **预览暴露并修复一处真实缺陷**：同一好友行的三个确认气泡（捣乱 / 加入黑名单 / 删除好友）共用同一个 `Popover::new(("friend-confirmation", index))` id。gpui-component 的 Popover 打开状态按 ElementId 缓存（`window.use_keyed_state(self.id, ...)`），因此点击任一确认会**同时打开同一行的三个气泡**——表面互相叠压、内层按钮 id 重复（既有测试用 `find` 取首个匹配，未能发现）。修复：popover id 改为 `friend-confirmation-{index}-{action}`，内容根 id 改为 `friend-confirm-content-{index}-{action}`；回归 `friend_destructive_action_cancel_does_not_mutate_state` 新增断言——打开黑名单确认后，捣乱/删除气泡的内容层不可见。
- 文案对照（Vue 语言包逐字）：账号「确认删除该账号吗？/Delete this account?」、游戏配置「确认删除吗？/Are you sure you want to delete?」、好友删除「删除后将加入黑名单，不再自动互动 / The friend will be blacklisted and no longer auto-interacted」均一致；按钮为 取消 + 确认（游戏配置删除为 取消 + 删除，属既有设计选择，不改）。
- 渲染核验：163 张全量重渲染；抽查账号/游戏配置/好友删除确认（浅色）与好友英文、深色——均为单个气泡、锚点在触发按钮上方、遮罩与深浅主题正常，气泡宽度与设定值一致（账号 260、好友 280 + 内边距）。
- **验证通过**：GPUI `ui-preview` 全量 **425 项测试**（好友组 54 项，含新回归）、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 163 张全量重渲染。

### 2026-10-13 — GPUI 选中态/表单编辑态基线 + 两处真实修复（预览 163→169）

- 新增 6 张预览：`accounts-selection-light/dark`、`game-config-selection-light/dark`（通过预览点击两行复选框进入多选：整行高亮、批量删除随选择启用、页脚「已选 2 条」、游戏配置出现「清除选择」）、`account-sheet-edit-light`（编辑账号抽屉，标题「编辑账号」，字段回填）、`game-config-editor-new-light`（新增种子表单，Vue 默认值回填）。
- **修复 1：账号表格选中行缺少高亮**。游戏配置表格的选中行有 `group_box` 底色（固定列泳道与中列一致），账号表格三个泳道都没有——与 Vue（NDataTable `type: 'selection'` + `checked-row-keys` 的选中行高亮）不一致。修复：`render_account_row` 统一按记录 id 计算 `is_selected` 并加 `group_box` 底色；三个泳道共用该渲染器，底色跨泳道连续（预览已对比确认）。
- **修复 2：种子表单「生长阶段」永远为空**。`growPhases`/`effectDesc` 注册在文本域映射（`extra_textareas`），但编辑器回填循环用 `form::set_field(&extra_fields, …)` 写入——对文本域是静默空操作。后果：新增种子不带 Vue 的首个阶段模板（`growPhaseTemplates[0]`，值 `种子:2400;发芽:2400;小叶子:2400;大叶子:2400;开花:2400;成熟:0;`），编辑带 `growPhases` 的种子与带 `effectDesc` 的道具同样丢失回填。修复：按字段所属映射分派到 `set_textarea_field`。新增回归 `game_config_new_seed_prefills_the_vue_phase_template`、`game_config_seed_edit_restores_stored_grow_phases`；既有「legacy 缺失 → 留空」用例继续通过（与 Vue `row.growPhases || ''` 一致）。修复后新增表单的阶段模板 chip 因值匹配显示为选中态，属既有 choice 字段语义。
- 同类隐患复扫（上一轮 popover 联动缺陷的扩展检查）：`use_keyed_state` 全部 5 处键名唯一（账号/游戏配置表格滚动句柄 4 处 + 活动内部 1 处）；`Popover::new` 全部 id 唯一（账号/游戏配置删除、批量删除、列设置）；表格行复选框 id 均按行唯一。未发现同类碰撞。
- **验证通过**：GPUI `ui-preview` 全量 **427 项测试**（game_config 组 38 项含两条新回归）、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 169 张全量重渲染并抽查新增六个场景（含深色）。

### 2026-10-13 — GPUI 搜索面板/列设置预览 + 列设置展开方向修复（预览 169→175）

- 新增 6 张预览：`accounts-search-light/dark`（账号折叠搜索面板：关键词 + 平台/运行状态/授权状态下拉 + 重置/搜索）、`game-config-search-light/dark`（游戏配置搜索面板：关键词 + 季节 + 重置/搜索）、`accounts-columns-light`、`game-config-columns-light`（列设置下拉全貌：全选 + 各列显隐、固定循环按钮、上移/下移）。
- **修复：列设置下拉的展开方向**。两处列设置此前用 `Anchor::BottomRight`（gpui-component 语义为向上展开），弹层会盖住工具栏与触发按钮；Vue 的 `components/advanced/table-column-setting.vue` 用 `NPopover placement="bottom-end"`（向下、右对齐）。现改为 `Anchor::TopRight`（从触发器右下方向下）。渲染核验：两页下拉均从「列设置」下方展开、右对齐覆盖表格区，工具栏与触发按钮保持可见；首行「上移」/末行「下移」禁用端点正确。删除确认保持向上（与 NaiveUI Popconfirm 默认 placement=top 一致），分页「每页 N 条」菜单保持在页脚向上（NaiveUI 在底部同样向上翻转），均不调整。
- 无障碍扫描扩展：全部 `Switch` 构造（5 处调用点覆盖自动化页全部开关、静默时段开关、种植顺序随机等）均带 label，无缺失项（此前已覆盖 Button/Checkbox）。
- **验证通过**：GPUI `ui-preview` 全量 **427 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 175 张全量重渲染并抽查六个新增场景。

### 2026-10-13 — GPUI 分支覆盖：商城会员态与分页多页态（预览 175→178）

- 新增 3 张预览，覆盖两处此前从未渲染过的分支：
  - `mall-non-svip-light`：普通商城 + `membership.isSvip=false`——头部 chip 显示「非 SVIP 会员」（muted 色），与 Vue 的 `'非 SVIP 会员'` 一致；商品卡片保持各自状态。
  - `mall-svip-locked-light`：SVIP 商城页签 + 非会员——「非 SVIP 会员」chip 与四种商品状态同屏（可购买 / 需要 SVIP 禁用 / 奖励已领取 / 已拥有），逐项与 Vue `statusLabel` 映射一致；`svip_required` 的分组逻辑此前只有单测、现已具备视觉基线。
  - `accounts-pagination-light`：23 行账号（新 fixture `accounts_many_snapshot`）→ 共享分页组件的多页形态（「1–10 条，共 23 条」、上一页禁用、页码 1/2/3 且当前页高亮、第 1/3 页、下一页可用），与 NaiveUI 分页几何一致；此前所有预览都是单页形态。
- 附带核查：desktop-ui 农场各表格均无排序列（`sorter`/`sortable` 0 命中），GPUI 同样没有——「表格排序指示」不存在缺口，无需移植。
- **验证通过**：GPUI `ui-preview` 全量 **427 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 178 张全量重渲染并抽查三张新增场景。

### 2026-10-13 — 看板日志跟随句柄修复 + 背包选择/购买提交中/日志截断预览（预览 178→181）

- **修复真实缺陷：看板「运行日志」的跟随（follow-tail）句柄从未接通**。日志列表此前用 gpui-component 的 `.overflow_y_scrollbar()`，该包装器会为滚动区自建按调用点 key 的私有句柄；外层 `track_scroll(&logs_scroll)` 绑定的句柄因此永远收不到偏移——测试实测 `max_offset()==0`，导致 `scroll_to_bottom()`（刷新后回到最新一条）与 `logs_are_near_bottom()`（是否继续跟随）全部失效。修复：改为表格已验证的配方——外层 `relative + vertical_scrollbar(&handle)` 覆盖层，内层原生 `overflow_y_scroll + track_scroll(&handle)`；`arrange` 随之为预览开放滚动（新增 `view` 参数）。回归 `dashboard_caps_rendered_logs_and_reveals_earlier_ones_on_demand`：305 条日志下断言 `scroll_to_bottom` 后 offset<0、底部「显示更早的 5 条日志」可见且可点击、点击后出现「收起」且原按钮消失。
- 新增 3 张预览：
  - `dashboard-logs-light`：305 条日志（新 fixture `dashboard_many_logs_snapshot`）→ 折叠渲染最新 300 条并停在底部，底部露出「显示更早的 5 条日志」——该按钮此前从未被渲染进任何截图。
  - `bag-selection-light`：背包批量选择态——两张可售卡主色描边 + 勾选、工具条 取消/全选/批量出售 (2)（新 fixture `bag_selection_snapshot` 让第二张卡也可售）。
  - `mall-confirm-pending-light`：购买确认弹窗的提交中状态——购买按钮 loading+禁用、取消仍可用、数量控件保持可改（与 Vue NModal `:loading` 的语义一致）。
- 附带核查：desktop-ui 的日志截断为 `filteredLogs.slice(-300)`（保留最新 300 条、最旧在前、按钮在列表底部追加更早日志），与 GPUI 的 `logs[hidden..]`（按 ts 升序存储）语义一致——本轮排查确认不是缺陷，并为其补上首个集成回归。
- **验证通过**：GPUI `ui-preview` 全量 **428 项测试**（含新回归）、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 181 张全量重渲染并抽查 dashboard-logs/bag-selection/mall-confirm-pending 与常规看板页。

### 2026-10-13 — 游戏配置编辑器果实/道具模式预览 + 同类隐患复扫（预览 181→183）

- 游戏配置编辑器此前只预览过「种子」模式；本轮补齐对应 Vue `fruit-operate-drawer.vue` / `item-operate-drawer.vue` 的两种编辑态：
  - `game-config-editor-fruit-light`：果实编辑——编号锁定（20001）、植物下拉在编辑态禁用且显示选中值 `植物 #20001 (当前目录未找到)`（命中 `plant_form_options` 的「本地目录缺失」回退分支；该函数匹配与回退两个分支均有单测，预览补上回退分支的视觉确认；与 Vue `row.plantId ?? undefined` + `:disabled` 语义一致）、名称/价格/货币/稀有度/堆叠上限/收获数/资源名/描述/效果说明/图片上传。
  - `game-config-editor-item-light`：道具编辑——编号锁定（30001）、动态类型选择器显示 `7 - 化肥`、名称/价格/货币/稀有度/可使用（否/是）/等级/堆叠上限/交互类型/资源名/描述/效果描述/图片上传。
  - 为让果实预览贴近真实数据，给预览 fixture 的草莓果实补上 `plantId` 字段（真实 DTO 含该字段，Vue 编辑态直接读 `row.plantId`）。
- 同类隐患复扫（承接上一轮「Scrollable 吞掉外接句柄」缺陷）：全仓 `track_scroll` / `scroll_to_bottom` / `scroll_to_top` / `max_offset` 用法逐处核对——账号与游戏配置表格为原生滚动容器（句柄有效）、看板日志已修复；其余 `.overflow_y_scrollbar()`（分析页、侧栏、页面内容、宠物日志弹窗）均未绑定外接句柄，无同类风险。
- **验证通过**：GPUI `ui-preview` 全量 **428 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 183 张全量重渲染并抽查两张新增场景。

### 2026-10-13 — 好友移出黑名单确认 + 筛选空态预览（预览 183→186）

- 三项此前从未渲染的状态纳入预览：
  - `friends-unblacklist-confirm-light`：黑名单页「移出黑名单」确认气泡——文案「确定将「黑名单访客」移出黑名单吗？」与 Vue `unblacklistConfirm` 逐字一致；黑名单页签计数徽标、页面提示行与危险色行内按钮同屏。注：好友行动作 id 的索引取自好友在 friends 数组中的位置（本轮首次点击因索引为 2 而失败并定位到该规则），故新增 `friends_unblacklist_snapshot`（与既有 `blacklist_rows_keep_the_unblock_action` 测试同形），保证行索引为 0。
  - `accounts-filter-empty-light`：筛选无结果——表内空态「没有符合条件的账号」+ 页脚「0–0 条，共 0 条 · 已选 0 条」（此前只有未筛选的 `accounts-empty`）。
  - `game-config-filter-empty-light`：筛选无结果——「没有符合条件的配置」+ 同款页脚；表头与工具栏保持可见。
- 实现说明：两处空态文案由 `*_filtering()` 分支驱动（账号 keyword/平台/运行/授权任一非空；游戏配置 keyword/季节/稀有度/类型任一非空），预览通过注入 applied 筛选状态触发，与真实筛选后的渲染路径一致。
- **验证通过**：GPUI `ui-preview` 全量 **428 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 186 张全量重渲染并抽查三张新增场景。

### 2026-10-13 — 下拉展开态/商城搜索无结果/黑名单空态预览（预览 186→189）

- 三项新增：
  - `game-config-select-open-light`：**全应用首个下拉菜单展开截图**——游戏配置「季节」选择器打开态（搜索面板内：选中项「全部季数」带对勾、菜单项间距与悬停高亮、选择器焦点环），锁定 gpui-component Select 弹出层的视觉。打开方式与既有英文用例一致：arrange 内 `window.within("game-config-season-select").click("input")`。
  - `mall-search-empty-light`：商城搜索无结果——顶部搜索框显示「不存在」+ 清除按钮，面板显示「没有匹配的商品」。此前只有空目录的「暂无商品」分支，`goods.is_empty()` 与「有商品但查询无匹配」两个条件中的后者首次进入截图。
  - `analytics-blacklist-empty-light`：分析·种植黑名单空态——「暂无黑名单作物」居中、页签计数徽标消失、「一键全部加入黑名单」保留（新 fixture `analytics_blacklist_empty_snapshot` 将 `plantBlacklist` 置空）。
- **验证通过**：GPUI `ui-preview` 全量 **428 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 189 张全量重渲染并抽查三张新场景。

### 2026-10-13 — 交付物刷新：release 重建（x86_64 + aarch64）与打包冒烟

- 源码自今天上午打包后已有多轮 UI 变更（错误态、弹层、选中态、日志滚动句柄等），本轮用**当前源码**重建 macOS 交付物：
  - `cargo build --release -p qq-farm-gpui`（3m37s）→ `scripts/package-gpui.sh release` → `dist/QQ Farm-x86_64.app` 与 `dist/qq-farm-gpui-macos-x86_64.app.zip`（Mach-O 64-bit x86_64，可执行 49,227,032 B）。
  - `cargo build --release -p qq-farm-gpui --target aarch64-apple-darwin`（3m42s）→ `TARGET=aarch64-apple-darwin scripts/package-gpui.sh release` → `dist/QQ Farm-aarch64.app` 与 `dist/qq-farm-gpui-macos-aarch64.app.zip`（Mach-O 64-bit arm64）。
- 包内容校验：Info.plist（`com.qqfarm.gpui`、版本 0.3.0、LSMinimumSystemVersion 12.0、图标/名称/类别）齐全；Resources 含 `assets/game_config`、`activity-assets`、`tsdk.wasm`、`tsdk-wx.wasm`、`AppIcon.icns`；两个架构的 `.app` 与对应 `.zip` 内可执行文件 SHA-256 完全一致（x86_64 `70cfa82b…5cedef`；aarch64 `45658951…c71f48`）。
- 运行冒烟：用隔离数据目录（`FARM_DATA_DIR=$(mktemp -d …)`）运行**打包含的 release 可执行文件** 15 秒——进程存活、收到 SIGTERM 后以 143 退出，stdout/stderr 全程 **0 字节**（无 panic、无资源缺失、无布局告警）。
- 说明：aarch64 仍属 x86_64 主机交叉构建，未经 ARM 实机运行；Windows/Linux 包与三平台原生 runner 仍待对应环境。
- **验证**：`cargo fmt --all -- --check`、`git diff --check` 通过；本轮无源码改动（同一源码上的全量 428 项测试与严格 Clippy 已于本会话早些时候通过）。

### 2026-10-13 — 展开态续扫：道具类型/种植策略下拉 + 加入黑名单确认（预览 189→192）

- 三项新增（延续上一轮「打开态截图」的做法，arrange 内 `window.within(<select-id>).click("input")` 或点击行内触发按钮）：
  - `game-config-types-open-light`：道具页签的「类型」选择器展开态——动态选项来自页面目录（全部类型 / 特殊道具 / 货币 / 经验 / 农场工具 / 化肥 / 宠物 / 宠物食品 / 头像框 / 礼品盒），选中项带对勾；首个验证 `item_type_options`（此前只有单测）的视觉基线。
  - `settings-strategy-open-light`：设置·种植策略的选择器展开态——指定种子 / 等级优先 / 最高经验 / 化肥经验 / 最高收益（选中）/ 化肥收益 / 背包优先，与 Vue 策略枚举一致。
  - `friends-blacklist-confirm-light`：好友列表行「加入黑名单」的确认气泡——「确定将「小麦同学」加入黑名单吗？」与 Vue `blacklistConfirm` 的 `{name}` 插值一致（此前该动作的确认文案没有截图）。
- **验证通过**：GPUI `ui-preview` 全量 **428 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 192 张全量重渲染并抽查三张新场景。

### 2026-10-13 — 运行时反馈面预览：商城/好友/背包（预览 192→195）

- 反馈链路反向核对：逐个检查各模块操作后的成功/失败反馈面（Vue 对应为 toast；GPUI 为内联着色文本行）——商城 `commerce.notice`（`view/content/commerce.rs`，有 id `commerce-action-notice`）、好友 `friends.feedback`（工具栏行内着色文本）、背包 `farm.bag.notice`（面板顶部）此前都没有截图，本轮补齐：
  - `mall-notice-light`：绿色「购买成功」显示在内容区顶部（商城/SVIP/神秘商店三个视图共用该通知位）。
  - `friends-feedback-light`：工具栏行首绿色「黑名单已更新」（真实成功文案，来自 `friends/blacklist.rs`）；颜色按等级分派（Success 绿 / Info 灰 / Error 红）。
  - `bag-notice-light`：背包面板顶部绿色「出售成功」（真实文案，`bag/actions.rs`）。
- 复核结论：三类反馈均沿用应用的「小字着色文本」模式并在正确位置渲染，无布局或层级问题；其余模块（账号、游戏配置、设置、农场操作）的反馈面此前已单测覆盖，其渲染模式与本轮三处一致。
- **验证通过**：GPUI `ui-preview` 全量 **428 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 195 张全量重渲染并抽查三张新场景。

### 2026-10-13 — 好友详情失败态核对（含一次误判纠正）+ 预览 195→196

- 反向核对细节态时排查 `friends.detail_error` 的渲染点：**首次排查因 grep 结果被 `head -n 8` 截断而误判**为「只写不读、静默失败」，按 Vue 语义（失败 toast + 空态）在行内新增了一条错误行。新预览 `friends-detail-error-light` 立刻暴露双份错误：行内新行 + **既有错误带**（`friend-detail-error-{gid}` 危险色文本 +「重试」按钮，重试调用 `open_friend_detail`，位于 `view/rows/friends.rs`）。已回退新增行；最终与既有实现一致（行内可见 + 可重试，优于 Vue 的 toast + 空态且不冲突）。
- 保留的改进：给既有错误带补 `aria_label` 与 `cfg(test) test_support`，并新增回归 `friend_detail_failure_shows_a_retryable_error_band_on_its_row`（断言错误文案、「重试」按钮、错误只出现在失败行）——该错误带此前**零测试覆盖**，本轮补上；预览场景 `friends-detail-error-light` 为其首个截图（196 张）。
- 教训记录：grep 结论不得在截断输出（`head`）上得出——先确认结果总数；同一状态在 UI 上可能有多个渲染点，改动前先完整读相关文件。
- **验证通过**：GPUI `ui-preview` 全量 **429 项测试**（含新回归）、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 196 张全量重渲染并抽查修正后的场景。

### 2026-10-13 — 账号表单微信分支/校验错误 + 看板空日志预览（预览 196→201）

- 五项此前从未渲染的细节态纳入预览（均为对应界面的首个截图）：
  - `account-sheet-wx-light`：账号表单「微信授权」页签的默认「本机微信」形态——蓝色「微」头像回退 + 「正在检测本机微信…」+「切换扫码登录」。
  - `account-sheet-wx-qr-light`：微信扫码形态——二维码占位（「二维码加载中…」）+「等待微信扫码」+「刷新二维码 / 重新检测本机微信」。
  - `account-sheet-wx-error-light`：扫码失败——状态行「二维码获取失败」+ 琥珀色警告横幅「二维码任务失败：timeout」（`wx_error` 警告横幅此前无截图）。
  - `account-sheet-form-error-light`：表单校验错误——红色危险横幅「请输入登录 code 或粘贴完整登录链接」（真实校验文案，来自 `accounts/form.rs`）。
  - `dashboard-empty-logs-light`：看板无日志——日志面板居中「暂无实时事件」（此前只有单测断言）。
- 覆盖路径说明：账号表单微信分支包含「本机检测」与「扫码」两种形态（`wx_mode` 默认 local），本轮两者与失败态均已有基线；`wx_error`（警告色）与 `form_error`（危险色）两条横幅的配色区分也已在截图确认。
- **验证通过**：GPUI `ui-preview` 全量 **429 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 201 张全量重渲染并抽查五张新场景。

### 2026-10-13 — 对照清单新增「预览状态 → 场景映射」验收附录

- 把 201 张预览按模块整理成「状态 → 场景」映射写为 [`GPUI_PARITY_CHECKLIST.md`](./GPUI_PARITY_CHECKLIST.md) 附录（账号 23 / 看板 7 / 我的农场 29 / 好友 28 / 收益分析 9 / 商城与神秘商店 28 / 设置 23 / 游戏配置 27 / 活动保留基线 27），随附矩阵目录与重生成命令，供收尾时逐项勾验。
- 缺口复核结论：非活动模块的可见状态已与场景一一对应，无新增缺口；活动模块按用户指示保持暂停（矩阵保留既有基线）。附录同时明确「不在预览范围」：真实账号逐页操作、真实窗口键鼠/滚动/托盘/多窗口唤起、Windows/Linux 打包与三平台原生 runner、ARM 实机、hover/focus/拖拽等指针瞬态；账号「最近在线」150% 截断为已知限制。
- **验证**：本轮为文档变更，`cargo fmt --all -- --check`、`git diff --check` 通过（同一源码上的全量 429 项测试与严格 Clippy 已于上一轮通过）。

### 2026-10-13 — 交付物再次刷新 + 完成度审计预演

- desktop-ui 参考端核对：仓库 HEAD（`a176032`，微信登录分支、协议 1.14.2.15 等）即当前工作树基线；`desktop-ui/src/views/farm/` 最后一次变更为上一轮同步提交且已被 GPUI 移植——参考端无漂移。
- 交付物刷新（当前源码，含好友详情错误带的 aria 改动）：x86_64 release（3m36s）→ 打包；aarch64 交叉构建（3m41s）→ 打包。两架构 `.app` 与 `.zip` 内可执行文件 SHA-256 一致（x86_64 `287c2015…`，aarch64 `2f712ec8…`）；包内 release 可执行文件以隔离数据目录冒烟 15 秒（存活、SIGTERM 143 退出、stdout/stderr 0 字节）。
- 对照清单新增「完成度审计预演」：把最初目标逐条映射到本机证据与状态（crate/功能对齐/Apple 风格/交付物为 ✅ 本机满足；活动页 ⏸ 按指示暂缓；真实账号、真实窗口、三平台与 ARM 为 ⏳ 环境依赖项），并注明在补齐这些证据前不标记完成。
- **验证**：`cargo fmt --all -- --check`、`git diff --check` 通过；同一源码上的全量 429 项测试与严格 Clippy 已通过。

### 2026-10-13 — 看板日志展开态预览 + hover 态取舍说明（预览 201→202）

- 新增 `dashboard-logs-expanded-light`：305 条日志下滚动到底部点击「显示更早的 5 条日志」后的展开形态——事件 #295–#305 全量渲染、底部「收起」按钮、右侧可见新原生滚动条覆盖层（同时再次佐证上一轮的 follow-tail 句柄修复在展开后仍成立）。打开方式：arrange 内 `scroll_to_bottom → click → scroll_to_bottom`（点击路径此前已有通过的集成回归背书）。
- hover 态取舍：全仓非活动代码仅 **一处** 应用自定义 hover（好友卡片行 `group_box` 60% 叠层，`view/rows/friends.rs`），且行头 div 的测试注册是 `#[cfg(test)]` 门控——在预览二进制中不可查询，若要出图需把逐行元素注册改为无条件（release 下每行一个观察包装），为一张演示图不值得改动生产代码。**决定：hover/focus 指针瞬态保持「不在预览范围」**（已在附录记录），其实现本身仍受代码审查与设计指南约束。
- **验证通过**：GPUI `ui-preview` 全量 **429 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 202 张全量重渲染并抽查新场景。

### 2026-10-13 — 最小窗口覆盖补齐：六个未覆盖版式（预览 202→208）

- 设计指南要求验证「最小窗口」；此前 1100×720 只覆盖看板、农场地块、好友、分析、商城、设置、游戏配置、账号与天气，而**背包、任务、宠物、图鉴、互动道具、神秘商店**这些不同版式的页面从未在最小窗口下渲染。本轮补齐 6 张：`bag-minimum`、`daily-gifts-minimum`、`pets-minimum`、`illustrated-minimum`、`interaction-minimum`、`mystery-shop-minimum`。
- 逐张检查结论：**未发现布局缺陷**——背包分类工具条与双卡并排、任务/礼包 3 列卡片与成长任务列表、宠物头部工具条（待领礼包/领取/守护记录/刷新）与狗粮行、图鉴页签+进度条+卡片格、互动道具行与选择器行、神秘商人卡片均在 1100×720 下完整呈现，无换行错误或裁切。
- **验证通过**：GPUI `ui-preview` 全量 **429 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 208 张全量重渲染并抽查六张新场景。

### 2026-10-13 — 150% 缩放压力场景补齐：六个页面（预览 208→214）

- 缩放维度此前只覆盖账号（面板/表格）与天气（125%/150%）；本轮给**看板、好友、设置、商城、背包、宠物**各加一张 150%（1440×900、浅色）压力场景。
- 逐张检查结论：内容区**无布局缺陷**——看板卡片与日志工具条保持单行、好友行五个动作全部内联、设置卡片按宽度自然换行、商城卡片与会员信息条正常、背包分类工具条与卡片、宠物行与狗粮行均完整呈现。
- 新增一条已知限制（与账号「最近在线」同类）：窗口边框标题「QQ Farm」在 150% 下截断显示为「QQ Fa…」——标题来自窗口标题选项，截断发生在 gpui-component `WindowBorder` 布局内部；应用没有用户可调的缩放入口，属设计指南要求的压力测试项，不构成本机可修复缺陷。侧栏长账号名在 150% 下显示省略号（`长名称账号 · 微…`）属既定 truncate 行为，100% 下完整。
- **验证通过**：GPUI `ui-preview` 全量 **429 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`；预览矩阵 214 张全量重渲染并抽查六张新场景。

### 2026-10-13 — 收尾卫生：TODO 扫描、README 预览文档、三层源头漂移核对

- 全 crate `TODO` / `FIXME` / `todo!()` / `unimplemented!()` 扫描：**零命中**（含活动模块）；无遗留占位实现。
- `crates/qq-farm-gpui/README.md` 新增「预览与视觉回归」一节：记录 `ui-preview` 场景矩阵结构（`scenes.rs` 定义 / `fixtures.rs` 快照、当前 214 场景的覆盖范围）、隔离数据目录的渲染命令、覆盖率回归名以及新增场景的方式；「开发与验证」的三条命令同步更新为带 `--features ui-preview` 的版本（与本会话所有轮次实际使用的命令一致）。
- 源头漂移三层核对：bot 参考端 HEAD 为 `864caf3`（2026-09-28，协议 1.14.2.15），与本仓库最近一次「同步 bot」基准**完全一致**；desktop-ui 农场视图在该基准后无新变更（前一轮已核对）；GPUI 已对齐同一快照——**三层均无漂移**。
- **验证**：`cargo fmt --all -- --check`、`git diff --check` 通过；本轮为文档变更，源码未动（全量 429 项测试与严格 Clippy 对同一源码通过）。

### 2026-10-13 — 桌面壳细节核对（窗口最小尺寸/几何持久化/外观跟随）

- 逐项核对此前未单独记录的桌面壳行为（全部为既有实现，本轮首次做文档化核对）：
  - 窗口最小尺寸：`WindowOptions::window_min_size = 1100×720`，与 Tauri `tauri.conf.json` 的 `minWidth/minHeight` 一致；
  - 几何持久化：启动时取 `preferences.window_width/height`（下限 1100×720），`observe_window_bounds`（`desktop/init.rs` 挂载）在调整窗口后持久化尺寸（仅当 ≥ 最小值且发生变化，避免频繁写盘）；
  - 外观跟随：`observe_window_appearance` 在「跟随系统」偏好下实时切换深浅主题；
  - 其余桌面能力（托盘、更新检查、第二实例唤起/冲突检测）均有既有实现与回归（详见对照清单「跨页面和桌面壳」条目与更早的 SYNC 记录）。
- 对照清单「完成度审计预演」新增该行证据；三行环境依赖项（真实账号、真实窗口、三平台/ARM）保持不变。
- **验证**：`cargo fmt --all -- --check`、`git diff --check` 通过；源码未动（全量 429 项测试与严格 Clippy 对同一源码通过）。

### 2026-10-13 — 交付脚本与 CI 一致性：gpui.yml 校验、package-gpui.ps1 漂移修正

- `gpui.yml`（三平台 CI 工作流）首次做语法校验：`YAML.load_file` 解析通过，包含 `rust` 与 `frontend` 两个 job——此前从未在本地校验过该文件。
- `scripts/package-gpui.ps1`（Windows 打包脚本，从未执行）与 `scripts/package-gpui.sh` 逐行对照，发现并修正两处漂移：
  1. zip 名硬编码 `qq-farm-gpui-windows-x86_64.zip` → 改为按 `TARGET` 推导架构（`aarch64*` → aarch64），arm64 Windows 构建不再产出错误命名；
  2. `Compress-Archive` 直接压缩暂存目录内容 → 改为打包 `qq-farm-gpui/` 顶层目录，与 Linux tarball 的目录结构一致（解压不散落文件）。
  资源清单（`assets/game_config`、`activity-assets`、双 tsdk wasm、README）与 `.sh` 一致。
- 文档一致性核对：`docs/ARCHITECTURE.md` 与根 `README.md` 已完整覆盖 GPUI crate（架构图、模块树、打包脚本、预览命令），无需改动。
- 说明：脚本与工作流的**实际执行**仍依赖 Windows runner / CI 环境（本机无 pwsh 与 MSVC 工具链）；本轮只完成语法校验与逐行对照修正，不改变「本机之外验收」的挂账状态。
- **验证**：`cargo fmt --all -- --check`、`git diff --check` 通过；源码未动（全量 429 项测试与严格 Clippy 对同一源码通过）。

### 2026-10-13 — 运行证据补充：发布包 75 秒空闲观测 + 纯 debug 构建冒烟

- 发布包空闲稳定性（`dist/QQ Farm-x86_64.app`，隔离数据目录）：运行 75 秒，15s/75s 两次采样均存活；RSS 97,016 KB → 97,148 KB（60 秒 +132 KB，分配器噪声级，无增长趋势）；CPU 两次采样均 0.0%（无忙循环）；SIGTERM 后退出码 143，stdout/stderr 全程 0 字节。
- 纯 debug 构建冒烟：`cargo build -p qq-farm-gpui`（不带 `ui-preview` feature，debug 断言生效，21s 构建完成）→ 隔离数据目录运行 20 秒——存活、SIGTERM 143 干净退出、stdout/stderr 0 字节（无 gpui debug 断言失败、重复元素 id、资源缺失或布局告警输出）。
- 说明：以上覆盖「启动 → 空闲 → 关闭」在 release/debug 两种构建下的无告警与无空闲泄漏迹象；不覆盖真实键鼠交互、小时级长稳与跨平台窗口行为。
- **验证**：`cargo fmt --all -- --check`、`git diff --check` 通过；源码未动（全量 429 项测试与严格 Clippy 对同一源码通过）。

### 2026-10-13 — 阻塞边界复核：容器路线排除与 CI 工作流就绪度确认

- 本机替代路线补查：`docker` / `podman` / `colima` 均未安装——「Linux 交叉检查缺 GTK/GIO sysroot 与目标 GCC」的既有结论维持，**容器化 Linux 编译路线在本机不可行**。
- 通读 `.github/workflows/gpui.yml`：rust 矩阵覆盖 macos-15（Apple Silicon）/ macos-15-intel / windows-latest / ubuntu-latest，含 Linux 依赖安装（GTK、wayland、xkbcommon 等 17 项）、protoc、fmt/clippy/测试/release 构建/按平台打包/产物上传（`if-no-files-found: error`），另有 Vue 前端 job；触发条件为限定路径的 push 到 `main`、PR 与手动 `workflow_dispatch`。
- **阻塞判定依据**：三项未验收条目（真实账号逐页操作、真实窗口键鼠/托盘交互、Windows/Linux 打包与 macOS ARM 实机）在本机的替代路线已穷尽——无容器运行时、无跨平台/Windows 工具链、无原生 UI 自动化入口、无 ARM 设备。推进路径只剩两条：用户授权将工作分支推送远端以触发 `gpui.yml`（runner 可直接补齐四平台编译/测试与 macOS ARM 原生运行），或用户在实机试用后反馈问题。此前十余轮已持续注明该外部依赖（对照清单三行 ⏳ 与审计预演「结论」），现进入等待用户输入状态。
- **验证**：本轮仅调查与记录，无源码改动；`cargo fmt --all -- --check`、`git diff --check` 通过。

### 2026-10-14 — 修复：工具栏命令按钮点击触发实体重复租约 panic（账号页「刷新」闪退）

- **事故**：用户报告账号页点击工具栏「刷新」后应用闪退。崩溃报告 `~/Library/Logs/DiagnosticReports/qq-farm-gpui-2026-10-14-154409.ips`：SIGABRT、`abort() called`、故障线程为主线程（AppKit `_routeMouseUpEvent` 栈内）；崩溃二进制与 `target/release/qq-farm-gpui`（UUID `50A6CFD3-…`）一致。
- **根因**：工具栏四个命令按钮（刷新 / 侧边栏 / 检查更新 / 保存设置）的 `on_click` 是 `cx.listener`（派发期间持有实体租约），其中又调用 `focus_handle.dispatch_action(...)`，把动作**同步**派发回根节点上同样由 `cx.listener` 注册的 `on_action` 处理器 → 对同一实体嵌套 `update` → `gpui-pre` `entity_map.rs:164`：`cannot update FarmDesktop while it is already being updated`。release 包为 `panic=unwind`，panic 从 AppKit 回调边界溢出 … 实际表现即为闪退（abort）。键盘 ⌘R / 应用菜单项走单次实体租约，不受影响。
- **复现与验证路径**：先在测试窗口内用真实鼠标事件（gpui-kit `window.click`）+ 真实数据目录副本（`FARM_DATA_DIR` 指向 `~/Library/Application Support/QQFarmRust` 的拷贝，`test_mode=false`）复现出同一条 panic；临时把旧写法还原后，新回归测试同样复现该 panic，修复后通过。
- **修复**：`desktop/view/content/toolbar.rs` 四个按钮改为在监听器内直接调用 `this.dispatch_ui_command(&Action, window, cx)`（同一命令入口，不再经过实体派发）；键盘与菜单路径保持 `on_action` 不变。`runtime/updates.rs` 的 `check_update` 增加 `test_mode` 短路（仅测试环境：不发真实网络请求，与 `refresh` 等既有 test_mode 分支一致）。
- **回归**：新增 `desktop/tests/toolbar.rs::toolbar_buttons_run_their_commands_without_reentering_the_view`——在账号页模拟真实点击「刷新 / 侧边栏 / 检查更新」并断言命令生效，再在设置页点击「保存设置」并断言「策略设置已保存」提示；还原旧写法时该测试以原 panic 失败（已实测）。
- **验证通过**：GPUI `ui-preview` 全量 **430 项测试**（429 + 新回归）、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`。
- **交付物**：`cargo build --release -p qq-farm-gpui`（4m17s，增量重链）→ `scripts/package-gpui.sh release`；`dist/QQ Farm-x86_64.app` 与 `dist/qq-farm-gpui-macos-x86_64.app.zip` 内可执行文件 SHA-256 一致（`44121e5684cf…8aa5f78`，UUID `552308FF-…`，与崩溃版本 `50A6CFD3-…` 为不同构建）；隔离数据目录冒烟 12 秒存活、SIGTERM 退出码 143、stdout/stderr 0 字节。真实账号点击「刷新」复测由用户实机完成。

### 2026-10-14 — 侧边栏账号改版（底部下拉）+ 启动自动重连（对齐 Tauri 壳）

- 用户反馈：账号列表在侧边栏顶部、底部又有「＋ 添加账号」，布局割裂；Tauri 版是左下角下拉选择、底部没有添加按钮；且账号不会自动上线。
- 侧边栏重构（`desktop/view/sidebar.rs`、`desktop/accounts/sidebar.rs`）：移除顶部账号单选列表、顶部「− / +」按钮与底部「＋ 添加账号」；底部改为账号下拉（Button + `dropdown_menu`/`PopupMenuItem`，触发器显示当前账号名与箭头，菜单项对当前账号打勾；无账号时显示禁用的「尚未添加账号」；侧边栏收起时隐藏，与 Tauri `v-if="!siderCollapse"` 一致）。新增账号仍走账号页工具栏「新增账号」（与 Tauri 同位置）。
- 启动自动重连：GPUI 入口此前未调用 `engine.schedule_wx_authorized_start()`（Tauri 在 setup 中调用同一方法）。`main.rs` 组装应用上下文后补上：已授权微信账号在 60 秒延迟后自动重连，多账号间 15–45 秒随机错峰，并按 `session_liveness` 的旧会话释放窗口额外等待——与 Tauri 完全同源。
- 测试：改写 `desktop/tests/layout.rs` 侧边栏回归——下拉切换账号（点击触发器 + 菜单项，断言 `selected_account`、请求 revision、无障碍名跟随新账号）、侧边栏收起时下拉隐藏且导航仍可见；侧边栏内「添加账号」入口移除后不再断言（新增账号由账号页工具栏路径覆盖）。
- 预览：矩阵 214 张全量重渲染；抽查 `accounts-light/dark`、`accounts-150`、`accounts-empty-light`、`dashboard-no-account-light`——底部下拉、空账号占位「请选择账号」与 150% 缩放下版式正常。
- **验证通过**：GPUI `ui-preview` 全量 **430 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`。

### 2026-10-14 — 列表铺满、收益分析等级同步、远端头像加载（实机反馈第三批）

- 用户反馈：表格类页面「割裂、不铺满页面」；收益分析取不到登录账号等级（按 1 级作物分析）；账号头像不显示。
- 表格铺满（`accounts/table*`、`game_config/table*`）：中部列原为固定宽度（列宽总和小于可用区域时在右侧固定操作列前留下死区、表头色带中断）。现在 `account_cell_frame` / `game_config_frame` 把中部列改为 `flex_basis=设计宽度、flex_grow=设计宽度（按设计比例分配剩余空间）、min_w=设计宽度`，容器改为 `w_full().min_w(px(基准宽度))`；固定列（pinned lane）保持设计宽度，窄窗口仍横向滚动且操作列常驻（既有最小窗口回归仍然通过）。
- 收益分析等级（`analytics/state.rs`、`runtime/loading.rs`）：对齐 Vue `syncStrategyLevel`——每次分析页快照到达后用 `page_raw.accountLevel` 重设「参考等级」输入（≤0 保留现值）并清除覆盖标记；等级观察器改为以 `level_last_value` 判定「文本真实变化才算用户编辑」，程序性同步与无关通知不再误判为用户覆盖（此前输入框停在启动前的 1，且该输入收到任何通知都可能被当成用户覆盖，于是按 1 级分析）。
- 远端头像（新增 `http.rs` + `application/mod.rs` 接线）：GPUI 默认 `NullHttpClient` 会让 `img(https://…)`（微信头像、好友头像）全部失败回退字母占位。新增 `ReqwestHttpClient` 实现 `http_client::HttpClient`（请求在应用自己的 Tokio runtime 上执行；区分 NoFollow/跟随重定向客户端；透传请求超时），通过 `Application::with_http_client` 注册。
- 测试：新增 `http::tests::remote_requests_return_the_fetched_bytes_and_status`（本地回环服务器验证状态码与正文）与 `analytics_page_load_syncs_the_reference_level_to_the_account`（快照到达同步等级、同一值的通知不误判、手动输入仍可覆盖）；预览夹具注入分析快照后调用同一同步逻辑，使预览与真实加载路径一致。
- 预览：214 张全量重渲染；抽查 `accounts-light/dark/minimum/150`、`game-config-light`、`analytics-light`——表格铺满、无死区；分析页参考等级显示账号等级（fixture 26）。
- **验证通过**：GPUI `ui-preview` 全量 **432 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets`、`cargo fmt --all -- --check`、`git diff --check`。
- **交付物与实机证据**：release 重建 + `scripts/package-gpui.sh release`（`dist/QQ Farm-x86_64.app`、UUID `A31C9681-…`；`.app` 与 `.zip` 内可执行文件 SHA-256 一致 `3223f595…5f2de6`）。关闭旧实例（SIGTERM 优雅退出）后启动新包，约 5 分钟内 `session-liveness.json` 三个已授权微信账号（6 → 7 → 5）依次刷新 `last_seen`——启动自动重连（60 秒延迟 + 旧会话释放窗口 + 15–45 秒错峰）在实机生效。

### 2026-10-15 — 同步 bot 864caf3..8303e7d：QQ TSDK 新构建 + 土地详情 + 等级预测 + 图鉴种子购买 + 多格预留 #89 修复（Tauri + GPUI 双端）

**基准**：bot `864caf3` → `8303e7d`（9b19af9 / e5f3552 / 62e1880 / 0f09702 / 1a5c23e）。客户端版本不变（仍 `1.14.2.15_20260922`，9b19af9 明确无需升版）。

**业务明细**：

- **QQ TSDK 构建对齐（bot 9b19af9）**：`assets/tsdk.wasm` 整份覆盖为 `v3.9.0.1790414929`（161111 字节，SHA-256 `f80c5f34…94704ff4`），`crypto/tsdk.rs` 的 `TSDK_BUILD_QQ` 版本/哈希/体积防呆断言同步更新（微信构建不变）。官方会话握手/登录/心跳仍为 `1.14.2.15_20260922`。
- **土地加成与升级/解锁详情（bot e5f3552）**：镜像 `assets/game_config/LandLevel.json` + `LandLevelNeed.json`（include_str! + OnceLock）；新增 `services/farm/land_upgrade.rs`（`buildLandUpgradeDetail` 1:1：unlock/max/upgrade 三态、服务器条件优先 + 官方配置回退、needLevel/prerequisite/costs、未知条件类型 fail-open 为 unknown）；`build_lands_panel_dto` 四个 Own 分支注入 `landInfo`（好友模式不带）；土地等级图 `land-assets/*.png` 镜像进 `desktop-ui/public/land-assets/`，GPUI 资产解析器新增 `/land-assets/` 分支。prost 表达不了 protobufjs 标量 presence：`LandInfo.buff` 消息存在即整体采用其字段、否则回落官方配置（代码注释已标注依据）。
- **等级升级时间预测（bot 62e1880）**：新增 `services/level_forecast.rs`（`build_harvest_events` 每足迹一次/仅当前季服务器成熟时间/缺 buff 不猜零加成；`calculateLevelForecast` 的 ready/planted/mixed/history 四源语义与 ceil 取整）；快照挂在 `Gateway`（每账号连接一份，随连接销毁即断线失效）：`AllLands` 回包按 revision 门登记（farm `Api::get_all_lands`，bot farm/api.ts 同位），任何 PlantService 写回包（除 AllLands）、ItemService.Use、本账号 LandsChanged 推送、会话结束均失效；`worker_loop/status.rs` 把 `levelForecast` 注入状态广播；`infra/stats.rs` 增加 `sessionElapsedSeconds`（init 时重置）；app `PanelStatus` 透传 `levelForecast`/`sessionElapsedSeconds`。原客户端本地历史估算已删除，改用服务端预测。
- **图鉴种子购买（bot 0f09702）**：`farm/planting.rs` 新增 `fetch_available_seeds`（商店种子列表含不可用状态；失败/为空回落本地目录 `unknownMeta`）与 `purchase_seed_via`（goodsId/count 校验、解锁卡拒绝、等级/限购/金币门控与错误文案逐条照抄；回包 cost_items 扣金币快照）；`illustrated.rs` 作物条目 seedId 归一（getPlantBySeedId/FruitId）并挂 `shopSeed`；app 门面 `friend::seed_purchase` + Tauri IPC `seeds_purchase`（generate_handler 与 ACL 两处注册，防回归测试兜底）。
- **多格种子预留 #89 修复（bot 1a5c23e）**：`layout_reservation.rs` 重写选择规则——已预留格数多者优先 → 沿用已登记布局 → 锚点最小；新增 `SelectFutureLayoutOptions { registered, exclude_land_ids }`；`PlantingEngine` 持有按 seedId 的登记表（bot 模块级 Map 的每账号等价物；rust 多账号同进程，挂引擎实例隔离）；`plant_from_bag_seeds_ex` 改为每种子每轮最多预留一组、各组整组目标布局互斥、种子可种或不再需要预留时清登记；删除 `land_analysis.rs` 里过时的模块内副本（死代码防漂移）。
- **双端 UI**：Tauri `desktop-ui`：图鉴面板购买按钮/批次弹窗/可用性提示（i18n zh+en + `app.d.ts` Schema 同步）、FarmPanel 土地卡「土地详情」+ `LandDetailModal.vue`（土壤对比/加成表/条件/成本余额对比，打开时拉状态+背包）、仪表盘 `LevelForecast.vue` 替换本地估算、`farm.ts`/`farm.d.ts` 补类型。GPUI：仪表盘账号卡渲染 `levelForecast` 行（`level_forecast_lines`，`estimate_level_up` 本地估算移除；expRate 分母优先 sessionElapsedSeconds）、图鉴卡片「购买」按钮 + 批次弹窗（复用 commerce 弹窗模式，成功关弹窗并刷新）+ 面板内结果提示、土地卡「详情」按钮 + 土地详情弹窗（打开时拉状态/背包做等级与余额对比）。

**测试**：`layout_reservation` 10 项（含 #89 回归、登记沿用、互斥排除）、`land_upgrade` 4 项（对齐 bot land-upgrade.test.js 全部断言）、`level_forecast` 7 项（对齐 bot level-forecast.test.js：issue 86、共享足迹、排除项、phase 6 成熟、ready/手动/静默、盛开末段、历史外推）、GPUI `level_forecast_lines` 文案对齐、`tsdk_builds_match_bot_literals` 更新至新构建。

**验证**：`RUSTFLAGS="-D warnings" cargo check --workspace --all-targets` 0 错 0 警；`cargo test --workspace` 全绿 1517 项（core lib 1059、GPUI 429）；`cargo fmt --all --check` 与 `git diff --check` 通过；desktop-ui `corepack pnpm typecheck` + `pnpm build` 通过。

**待验（实机）**：QQ 账号用新 TSDK wasm 长时间挂机；图鉴种子真实下单一次（门控文案与金币扣减）；土地详情弹窗在有解锁/升级条件的地块上核对服务器下发的条件数据；升级预测在自动收获开启时与实际收获时间对照；多格种子预留（开关打开 + 多格背包种子）在 #89 场景（后空出的地被纳入预留）下实盘观察。

### 2026-10-15 — 热修：naive-ui 2.45 下自闭合 NTabPane 的 tab 标签渲染为空（图鉴/游戏配置）

- **现象**：实机查看图鉴面板时「作物图鉴 / 变异图鉴」切换 tab 名称全部丢失（渲染为空）；排查发现游戏配置页「种子 / 果实 / 道具」tab 同样为空。商城页同款 segment tab 正常——商城在该依赖升级时已修过并留有注释，图鉴与游戏配置两处漏改。
- **根因**：887f0db 前端依赖批量升级带入 naive-ui 2.45.3：自闭合（无子节点）的 `<NTabPane :tab="…" />` 的 label 会被 normalizeSlots 包成空插槽，渲染为空；`tab` prop 只有配合默认插槽内容才生效，必须改走 `#tab` 具名插槽。jsdom 实测四种写法：自闭合 NTabPane ✗（line/card/segment 全类型）、#tab 插槽 ✓、NTab 自闭合 ✓、NTabPane 带内容 ✓——仅图鉴与游戏配置两处命中。
- **修复**：`personal/IllustratedPanel.vue` 与 `game-config/index.vue` 的 tab 全部改为 `#tab` 具名插槽写法（与商城/秋日活动一致）；全库扫描确认无其余自闭合 NTabPane 残留。
- **验证**：`corepack pnpm typecheck` + `pnpm build` 通过；dev 实例重启后 tab 文案可见。GPUI 端不受影响（自有控件）。

### 2026-10-15 — 热修：土地详情弹窗成本图标丢失 + 样式重排

- **图标丢失根因**：`LandDetailModal.vue` 成本项图片直接用了 `/game-config/...` 相对路径当 `src`，违反图片路径黄金规则 2（必须经 `resolveCatalogImage()` 转 `farmcfg://` 协议地址），webview 404 显示破图。土壤图（`/land-assets/`，dist 静态文件）按规则本就原样使用，显示正常。已改为 `resolveCatalogImage(cost.image)`。
- **样式重排**：表头第一格误用「产量加成」改为新增 locale 键 `landDetail.bonusHeader`（土地加成 / Land bonuses，zh+en+Schema 三处同步）；加成对比改为圆角分组行 + 表格数字右对齐（tabular-nums），提升项绿色 ↑；条件与成本行改为圆角卡片行，成本图标带容器底框，余额状态（余额充足/还差 X/待同步）右对齐着色；标题行 #id 移到 header-extra。
- **验证**：`corepack pnpm typecheck` + `pnpm build` 通过，dev 实例重启加载新 dist。

### 2026-10-15 — 性能：修复 levelForecast/sessionElapsedSeconds 破坏状态广播内容门控

- **排查**：用户反馈机器卡顿。实测 qq-farm-desktop 瞬时 CPU 仅 0~5%（47MB），机器负载大头是 ZCode 渲染进程（约 100%）与 WindowServer（50%+）、Chrome——农场应用不是主因。
- **发现的真实回归**：本次同步新增的 `sessionElapsedSeconds`（1s 精度）与 `levelForecast.generatedAt`（1s 精度）让 status 广播的内容哈希每轮必变，3s 一轮的 status_sync 从「内容变化才推送」（原有 5s 量化门控）退化为每 3s 每账号全量推送，白白增加序列化与前端重渲染。
- **修复**：两处均量化到 5s 桶（对齐 status.rs 里 uptime/nextChecks 的既有策略）：`infra/stats.rs` 的 sessionElapsedSeconds 取整到 5s；status.rs 在插入 levelForecast 前把 generatedAt 量化为 `(now_ms/5000)*5000`（UI 仅用它换算「还有几分钟」，±5s 不影响分钟粒度显示）。core 的 `calculate_level_forecast` 保持与 bot 1:1 不动。
- **验证**：`RUSTFLAGS="-D warnings" cargo check -p qq-farm-core --all-targets` 0 错，core lib 1059 测试全绿，`cargo fmt --check` 通过。

### 2026-10-15 — GPUI 实机反馈三连修 + 图鉴滚动修复 + 仪表盘对齐第一批

- **图鉴页滚动不了（用户实机报告）**：图鉴网格 `v_virtual_list` 使用 Infer 尺寸会长到内容高度，容器内部无溢出 → 滚轮无效且整页也未挂滚动容器（图鉴分支独享「无页面滚动条」路径）。修复：列表改 `Auto` 尺寸（元素高度取视口、内部滚动）+ `IllustratedState.grid_scroll_handle` 持久滚动句柄（滚动位置不随重渲染丢失）；页面容器统一挂 `overflow_y_scrollbar`。新增回归测试 `illustrated_grid_scrolls_to_reach_later_rows`（200 项注入 + `scroll_to_bottom` + 虚拟化断言；对齐 gpui-base 自身测试模式——合成滚轮事件在该测试栈不可靠，handle 驱动才是组件的规范路径）。
- **仪表盘对齐第一批（对照 Tauri 截图）**：农场资产卡从流式改 2×2 网格 + 彩色数值（金币橙/点券绿/钻石蓝/金豆豆橙，新增 `metric_colored`）；今日统计改 2 列网格并按 Tauri 固定顺序（收获/任务/偷菜/出售/一键务农/施肥/帮务农/种植）排列。
- **UI 全量对齐启动**：渲染 214 张预览图逐模块对照 Tauri；差异清单与工作约定落盘 `docs/UI-ALIGNMENT.md`（设置页栅格化表单为最大差异，后续按模块推进）。
- **验证**：GPUI `--features ui-preview` 全量 433 项测试通过（含新增滚动回归）；214 张预览重渲染核对仪表盘布局。

### 2026-10-15 — GPUI 实机反馈第四批：账号表格居中 + 神秘商店菜单移除 + 工具栏图标

- **账号页表格**：表头与全部字段文字居中（`account_cell_frame` 加 `text_center`）；运行状态/授权状态标签（胶囊）内文字居中（flex 三件套）；最近在线时间居中；操作按钮组居中。
- **神秘商店菜单**：侧边栏移除独立「神秘商店」入口（`Page::MysteryShop` 从导航过滤），与 Tauri 一致——入口在商城「神秘商人」tab；页面本身保留（持久化偏好直达仍可渲染）。两个旧断言「侧边栏独立入口」的测试按新行为更新（`mystery_shop_remains_available_as_a_mall_tab`、`mystery_shop_page_still_renders_without_sidebar_entry`）。
- **内容工具栏**：侧边栏/刷新/检查更新三个按钮补图标（PanelLeft/RefreshCw/Download，来自 gpui-kit-assets 完整 Lucide 目录 `gpui_kit::assets::IconName`），消除裸文字链接感。
- **验证**：GPUI 全量 433 项测试通过；214 张预览重渲染，账号页居中/工具栏图标/侧边栏项核对无误。

### 2026-10-16 — GPUI 设置页对齐第一批：策略设置 tab 栅格化（对照 Tauri 截图）

- **顶部表单**：种植策略 / 指定种子(或策略预览) 从「label 上 + 控件下」竖排单列改为「label 左 140px 右对齐 + 控件 flex_1」的两列并排网格（对齐 Tauri `md:grid-cols-2` + `label-placement=left`）；label 补 `aria_label`（observation 的 label() 依赖它，strategy-preview 断言需要）。
- **分组卡片**：策略 tab 容器从横排流式改为竖排整行（v_flex），巡田间隔/静默时段/种植延迟三个分组卡片各占整行；巡田间隔六项改两列网格并约束 max-w 640（同 Tauri `sm:grid-cols-2 max-w-640`）；静默时段时间行与延迟行同样约束 640。
- **测试**：`strategy_sections_top_align_with_the_strategy_controls`（旧横排顶对齐断言）改写为竖排堆叠 + 整行宽度断言；`strategy_preview_shows_vue_level_choice_and_empty_state` 随 aria_label 修复恢复。settings 全量 50 项通过。

### 2026-10-16 — 设置页对齐第二批：整数输入框 Tauri 化（用户实机反馈）

- **问题**：策略 tab 的数字设置是「纯文本 + 编辑按钮」，用户实机指出与 Tauri 的 NInputNumber 框体输入差距大（看着像普通文本）。
- **修复**：`SettingsState` 新增 `number_inputs`（路径 → InputState 实体表）与 `NUMBER_PATHS` 全路径清单（巡田间隔×6、延迟×2、化肥购买×4、好友互动阈值×3，含默认值与 min/max）；构造时逐个创建 InputState（set_value/set_min/set_max）并订阅 `InputEvent::Change` 回写 draft；`sync_setting_selectors` 反向同步（draft → 输入框，账号切换/载入后显示正确值）。`append_integer_setting(_disabled)` 改用 `NumberInput`（框体 + 内嵌 − + 步进、禁用态透传），「编辑」按钮移除；网格容器包 `settings-number-<path>` id 供测试。
- **策略 tab 补缺**：bag_priority 下补「第二优先策略」下拉（bag_fallback 实体，sync/订阅已有）与「多格种子预留空地」开关行。
- **测试**：`disabled_auto_accept_keeps_its_threshold_fields_visible_but_inactive` 更新为新交互（框体可见、禁用态点击不改值）；全量 433 项通过；预览重渲染核对（深浅主题框体样式与 Tauri 一致）。

### 2026-10-16 — 设置页对齐第三批：自动化 tab 网格化 + tab 命名对齐

- **自动化 tab**：基础自动化/化肥与购买/神秘商人三组开关容器从流式换行改为 3 列网格（对齐 Tauri `auto-switch-grid` ≥1024px 断言）；自动好友互动保持「开关 + 阈值」行式混排（与 Tauri 语义一致，不强行网格化）。
- **tab 命名对齐 Tauri**：中文「种植策略→策略设置」「自动化→自动控制」（`pages/model.rs` 的 Settings 分支单列，不影响 Analytics 同名子区）；英文 Planting strategy→Strategy settings、Automation→Auto control。section key 不变，持久化偏好不受影响。navigation 测试断言同步更新。
- **验证**：全量 433 项测试通过；预览重渲染核对自动化 tab（3 列开关网格 + 新 tab 名）。

### 2026-10-16 — UI 对齐收尾：剩余模块全量预览核对

- 逐模块核对 214 张预览（我的农场/背包/任务/宠物/图鉴/互动道具/好友/活动目录/商城含神秘商人 tab/账号/游戏配置/收益分析/仪表盘）：结构、栅格、徽标、操作入口与 Tauri 版一致，未发现新的「不伦不类」级偏差。此前三轮（表格居中+菜单+工具栏、仪表盘网格、设置页栅格化+输入框化、图鉴滚动）已覆盖用户实机点名的全部问题。
- `docs/UI-ALIGNMENT.md` 更新：已完成项打勾，剩余仅实机细验项（levelForecast 真实数据展示、极端窗口微调），发现一处修一处。

### 2026-10-16 — 反馈通道收口：操作结果类页面内提示全面移除（用户实机反馈）

- **问题**：农场操作与图鉴购买后，toast 与页面内横幅同时出现（用户实机截图框出「操作已提交」双通道重复）。
- **修复**：删除四处「操作结果类」页面内提示渲染——农场 operation_notice（content.rs 的 farm-operation-feedback 横幅）、图鉴 illustrated-notice、商城 commerce-action-notice、账号 success-feedback 横幅；对应 show_* 函数只发全局 toast。**设置 save-notice 保留**（表单保存反馈贴近表单、6 处测试断言该元素）。
- **附带**：删除工具栏页面副标题（`summary_for`「土地、仓库与每日任务」等，Tauri 无此元素），`summary_for` 方法与 Page summary 字段保留（数据层仍在用）；我的任务礼包卡片改均匀网格（列数随内容区宽度自适应，对齐 Tauri auto-fill minmax(200px,1fr)）。
- **验证**：全量 433 项测试通过；GPUI 重启后确认窗口与购买 toast 单通道显示。

### 2026-10-16 — 反馈通道收口补遗：账号页横幅渲染移除 + 死代码清理

- 账号页 success-feedback 横幅渲染移除（toast 已覆盖；字段保留供保存流程读写），对应测试改写为断言「页面不再渲染横幅、字段值保留」。
- 删除失去消费者的 `Page::summary_for` 方法与 `page_summaries_are_localized` 测试（工具栏副标题移除后的死代码清理）。
- 全量 432 项测试通过。
