# GPUI 与 desktop-ui 功能对照清单

本清单以 `desktop-ui/src/views/farm/` 的可见行为、表单字段、门控和操作结果为对照基线。GPUI 的页面实现和自动化按业务目录维护；代码检查或模拟窗口测试只能证明对应范围，不等于真实账号端到端验证。每次发现差异，先修代码和回归，再在 [`SYNC.md`](./SYNC.md) 记录证据。

## 页面与业务行为

| 页面 | Vue 对照 | GPUI 实现 | 回归测试位置 | 逐项对照范围 |
|---|---|---|---|---|
| 总览 | `views/farm/dashboard/index.vue`、`dashboard/log-events.ts` | `desktop/dashboard/`、`pages/mapper/dashboard.rs` | `desktop/tests/dashboard.rs`、`desktop/dashboard/logs/filter.rs` | 账号状态和资产、检查计划、实时日志、模块/事件/级别/关键词筛选、清空、日志上限和展开收起、操作统计及离线态。关键词范围是消息、模块标签和事件文案；不含时间。 |
| 我的农场 | `views/farm/personal/{index,FarmPanel,BagPanel,DailyOverview,TaskPanel,PetPanel,IllustratedPanel,InteractionItemsPanel}.vue` | `desktop/personal/` | `desktop/tests/personal/` | 分段导航；地块布局/成熟倒计时/合并地块/务农门控；仓库分类、搜索、批量选择/出售、道具使用；礼包与成长任务；宠物授权、部署、撤回、喂食、奖励和日志；图鉴与互动道具。 |
| 好友 | `views/farm/friends/index.vue` | `desktop/friends/`、`pages/mapper/friends.rs` | `desktop/tests/friends/` | 搜索、排序、分页和黑名单；好友详情、地块和生涯；偷取/帮忙/捣乱、批量偷取、删除；访客记录和好友互动道具；实时事件合并、加载失败和账号切换。 |
| 活动 | `views/farm/activity/*.vue`、`activity/rules.ts` | `desktop/activities/`、`pages/mapper/activities.rs` | `desktop/tests/activities/`、`desktop/tests/weather.rs` | 活动目录、状态/时间排序、玩法路由、规则；通行证、星座、星砂、节气、青梅、七夕、秋日、公益、萌宠日记和天气玩法的进度、奖励、选择器、操作门控、确认、刷新、失败重试。未知玩法按 Vue 门控禁用。 |
| 收益分析 | `views/farm/analytics/index.vue` | `desktop/analytics/` | `desktop/tests/analytics.rs` | 五项排名指标、参考等级、并列排序、当前种植策略、种子候选、黑名单增删/保存/失败回退。 |
| 商城 | `views/farm/game-mall/index.vue` | `desktop/commerce/`、`desktop/activities/shop.rs` | `desktop/tests/commerce/` | 普通及会员商店入口、分类/搜索、余额与商品状态、数量上限、购买确认、在途门控和购买后刷新。 |
| 神秘商店 | `views/farm/mystery-shop/{index,panel}.vue` | `desktop/commerce/shop/mystery.rs` | `desktop/tests/commerce/mystery.rs` | 独立入口和商城标签入口、折扣/限购/余额、商品选择与共享购买流程。 |
| 设置 | `views/farm/settings/index.vue` | `desktop/settings/` | `desktop/tests/settings/` | 种植策略与时间字段、自动化开关和数值门槛、分组保存、化肥检查；离线提醒渠道/绑定/测试；QQ Bot 绑定；运行环境设备字段、预设、时区及重置。 |
| 游戏配置 | `views/farm/game-config/index.vue`、`game-config/shared.ts` | `desktop/game_config/`、`qq-farm-app::config` | `desktop/tests/game_config/`、`desktop/tests/assets.rs` | 种子/果实/道具搜索、筛选、表格列和分页；新增/编辑/删除与确认；字段默认值/回填；图片上传及 `/game-config/`、`/api/config/images/` 兼容解析。 |
| 账号 | `views/farm/account/index.vue`、`account/modules/account-operate-drawer.vue` | `desktop/accounts/`、`qq-farm-app::account` | `desktop/tests/accounts/` | 账号筛选/搜索/分页/列设置；新增编辑、备注、启停、删除；Code 登录、微信扫码、本机授权、头像与状态；切换账号时使旧请求失效。 |

