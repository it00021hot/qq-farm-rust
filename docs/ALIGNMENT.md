# 功能对齐矩阵

对照 qq-farm-bot `c500e085c5e3be790d3b723965fecc6193dfb176`（2026-11-18 15:40 +0800，`git fetch` 后的 `origin/master`；本地 bot 工作区仍停在 `6765449723270e3e1b8b4e83d6f545f10fa92eba`，没有 checkout）。`docs/SYNC.md` 文首的 bot `28a9614` 只是 2026-10-25 的历史评分。

四层证据分开记。代码对照是读过的源码符号。mock 只表示自动测试，窗口和真实接口在本文件里全部是未验。代码已有不等于验收通过。

基线只收 bot 实际有的用户行为。天气任务领取不在基线里。旧活动不因为 SYNC 里的删除叙述就从目标里划掉。

## 登录与账号

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| AUTH-01 | 用登录码添加账号 | 代码已有。bot 账号路由；`qq-farm-app::accounts::upsert_account`；NSH `accounts.rs` 登录码输入 | 无 | 未验 | 未验 |
| AUTH-02 | 微信扫码登录 | 代码已有。bot `wx-login/service.ts`、`/api/wx-login/tasks`；`qq-farm-app::wx_login`；NSH `accounts.rs` 二维码 | 无 | 未验 | 未验 |
| AUTH-03 | QQ NapCat 扫码 | 代码已有，可选。bot `qq-login/service.ts` 在 `qqQrLogin` 开启且地址和签名都有时，向 NapCat 发 `/api/qq/login/qrcode`、`/status`、`/miniapp/code`、`/logout`，头是 `X-API-Signature`。默认关闭，登录码和微信扫码不改道。NSH 账号页可保存这三项并走扫码状态；确认后的 code 交给现有 `upsert_account`，平台仍是 qq。取消或换代后的迟到结果不入库 | `services::napcat_login` 与 `stale_napcat_code_is_not_stored`：代次不一致时 `store_code` 不调用入库。Debug 不打印 code 和回包体。没有真实 NapCat | 未验 | 未验 |
| AUTH-04 | 启停和删除账号 | 代码已有。bot `/api/accounts/:id/start`、`stop`；`accounts::start_account`、`stop_account`、`delete_account`；NSH 账号页 | 无 | 未验 | 未验 |
| AUTH-05 | Web 面板密码登录 | 桌面差异。产品决定桌面保留本机直接进入，不照搬 bot `auth-routes.ts` 的 `POST /api/login` 密码门。`auth::local_owner_session` 继续不设密码。这不是缺口 | 无 | 未验 | 未验 |

本组 5 项：代码已有 4，桌面差异 1。

## 农场

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| FARM-01 | 巡查，以及收获、浇水、除草、除虫、施肥、种植、照料、铲除、升级、解锁、一轮 | 代码已有。bot `services/farm/*`、`/api/farm/operate`。NSH `farm.rs` `FarmOp` | 仓库有操作可见性测试，本批未跑 | 未验 | 未验 |
| FARM-02 | 一键施肥 | 代码已有。bot `/api/farm/fertilize-all`；`farm::fertilize_all`；NSH 农场施肥入口 | 无 | 未验 | 未验 |
| FARM-03 | 保存化肥购买设置后按已存数量和阈值检查 | 代码已有。bot `Settings.vue` 的 `saveAutomationSettings` 调用 `/api/fertilizer/check-and-buy`。`farm::fertilizer_check_and_buy` 读 `get_account_config_snapshot`。NSH `settings.rs` `fertilizer_patch` | 本批 `fertilizer_check_uses_saved_counts_and_thresholds`、`fertilizer_purchase_edits_check_only_when_a_buy_switch_stays_on` 通过。真实下单未跑 | 未验 | 未验 |
| FARM-04 | 运行中按间隔自动买化肥 | 代码已有。`worker_loop::check_fertilizer_buy_once` 组装 `FertilizerBothOptions` | 无 | 未验 | 未验 |

本组 4 项：代码已有 4。

