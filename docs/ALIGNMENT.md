# 功能对齐矩阵

对照 qq-farm-bot `6765449723270e3e1b8b4e83d6f545f10fa92eba`（2026-11-17）。`docs/SYNC.md` 文首的 bot `28a9614` 只是 2026-10-25 的历史评分。

四层证据分开记。代码对照是读过的源码符号。mock 只表示自动测试，窗口和真实接口在本文件里全部是未验。代码已有不等于验收通过。

基线只收 bot 实际有的用户行为。天气任务领取不在基线里。旧活动不因为 SYNC 里的删除叙述就从目标里划掉。

## 登录与账号

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| AUTH-01 | 用登录码添加账号 | 代码已有。bot 账号路由；`qq-farm-app::accounts::upsert_account`；NSH `accounts.rs` 登录码输入 | 无 | 未验 | 未验 |
| AUTH-02 | 微信扫码登录 | 代码已有。bot `wx-login/service.ts`、`/api/wx-login/tasks`；`qq-farm-app::wx_login`；NSH `accounts.rs` 二维码 | 无 | 未验 | 未验 |
| AUTH-03 | QQ NapCat 扫码 | 代码缺失。bot `qq-login/service.ts` 的 `loginSettings` 要求 `qqQrLogin`、`napCatEndpoint`、`napCatSignature`。Rust `qrlogin.rs` 是另一条小程序扫码，NSH 没有 NapCat 入口 | 无 | 未验 | 未验 |
| AUTH-04 | 启停和删除账号 | 代码已有。bot `/api/accounts/:id/start`、`stop`；`accounts::start_account`、`stop_account`、`delete_account`；NSH 账号页 | 无 | 未验 | 未验 |
| AUTH-05 | Web 面板密码登录 | 语义待确认。bot `auth-routes.ts` 的 `POST /api/login`。桌面 `auth::local_owner_session` 不设密码门。这不是已对齐，本批也不加密码门 | 无 | 未验 | 未验 |

本组 5 项：代码已有 3，代码缺失 1，语义待确认 1。

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

本组 5 项：代码已有 5。

## 牧场

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| RANCH-01 | 牧场手动操作和巡查 | 代码已有。bot `services/ranch/*`、`/api/ranch/:action`。`qq-farm-app::ranch`。NSH `ranch.rs` | 无 | 未验 | 未验 |
| RANCH-02 | 牧草低于阈值才补，自动出售留下 7001 | 代码已有。bot `ranch/automation.ts` 用 `ranch_grass_threshold_percent`；`warehouse.ts` `id === 7001` 跳过。Rust `ranch_patrol::grass_top_up`、`warehouse::included_in_auto_sell` | 仓库有对应测试，本批未跑 | 未验 | 未验 |

本组 2 项：代码已有 2。

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

本组 2 项：代码已有 2。

## 天气

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| WEATHER-01 | 扫描、采集、召唤、研究、青蛙和乌云 | 代码已有。bot `weather-activity.ts`、`weather-routes.ts`。`WeatherService` 同名方法。NSH `features/weather.rs` | 先前批次的扫描测试通过；本批未重跑。真实 Enter 未跑 | 未验 | 未验 |
| WEATHER-02 | 天气通知使扫描缓存失效 | 代码部分。bot `network.ts` 解析 `WeatherChangeNotify` 后，`weather-activity.ts` 不看 `hostGid`，清掉整个进程缓存。Rust `notify::parse_event` 得到 `WeatherChanged`，`WeatherService::invalidate_visits` 只删该账号里被点名的农场；`host_gid == 0` 才清该账号全部。NSH 已显示的扫描结果不会自动清掉 | 先前批次 mock 通过；本批未重跑 | 未验 | 未验 |

本组 2 项：代码已有 1，代码部分 1。

## 活动

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| ACT-01 | 节令、快乐不独享、奶茶、牧歌、牧场活动目录 | 代码已有。bot `solar-calendar.ts`、`share-reward-activity.ts`、`milk-tea.ts`、`muge-activity.ts`。NSH `activities.rs` 的 `Shelf::Solar`、`Share`、`Tea`、`Muge`、`Ranch` | 无 | 未验 | 未验 |
| ACT-02 | 千星、七夕、青梅、公益、萌宠日记、秋日 | 范围待核实。bot `activity-center/` 与 `StellarActivityPage.vue`、`QixiActivityPage.vue`、`QingMeiActivityPage.vue`、`CharityActivityPage.vue`、`PetDiaryPage.vue`、`AutumnActivityPage.vue` 仍在。SYNC 2026-10-16 写了删除，但没有可核对的原始指令；名单还和现况冲突：鹊桥/七夕被写进删除，天气雨落成诗也曾被写进删除，而 WEATHER-01 现在有实现。本聊天的目标是对齐 bot 全部功能，所以这项保留为上游差异，不恢复，也不从目标里删掉 | 无 | 未验 | 未验 |

本组 2 项：代码已有 1，范围待核实 1。

## 调度、通知、日志与配置

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| SCHED-01 | 心跳、掉线后不再用旧码重连，微信可走应用宝授权 | 代码已有。bot worker 重连。Rust `worker.rs` 心跳超时 `force_disconnect_with_reason("heartbeat_timeout")`，并区分应用宝授权重连和不再使用旧 Code | 无 | 未验 | 未验 |
| SCHED-02 | 网关心跳漏拍或在途请求卡住时，推迟农场和好友巡查 | 代码已有。bot `worker.ts` `nextBusinessTickDeferMs`：健康则清退避，否则首次 30 秒、之后翻倍、封顶 60 秒，农场和好友分开记。Rust 原有 `is_gateway_healthy_for_business` 和 `next_business_backoff_ms`，但 `run_farm_tick` / `run_friend_tick` 没调用。本批用 `step_business_tick` 接上，并在第一次退避和恢复时写面板日志 | 本批 `business_backoff_first_30s_then_double_capped_60s` 通过。没有用假网关跑完整巡查 | 未验 | 未验 |
| LOG-01 | 运行日志和今日统计 | 代码已有。bot `/api/logs`、`stats.ts`。NSH `dashboard.rs` | 无 | 未验 | 未验 |
| REMIND-01 | 下线提醒 | 代码已有。bot `push.ts`。NSH `offline_reminder.rs`、`qq_bot_bind` | 无 | 未验 | 未验 |
| COLL-01 | 图鉴和藏品 | 代码已有。bot `/api/illustrated`、`/api/collections`。NSH `illustrated.rs`、`collections.rs` | 无 | 未验 | 未验 |
| CONF-01 | 游戏配置编辑 | 代码已有。bot `ConfigManage.vue`、`/api/config/seeds`。`qq-farm-app::config`。NSH `game_config.rs` | 无 | 未验 | 未验 |

本组 6 项：代码已有 6。

## 基线计数

基线 30 项。代码已有 26，代码部分 1（WEATHER-02），代码缺失 1（AUTH-03），语义待确认 1（AUTH-05），范围待核实 1（ACT-02）。窗口通过 0，真实接口通过 0。没有「验证通过」项。

## 非基线

| ID | 来由 | 结论 |
| --- | --- | --- |
| EXTRA-01 | 早前进度把「天气任务领取」写成还没做 | 不占基线。bot `weather-routes.ts` 没有领取路由；`WeatherActivityView.vue` 只渲染任务进度；`weather-activity.ts` 不发送领取；`activitypb.proto` 写明 `WeatherTaskOperateRequest` 是采集 field 107 的兼容别名，没有单独的领取 `operate_type`。不另造协议 |