## 跨页面和桌面壳

- [ ] 用真实账号逐页完成上表操作，记录成功、失败、空数据、离线、活动未开放、余额不足和授权过期行为；确认请求参数、服务端状态变化和页面回读。
- [ ] 在真实窗口检查中英文输入、键盘导航、Escape/焦点恢复、重复提交、浅深主题、最小尺寸、125%/150% 缩放、长名称、滚动和弹层。
- [x] 账号切换会启动新的页面请求代次：侧栏账号切换窗口测试断言 `request_revision` 递增。
- [x] 账号切换后将旧页面结果交给生产提交路径，验证旧结果不覆盖新账号页面，也不改动当前加载状态：`desktop/tests/accounts/navigation.rs::switching_accounts_discards_a_late_page_response`。测试驱动真实切号生命周期和结果提交路径，不依赖网络调度时序。
- [x] 事件总线积压触发快照恢复并继续接收新事件：`desktop/runtime/events/subscription.rs` 有容量为 1 的广播队列回归。
- [x] app 层 Tauri/GPUI owner 对同一数据目录互斥，异常进程退出后系统锁释放：`qq-farm-app/src/runtime_lease.rs` 覆盖两种 owner、同宿主唤起令牌和子进程退出。
- [ ] 在两个真实桌面进程间验证已有窗口唤起、另一端冲突提示、关窗后台运行、托盘恢复及明确退出。
- [x] 修复账号与游戏配置表格的纵向溢出：改为冻结表头 + 可滚动表体 + 固定列共享滚动句柄（表头/中间列另共享横向句柄）。回归：`account_table_scrolls_the_body_and_keeps_the_header_fixed`、`game_config_table_scrolls_the_body_and_keeps_the_header_fixed`（滚动前后行是否在可视表体内、表头固定、固定列与中间行基线一致）；真实滚轮/拖动滚动条手感待实窗。
- [x] 更新器只选择带有匹配 GPUI 产物的版本；`update.rs` 有目标产物过滤回归。发布页下载链路仍需真实发布响应验收。
- [x] macOS x86_64 与 aarch64 release `.app` / `.app.zip` 均已由当前源码在本机构建：Mach-O 架构分别为 x86_64 / arm64，bundle identifier `com.qqfarm.gpui`、版本与包内资源（`assets/game_config`、`activity-assets`、`tsdk.wasm`/`tsdk-wx.wasm`、图标）齐全，`.app` 与对应 `.zip` 内可执行文件 SHA-256 一致；aarch64 仍属本机交叉构建，尚未在 ARM 实机运行。（2026-10-13 晚随最新源码重建：x86_64 SHA-256 `70cfa82b…`，aarch64 `45658951…`；包内 release 可执行文件在隔离数据目录冒烟 15 秒无任何输出。）
- [ ] 在原生 runner/设备上完成 macOS ARM、Windows 和 Linux 的编译、测试、打包及窗口检查；本机 Linux 交叉检查缺 GTK/GIO sysroot 与目标 GCC，Windows 检查缺 `llvm-rc` 和 MSVC SDK。

本机已完成的源码、自动化、构建和预览证据记在 [`SYNC.md`](./SYNC.md)。未勾选项仍是验收工作，不应从编译成功或代码入口存在推断为已对齐。

### 2026-10-10 本机追加核验