## 好友

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| FRIEND-01 | 好友列表和刷新 | 代码已有。bot `Friends.vue` 调 `POST /api/friends/clear-cache`。Rust `FriendService::get_friends_list` 在 `force` 时 `invalidate_list_cache` 并跳过 TTL。NSH `friends.rs` 刷新把 generation 加一后 `list_friends(..., generation > 0)` | 无 | 未验 | 未验 |
| FRIEND-02 | 帮助、偷菜、捣乱、删除 | 代码已有。bot `POST /api/friend/:gid/op`、`delete`。`friend::friend_op`、`delete_friend`。NSH 好友行操作 | 无 | 未验 | 未验 |
| FRIEND-03 | 偷菜黑名单 | 代码已有。bot `/api/plant-blacklist`。`farm::plant_blacklist`。NSH `analytics.rs` | 仓库有编辑测试，本批未跑 | 未验 | 未验 |
| FRIEND-04 | 好友黑名单 | 代码已有。bot `/api/friend-blacklist/toggle`。`friend::toggle_friend_blacklist`。NSH `next_blacklist` | 仓库有切换测试，本批未跑 | 未验 | 未验 |
| FRIEND-05 | 已知好友 GID | 代码已有。bot `/api/friend-known-gids`。NSH `known_friends.rs` | 无 | 未验 | 未验 |

本组 5 项：代码已有 5。大量好友时的巡查顺序记在 SCHED-03，不在本组重列。

## 牧场

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| RANCH-01 | 牧场手动操作和巡查 | 代码已有。bot `services/ranch/*`、`/api/ranch/:action`。`qq-farm-app::ranch`。NSH `ranch.rs` | 无 | 未验 | 未验 |
| RANCH-02 | 牧草低于阈值才补，自动出售留下 7001 | 代码已有。bot `ranch/automation.ts` 用 `ranch_grass_threshold_percent`；`warehouse.ts` `id === 7001` 跳过。Rust `ranch_patrol::grass_top_up`、`warehouse::included_in_auto_sell` | 仓库有对应测试，本批未跑 | 未验 | 未验 |
| RANCH-03 | 便便优先兑换有机化肥 | 未实现。`c500e08` 的 `ranch/automation.ts` 在 `ranch_market_organic` 打开时，只用来源 7002 的便便，按商品 80012、80011 的顺序兑到付不起为止。便便增加或到点会再跑。Rust `ranch::exchange` 能按当前标价兑一件并拒绝余额不足，巡查没有这条自动兑换 | 无 | 未验 | 未验 |
| RANCH-04 | 窝棚分组、幼崽补栏和一键喂养 | 未实现。`c500e08` 增加窝棚幼崽优先级、先放背包再购买，以及一键喂养。Rust 巡查仍按原有放养和分桶喂养，没有这组入口 | 无 | 未验 | 未验 |

本组 4 项：代码已有 2，未实现 2。单次集市兑换拒绝余额不足已在 `ranch::exchange`。新提交里禁用购买按钮和收紧数量上限是界面，不单列、不重做。

## 背包与商店

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| BAG-01 | 使用、出售、锁定 | 代码已有。bot `/api/bag/use`、`sell`、`lock`。`qq-farm-app::farm` 同名函数。NSH `bag.rs` | 无 | 未验 | 未验 |
| BAG-02 | 普通商城、神秘商店、免费礼包 | 代码已有。bot `commerce-routes.ts`。`qq-farm-app::commerce`。NSH `mall.rs`；免费礼包在 `gifts.rs` 的 `mall` | 无 | 未验 | 未验 |

本组 2 项：代码已有 2。

