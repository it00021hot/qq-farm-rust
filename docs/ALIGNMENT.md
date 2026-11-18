# 功能对齐矩阵

对照 qq-farm-bot `6765449723270e3e1b8b4e83d6f545f10fa92eba`（2026-11-17，`Merge pull request #145`）。qq-farm-rust 盘点时为 `e0119e40b79e90c88bb4c917245445078ed34f22` 之上的本批改动。`docs/SYNC.md` 文首表格里的 bot `28a9614` 是 2026-10-25 的历史评分，不是当前对照。

状态只使用：缺失、部分、实现待验、验证通过、阻塞。本表没有「验证通过」：已有代码和 mock 测试不能代替窗口或真实网关。一项一行，按 bot 里实际存在的服务、路由或页面归并，不按文件拆碎。

| 项 | 状态 | bot 证据 | Rust 服务 | NSH 入口 | 缺口与证据 |
| --- | --- | --- | --- | --- | --- |
| QQ 登录码添加账号 | 实现待验 | `web/src/views` 账号流程；账号路由 | `qq-farm-app::accounts` | 账号页登录码 | 未做实机登录 |
| 微信扫码登录 | 实现待验 | `core/src/services/wx-login/service.ts`；`/api/wx-login/tasks` | `qq-farm-app::wx_login` | 账号页微信二维码 | 未做实机扫码 |
| QQ NapCat 扫码 | 缺失 | `core/src/services/qq-login/service.ts` 依赖 NapCat 地址和签名 | `qrlogin.rs` 是小程序扫码，不是 NapCat | 无 | 桌面没有这条入口 |
| 账号启停与删除 | 实现待验 | `/api/accounts/:id/start` 与 `stop` | `qq-farm-app::accounts` | 账号页 | 未做实机 |
| 面板密码登录 | 部分 | `auth-routes.ts` 的 `/api/login` | `auth::local_owner_session` | 无密码页 | 桌面用本地会话，不对齐 Web 密码登录 |
| 农场巡查与地块手动操作 | 实现待验 | `services/farm/*`；`/api/lands`、`/api/farm/operate` | `services/farm` | 我的农场 | 未做实机一轮 |
| 一键施肥 | 实现待验 | `/api/farm/fertilize-all` | `farm::fertilize_all` | 我的农场 | 未做实机 |
| 保存化肥购买设置后立即检查 | 实现待验 | `web/src/views/Settings.vue` 保存自动控制后调用 `/api/fertilizer/check-and-buy` | `farm::fertilizer_check_and_buy` 读取已保存数量和阈值 | 设置里打开购买开关或改数量、阈值、间隔 | mock 覆盖何时触发、失败和购买数量；真实商城购买未跑 |
| 运行中按间隔自动买化肥 | 实现待验 | `mall.ts` 与设置里的检查间隔 | `worker_loop::check_fertilizer_buy_once` | 无单独按钮，随巡查 | 未做实机 |
| 好友列表、帮助、偷菜、捣乱、删除 | 实现待验 | `services/friend/*`；`friend-routes.ts` | `services/friend` | 好友页 | 未做实机 |
| 刷新好友列表 | 实现待验 | `Friends.vue` 调 `/api/friends/clear-cache` | `friend::list_friends(..., force)` | 好友页「刷新」 | 未做实机 |
| 偷菜黑名单 | 实现待验 | `/api/plant-blacklist`；`Analytics.vue` | `farm::plant_blacklist` | 收益分析 | 未做实机 |
| 好友黑名单 | 实现待验 | `/api/friend-blacklist` | `friend::toggle_friend_blacklist` | 好友页 | 未做实机 |
| 已知好友 GID | 实现待验 | `/api/friend-known-gids` | 好友 GID 存储 | 已知好友区 | 未做实机 |
| 牧场手动操作与巡查 | 实现待验 | `services/ranch/*`；`/api/ranch` | `services/ranch`、`ranch_patrol` | 牧场页 | 未做实机 |
| 牧草按阈值补充且不自动出售 | 实现待验 | `ranch/automation.ts`；`warehouse.ts` 跳过 7001 | `ranch_patrol`、`warehouse::included_in_auto_sell` | 设置里的牧草开关；出售不单列按钮 | 有单测，未在本批重跑，也未实机 |
| 背包使用、出售、锁定 | 实现待验 | `/api/bag`、`/api/bag/use`、`sell`、`lock` | `qq-farm-app::farm` 背包函数 | 背包 | 未做实机 |
| 商城、神秘商店、免费礼包 | 实现待验 | `commerce-routes.ts` 的 game-mall、mystery-shop | `qq-farm-app::commerce` | 商城；礼物里的免费礼包 | 未做实机 |
| 邮箱、会员、月卡、分享、成长任务 | 实现待验 | `/api/daily-gifts`；`task.ts`、`email.ts` | `farm::daily_gift_overview` 与各领取函数 | 礼物区 | 未做实机 |
| 邀请类成长任务上报 | 实现待验 | `task.ts` 的 `reportClientProgress`（`ae71408`） | `services/task.rs` 的 `client_reported_tasks` | 无单独按钮，随任务检查 | 有单测，未在本批重跑 |
| 雨落成诗扫描、采集、召唤、研究、使坏瓶 | 实现待验 | `weather-activity.ts`；`weather-routes.ts` | `services/weather.rs` | 活动页雨落成诗 | mock 覆盖扫描和通知失效；真实 Enter 未跑 |
| 天气变更后的扫描缓存 | 实现待验 | `network.ts` 发 `weatherChanged`；bot 清整份进程缓存 | 只清收到通知的账号里被点名的农场 | 下次扫描才变，页面上一次结果不自动消失 | 与 bot 全清不同，已记在 SYNC |
| 天气任务领取 | 阻塞 | 路由和 `WeatherActivityView.vue` 都没有领取；`WeatherTaskOperateRequest` 注明是采集 field 107 的兼容别名 | 只展示任务进度 | 只展示进度 | 上游没有领取接口。这是待核实的原假设，不是必须新造的 bot 功能 |
| 节令、快乐不独享、奶茶、牧歌、牧场活动 | 实现待验 | `solar-calendar.ts`、`share-reward-activity.ts`、`milk-tea.ts`、`muge-activity.ts`、`ranch_activities` | 同名服务 | 活动页对应分栏 | 未做实机 |
| 千星、七夕、青梅、公益、萌宠日记、秋日 | 缺失 | `activity-center/*` 与对应页面仍在 bot | 2026-10-16 按项目决策删除 | 无 | 与 bot 现存代码不一致；本批不恢复 |
| 调度、心跳与掉线重连 | 实现待验 | `scheduler.ts`、`/api/scheduler`；worker 重连 | `runtime/worker.rs`、`worker_loop` | 总览的下次巡查 | 未做实机长挂 |
| 日志与今日统计 | 实现待验 | `/api/logs`、`stats.ts` | `panel_log`、`stats` | 总览 | 未做实机 |
| 下线提醒 | 实现待验 | `push.ts`；设置里的提醒 | `offline_reminder`、`qq_bot_bind` | 设置里的下线提醒 | 未做实机推送 |
| 图鉴与藏品 | 实现待验 | `/api/illustrated`、`/api/collections` | `illustrated`、`collections` | 图鉴、藏品 | 未做实机 |
| 游戏配置编辑 | 实现待验 | `/api/config/seeds` 等；`ConfigManage.vue` | `qq-farm-app::config` | 游戏配置 | 未做实机 |

口径：上表 30 项。缺失 2，部分 1，阻塞 1，实现待验 26，验证通过 0。
