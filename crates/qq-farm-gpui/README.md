# qq-farm-gpui

GPUI Kit 桌面客户端。业务页面和页面状态按业务域放在 `src/desktop/` 下；不要把新增页面的操作、表单状态或交互塞进公共 shell 文件。

## 业务模块

| 页面能力 | 业务代码 | 交互回归 |
|---|---|---|
| 总览 | `src/desktop/dashboard/` | `src/desktop/tests/dashboard.rs` |
| 我的农场 | `src/desktop/personal/` | `src/desktop/tests/personal/` |
| 好友 | `src/desktop/friends/` | `src/desktop/tests/friends/{presentation,operations,loading,realtime,visitors}.rs` |
| 活动 | `src/desktop/activities/` | `src/desktop/tests/activities/` |
| 收益分析 | `src/desktop/analytics/` | `src/desktop/tests/analytics.rs` |
| 商城 | `src/desktop/commerce/` | `src/desktop/tests/commerce/{mall,purchase,star_sand,mystery}.rs` |
| 神秘商店 | `src/desktop/commerce/`（复用商品与购买组件） | `src/desktop/tests/commerce/mystery.rs` |
| 设置 | `src/desktop/settings/` | `src/desktop/tests/settings/` |
| 游戏配置 | `src/desktop/game_config/` | `src/desktop/tests/game_config/catalog/{search,table,deletion,lifecycle}.rs`、`forms.rs`、`persistence.rs` |
| 账号 | `src/desktop/accounts/` | `src/desktop/tests/accounts/` |

每个业务目录负责自己的状态、页面区域、输入和操作；`src/desktop/view/` 只放侧栏、页面骨架、通用布局和确实跨业务复用的展示原语。`src/pages/mapper/` 负责将 app 快照映射为页面模型，`src/desktop/runtime/` 负责事件订阅、快照刷新和异步结果回传。

业务目录内部继续按工作流拆分。例如总览计时和刷新分别位于 `desktop/dashboard/{clock,refresh}.rs`；好友数据规则位于 `desktop/friends/helpers/{search,status,snapshot}.rs`，访客和互动道具各自在 `desktop/friends/loading.rs` 处理独立刷新；天气活动好友区拆成 `desktop/activities/weather/friends/{model,list,detail}.rs`，目录文件只负责组合宽屏/窄屏布局。看到根目录的 `friends.rs` 或 `activities.rs` 时，它们只是 Rust 模块入口，完整实现位于同名目录中。

如果 `git status --short` 只显示 `?? crates/qq-farm-gpui/`，这是 Git 对未跟踪目录的折叠显示，不表示源码只有一个文件；`git diff --stat` 默认也不统计未跟踪文件。使用 `git status --short --untracked-files=all -- crates/qq-farm-gpui/src` 可展开模块清单。

交互回归也按页面内的工作流拆分：好友的展示、操作、加载、实时事件和访客记录分别维护；商城的普通商城、购买确认、星砂商店和神秘商店分别维护；游戏配置目录将搜索、表格、删除和页面生命周期分开。测试只验证可见行为，不承载业务实现。

页面组装从 `src/desktop/view/content.rs` 进入，外层窗口与共享状态分别在 `src/desktop/shell.rs`、`src/desktop/state.rs`。这些入口负责分发和组合，具体业务行为仍在对应业务目录。

`src/desktop/runtime/loading.rs` 只负责页面请求、代次校验和结果分发。好友/农场快照合并、账号选择回收、设置草稿同步、活动默认选项及总览资产基线等刷新后规则，分别位于对应业务目录的 `refresh.rs`；不要在全局刷新器里继续添加某一页的字段或门控逻辑。

侧栏账号列表及添加/管理入口位于 `src/desktop/accounts/sidebar.rs`；账号切换仍通过统一导航生命周期执行，不维护第二份选择器状态。

## 预览与视觉回归

视觉基线由 `ui-preview` feature 下的场景矩阵维护（`src/desktop/preview/scenes.rs` 定义场景，`fixtures.rs` 提供隔离快照；当前 214 个场景，覆盖逐页浅/深/英文、最小窗口 1100×720、125%/150% 字体缩放，以及空态、错误态、弹层/抽屉、多选、下拉展开等分支）。在仓库根目录执行：

```sh
QQ_FARM_ASSET_DIR=$(pwd) cargo run -p qq-farm-gpui --features ui-preview -- --render-preview /tmp/qq-farm-gpui-preview
```

渲染使用隔离临时数据目录，逐场景输出 PNG；逐页状态与场景的对应关系见 `docs/GPUI_PARITY_CHECKLIST.md` 的状态映射附录。新增页面或状态时在 `scenes.rs` 增补场景即可，覆盖率回归 `preview_scenes_cover_every_visible_page_section_in_all_conditions` 会强制每个页签分段都具备浅/深/英文截图。

## 开发与验证

```sh
cargo run -p qq-farm-gpui
cargo test -p qq-farm-gpui --features ui-preview -- --test-threads=1
cargo clippy -p qq-farm-gpui --features ui-preview --all-targets --no-deps -- -D warnings
```

跨平台验收和未完成的实机检查记录在仓库的 [`docs/SYNC.md`](../../docs/SYNC.md)；逐页行为对照项见 [`docs/GPUI_PARITY_CHECKLIST.md`](../../docs/GPUI_PARITY_CHECKLIST.md)，完整 crate 拓扑见 [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md)。