- 好友页再次对照 Vue 搜索/排序/分页、黑名单排除、详情展开、逐个及批量操作回写、访客记录筛选和互动道具入口；星砂兑换数量复用商城确认弹窗并限制在 1–99。自动化覆盖了本轮检查的相关路径。
- 实时事件队列新增 lag 恢复回归：容量为 1 的 Tokio 广播队列溢出后，GPUI 转换为快照刷新事件，并继续接收最新事件。该测试覆盖恢复分支，不代替运行中的长时间压力测试。
- **验证通过**：GPUI `ui-preview` 全量 393 项测试、严格 Clippy、Rust 格式检查和差异检查。
- 本机尝试 Linux `x86_64-unknown-linux-gnu` 检查时，缺少 Linux 目标 GCC、GTK 开发库和交叉 `pkg-config` sysroot；Windows `x86_64-pc-windows-msvc` 检查时缺少 `llvm-rc`、MSVC 编译器及 Windows SDK。仓库 [`gpui.yml`](../.github/workflows/gpui.yml) 配有相应原生 runner，但当前工作树没有这些 runner 的 CI 结果，因此 Windows/Linux 原生编译、打包和窗口验收仍未完成。

### 2026-10-13 本机追加核验

- 非活动模块（总览、我的农场六分段、好友四分段、收益分析两分段、商城三标签、神秘商店、设置五分段、游戏配置三页签、账号）完成多轮逐项对齐：卡片/行/控件样式、按钮层级与配色、空态与状态文案、运行时成功/失败提示均已按 Vue 页面与 locale 复核（明细见 [`SYNC.md`](./SYNC.md) 同日各条目）。
- 预览矩阵扩展到 **214 张**，新增回归 `preview_scenes_cover_every_visible_page_section_in_all_conditions` 强制每个渲染页签的分段都具备浅色/深色/英文截图；最小窗口（1100×720）覆盖账号、我的农场（含地块/背包/任务/宠物/图鉴/互动道具）、好友、收益分析、商城与神秘商店、设置、游戏配置、看板与天气，125%/150% 缩放覆盖账号面板、表格与天气样例，并新增看板/好友/设置/商城/背包/宠物的 150% 压力场景，另有提醒渠道（钉钉/已绑定）、商城空目录/非会员分支（普通/SVIP 页签）/搜索无结果、神秘商店非活跃、空背包/礼包/宠物/好友/配置目录、账号列表为空/筛选无结果与分析黑名单空态、看板/分析/商城/好友的未选账号分支、账号表多页分页、看板日志截断（305 条、「显示更早」按钮）与展开后形态（全量 305 条 + 「收起」）及无日志空态、背包批量选择、购买确认提交中、下拉菜单展开态（游戏配置季节与道具类型选择器、设置种植策略选择器）、加入黑名单确认、好友详情失败带（错误文案 + 重试）、账号表单微信分支（本机检测/扫码等待/扫码失败警告横幅）与表单校验错误横幅、运行时反馈行（商城购买成功、好友操作反馈、背包出售成功）、更新提示横幅、加载中状态、三类错误态（页面级错误横幅、好友访客分节错误、我的农场地块分节错误），以及弹层/抽屉基线（账号表单新增/编辑/微信授权、游戏配置编辑器种子/果实/道具三种模式的新增与编辑、商城与神秘商店购买确认、宠物守护记录、账号/游戏配置/好友删除与移出/加入黑名单确认；均含浅/深，购买确认与好友删除确认另含英文）、游戏配置筛选空态、表格多选态与折叠搜索面板/列设置下拉（账号/游戏配置，面板含浅/深）。已知限制：账号表格「最近在线」在 150% 字体下按列宽截断（悬停提示提供完整时间）；窗口边框标题「QQ Farm」在 150% 下显示为「QQ Fa…」（固定几何的缩放压力项，应用无缩放入口）。
- 文案全量比对：脚本提取非活动模块 355 条中文 i18n 文案与 Vue 语言包比对，139 条不存在的条目逐条确认属于「Vue 模板硬编码后由 GPUI 翻译」或 GPUI 专属界面文案，未发现遗漏或错译。
- **验证通过**：GPUI `ui-preview` 全量 **412 项测试**、严格 GPUI Clippy（`--features ui-preview --all-targets --no-deps -- -D warnings`）、`cargo fmt --all -- --check`、`git diff --check`。
- macOS ARM 包补齐：在本机（x86_64）交叉构建 aarch64 release 客户端（`cargo build --release -p qq-farm-gpui --target aarch64-apple-darwin`）并用 `TARGET=aarch64-apple-darwin scripts/package-gpui.sh release` 打包，验证 arm64 Mach-O、Info.plist 标识与资源清单、以及 `.app`/`.zip` 内可执行文件 SHA-256 一致（`15e95f63…bcb0322`）；ARM 实机运行仍待原生环境。
- 表格中的未勾选项（真实账号逐页操作、真实窗口交互与缩放、双进程唤起/托盘、三平台原生 runner）仍需本机之外的环境，不能由本轮证据替代。

