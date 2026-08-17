# Architecture — qq-farm-rust

多前端共享业务语义：Vue 面板走 HTTP；Tauri 桌面端进程内嵌引擎，经 IPC 调同一套 app。

## Crate 拓扑

```text
Vue 面板 (qq-farm-bot/web) ──HTTP/Socket.IO──► qq-farm-server ──► qq-farm-app ──► qq-farm-core
desktop-ui (SoybeanAdmin) ──Tauri IPC──► qq-farm-desktop ──► qq-farm-app ──► qq-farm-core
CLI / demo ─────────────────────────────────────────────────► qq-farm-core（运维可调 app）
```

- `qq-farm-core`：零 UI、零进程入口的库。
- `qq-farm-app`：UI 无关门面（Account / Farm / Friend / Auth / Activity / Commerce / Settings / Config / Admin + `AppEvent` + `bootstrap`）。**禁止**依赖 `tauri` / `axum`。
- `qq-farm-server`：仅协议适配；不堆领域算法。
- `qq-farm-desktop`：Tauri v2 适配层；进程内嵌引擎；前端为仓库根 `desktop-ui/`。
- server ↔ desktop **互不依赖**。

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
Tauri App (qq-farm-desktop)
  └─ DesktopState / commands   (IPC 适配)
       └─ AppContext / Facades (qq-farm-app)
            └─ RuntimeEngine + stores (qq-farm-core)
```

前端不走 localhost HTTP / Socket.IO；实时推送由 `subscribe_events` → `emit("app-event")`。

侧栏菜单对齐 `qq-farm-web` 农场项（去掉 `/system/admin`）：home、personal、friends、activity、analytics、game-mall、mystery-shop、settings、game-config、account。

## 事件流

```text
core runtime / panel_log
        │
        ▼
   AppEvent 总线 (qq-farm-app)
        │
   ┌────┴────┐
   ▼         ▼
Socket.IO   Tauri emit("app-event")
适配器      （desktop-ui listen）
```

## 分层（core）

```text
constants → config → models → proto → network / crypto → runtime → infra → services(domain)
```

## Vue 路径（浏览器面板）

`浏览器 → server → app → core`；面板 HTTP/Socket 契约见 [SYNC.md](./SYNC.md)。面板代码仍在 `qq-farm-bot/web`。