## 任务与礼物

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| TASK-01 | 邮箱、会员、月卡、分享、成长任务 | 代码已有。bot `email.ts`、`task.ts`、`qqvip.ts`、`monthcard.ts`、`share.ts`。NSH `gifts.rs` 的 `email`、`vip`、`month`、`share` 和 `claim_growth_task` | 仓库有礼物状态测试，本批未跑 | 未验 | 未验 |
| TASK-02 | 邀请类成长任务先上报再领 | 代码已有。bot `task.ts` `reportClientProgress`（`ae71408`）。Rust `task.rs` `complete_client_reported_tasks` | 仓库有 `client_report_picks_unfinished_invite_tasks`，本批未跑 | 未验 | 未验 |
| TASK-03 | 设置 QQ 状态和牧场打开好友面板后自动完成成长任务 | 未实现。`c500e08` 把客户端上报条件从只有 15 扩到 15 和 25。牧场每日任务另调用 `reportEntry`，入口是 `friends`。Rust 仍只上报条件 15，牧场巡查没有这条上报 | 无 | 未验 | 未验 |

本组 3 项：代码已有 2，未实现 1。

## 天气

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| WEATHER-01 | 扫描、采集、召唤、研究、青蛙和乌云 | 代码已有。bot `weather-activity.ts`、`weather-routes.ts`。`WeatherService` 同名方法。NSH `features/weather.rs` | 先前批次的扫描测试通过；本批未重跑。真实 Enter 未跑 | 未验 | 未验 |
| WEATHER-02 | 天气通知和断线使扫描缓存失效 | 代码已有。bot `weather-activity.ts` 的 `weatherChanged` 不看 `hostGid`，`friendWeatherCache.clear()`。`disconnected` 再清 `pendingSnapshot`、`friendWeatherInspections` 和 `friendWeatherCache`。`activity-windows.ts` 的 `disconnected` 把 `loadedAt` 置 0。Rust 会话结束走 `worker.rs` 里的 `quiesce_bot`：关掉该账号天气写入，清扫描缓存并增加代次；锁被占用时由在途扫描在写回前丢弃。`resume_bot` 或新建 worker 后可以再扫。活动窗口用同一个 `invalidate_activity_windows`，不再当作已加载。窗口表是全服一份，一个账号断线会让这份日程变陈旧。另一账号的 `WeatherService` 不共用扫描缓存。Rust 没有 bot 的 `pendingSnapshot` 合并请求。NSH 同时比较 `visitSession` 和 `visitGeneration`，代次或会话号不一致就不再显示 | `named_farm_notify_clears_the_account_after_the_scan_writes`、`closed_session_discards_a_scan_that_was_waiting_to_write`、`session_end_drops_that_accounts_weather_and_stales_activity_windows`、`displayed_scan_hides_when_the_visit_generation_moves`。摘掉 `quiesce_bot` 里的清理后，会话结束测试失败。窗口未打开 | 未验 | 未验 |

本组 2 项：代码已有 2。

## 活动

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| ACT-01 | 节令、快乐不独享、奶茶、牧歌、牧场活动目录 | 代码已有。bot `solar-calendar.ts`、`share-reward-activity.ts`、`milk-tea.ts`、`muge-activity.ts`。NSH `activities.rs` 的 `Shelf::Solar`、`Share`、`Tea`、`Muge`、`Ranch` | 无 | 未验 | 未验 |
| ACT-02 | 千星、七夕、青梅、公益、萌宠日记、秋日 | 范围待核实，页面仍挂着。`c500e08` 的 `ActivityCenter.vue` 在目录打开 `wish`、`stellar`、`qixi`、`qingmei`、`charity`、`pet` 时分别挂秋日、千星、七夕、青梅、公益、萌宠日记。这次 13 个提交没有删除它们。旧 SYNC 的删除叙述仍没有可核对的原始指令，不能据此排除，本批也不恢复 | 无 | 未验 | 未验 |

本组 2 项：代码已有 1，范围待核实 1。

