# 历史踩坑全录（每条都真实发生过）

按"再犯概率"排序。改相关代码前扫一眼对应条目。

## 1. 图片显示不出来（犯过 3 次：ee9cc2c / f56a6ff / 2026-09-10 热修）

**机制**：游戏配置图（作物/道具 webp）由后端 `mapped_item_image()` 产出 `/game-config/...` 相对路径，实际文件在 `assets/game_config/`，由 Tauri 自定义协议 `farmcfg://` 服务。该协议在不同平台/页面 origin 下的可用 URL 形式不同：

| 运行环境 | 页面 origin | 唯一可用形式 |
|---|---|---|
| Windows 安装版 | `http://tauri.localhost` | `http://farmcfg.localhost/<rel>`（`farmcfg://localhost/` 也可） |
| Windows `cargo tauri dev` | `http://127.0.0.1:<随机>`（tauri-cli serve dist） | **仅** `http://farmcfg.localhost/<rel>` |
| macOS（WKWebView） | `tauri://localhost` | **仅** `farmcfg://localhost/<rel>` |

三次事故：
1. `ee9cc2c`：Windows 安装包图片加载回归（协议形式/platform 差异）；
2. `f56a6ff`：跨平台窗口和图片兼容；
3. 2026-09-10：两层叠加——(a) `pet-diary-view.vue` 绕过 `resolveCatalogImage` 直接 `:src="item.image"`（且第一版参数传错：该函数收 **path** 不收 item id）；(b) dev 实例 origin 是 `127.0.0.1`，`farmcfg://` 原始形式全挂，连游戏配置页一起挂。

**规则**：所有需要转换的图片一律 `resolveCatalogImage(item.image)`，`shared.ts` 内已做平台分支，**任何人不要再手拼协议 URL**。裸 `:src="item.image"` 禁止。`/activity-assets/...`（dist 静态文件）、`http(s)://`、`data:` 不需要转换。后端 `assets.rs` 同时支持 `farmcfg://` 与 `http://farmcfg.localhost/` 两种请求形式（有单测）。

## 2. 擅改 bot 镜像文件（proto 等）——2026-09-11 施肥事故，用户明令禁止

**事故**：修「普通+有机模式下有机肥恒为 0」时，擅自给 `proto/plantpb.proto` 的 `left_inorc_fert_times`（field 17）加 `optional`，理由是"prost 需要 presence、wire 兼容、只影响解码"。技术论证成立但**违反铁律**：bot 镜像文件必须与 bot 逐字符一致，最终被打回重做，修复改为纯代码侧（官方向量证实服务端从不显式发 0，`Object.hasOwn` 的 ≤0 分支在真实报文上不可达，直接不过滤该字段即 wire 等价）。

**规则**：`proto/`、`assets/activity-data/*.json` 等一切从 bot 拷来的文件**只整份拷贝、零修改**——`optional`、注释、格式都不行，"看起来无害"也不行。prost/serde 表达不了 bot 语义时，在 rust 代码层近似，注释写明依据（官方向量：`core/tests/fixtures/`、抓包测试 `farm-fertilize-proto.test.js`）。想动镜像文件 = 先停下来问用户。

## 3. tokio::sync::Mutex 重入死锁（2026-09-10，pet solar）

`operate_pet_diary` 拿了 `mutation_lock` 后分流调用 `claim_pet_diary_solar_term`，后者也拿同一把锁 → 同 task 二次 lock 永久等待。tokio Mutex **不可重入**。

**规则**：公开写方法拿锁；锁内只能调不拿锁的 `_inner`/私有方法；"动作分流"必须在拿锁**之前**完成（bot 的 `serializeMutation` 外分流同理）。写完自查：新方法是否可能被另一个持锁方法调用。

## 4. IPC 命令漏 ACL 注册（SYNC.md 2026-09-02 有前科）

