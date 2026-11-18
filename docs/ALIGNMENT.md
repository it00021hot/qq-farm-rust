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
| RANCH-03 | 便便优先兑换有机化肥 | 代码已有。`6d7aebe` 起、`c500e08` 的 `ranch/automation.ts`：`ranch_market_organic` 默认开，旧配置缺这个字段时也按开。只使用物品 7002 的便便，先兑商品 80012（4 小时有机肥），再兑 80011（1 小时）。限购和余额决定数量，付不起就停，不使用其它货币的同名商品。集市没开仍记下一轮；接口失败不把这轮记成完成，也不立刻重试。便便增加会把下一轮巡查提前到约 20 秒。Rust 巡查在出售之后调用 `exchange_poop_organic`，设置页开关走原来的保存 | `patrol_exchanges_four_hour_organic_before_one_hour_and_stops_cleanly`、`missing_market_switch_stays_on_and_the_save_path_can_change_it`。没有真实兑换 | 未验 | 未验 |
| RANCH-04 | 窝棚分组、幼崽补栏和一键喂养 | 代码已有，分三块。分组：设施按模块 6 窝、7 棚、8 牧草机排列，同组合计只数和容量，组内动物按栏位再按编号。幼崽：`ranch_coop_cub_priority` / `ranch_shed_cub_priority` 默认空；旧配置缺字段仍是空。背包优先时先按这份顺序放养背包幼崽，放满或失败后才打开商店；指定动物时只放该动物再买。数量仍受空位、余额、单次上限和限购约束，取消确认不发购买。一键喂养：普通罐头要求桶里有时长且有不受限的可喂动物，有机罐头要求桶里有时长且有可喂动物；不满足时按钮不可用，不发请求 | `patrol_places_saved_cubs_before_opening_the_shop`、`bag_cubs_place_the_saved_order_ahead_of_the_default_order`、`missing_cub_priority_stays_empty_and_a_mismatched_id_is_kept_out`、`pens_are_grouped_coop_then_shed_and_animals_follow_slot`、`feed_all_stays_off_without_time_or_a_feedable_animal`、`moving_a_cub_earlier_keeps_cubs_that_are_not_on_screen`。没有真实放养、购买或喂养 | 未验 | 未验 |

本组 4 项：代码已有 4。单次集市兑换拒绝余额不足已在 `ranch::exchange`。新提交里禁用购买按钮和收紧数量上限是界面，不单列、不重做。

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
| TASK-03 | 设置 QQ 状态和牧场打开好友面板后自动完成成长任务 | 代码已有。`c500e08` 的 `task.ts` 在 `task` 开关打开时，对未完成、已解锁、未领取且 `cond_type` 为 15 或 25 的成长任务调用 `TaskService.ClientReportProgress`，进度补齐后才按原领取流程处理；上报失败或回包未达标不领取。不写死任务号。牧场 `claim_tasks_if_due` 在领取前，对场景 2 任务里配置条件为「客户端上报进度」的未完成项走同一 RPC；`MCTask.json` 里 90005、91003、300008 都是这条条件，其中 300008 的描述是打开好友面板。回包没补齐进度就停止后续上报，请求失败仍继续原领取，不把任务记成已完成 | `check_and_claim_reports_condition_25_and_does_not_claim_a_failed_report` 走 `TaskService::check_and_claim_tasks`。`claim_schedule_reports_the_friend_panel_task_before_claiming` 走 `claim_tasks_if_due`。开关关闭不发包。没有真实上报 | 未验 | 未验 |

本组 3 项：代码已有 3。

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
| ACT-02 | 秋祈良愿、千星、七夕、青梅、公益、萌宠日记 | 未实现。`c500e08` 的 `ActivityCenter.vue` 仍能打开这六页。调用都在 worker 的页面 case 里，`worker.ts` 的定时器没有这些方法。时间取服务端回包的开始和结束；活动号里的日期、以及开始或结束为 0，都不能当成已经过期或当前可用。秋祈良愿和千星已独立离线通过，窗口和真实接口未验。`ec905c8` 的定向测试过了，但审计不接受那一版缩水界面；023A、023B 之后千星才算独立离线通过。七夕已接到目录和 core，待独立复验。青梅、公益、萌宠日记仍没有读取和写操作。母项因此仍是未实现。快乐不独享仍是 ACT-01。当前是否仍在服务端开放未知 | 秋祈、千星见子表。其余无 | 未验 | 未验 |