## 附录：预览状态 → 场景映射（2026-10-13，共 214 张）

矩阵目录 `/tmp/qq-farm-gpui-preview-1013z`；重生成命令：
`QQ_FARM_ASSET_DIR=<repo> cargo run -p qq-farm-gpui --features ui-preview -- --render-preview <dir>`。
下列分组把各模块代码中的可见状态与场景对齐（`a|b` 表示同名浅色/深色/英文等变体），供收尾时逐项勾验。

### 账号（23 张）

- 常规/深色/英文/最小窗 1100×720/150% 缩放：`accounts-light|dark|english|minimum|150`
- 列表为空：`accounts-empty-light`；多选（整行高亮 + 批量删除激活）：`accounts-selection-light|dark`
- 筛选无结果：`accounts-filter-empty-light`；多页分页（23 条 → 1–3 页）：`accounts-pagination-light`
- 折叠搜索面板（关键词 + 平台/运行/授权）：`accounts-search-light|dark`；列设置下拉：`accounts-columns-light`
- 行删除确认：`account-delete-confirm-light|dark`
- 新增表单（最小窗/125%/150%）：`account-sheet-minimum|125|150`；编辑表单：`account-sheet-edit-light`
- 微信授权：本机检测 `account-sheet-wx-light`、扫码等待 `account-sheet-wx-qr-light`、扫码失败警告横幅 `account-sheet-wx-error-light`
- 表单校验错误横幅：`account-sheet-form-error-light`

### 看板（7 张）

- 常规/深色/英文/最小窗：`dashboard-light|dark|english|minimum`；未选账号（仅账号卡占位）：`dashboard-no-account-light`
- 日志截断（305 条 → 「显示更早的 5 条日志」+ 续跟随）：`dashboard-logs-light`；展开后（全量 305 条 + 「收起」）：`dashboard-logs-expanded-light`；无日志空态：`dashboard-empty-logs-light`

### 我的农场（29 张）

- 地块：`farm-light|dark|english|minimum`；分节加载失败（横幅 + 重试）：`farm-section-error-light|dark`
- 背包：`bag-light|dark|english`、空态 `bag-empty-light`、批量选择 `bag-selection-light`、出售反馈 `bag-notice-light`
- 任务/礼包：`daily-gifts-light|dark|english|empty-light`；宠物：`pets-light|dark|english|empty-light`、守护记录 `pets-logs-light|dark`
- 图鉴：`illustrated-light|dark|english`、变异书 `illustrated-mutants-light`；互动道具：`interaction-light|dark|english`
- 分段最小窗口（1100×720）：`bag-minimum`、`daily-gifts-minimum`、`pets-minimum`、`illustrated-minimum`、`interaction-minimum`

### 好友（28 张）

