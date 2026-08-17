# desktop-ui

QQ Farm 桌面前端，基于 [SoybeanAdmin](https://github.com/soybeanjs/soybean-admin)（Vue3 + NaiveUI + UnoCSS），经 Tauri IPC 调用 `qq-farm-desktop`。

浏览器面板仍在 `qq-farm-bot/web`，本目录仅服务桌面端。

## 开发

```bash
pnpm i
# 完整桌面：cargo tauri dev 会先 pnpm build，WebView 直接加载 dist（无开发端口）
# 仅浏览器预览（无 IPC）：pnpm dev
```

## 分层约定

- `typings/desktop.d.ts` — IPC DTO
- `service/tauri/` — invoke / listen
- `store/modules/farm-account/` — Pinia
- `views/farm/*` — 页面 + `modules/*`

主题与组件优先用 Soybean / Naive UI，禁止私自主题皮肤。