`generate_handler![]` 注册了但 `permissions/desktop.toml` 没加 → 运行时被 ACL 拦截，操作全部失败。现有防回归测试 `every_handler_command_is_allowed_by_acl` 会在 `cargo test -p qq-farm-desktop` 时抓到，但正确做法是写的时候就两处一起改。

## 5. 活动目录状态与实际不一致（2026-09-10，雨落成诗）

目录条目状态由起止时间判定，而 weather 活动没有 `directory.rs` 绑定，前端兜底入口硬编码 `startTime: 0, endTime: 0` → 状态判定把"时间 0"当作进行中；页面内部却用活动快照判定已结束——两处数据源不一致，用户看到"目录进行中、点进去已结束"。

**规则**：新活动必须在 `directory.rs` 注册 gameplay 绑定（对齐 bot registry 的静态 ID + priority），目录条目继承服务端 List 窗口真实时间；前端兜底入口的时间也必须来自真实快照（参照 `index.vue` 的 `weatherActivity`），不许硬编码 0。

## 6. prost / json! 相关编译期陷阱

- proto 字段名 `type` → Rust `r#type`；`count` 等正常。**先看生成代码再猜字段名**（`target/debug/build` 下或查现有用法）。
- proto `int32` → Rust `i32`，与配置里的 `i64` 比较前先 `i64::from(...)`。
- `serde_json::json!({...})` 宏体内**不能写方法链**（`[1,2].iter().map(...)` 之类报 "no rules expected this token"）：先在宏外算成变量，再放进宏。
- int64 序列化口径对齐 bot：bot `str()` 的输出字符串（大数值/ID），`num()` 的输出数字（计数/毫秒时间戳）。照 bot 逐字段抄，别统一。

## 7. 桌面端运行环境坑

- `cargo tauri dev` 的 `beforeDevCommand`（pnpm build）**只跑一次**：改前端后必须手动 `corepack pnpm -C desktop-ui build` + 重启实例，否则看到的是旧产物。
- pnpm 命令损坏（shim 报 "not recognized"）时用 `corepack pnpm ...`；`corepack enable` 可修 shim。node_modules 被进程锁住时 pnpm install 报 os error 5，先关占用进程再装。
- dev 实例读 `data/` 下真实账号并自动登录；与安装版同账号双开会顶号/互踢重连。清理进程用 `taskkill /IM qq-farm-desktop.exe` 会**连安装版一起杀**（同名进程），要么接受要么按 PID 杀。
- dev 弹"发现新版本 0.2.x"更新框是正常的（线上 release 版本号比本地新），点「稍后」。
- 自动化工具（CUA）对 WebView2 内的 Vue 菜单点击不可靠（AXPress/坐标都可能无效）：UI 自动验证走不通时改用日志 + curl 1430 静态资源 + dist 产物 grep 佐证，最终 UI 效果让用户确认。

## 8. 长内容写入 shell heredoc 截断

Git Bash 里长 heredoc（几百行含特殊字符）会静默截断（"warning: here-document delimited by end-of-file"）。**规则**：写长文件用 Write 工具；批量文本变换写临时 python 脚本文件执行后删除，不要内联超长 heredoc。写入后必须校验文件尾部完整性。

## 9. 其他确认过的事实（避免重新踩）

- 测试断言失败先检查测试数据本身：成本合并测试 `300×2 ≤ 700` 本就该可用，写反断言会浪费一轮。
- `cargo check` 通过 ≠ 无警告：严格门槛是 `RUSTFLAGS="-D warnings"`；测试才编译的 `#[cfg(test)]` 专用 import 要放测试模块内（lib 编译会报 unused）。
- bot `network.ts` 的 sendTail 串行化被 bot 自己回退了；rust 网关本就账号内串行，不需要跟进。
- charity 的每日礼包静默领取（`openCharitySettlementGiftPacksSilently`）**有意不移植**（固定道具 id 当前游戏不存在，SYNC.md 有记录）——别"顺手补齐"。
- MeoW 推送渠道不对齐（用户决策 2026-09-01），同理。
