# 官方资源同步

本文说明官方小程序升级后，如何取得游戏配置和图片、如何从
`qq-farm-bot` 同步到 `qq-farm-rust`，以及协议抓包目录与资源目录的区别。

## 两种路径不是一回事

- 资源下载工具的 `--source`：**小程序解包/反编译后的源码根目录**，其下必须有
  `src/settings.json` 或 `src/settings.<hash>.json`。
- 协议分析工具的 `<capture-dir>`：WebSocket recorder 输出的 `*.bin` 目录，每个
  文件是一帧完整的 `gatepb.Message`。

配置和图片来自小程序 CDN，不需要先抓 WebSocket。`--source` 只用于读取 CDN 地址、
remote bundle 列表和 bundle 版本。

## 1. 获取小程序包

QQ 农场微信小程序 AppID 为：

```text
wx5306c5978fdb76e4
```

先在桌面微信中打开 QQ 农场并等待资源加载完成，再关闭小程序。微信 4.1.x 常见缓存
位置如下；`<hash>` 是当前微信账号的本地用户目录，`<版本>` 通常是数字目录。

### Windows

当前多用户目录：

```text
C:\Users\<用户>\AppData\Roaming\Tencent\xwechat\radium\users\<hash>\applet\packages\wx5306c5978fdb76e4\<版本>\
```

部分 4.x 安装仍使用：

```text
C:\Users\<用户>\AppData\Roaming\Tencent\xwechat\radium\Applet\packages\wx5306c5978fdb76e4\<版本>\
```

可在 PowerShell 中按修改时间查找：

```powershell
Get-ChildItem "$env:APPDATA\Tencent\xwechat\radium" -Recurse -Filter *.wxapkg |
  Where-Object FullName -Match 'wx5306c5978fdb76e4' |
  Sort-Object LastWriteTime -Descending |
  Select-Object LastWriteTime, FullName
```

### macOS

当前多用户目录：

```text
~/Library/Containers/com.tencent.xinWeChat/Data/Documents/app_data/radium/users/<hash>/applet/packages/wx5306c5978fdb76e4/<版本>/
```

部分 4.x 安装仍使用：

```text
~/Library/Containers/com.tencent.xinWeChat/Data/Documents/app_data/radium/Applet/packages/wx5306c5978fdb76e4/<版本>/
```

可按修改时间查找：

```bash
rg --files "$HOME/Library/Containers/com.tencent.xinWeChat/Data/Documents/app_data/radium" \
  | rg 'wx5306c5978fdb76e4/.+\.wxapkg$'
```

复制整个 AppID/版本目录到工作目录再处理，保留主包与分包，不要直接修改微信缓存。
缓存位置会随微信版本变化；找不到时，以打开小程序后新出现或修改的 `.wxapkg` 为准。

## 2. 解包并确认 `--source`

使用可信且支持当前微信版本、主包和分包的 wxapkg 解包器。不同工具命令不同，输出
完成后必须能找到：

```text
<反编译源码根目录>/
└── src/
    └── settings.<可选hash>.json
```

例如最终目录为：

```text
D:\wxsource\wx5306c5978fdb76e4-code\src\settings.abc123.json
```

那么参数应写：

```text
--source "D:\wxsource\wx5306c5978fdb76e4-code"
```

不要传以下路径：

- `__APP__.wxapkg` 文件本身；
- 包文件所在的 `<版本>` 目录；
- `src/` 本身；
- Charles、mitmproxy、Wireshark 或 recorder 的抓包目录。

解包后可先确认 settings：

```powershell
Get-ChildItem "D:\wxsource\wx5306c5978fdb76e4-code\src" -Filter "settings*.json"
```

```bash
rg --files "/path/to/wx5306c5978fdb76e4-code/src" | rg '/settings(?:\.[^.]+)?\.json$'
```

目录中应只有一份包含 `assets.server` 和 `assets.bundleVers.mainscene` 的有效 settings。
如果保留了多个版本，bot 工具会拒绝继续，避免混用 CDN 版本。

## 3. 在 qq-farm-bot 更新规范资源

以下命令只使用 bot 已有工具，不修改其工具代码。先把下载结果写入 `tools/json` 和
`tools/img`：

