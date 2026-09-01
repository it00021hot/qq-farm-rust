# Architecture — qq-farm-rust

只维护桌面版：Tauri 桌面端进程内嵌引擎，经 IPC 调用同一套 app（原 HTTP API 服务 crate `qq-farm-server` 已删除）。

## Crate 拓扑

```text
desktop-ui (SoybeanAdmin) ──Tauri IPC──► qq-farm-desktop ──► qq-farm-app ──► qq-farm-core
```

- `qq-farm-core`：零 UI、零进程入口的库。
- `qq-farm-app`：UI 无关门面（Account / Farm / Friend / Activity / Commerce / Settings / Config / Admin + `AppEvent` + `bootstrap`）。**禁止**依赖 `tauri`。
- `qq-farm-desktop`：Tauri v2 适配层；进程内嵌引擎；前端为仓库根 `desktop-ui/`。

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

壳层对齐原 Wails 桌面端：macOS 原生菜单、全平台托盘、关窗进托盘、GitHub Releases 自动更新。安装包内嵌 `tsdk.wasm` / `game_config`。无「在浏览器中打开」。发版见 [RELEASE_CHECKLIST.md](./RELEASE_CHECKLIST.md)。

侧栏菜单对齐 `qq-farm-web` 农场项（去掉 `/system/admin`）：home、personal、friends、activity、analytics、game-mall、mystery-shop、settings、game-config、account。

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

