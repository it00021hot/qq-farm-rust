# Architecture — qq-farm-rust

只维护桌面版：Tauri 和 GPUI Kit 是两个并行桌面宿主，共用进程内引擎与 `qq-farm-app` 门面（原 HTTP API 服务 crate `qq-farm-server` 已删除）。

## Crate 拓扑

```text
desktop-ui (SoybeanAdmin) ──Tauri IPC──► qq-farm-desktop ──┐
                                                         ├──► qq-farm-app ──► qq-farm-core
GPUI Kit ─────────────────────────────────► qq-farm-gpui ──┘
```

- `qq-farm-core`：零 UI、零进程入口的库。
- `qq-farm-app`：UI 无关门面（Account / Farm / Friend / Commerce / Settings / Config / Admin + `AppEvent` + `bootstrap`）。**禁止**依赖 `tauri`。
- `qq-farm-desktop`：Tauri v2 适配层；进程内嵌引擎；前端为仓库根 `desktop-ui/`。
- `qq-farm-gpui`：GPUI Kit 原生桌面适配层；页面按业务能力组织，通过 app 门面操作真实引擎，不依赖 Tauri IPC。
- `qq-farm-app::runtime_lease::EngineLease`：两个宿主启动时锁住同一数据目录的 `engine.lock`，由进程持有到退出，避免重复 worker 和并发写入。

### GPUI crate 内部结构

快速定位页面业务代码与交互测试，可先看 [`qq-farm-gpui/README.md`](../crates/qq-farm-gpui/README.md)；下面保留完整模块拓扑和职责细节。

首轮仍只有一个客户端 crate，内部按业务能力组织。业务目录同时拥有自己的状态、输入订阅、视图和操作；窗口协调层负责组装、导航和跨页面请求版本。