## 调度、通知、日志与配置

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| SCHED-01 | 心跳、掉线后不再用旧码重连，微信可走应用宝授权 | 代码已有。bot worker 重连。Rust `worker.rs` 心跳超时 `force_disconnect_with_reason("heartbeat_timeout")`，并区分应用宝授权重连和不再使用旧 Code | 无 | 未验 | 未验 |
| SCHED-03 | 大量好友时先巡农场和好友，再跑牧场 | 未实现。`c500e08` 的 `worker.ts` 要等农场和好友各完成首轮，并且这两项都不在跑，牧场例程才开始。好友读取遇到网关让路或 `stage=pending` 会停下。Rust 巡查仍按自己的到期时间进入牧场 | 无 | 未验 | 未验 |
| SCHED-02 | 网关心跳漏拍或在途请求卡住时，推迟农场和好友巡查 | 代码已有。bot `worker.ts` `nextBusinessTickDeferMs`：健康则清退避，否则首次 30 秒、之后翻倍、封顶 60 秒，农场和好友分开记。Rust `run_farm_tick` / `run_friend_tick` 经 `defer_unhealthy_tick` 调用 `step_business_tick`。第一次退避和恢复各写一条面板日志 | `patrol_ticks_wait_out_a_silent_gateway_and_resume_separately` 用离线 Gateway，把最近入站时间拨过心跳静默，再直接调用两次巡查。无回包时农场不进入正常 20–25 秒排期，好友不出现「巡查异常」；两种巡查各自先 30 秒、再封顶 60 秒，状态互不覆盖；恢复后农场回到正常间隔，好友发出巡查并记「巡查异常」，各自退避清零；首条退避和恢复日志不重复。摘掉 `defer_unhealthy_tick` 接线时农场退避停在 0，该测试失败。`business_backoff_first_30s_then_double_capped_60s` 仍只覆盖计算。农场 AllLands 帧没有单独计数，农场「未发请求」由函数在正常排期前返回推出。在途请求卡住满 5 秒的那条健康度条件没有单独拨时钟 | 未验 | 未验 |
| LOG-01 | 运行日志和今日统计 | 代码已有。bot `/api/logs`、`stats.ts`。NSH `dashboard.rs` | 无 | 未验 | 未验 |
| REMIND-01 | 下线提醒 | 代码已有。bot `push.ts`。NSH `offline_reminder.rs`、`qq_bot_bind` | 无 | 未验 | 未验 |
| COLL-01 | 图鉴和藏品 | 代码已有。bot `/api/illustrated`、`/api/collections`。NSH `illustrated.rs`、`collections.rs` | 无 | 未验 | 未验 |
| COLL-02 | 图鉴全部已读 | 代码已有。`c500e08` 的 `clearIllustratedNew` 把正数 `fruitId` 发给 `ClearNewUnlockedFruitsV2`。Rust `IllustratedService::clear_new` 同样丢掉非正数，空名单不发请求；NSH 图鉴页有「全部已读」，只提交当前页 `isNew` 且果实号为正的条目 | `clear_new_keeps_only_positive_ids_and_reads_the_reply`、`clear_new_with_no_ids_does_not_ask_the_gateway`、`read_all_uses_only_positive_fresh_fruit_ids`。没有真实回包，没有窗口点击 | 未验 | 未验 |
| CONF-01 | 游戏配置编辑 | 代码已有。bot `ConfigManage.vue`、`/api/config/seeds`。`qq-farm-app::config`。NSH `game_config.rs` | 无 | 未验 | 未验 |

本组 8 项：代码已有 7，未实现 1。

## 基线计数

基线 35 项。代码已有 29，未实现 4（RANCH-03、RANCH-04、TASK-03、SCHED-03），桌面差异 1（AUTH-05），范围待核实 1（ACT-02）。窗口通过 0，真实接口通过 0。没有「验证通过」项。空账号总览曾打开过；导航和其他页面没有窗口证据。`54fda84` 上的 1203 项测试不是本基准的全量回归。

## 非基线

| ID | 来由 | 结论 |
| --- | --- | --- |
| EXTRA-01 | 早前进度把「天气任务领取」写成还没做 | 不占基线。bot `weather-routes.ts` 没有领取路由；`WeatherActivityView.vue` 只渲染任务进度；`weather-activity.ts` 不发送领取；`activitypb.proto` 写明 `WeatherTaskOperateRequest` 是采集 field 107 的兼容别名，没有单独的领取 `operate_type`。不另造协议 |