- 列表：`friends-light|dark|english|minimum`；空列表 `friends-empty-light`；未选账号 `friends-no-account-light`
- 全局态：更新提示 `friends-update-light`、加载中 `friends-loading-light`、页面错误横幅 `friends-error-light|dark`、访客分节错误 `friends-records-error-light|dark`、操作反馈 `friends-feedback-light`
- 分节：互动道具 `friends-interaction-light|dark|english`、黑名单 `friends-blacklist-light|dark|english`、访客 `friends-visitors-light|dark|english`
- 确认气泡：删除 `friends-delete-confirm-light|dark|english`、加入黑名单 `friends-blacklist-confirm-light`、移出黑名单 `friends-unblacklist-confirm-light`
- 详情加载失败带（错误文案 + 重试）：`friends-detail-error-light`

### 收益分析（9 张）

- 常规/深色/英文/最小窗：`analytics-light|dark|english|minimum`；未选账号：`analytics-no-account-light`
- 种植黑名单：`analytics-blacklist-light|dark|english`、空态 `analytics-blacklist-empty-light`

### 商城与神秘商店（28 张）

- 商城：`mall-light|dark|english|minimum`；未选账号 `mall-no-account-light`；空目录 `mall-empty-light|dark`；搜索无结果 `mall-search-empty-light`
- 会员分支：SVIP `mall-svip-light|dark|english`、非会员普通页 `mall-non-svip-light`、非会员 SVIP 页 `mall-svip-locked-light`
- 购买确认：`mall-confirm-light|dark|english`、提交中 `mall-confirm-pending-light`、成功通知 `mall-notice-light`
- 神秘商店：`mystery-shop-light|dark|english|minimum`、非活跃 `mystery-shop-inactive-light|dark`、独立页签 `mall-mystery-light|dark|english`、购买确认 `mystery-confirm-light|dark`

### 设置（23 张）

- 五页签（策略/自动化/下线提醒/运行环境/界面偏好）：`settings-light|dark|english|minimum` + `settings-automation|offline|system|ui` 的浅/深/英文
- 下线提醒渠道：钉钉 `settings-dingtalk-light|dark|english`、QQ 机器人已绑定 `settings-offline-bound-light|dark|english`
- 种植策略下拉展开：`settings-strategy-open-light`

### 游戏配置（27 张）

- 三页签：`game-config-light|dark|english|minimum|150` + `game-config-fruits|items` 的浅/深/英文；空目录 `game-config-empty-light`
- 表格：多选 `game-config-selection-light|dark`、筛选无结果 `game-config-filter-empty-light`、折叠搜索面板 `game-config-search-light|dark`、列设置 `game-config-columns-light`、下拉展开季节/类型 `game-config-select-open-light`、`game-config-types-open-light`
- 编辑抽屉：种子 `game-config-editor-light|dark`、新增 `game-config-editor-new-light`、果实 `game-config-editor-fruit-light`、道具 `game-config-editor-item-light`
- 删除确认：`game-config-delete-confirm-light|dark`

### 活动（暂停；矩阵保留既有基线 27 张）

- 目录/详情：`activity-directory-light|dark|english`、`activity-detail-light|english`
- 玩法：`pet-diary-light|dark|english`、`charity-light|dark|english`、`qixi-light|dark|english`、`autumn-wish|happy-light|dark|english`
- 天气：`weather-light|dark|english|minimum|125|150`、`weather-friends-light|dark|english`

### 缩放压力场景（字体放大）

- 125%：`account-sheet-125`、`weather-125`
- 150%：`account-sheet-150`、`accounts-150`、`game-config-150`、`weather-150`、`dashboard-150`、`friends-150`、`settings-150`、`mall-150`、`bag-150`、`pets-150`
- 已知限制：账号「最近在线」列在 150% 下按列宽截断（悬停提示兜底）；窗口边框标题在 150% 下显示为「QQ Fa…」。应用当前没有用户可调的缩放入口，以上为设计指南要求的最小/缩放压力测试项。

