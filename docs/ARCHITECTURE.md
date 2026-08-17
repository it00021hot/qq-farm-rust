# Architecture — qq-farm-rust

多前端共享业务语义：Vue 面板走 HTTP，GPUI 桌面端进程内嵌引擎。

## Crate 拓扑

```text
Vue 面板 ──HTTP/Socket.IO──► qq-farm-server ──► qq-farm-app ──► qq-farm-core
GPUI 桌面 ──────────────────► qq-farm-desktop ──► qq-farm-app ──► qq-farm-core
CLI / demo ─────────────────────────────────────► qq-farm-core（运维可调 app）
```

- `qq-farm-core`：零 UI、零进程入口的库。
- `qq-farm-app`：UI 无关门面（Account / Farm / Friend / Auth / Activity / Commerce / Settings / Config / Admin + `AppEvent` + `bootstrap`）。
- `qq-farm-server`：仅协议适配；不堆领域算法。start/stop、账号列表、settings 面板、status 等已委托 `qq-farm-app`。
- `qq-farm-desktop`：GPUI + gpui-component；进程内嵌引擎，侧栏导航对齐 web 菜单。
- server ↔ desktop **互不依赖**。

## 桌面嵌入模式（默认）

```text
GPUI App
  └─ AppSession / Facades   (qq-farm-app)
       └─ RuntimeEngine + stores (qq-farm-core)
```

可选第二路径：desktop 作为薄客户端连接已有 server（实现期再开）。

## 事件流

```text
core runtime / panel_log
        │
        ▼
   AppEvent 总线 (qq-farm-app)
        │
   ┌────┴────┐
   ▼         ▼
Socket.IO   GPUI Model 订阅
适配器
```

## 分层（core）

```text
constants → config → models → proto → network / crypto → runtime → infra → services(domain)
```

## Vue 路径（不变）

`浏览器 → server → app → core`；面板 HTTP/Socket 契约见 [SYNC.md](./SYNC.md)。
