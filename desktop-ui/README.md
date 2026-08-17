# desktop-ui

QQ Farm 桌面前端，基于 [SoybeanAdmin](https://github.com/soybeanjs/soybean-admin)（Vue3 + NaiveUI + UnoCSS），经 Tauri IPC 调用 `qq-farm-desktop`。

浏览器面板仍在 `qq-farm-bot/web`，本目录仅服务桌面端。

## 开发

```bash
pnpm i
pnpm dev          # 仅 Vite（无 IPC 时可进登录壳）
# 完整桌面：在仓库根或 crates/qq-farm-desktop 执行 cargo tauri dev
```

## 分层约定

- `typings/desktop.d.ts` — IPC DTO
- `service/tauri/` — invoke / listen
- `store/modules/desktop/` — Pinia
- `views/home|settings` — 页面 + `modules/*`

主题与组件优先用 Soybean / Naive UI，禁止私自主题皮肤。