### 明确不在预览范围（需外部环境）

- 真实账号逐页操作、真实窗口键鼠/滚动/托盘/多窗口唤起、Windows/Linux 打包与三平台原生 runner、ARM 实机运行；
- hover / focus / 拖拽滚动条等指针瞬态（属真实窗口交互范畴）；账号表格「最近在线」150% 截断为已知限制（悬停提示兜底）。

## 完成度审计预演（2026-10-13）

对照最初目标逐条列出现有证据与状态，供最终收尾勾验：

| 目标要求 | 本机证据 | 状态 |
|---|---|---|
| 基于 gpui-kit 创建桌面端 crate | `crates/qq-farm-gpui`（gpui-kit 0.7.0；独立 crate + 预览/测试基建；check/test/clippy/fmt 全绿） | ✅ 本机满足 |
| 页面功能与 desktop-ui 一致（非活动模块） | 本页对照表与状态映射附录 + [`SYNC.md`](./SYNC.md) 同日各条目 + 429 项测试（页面/控件/门控/文案/错误态/反馈/确认）+ 214 张预览矩阵 | ✅ 本机满足 |
| 活动页 | 用户指示暂停（活动已过期）；既有移植基线保留在矩阵中 | ⏸ 按用户指示暂缓 |
| Apple 风格设计 | 应用主题层 + 214 张浅/深/英文/最小窗/缩放预览，按 GPUI Kit 设计指南核对（层级、语义色、圆角/间距 token、桌面交互、状态可见性、无障碍名） | ✅ 本机满足（以预览基线与设计指南核对为准） |
| 交付物 | `dist/` x86_64 与 aarch64 `.app`/`.zip` 由当前源码构建，`.app` 与 `.zip` 内可执行文件 SHA-256 一致；release 冒烟 15 秒与 75 秒空闲观测（RSS 60 秒仅 +132KB、CPU 0%）均无输出无 panic；纯 debug 构建冒烟 20 秒亦无任何断言/布局告警输出 | ✅ 本机满足（aarch64 属交叉构建） |
| 真实账号逐页操作 | 需要真实 QQ 账号环境 | ⏳ 待外部环境 |
| 真实窗口交互（键鼠/滚动/托盘/多窗口唤起/缩放） | 需要原生窗口环境 | ⏳ 待外部环境 |
| Windows/Linux 打包、三平台原生 runner、ARM 实机 | 需要对应工具链与设备（[`gpui.yml`](../.github/workflows/gpui.yml) 已备） | ⏳ 待外部环境 |

- 桌面壳细节（代码级核对）：窗口最小尺寸 1100×720 由 `WindowOptions::window_min_size` 强制（与 Tauri `tauri.conf.json` 的 `minWidth/minHeight` 一致）；启动尺寸取 `preferences.window_width/height` 并按最小值兜底；`observe_window_bounds`（由 `desktop/init.rs` 挂载）在尺寸变化时持久化（仅当 ≥ 最小值且发生变化）；`observe_window_appearance` 在「跟随系统」偏好下实时切换深浅主题；托盘、更新检查、第二实例唤起与冲突检测均为既有实现并各有回归（见 [`SYNC.md`](./SYNC.md)）。

- 交付脚本与 CI 一致性：Windows 打包脚本 `package-gpui.ps1` 已与 `.sh` 逐行对照并修正两处漂移（zip 名按 `TARGET` 推导架构、压缩包含 `qq-farm-gpui/` 顶层目录）；`gpui.yml` 通过 YAML 解析校验（jobs: rust/frontend）——两者的**实跑**仍待 Windows runner / CI 环境（本机无 pwsh 与 MSVC 工具链）。

结论：目标中可在本机验证的部分均已满足并有证据；剩余三项为环境依赖的验收项。在这些证据补齐前，不将目标标记为完成。