本组 2 项：代码已有 1，未实现 1。ACT-02 仍是一个母项。下面六条是组内行为，不另占 35 项分母。

| 子行为 | bot `c500e08` | Rust | 静态结论 |
| --- | --- | --- | --- |
| 秋祈良愿 `wish` | `ActivityCenter.vue` 在 `wish` 挂 `AutumnActivityPage`。`autumn-activities.ts` 组 `2026092400`、活动 `2026092401`，读取 `ActivityService.GetGroup`。抽签、领取是 `operateAutumnActivity` 的 `Operate` 51、52。写之前用回包开始、结束和服务器时间拒绝，结束时刻本身不算开放。抽签请求只带 `choose_id`。配置和页面都没有消耗字段，这不能证明服务端不扣背包。页面没有确认框 | `Shelf::Wish` 打开 `autumn_wish::panel`。`press` 的命令由 `dispatch_wish` 交给 `qq-farm-app::autumn_wish`，再进当前 worker 上的 `AutumnWishService`。领取按服务端 pending 的 `choose_id` 原样发送，包括 0 和配置外的编号，不用用户选项覆盖。抽签仍只接受配置里的方向。结束时刻不开放。开放是开始 <= 现在且结束 > 现在：开始为 0、结束在将来时仍可写，(0, 0) 不写。页面只在不能归入未开始、已结束或进行中时写「活动时间未知」；(0, 将来) 且已开放时显示进行中。真实消耗未知 | 代码已有。022 独立实跑过，审计未接受。022A 补了领取合同、同账号串行和官方向量。窗口未打开。真实接口未发。是否仍开放未知 |
| 千星 `stellar` | `StellarActivityPage` 四段：游记等级、进度、逐级奖励和一键领取；星砂全部商品、分类、已拥有和可调数量；二十八星宿列表及已亮、可点、未开、待同步，另有今日已领和真正 rules；节令是 `SolarTermsService.GetSolarTerms` / `ClaimSolarTerms`，只在 `status==2` 时带 `term_id`。没有免费/付费分档，也没有买通行证按钮。商店 status 恒不当售罄。点星 `Operate` 21 在活动存在时就发。`1034038` 只在现在和开始都大于 0、现在不早于开始、并且结束小于等于 0 或现在不超过结束时，且日序为 1 到 28，才吞掉。已确认的 field 2/3 单调合并，文件键是 `activity-center-state-{sha256(账号)}.json` 里的 `赛季:活动:v目录版本` | `Shelf::Stellar` 页内列出通行证节点、全部商品和数量、星宿视觉态与 rules、V1 节令。`press` 经确认后由 `dispatch_stellar` 进入当前 worker。成功读包缺货币记已知 0，失败才未知；兑换成功后重读背包。`1034038` 显示「今日已领取」，不把该错误画成已点亮。已确认进度按账号和活动写入上述文件，测试只用临时目录。写回没有顶层 season 时用 `snapshot` 刷新；`refreshRequired` 保留结果并停用旧的可领、可点、可兑，刷新本身不再发写。V2 节令架保留，不再拿它代替 V1。没有付费购买入口。固定基准没有千星官方 hex | `ec905c8` 定向测试过，审计不接受缩水界面和 V2 替代。023A 接受列表呈现。023B 独立离线通过。窗口未打开。真实接口未发。是否仍开放未知。真实消耗未知 |
| 七夕 `qixi` | `QixiActivityPage` / `QixiActivityView`。`GetGroup` 组 `2026081800`，鹊桥 `2026081801`，香囊 `2026081802`。领鹊桥 `Operate` 25，`params.step` 固定 0。送香囊 `Operate` 26，`target_gid` 加默认文案号 15，协议没有数量字段，页面写明每次 1 个。窗口是开始为 0 或已到，并且结束为 0 或含结束时刻；(0, 0) 仍开放。活动号里的日期不拿来排除。全部阶段、消耗、奖励、四种资源、已赠上限和 rules 都在页面上。没有确认框 | `Shelf::Qixi`。`press` 交给 `dispatch_qixi`，再进当前 worker 的 `QixiService`。阶段全部列出。好友可搜索，未选好友不发。香囊余额已知为 0 不发，读包失败不当成免费。成功后重读；刷新失败保留奖励并停用旧按钮。空账号不发。固定基准没有七夕官方 hex | 代码已有，待独立复验。窗口未打开。真实接口未发。是否仍开放未知。真实香囊消耗未知 |
| 青梅 `qingmei` | `QingMeiActivityPage`。`qingmei.ts` 领取种子、开始、继续、结算是 `Operate` 4、14、15、16。`getCurrentQingMeiActivity` 在 `2026081200` 及每日、酿造窗口都不活跃时返回空，不继续查询。这四个写函数本身没有再看窗口 | 仅生成类型 | 缺口。页面是否把按钮关掉，这次没逐个控件核对。是否仍开放未知 |
| 公益 `charity` | `CharityActivityPage`。`charity.ts` 用 `ActivityService.List` 取窗口，领种子、捐爱心、领进度、领每日礼是 `Operate` 35、36、37、38。`active` 写在展示和可点标记上；写函数这次没看到单独的时间拒绝 | 仅生成类型 | 缺口。是否仍开放未知 |
| 萌宠日记 `pet` | `PetDiaryPage`。`pet-diary.ts` 组 `2026090100`，`GetGroup` 后 `Operate`：27 初始化、29 喂养、30 寻宝、32 故事、41 刷新、42 装备、43 战斗、45 开宝、46 补偿、48 领犬、49 标记、50 跳过、21 种子、1 兑换。写之前 `isActive` 看头部开始和结束。节令领取复用 `ClaimSolarTerms` | `ranch_activities.rs` 只改目录名。`pets.rs` 有同名图鉴说明，不是这页 | 缺口。是否仍开放未知 |

