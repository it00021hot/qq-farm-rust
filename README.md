# qq-farm-rust

> QQ 农场多账号挂机工具的 Rust 重写版。
>
> 原项目 [qq-farm-bot](https://github.com/...)（Node.js 24,542 行 → Rust 39,549 行）
>
> 目标：**完整复刻原 TS 业务逻辑 + 显著降低运行时内存占用**（10 账号场景：Node.js ~300-500MB → Rust ~30-80MB，5-10x 改善）

---

## 项目结构

```
qq-farm-rust/
├── crates/
│   ├── qq-farm-core/        # 核心业务逻辑（services / runtime / network / models / utils）
│   ├── qq-farm-server/      # HTTP + WebSocket 服务（89 端点）
│   └── qq-farm-cli/         # CLI 工具（farm demo / worker demo）
├── proto/                   # protobuf 定义（prost 编译生成 33 个 Rust 模块）
├── docs/                    # 文档
└── README.md
```

## 模块覆盖度

| 模块 | 原 TS 文件 | Rust 文件 | 复刻情况 |
|------|----------|-----------|----------|
| services | 41 | 42 | ✅ 全覆盖 |
| runtime | 5 | 10 | ✅ 全覆盖（含更细拆分） |
| models | 10 | 15 | ✅ 全覆盖 |
| utils | 9 | 7 | ✅ 覆盖 |
| config | 3 | 5 | ✅ 覆盖 |
| network | - | 10 | ➕ 新增（WS 鉴权 / 帧编解码） |
| proto | - | 1 | ➕ 新增（prost 编译 33 个 proto） |

**行数**：原 TS 24,542 行 / Rust 39,549 行 = **161% 复刻度**（含模块化封装、文档注释、测试代码）

## 测试

```bash
# 全量测试
cargo test --workspace

# 输出示例：
# test result: ok. 724 passed; 0 failed  (qq-farm-core)
# test result: ok. 12 passed; 0 failed   (qq-farm-server lib)
# test result: ok. 10 passed; 0 failed   (qq-farm-server E2E)
# 总计：746 passed, 0 failed
```

### E2E 集成测试覆盖

- 注册 / 登录 / 验证 token / 改密码 / 登出
- 重复用户名拒绝
- 错密码 401
- admin 登录 + 拉 login-logs
- health / game-version / 404
- 账号创建返回 user 字段

## 启动

### 1. 前置条件

- Rust 1.97+
- macOS / Linux（x86_64 / aarch64）

### 2. 编译

```bash
cargo build --release
```

产物：
- `target/release/qq-farm-server` — HTTP + WebSocket 服务
- `target/release/qq-farm` — CLI 工具

### 3. 配置（环境变量）

| 变量 | 默认 | 说明 |
|------|------|------|
| `FARM_SERVER_URL` | `https://game.qq.com` | 真实游戏 WebSocket URL |
| `FARM_OS` | `linux` | 客户端 OS 标识 |
| `FARM_CLIENT_VERSION` | `1.0.0` | 客户端版本 |
| `FARM_ACCOUNT_ID` | - | 默认账号 ID |
| `ADMIN_PORT` | `3007` | HTTP 监听端口 |
| `RUST_LOG` | `info` | 日志级别 |

### 4. 启动 server

```bash
# 启动 HTTP + WebSocket 服务
RUST_LOG=info ADMIN_PORT=3007 ./target/release/qq-farm-server

# 访问
curl http://localhost:3007/health
```

### 5. CLI 工具

```bash
# Farm demo（单机演示）
./target/release/qq-farm farm-demo

# Worker demo
./target/release/qq-farm worker-demo
```

## HTTP API 端点（部分）

### 鉴权
- `POST /api/register` — 注册（用卡密）
- `POST /api/login` — 登录（返回 token + role）
- `POST /api/logout` — 登出
- `GET /api/auth/validate` — 验证 token
- `POST /api/user/change-password` — 改密码

### 农场
- `GET /api/farm/lands` — 拉取土地详情
- `GET /api/farm/seeds` — 拉取可买种子
- `POST /api/farm/operate` — 执行农场操作（按 op 分派）

`op` 支持：`harvest` / `water` / `weed` / `insecticide` / `fertilize` / `plant` / `remove` / `upgrade` / `unlock` / `cycle`

### 好友
- `GET /api/friends` — 拉取好友列表
- `GET /api/friends/{gid}/lands` — 拉取好友土地
- `POST /api/friends/operate` — 好友操作（steal / water / weed / bug / bad / farming）

### 活动中心
- `GET /api/activity-center` — 活动快照
- `GET /api/activity-center/season` — 赛季事件
- `POST /api/activity-center/pass/claim` — 领战斗通行证奖励
- `POST /api/activity-center/constellation/light` — 点亮星座
- `POST /api/activity-center/shop/exchange` — 兑换星砂商品
- `POST /api/activity-center/solar-terms/{term_id}/claim` — 领取节气奖励

### Admin
- `GET /api/admin/users` — 用户列表（admin 鉴权）
- `POST /api/admin/cards` — 创建卡密
- `GET /api/admin/login-logs` — 登录日志
- ...（共 89 端点）

### WebSocket
- `WS /ws` — 实时事件订阅（subscribe / status / log）

## 数据持久化

数据目录（默认 `$DATA_DIR`，fallback `~/.qq-farm-rust/`）：

```
data/
├── accounts.json             # 账号
├── users.json                # 用户
├── cards.json                # 卡密
├── global-config.json        # 全局配置
├── login-attempts.json       # 登录尝试
├── login-logs.json           # 登录日志
├── sessions/
│   └── admin-sessions.json   # admin token 持久化
└── logs/
    ├── combined.log          # 所有日志
    └── error.log             # 错误日志
```

## 关键架构决策

1. **多 crate workspace**：`qq-farm-core` / `qq-farm-server` / `qq-farm-cli`，清晰分层
2. **错误类型分层**：`core::Error` ← `network::NetworkError` + `services::*::Error`
3. **异步锁选择**：`tokio::sync::Mutex`（跨 await）vs `parking_lot::Mutex`（短临界区）
4. **MD5 自实现**（RFC 1321）
5. **AES-GCM 0.10** 解密接收完整 ciphertext+tag
6. **GCM 格式**：`ciphertext + iv + tag`
7. **protobuf bytes 字段**：`bytes::Bytes` + `Bytes::from_static(...)`
8. **测试隔离**：`serial_test = "3"`，全局状态测试用 `#[serial]`
9. **WS 加密**：`aes-gcm` 0.10 + `lz4_flex` 0.11
10. **微信登录依赖**：`aes` 0.8 / `aes-gcm` 0.10 / `lz4_flex` 0.11 / `p256` 0.13
11. **camelCase serde rename**：`activity_center_state` 的 `confirmedOpenedNodeIds` 风格
12. **敏感信息脱敏**：`logger.redact_string` state machine（避免 while-let 死循环）

## 性能

- **运行时内存**：10 账号场景目标 ~30-80MB（Node.js 版本 300-500MB）
- **启动时间**：< 1s（Node.js 冷启动 3-5s）
- **API 延迟**：本地 < 5ms（p99）

## 已知限制

1. **微信扫码登录协议层已实现，真请求留到集成时**（架构决定）
2. **真实游戏 WebSocket 连接未自动化**（需要真实 auth_code / open_id）
3. **前端 admin UI** 在原项目（qq-farm-bot），Rust 端只提供 API

## 贡献

```bash
# 跑单个测试
cargo test -p qq-farm-core --lib services::friend::visit_strategy

# 跑 E2E
cargo test -p qq-farm-server --test e2e_integration

# 跑全部
cargo test --workspace
```

## 许可

仅供学习使用。