```bash
cd /path/to/qq-farm-bot

node tools/download-game-config.js \
  --source "/path/to/wx5306c5978fdb76e4-code" \
  --output "tools/json"

node tools/download-game-images.js \
  --source "/path/to/wx5306c5978fdb76e4-code" \
  --input "tools/json" \
  --output "tools/img" \
  --concurrency 8 \
  --retries 3
```

检查 `download-images-report.json` 和脚本退出状态。配置工具只有四份 JSON 全部通过
引用与结构校验才会发布；图片工具可能部分完成，部分完成时不要直接覆盖规范资源。

人工检查差异后，将确认过的结果更新到 bot 规范资源目录：

```text
qq-farm-bot/core/src/gameConfig/ItemInfo.json
qq-farm-bot/core/src/gameConfig/Plant.json
qq-farm-bot/core/src/gameConfig/RoleLevel.json
qq-farm-bot/core/src/gameConfig/Land.json
qq-farm-bot/core/src/gameConfig/seed_images_named/
```

Rust 同步工具只读取这些目录，不修改 bot。若 bot 尚未建立规范图片目录，工具也会
读取下载器默认的 `qq-farm-bot/tools/img/`；显式执行 `--only images` 且两处都不存在
时会失败。

## 4. 同步到 qq-farm-rust

在 Rust 仓先预览：

```bash
cd /path/to/qq-farm-rust
node tools/sync-from-bot.mjs --bot-root "../qq-farm-bot"
```

输出含义：

```text
+ 新增
~ 内容变化
- Rust 中存在但 bot 中已不存在
= 哈希一致
```

确认后应用：

```bash
node tools/sync-from-bot.mjs --bot-root "../qq-farm-bot" --apply
```

只同步单类资源：

```bash
node tools/sync-from-bot.mjs --bot-root "../qq-farm-bot" --only config
node tools/sync-from-bot.mjs --bot-root "../qq-farm-bot" --only images
node tools/sync-from-bot.mjs --bot-root "../qq-farm-bot" --only proto
```

`images` 和 `proto` 是精确镜像，`--apply` 会删除 Rust 目标目录中 bot 已不存在的同类
文件，因此应始终先看 dry-run。工具会先校验 JSON、PNG 签名和 proto syntax，再以
临时文件/目录替换 Rust 资源；bot 目录全程只读。

同步后验证：

```bash
node --test tools/sync-from-bot.test.mjs
cargo check -p qq-farm-core
pnpm -C desktop-ui build
```

## 5. proto 与协议抓包

proto 文件从以下目录镜像：

```text
qq-farm-bot/core/src/proto/  ->  qq-farm-rust/proto/
```

官方升级后，仅复制 proto 并不能证明协议兼容。bot 的分析脚本读取 `<capture-dir>/*.bin`；
这些 `.bin` 不是 pcap、HAR 或整条 WebSocket 导出，而是外部 websocket recorder 保存
的完整 `gatepb.Message` 二进制帧。三个仓库当前都没有提供该 recorder。

已有 capture 时，在 bot 根目录运行：

```bash
pnpm install
pnpm build:core

pnpm -C core exec tsx ../tools/audit-capture-compatibility.js "/path/to/captures"
pnpm -C core exec tsx ../tools/decode-latest-protocols.js "/path/to/captures" "Operate" --shape
pnpm -C core exec node ../tools/decode-shop-protocols.js "/path/to/captures"
```

协议请求体可能经过 TSDK 加密，普通代理抓到的网络字节不能直接替代上述逐帧 recorder
输出。抓包可能包含登录 code、token、openid 和账号数据；只在有权测试的账号与环境
中操作，提交文件、日志或截图前必须脱敏，不要把 capture 放进仓库。

## 6. 升级检查清单

1. 打开官方小程序，确认缓存包修改时间和 AppID。
2. 复制主包与全部分包，在副本上解包。
3. 确认 `--source/src/settings*.json` 唯一且包含当前 CDN/bundle 版本。
4. 用 bot 工具下载四份配置和图片，检查报告与失败项。
5. 人工审查并更新 bot 规范资源，bot 工具代码保持不变。
6. 在 Rust 仓 dry-run，再以 `--apply` 镜像 config/images/proto。
7. 若协议有变化，用脱敏 capture 运行 bot 兼容审计。
8. 运行 Node 工具测试、Rust check 和 desktop-ui build。
9. 在 `docs/SYNC.md` 记录来源版本、差异和验证结果。