## 调度、通知、日志与配置

| ID | 行为 | 代码对照 | mock | 窗口 | 真实接口 |
| --- | --- | --- | --- | --- | --- |
| SCHED-01 | 心跳、掉线后不再用旧码重连，微信可走应用宝授权 | 代码已有。bot worker 重连。Rust `worker.rs` 心跳超时 `force_disconnect_with_reason("heartbeat_timeout")`，并区分应用宝授权重连和不再使用旧 Code | 无 | 未验 | 未验 |
| SCHED-03 | 大量好友时先巡农场和好友，再跑牧场 | 代码已有。`c500e08` 的 `runUnifiedTick` 先等农场巡查，再启动好友巡查；牧场要等这两项都完成过首轮，并且当前都不在跑、也没有再次到期。网关不健康时牧场单独走 30/60 秒退避，不改农场和好友的退避；网关不空闲则牧场再等 1 秒。好友总开关关闭仍算首轮结束，避免牧场一直等。Rust `run_unified_tick` 按这个顺序调用 `run_farm_tick`、`run_friend_tick` 和 `run_ranch_after_farm_and_friends`。分享、任务和每日福利仍走原来的定时器，不再从那里插入牧场巡查 | `unified_tick_runs_farm_and_friend_before_ranch`、`ranch_waits_for_the_first_friend_tick_and_a_healthy_gateway` 走 `run_unified_tick`。`patrol_ticks_wait_out_a_silent_gateway_and_resume_separately` 仍通过。没有真实巡查 | 未验 | 未验 |
| SCHED-02 | 网关心跳漏拍或在途请求卡住时，推迟农场和好友巡查 | 代码已有。bot `worker.ts` `nextBusinessTickDeferMs`：健康则清退避，否则首次 30 秒、之后翻倍、封顶 60 秒，农场和好友分开记。Rust `run_farm_tick` / `run_friend_tick` 经 `defer_unhealthy_tick` 调用 `step_business_tick`。第一次退避和恢复各写一条面板日志 | `patrol_ticks_wait_out_a_silent_gateway_and_resume_separately` 用离线 Gateway，把最近入站时间拨过心跳静默，再直接调用两次巡查。无回包时农场不进入正常 20–25 秒排期，好友不出现「巡查异常」；两种巡查各自先 30 秒、再封顶 60 秒，状态互不覆盖；恢复后农场回到正常间隔，好友发出巡查并记「巡查异常」，各自退避清零；首条退避和恢复日志不重复。摘掉 `defer_unhealthy_tick` 接线时农场退避停在 0，该测试失败。`business_backoff_first_30s_then_double_capped_60s` 仍只覆盖计算。农场 AllLands 帧没有单独计数，农场「未发请求」由函数在正常排期前返回推出。在途请求卡住满 5 秒的那条健康度条件没有单独拨时钟 | 未验 | 未验 |
| LOG-01 | 运行日志和今日统计 | 代码已有。bot `/api/logs`、`stats.ts`。NSH `dashboard.rs` | 无 | 未验 | 未验 |
| REMIND-01 | 下线提醒 | 代码已有。bot `push.ts`。NSH `offline_reminder.rs`、`qq_bot_bind` | 无 | 未验 | 未验 |
| COLL-01 | 图鉴和藏品 | 代码已有。bot `/api/illustrated`、`/api/collections`。NSH `illustrated.rs`、`collections.rs` | 无 | 未验 | 未验 |
| COLL-02 | 图鉴全部已读 | 代码已有。`c500e08` 的 `clearIllustratedNew` 把正数 `fruitId` 发给 `ClearNewUnlockedFruitsV2`。Rust `IllustratedService::clear_new` 同样丢掉非正数，空名单不发请求；NSH 图鉴页有「全部已读」，只提交当前页 `isNew` 且果实号为正的条目 | `clear_new_keeps_only_positive_ids_and_reads_the_reply`、`clear_new_with_no_ids_does_not_ask_the_gateway`、`read_all_uses_only_positive_fresh_fruit_ids`。没有真实回包，没有窗口点击 | 未验 | 未验 |
| CONF-01 | 游戏配置编辑 | 代码已有。bot `ConfigManage.vue`、`/api/config/seeds`。`qq-farm-app::config`。NSH `game_config.rs` | 无 | 未验 | 未验 |