```text
qq-farm-gpui/src/
├── main.rs                                          # 环境、共享锁、app 启停入口
├── application/{mod,commands}.rs                    # 主窗口、托盘恢复、菜单与平台快捷键
├── theme.rs / i18n.rs                                # 产品浅深主题、共用中英文文案
├── assets/{local,data_url,tests}.rs                  # 游戏资源、内联图片适配
├── runtime.rs / tray.rs / update.rs                  # 环境与偏好、托盘、发布产物检查
├── pages/                                           # 页面模型、快照加载、显示行映射
│   ├── loader/personal.rs                           # 各农场能力独立成功/错误结果
│   ├── mapper/{dashboard,personal,friends,analytics,commerce,settings,game_config,accounts}.rs
│   │   └── friends/visitors.rs                     # 访客记录身份、动作和详情映射
│   └── model_tests.rs                               # 页面模型与文案映射测试
└── desktop/
    ├── mod.rs / init.rs / state.rs / land_grid.rs    # 模块组合、窗口初始化、共享状态、好友/本人共用地块几何
    ├── navigation/                                 # 页面切换、业务分段切换、账号切换、界面偏好
    │   ├── page.rs / section.rs                     # 页面路由与分段路由
    │   ├── account.rs / preferences.rs              # 账号上下文与界面偏好
    │   └── sections.rs                              # 页面分段目录
    ├── shell.rs / commands.rs                       # 布局组合、按当前表单保存
    ├── runtime/{events,loading,updates,window,shutdown}.rs
    │   └── events/{subscription,account_filter}.rs    # 总线订阅与账号事件隔离
    │                                                # 事件桥、快照、窗口尺寸/外观观察
    ├── view/                                        # 侧栏、内容骨架、共用控件与图片展示
    │   ├── grid.rs                                  # 响应窗口宽度的网格列与行分组
    │   ├── pagination.rs                            # 对齐 Naive UI pageSlot 的共享页码窗
    │   ├── content/{filter,rows,toolbar,state,personal,commerce}.rs
    │   └── rows/{dashboard,personal,friends,commerce,settings,game_config}.rs
    │       └── friends/{avatar,visitors}.rs         # 好友头像兜底、访客卡片及相对时间
    ├── accounts/                                    # 账号筛选、表格、表单、授权和删除
    │   ├── state.rs / lifecycle.rs / sidebar.rs     # 输入与筛选状态、独立启停、账号切换器
    │   ├── auth_status.rs / search.rs                # 授权状态及与 Vue 一致的账号搜索字段
    │   ├── columns.rs / pagination.rs              # 列配置、分页选择
    │   ├── table.rs + table/{header,pinned,rows,cells}.rs # 表格组合、固定列、表头、账号行和单元格
    │   ├── filters.rs / search_selectors.rs / list.rs / form.rs / delete_view.rs
    │   │                                             # Vue 表单草稿、三类单选条件与列表筛选
    │   ├── messages.rs / sheet.rs                   # 账号状态文案与添加/编辑流程
    │   ├── wx_login/{local,qr}.rs
    │   └── sheet/{code_login,wx_login}.rs
    ├── dashboard/                                   # 状态、资产、运行计划、日志筛选
    │   ├── state.rs                                 # 总览状态与输入订阅
    │   ├── clock.rs                                 # 在线状态计时与定时刷新调度
    │   └── refresh.rs                               # 状态及资产异步刷新、过期请求校验
    ├── personal/                                    # 农场页面能力组合
    │   ├── state.rs / status.rs                     # 组合子状态、无账号/离线/分段失败
    │   ├── daily_gifts/{mod,labels}.rs / farming.rs  # 只读礼包/任务、务农门控与操作
    │   ├── lands/{state,summary,clock}.rs             # 地块布局算法由 desktop/land_grid.rs 共享
    │   │   └── card/{mod,crop,effects,labels}.rs     # 地块组合、作物、互动效果与状态徽标
    │   ├── bag/{state,helpers,controls,card,actions,feedback,clock}.rs
    │   │                                            # 大型仓库卡片网格使用共享虚拟网格
    │   ├── interaction_items/{state,picker,view,actions}.rs
    │   ├── illustrated/{state,summary,card}.rs
    │   └── pets/{state,view,actions,food,logs,refresh}.rs
    ├── friends/                                     # 好友列表、详情、互动、黑名单、确认
    │   ├── state.rs / controls.rs / loading.rs         # 好友输入、按分段刷新、分页/筛选
    │   ├── helpers/{search,status,snapshot}.rs         # 搜索排序、状态文案、操作快照
    │   ├── realtime.rs                               # 好友地块事件、偷菜/访客日志与合并刷新
    │   ├── row_actions.rs / confirmations.rs         # 好友操作、访客操作和确认
    │   ├── actions.rs / interaction.rs / blacklist.rs / delete.rs
    │   │                                             # 单项/批量操作及结果更新
    │   └── detail.rs / overlays.rs + overlays/{career,lands}.rs # 好友农场、地块详情与生涯统计
    ├── activities.rs                                 # 活动中心：现役活动目录入口
    ├── solar_calendar/{mod,state,view}.rs            # 节令日历：季度/节令列表、领取与答题
    ├── milk_tea/{mod,state,view}.rs                  # 奶茶活动：店铺/工坊/雇佣/工作等分段与操作
    ├── share_reward/{mod,state,view}.rs              # 快乐不独享：分享、档位奖励与互动日志
    ├── analytics/                                   # 策略对比、排名与种植黑名单
    │   ├── state.rs / blacklist.rs / preferred_seed.rs  # 分析状态、黑名单保存与种子偏好操作
│   └── view/{blacklist.rs,strategy/{mod,metrics,plant,comparison}.rs}
    ├── commerce/                                    # 普通/会员/神秘商城、数量与购买确认
    │   └── state.rs / purchase.rs / confirmation.rs + shop/{mall,mystery,card}.rs
    │       # 状态持有按账号隔离的普通/SVIP目录缓存；离开商城页时清空
    ├── settings/                                    # 草稿、种植策略、自动化、提醒和集成
    │   ├── state.rs / draft.rs / save.rs               # 分组草稿与策略/自动化保存
    │   ├── system_config/{mod,view}.rs                 # 运行环境字段写入/重置与独立表单视图
    │   ├── controls/planting.rs / controls/planting/bag_seed_priority.rs
    │   └── controls/automation/{basic,fertilizer,friends,mystery_shop}.rs
    ├── game_config/                                 # 搜索、筛选、目录表格、配置编辑和图片上传
    │   ├── state.rs / search.rs / search_panel.rs / selectors.rs
    │   │                                             # 草稿提交面板、单选筛选器与 Vue 搜索范围
    │   ├── labels.rs                                 # 字段双语文案
    │   ├── columns.rs / column_settings.rs           # 分类别列定义、显隐/重排/固定设置
    │   ├── table.rs + table/{header,rows,cells,selection,shared}.rs # 表格骨架、表头、行、选择和格式化
    │   ├── table/{seeds,fruits,items}.rs             # 三类配置各自的字段单元格
    │   ├── delete.rs / delete_view.rs / row_actions.rs # 删除操作、确认弹层与行操作
    │   └── form_view/{seed,fruit,item,common}.rs
    ├── preview/{mod,scenes,fixtures}.rs              # 可选原生字体/GPU 预览，隔离展示数据
└── tests/                                       # 按业务页及页内工作流维护 GPUI 交互回归
        ├── accounts/{navigation,filters,search,actions,presentation,table}.rs
        ├── commerce/{mall,purchase,mystery}.rs
        ├── friends/{presentation,operations,loading,realtime,visitors}.rs
        ├── personal/{daily_gifts,farm,warehouse,interaction_items,illustrated,pets}.rs
        ├── settings/{navigation,offline_reminder,system_config,automation,planting,qq_bot}.rs
        ├── assets.rs                                # 配置图片上传与 Vue 资源 URL 读回
        ├── activities.rs / milk_tea.rs / share_reward.rs / solar_calendar.rs # 活动中心与活动页窗口回归
        ├── game_config/{catalog/{search,table,deletion,lifecycle},forms,persistence}.rs
        └── support.rs                               # 隔离数据目录、runtime 和测试窗口
```