本组 8 项：代码已有 8。

## 基线计数

基线 35 项。代码已有 33，未实现 1（ACT-02），桌面差异 1（AUTH-05），范围待核实 0。窗口通过 0，真实接口通过 0。没有「验证通过」项。33 项代码已有不是 33 项验收。空账号总览曾打开过，那不是功能验收；导航和其他页面没有窗口证据。`54fda84` 上的 1203 项测试不是本基准的全量回归。现有 debug 客户端仍是那次构建，没有按后面的提交重做。源码工具链文件仍是 1.98.1；1086 项 core 测试用的是已安装的 1.99.1。

SCHED-03 是农场和好友巡查先于牧场。TASK-03 是成长任务和牧场好友面板的客户端上报。两者不是同一件事。

其余 34 项这次对照矩阵里已写的入口、服务和保存路径，没有新跑测试，也没有补功能。没有发现新的用户入口缺口。AUTH-05 继续是桌面本机直接进入。EXTRA-01 继续不占基线。

接受层级停在这里：33 项是代码已有，0 项窗口通过，0 项真实接口通过。mock 列点了测试名的，只说明那次离线测试曾经写过或跑过，不是本批重跑，也不是窗口验收。mock 为「无」的代码项是 AUTH-01、AUTH-02、AUTH-04、FARM-02、FARM-04、FRIEND-01、FRIEND-02、FRIEND-05、RANCH-01、BAG-01、BAG-02、ACT-01、SCHED-01、LOG-01、REMIND-01、COLL-01、CONF-01。这 17 行是证据欠账，矩阵没有列出测试名；不表示项目里完全没有相关测试，也不表示本批重新验收过。SCHED-02 在途请求卡住满 5 秒那条仍没有单独拨时钟，留到后面补。

## 非基线

| ID | 来由 | 结论 |
| --- | --- | --- |
| EXTRA-01 | 早前进度把「天气任务领取」写成还没做 | 不占基线。bot `weather-routes.ts` 没有领取路由；`WeatherActivityView.vue` 只渲染任务进度；`weather-activity.ts` 不发送领取；`activitypb.proto` 写明 `WeatherTaskOperateRequest` 是采集 field 107 的兼容别名，没有单独的领取 `operate_type`。不另造协议 |