Rust 业务模块的入口文件与同名目录是同一个模块的两层：例如 `desktop/friends.rs` 只声明和导出模块，视图与操作实现在 `desktop/friends/`；`desktop/settings.rs`、`game_config.rs` 也采用相同结构。`desktop/view/content.rs` 负责组装页面内容，具体页面区块在 `desktop/view/content/` 和各业务目录中实现。阅读时应展开业务目录，入口文件本身不代表该业务的全部实现。

`desktop/state.rs` 仅组合业务状态、运行时、窗口句柄与当前页面；不再集中保存各业务输入控件的创建逻辑。农场子能力分别持有状态，例如 `farm.bag.selected_ids`、`farm.pets.food_inputs`，互不复用等待标记。输入订阅随业务状态初始化，窗口尺寸与系统外观由 `desktop/runtime/window.rs` 观察，渲染不写偏好文件。异步任务在 Tokio 执行，结果通过 GPUI 实体更新回到主线程；页面和账号切换使旧请求失效。

资源适配保持后端图片协议：`/game-config/` 指向 `assets/game_config/`，`/land-assets/` 指向 `desktop-ui/public/land-assets/`（包内 `land-assets/`）；上传生成的 `/api/config/images/{name}` 由 `qq-farm-app` 从共享游戏配置覆盖目录读回。外链交由 GPUI 资源系统加载，`data:` 转为带格式的图片字节；缺文件、无效数据或远端加载失败统一显示缺图状态。包内与源码目录的两个 TSDK WASM 路径均有解析覆盖。

GPUI 进程在 UI 退出后继续持有 `EngineLease`，完成 app 停机并关闭 Tokio runtime 后释放；菜单、托盘及无托盘关窗共用退出请求。app 的 `lifecycle::shutdown` 调用 core 停机：关闭会话启动入口，取消工作任务、重连和事件/微信保活循环，等待工作任务结束，超时则中止并回收。Tauri 的最终退出也使用该门面。账号编辑使用 GPUI Kit Sheet 管理焦点，侧栏使用 ResizablePanel 管理拖动和约束；菜单、按钮与快捷键共用 Actions 和 `desktop/commands.rs` 的分发方法，窗口级后备处理兼容弹层的焦点树。保存按当前账号/配置表单或设置分段分发。macOS 关窗到托盘使用应用隐藏，Dock 再次打开、单实例唤起和托盘恢复共用激活主窗口的入口。

可选 `ui-preview` 功能提供隔离数据目录的原生字体/Metal 像素预览，展示快照与在线验收分开记录。每个场景的预览偏好显式写入浅色/深色模式，避免系统外观观察覆盖预览主题。产品使用 Kit `AllAssets` 提供完整导航与业务图标目录。

代码结构和自动测试不代表完整业务等价。逐页验收进度、真实后端与实机待验项目统一记录在 `docs/SYNC.md`。

## 桌面嵌入模式（默认）

**产品定位**：个人免费开源客户端；**无登录、无用户/权限管理、无卡密**；只处理多农场账号业务。ACL 固定 `AclPolicy::LocalOwner`。

```text
开窗即进 /home（无登录门闸）
desktop-ui 农场页（10 项侧栏）
        │ invoke
        ▼
qq-farm-desktop（LocalOwner）
        ▼
qq-farm-app → core（多农场账号）
```

```text
Tauri App (qq-farm-desktop)                  GPUI App (qq-farm-gpui)
  └─ DesktopState / commands   (IPC 适配)        └─ GPUI 页面 / Tokio 运行时
                          └───────┬───────────────┘
                                  ▼
                       AppContext / Facades (qq-farm-app)
                                  ▼
                       RuntimeEngine + stores (qq-farm-core)
```

两端均不走 localhost HTTP / Socket.IO。Tauri 实时推送由 `subscribe_events` → `emit("app-event")`；GPUI 通过异步事件桥订阅同一 app 事件总线，事件积压时刷新当前页面快照。

壳层对齐原 Wails 桌面端：macOS 原生菜单、全平台托盘、关窗进托盘、GitHub Releases 自动更新。安装包内嵌 `tsdk.wasm` / `game_config`。无「在浏览器中打开」。发版见 [RELEASE_CHECKLIST.md](./RELEASE_CHECKLIST.md)。

侧栏包含总览、我的农场、好友、活动中心、收益分析、商城、神秘商店、设置、游戏配置和账号；活动中心内展示节令日历、奶茶活动、快乐不独享等现役活动。商城保留普通、SVIP、神秘商人三个标签；独立神秘商店入口与商城标签共用同一数据加载、商品展示及购买流程，旧版选中 `mystery-shop` 的偏好仍可直接恢复该页面。

展开侧栏时直接列出可切换的农场账号，并提供账号管理、新增账号和底部新增入口；窄侧栏收起账号列表并保留导航和快速新增。账号行的选择会走统一 `set_account` 生命周期，页面请求按新账号刷新。

## 事件流

```text
core runtime / panel_log
        │
        ▼
   AppEvent 总线 (qq-farm-app)
        │
        ▼
Tauri emit("app-event")（desktop-ui listen）
```

## 分层（core）

```text
constants → config → models → proto → network / crypto → runtime → infra → services(domain)
```
