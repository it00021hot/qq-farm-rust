//! 网关请求的五班次调度模型。
//!
//! 移植自 bot `core/src/utils/request-priority.ts` 与 `low-priority-gate.ts`
//! （对照 Go 移植版 `qq-farm-core/internal/farm/protocol/priority.go`，1:1 对齐）。
//!
//! 背景：Gateway 是单条 WebSocket 复用，所有业务共享一条连接。以前 rust 只用
//! 「共享 4 槽 + 前台保留 1 槽」的信号量近似，后台扫描（好友巡查、宠物同步）和
//! 用户前台操作挤在同一池里，服务端一旦变慢整条连接就被拖垮。现在按「班次」分层，
//! 优先级从高到低：
//!
//! 1. critical   —— 心跳 / ACE AntiData。掉了就直接下线，两条通道各有独立保留槽位。
//! 2. foreground —— 用户在面板上的前台操作。人在等结果，优先级仅次于保命流量。
//! 3. farm       —— 自己农场的后台定时任务。
//! 4. friend     —— 好友农场的后台定时任务。
//! 5. background —— 宠物同步等「补数据」任务，只在网关完全空闲时才发。
//!
//! 容量约束（数值与 bot 完全一致）：
//! - critical 的两条通道（heartbeat / ace）各自保留一个槽位，互不挤占；
//! - 业务流量（foreground/farm/friend）总在途不超过 [`MAX_BUSINESS_IN_FLIGHT`]；
//! - 其中非前台业务（farm/friend）不超过 [`MAX_NON_FOREGROUND_BUSINESS_IN_FLIGHT`]，
//!   因此前台操作至少保留两个槽位；前台请求排队时，新的后台业务请求会让路；
//! - background 只在连接彻底空闲（没有在途请求、队列里也没有别的班次）时才发；
//! - 低优先班次等待超过 [`CLASS_STARVATION_MS`] 时会被提升到队首；前台请求排队时除外。
//!
//! 模块结构仿照 go priority.go：[`select_dispatch_index`] 等调度决策是纯函数
//! （可单测、时钟注入），[`RpcScheduler`] 负责容量记账（队列 / 在途台账 + 授权唤醒）。

use std::sync::Weak;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use parking_lot::Mutex;
use tokio::sync::oneshot;

// =====================================================================
// 班次定义
// =====================================================================

/// 请求班次（对齐 bot `RequestClass`，从高到低）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestClass {
    /// 心跳 / ACE AntiData：两条保留通道（见 [`CriticalLane`]）
    Critical,
    /// 面板上的前台操作，人在等结果
    Foreground,
    /// 自己农场的后台定时任务
    Farm,
    /// 好友农场的后台定时任务
    Friend,
    /// 宠物同步等「补数据」任务，只在网关完全空闲时才发
    Background,
}

impl RequestClass {
    /// 班次优先级顺序表（对齐 bot `REQUEST_CLASS_ORDER`）。
    pub const ORDER: [RequestClass; 5] = [
        RequestClass::Critical,
        RequestClass::Foreground,
        RequestClass::Farm,
        RequestClass::Friend,
        RequestClass::Background,
    ];

    /// 是否业务班次：会互相争抢在途预算的三档（对齐 bot `BUSINESS_CLASSES`）。
    #[must_use]
    pub fn is_business(self) -> bool {
        matches!(self, RequestClass::Foreground | RequestClass::Farm | RequestClass::Friend)
    }

    /// 队列压力日志里的班次标记（对齐 bot `REQUEST_CLASS_MARKER`）。
    #[must_use]
    pub fn marker(self) -> &'static str {
        match self {
            RequestClass::Critical => "!",
            RequestClass::Foreground => "",
            RequestClass::Farm => "#",
            RequestClass::Friend => "&",
            RequestClass::Background => "~",
        }
    }
}

/// critical 的两条保留通道（对齐 bot `CriticalLane`）：心跳 / ACE 各占一槽，互不挤占。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CriticalLane {
    /// 游戏心跳
    Heartbeat,
    /// ACE AntiData 上报
    Ace,
}

// =====================================================================
// 常量（数值与 bot request-priority.ts / low-priority-gate.ts 完全一致）
// =====================================================================

/// 业务流量（前台 + 农场 + 好友）的总在途预算。
pub const MAX_BUSINESS_IN_FLIGHT: usize = 3;
/// 非前台业务的在途上限：后台自动任务最多占一个业务槽位（前台至少保留 2 槽）。
pub const MAX_NON_FOREGROUND_BUSINESS_IN_FLIGHT: usize = 1;

/// 排队超过这个时长的低优先班次会被提升，防止被高优先班次持续插队饿死。
pub const CLASS_STARVATION_MS: i64 = 4000;

/// background 请求在队列里最多等这么久；等不到槽位就按「让路」失败
/// （对齐 bot `LOW_PRIORITY_QUEUE_WAIT_MS`，Go 侧以 GatewayBusyError 收场）。
pub const LOW_PRIORITY_QUEUE_WAIT_MS: u64 = 8000;
/// 后台任务发请求之前等网关空闲的最长时间（对齐 bot `LOW_PRIORITY_IDLE_WAIT_MAX_MS`）。
pub const LOW_PRIORITY_IDLE_WAIT_MAX_MS: u64 = 8000;
/// 等网关空闲的轮询间隔（对齐 bot `LOW_PRIORITY_IDLE_POLL_MS`）。
pub const LOW_PRIORITY_IDLE_POLL_MS: u64 = 250;
/// 在途请求超过这个年龄就当成「网关正在卡住」：后台请求必须立刻停手
/// （对齐 bot `GATEWAY_STALL_PENDING_MS`）。
pub const GATEWAY_STALL_PENDING_MS: i64 = 5000;
/// 网关卡住时定时任务退避区间下限（对齐 bot `BUSINESS_BACKOFF_MIN_MS`）。
pub const BUSINESS_BACKOFF_MIN_MS: u64 = 30_000;
/// 网关卡住时定时任务退避区间上限（对齐 bot `BUSINESS_BACKOFF_MAX_MS`）。
pub const BUSINESS_BACKOFF_MAX_MS: u64 = 60_000;
/// 队列压力日志节流间隔（对齐 Go `requestPressureLogIntervalMs`）。
pub const REQUEST_PRESSURE_LOG_INTERVAL_MS: i64 = 5000;

/// 每个班次自身的在途上限（对齐 bot `MAX_IN_FLIGHT_BY_CLASS`）。
#[must_use]
pub const fn max_in_flight_for_class(class: RequestClass) -> usize {
    match class {
        RequestClass::Critical => 2,
        RequestClass::Foreground => 3,
        RequestClass::Farm | RequestClass::Friend | RequestClass::Background => 1,
    }
}

/// 每个班次的排队上限（对齐 bot `MAX_QUEUED_BY_CLASS`）：
/// 后台班次故意留得很小，队列长了就该让路而不是硬排到超时。
#[must_use]
pub const fn max_queued_for_class(class: RequestClass) -> usize {
    match class {
        RequestClass::Critical => 8,
        RequestClass::Foreground => 60,
        RequestClass::Farm => 40,
        RequestClass::Friend => 30,
        RequestClass::Background => 10,
    }
}

// =====================================================================
// 班次解析（对齐 bot resolveRequestClass + request-context.ts）
// =====================================================================

/// 决定一个请求属于哪个班次。
///
/// - 方法名 Heartbeat / AntiData 直接映射 critical 的两条保留通道
///   （bot 由调用方显式传 criticalLane，rust 按 go 的做法用方法名识别）；
/// - 有环境班次（调度器任务入口注入，见 `gateway::request_class_scope`）就继承它；
/// - 没有任何表态（面板 HTTP / IPC 调用链）默认 foreground：那边确实有人在等结果。
#[must_use]
pub fn resolve_request_class(
    method: &str,
    ambient: Option<RequestClass>,
) -> (RequestClass, Option<CriticalLane>) {
    if method.eq_ignore_ascii_case("Heartbeat") {
        return (RequestClass::Critical, Some(CriticalLane::Heartbeat));
    }
    if method.eq_ignore_ascii_case("AntiData") {
        return (RequestClass::Critical, Some(CriticalLane::Ace));
    }
    (ambient.unwrap_or(RequestClass::Foreground), None)
}

/// 调度器命名空间 → 环境请求班次（对齐 bot `classForSchedulerNamespace`）。
///
/// - `ace` 是基础设施：AntiData 按方法名自行解析成 critical 保留通道，不注入班次；
/// - `worker:{id}` 命名空间同时驱动农场等多种任务，只能给保守默认值 farm
///   （真正的 farm/friend 区分由 FarmService / FriendService 各自的调度器完成）；
/// - 其余按前缀：friend → friend，否则 → farm。
#[must_use]
pub fn class_for_scheduler_namespace(namespace: &str) -> Option<RequestClass> {
    let name = namespace.trim();
    if name.is_empty() || name == "ace" {
        return None;
    }
    if name.starts_with("friend") {
        return Some(RequestClass::Friend);
    }
    Some(RequestClass::Farm)
}

// =====================================================================
// 调度决策纯函数（对齐 bot selectDispatchIndex）
// =====================================================================

/// 队列 / 在途条目的只读快照（纯函数入参，调用方持有真数据）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    /// 所属班次
    pub class: RequestClass,
    /// critical 保留通道（仅 critical 班次有值）
    pub lane: Option<CriticalLane>,
    /// 入队时刻（单调毫秒；测试可注入合成值）
    pub enqueued_at_ms: i64,
}

impl Candidate {
    /// 便捷构造
    #[must_use]
    pub fn new(class: RequestClass, lane: Option<CriticalLane>, enqueued_at_ms: i64) -> Self {
        Self { class, lane, enqueued_at_ms }
    }
}

/// 从队列里挑出下一个可以发送的请求下标；没有可发送的就返回 `None`。
///
/// 只读入参，由调用方负责把选中的请求移出队列。规则（对齐 bot）：
/// 1. 心跳 / ACE 各占一个独立保留槽位，谁也挤不掉谁；
/// 2. 没有标记通道的 critical 请求只吃 critical 的普通预算（目前业务里没有）；
/// 3. 业务班次受 总预算 + 每班次上限 + 前台保留槽位 三重约束；先救被插队太久的
///    （等待 ≥ [`CLASS_STARVATION_MS`] 且最久的优先），否则按班次优先级、同班次 FIFO；
/// 4. background 是「补数据」：只在连接彻底空闲、且队列里没有别的班次时才发。
#[must_use]
pub fn select_dispatch_index(
    queue: &[Candidate],
    in_flight: &[Candidate],
    now_ms: i64,
) -> Option<usize> {
    if queue.is_empty() {
        return None;
    }

    // 1) 心跳 / ACE 各占一个独立保留槽位，谁也挤不掉谁。
    for lane in [CriticalLane::Heartbeat, CriticalLane::Ace] {
        let lane_busy =
            in_flight.iter().any(|r| r.class == RequestClass::Critical && r.lane == Some(lane));
        if lane_busy {
            continue;
        }
        if let Some(idx) = queue
            .iter()
            .position(|r| r.class == RequestClass::Critical && r.lane == Some(lane))
        {
            return Some(idx);
        }
    }

    // 2) 没有标记通道的 critical 请求只吃 critical 的普通预算。
    let critical_in_flight = in_flight.iter().filter(|r| r.class == RequestClass::Critical).count();
    if critical_in_flight < max_in_flight_for_class(RequestClass::Critical) {
        if let Some(idx) =
            queue.iter().position(|r| r.class == RequestClass::Critical && r.lane.is_none())
        {
            return Some(idx);
        }
    }

    // 3) 业务班次：总预算 + 每班次上限 + 前台保留槽位三重约束。
    let business_in_flight = in_flight.iter().filter(|r| r.class.is_business()).count();
    if business_in_flight < MAX_BUSINESS_IN_FLIGHT {
        let has_queued_foreground = queue.iter().any(|r| r.class == RequestClass::Foreground);
        let non_foreground_in_flight = in_flight
            .iter()
            .filter(|r| r.class.is_business() && r.class != RequestClass::Foreground)
            .count();
        let per_class_in_flight =
            |class: RequestClass| in_flight.iter().filter(|r| r.class == class).count();

        let mut eligible: Vec<usize> = Vec::new();
        for (index, item) in queue.iter().enumerate() {
            let class = item.class;
            if !class.is_business() {
                continue;
            }
            if per_class_in_flight(class) >= max_in_flight_for_class(class) {
                continue;
            }
            if class != RequestClass::Foreground
                && (has_queued_foreground
                    || non_foreground_in_flight >= MAX_NON_FOREGROUND_BUSINESS_IN_FLIGHT)
            {
                continue;
            }
            eligible.push(index);
        }

        if !eligible.is_empty() {
            // 先救被插队太久的：等待最长且已超阈值的优先发送。
            let mut starved_index: Option<usize> = None;
            let mut starved_wait_ms = CLASS_STARVATION_MS;
            for &index in &eligible {
                let waited = (now_ms - queue[index].enqueued_at_ms).max(0);
                if waited >= starved_wait_ms {
                    starved_wait_ms = waited;
                    starved_index = Some(index);
                }
            }
            if let Some(index) = starved_index {
                return Some(index);
            }

            // 否则按班次优先级、同班次内 FIFO。
            for class in RequestClass::ORDER {
                if !class.is_business() {
                    continue;
                }
                for &index in &eligible {
                    if queue[index].class == class {
                        return Some(index);
                    }
                }
            }
        }
    }

    // 4) background 是「补数据」：只在连接彻底空闲、且队列里没有别的班次时才发。
    if !in_flight.is_empty() {
        return None;
    }
    if queue.iter().any(|r| r.class != RequestClass::Background) {
        return None;
    }
    queue.iter().position(|r| r.class == RequestClass::Background)
}

// =====================================================================
// 低优先闸门（对齐 bot low-priority-gate.ts / go gatewayLoad）
// =====================================================================

/// 网关负载快照（对齐 bot `getGatewayLoad` / go `gatewayLoad`）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GatewayLoadSnapshot {
    /// 在途请求总数（含 critical / background）
    pub pending: usize,
    /// 排队请求总数
    pub queued: usize,
    /// 排队中的非 background 请求（业务正在排队）
    pub blocking_queued: usize,
    /// 在途 critical（心跳 / ACE）
    pub critical_pending: usize,
    /// 在途业务请求（foreground/farm/friend）
    pub business_pending: usize,
    /// 在途前台请求
    pub foreground_pending: usize,
    /// 在途 background 请求
    pub background_pending: usize,
    /// 心跳漏拍计数（0 = 连接健康）
    pub heartbeat_misses: usize,
    /// 最老在途请求的年龄（ms）
    pub oldest_pending_age_ms: i64,
}

/// 网关空闲判定：只有完全没有主流程流量、且连接看起来健康时才允许 background 请求
/// （对齐 bot `isGatewayIdleForLowPriority`）。
///
/// - 队列里有任何非 background 请求 → 业务流量正在排队，必须让路；
/// - 有业务请求在飞 → 有人正在等回包，必须让路；
/// - 已经有 background 在飞 → 那唯一的后台槽位被占着，别再叠加；
/// - 心跳已经漏过、或有在途请求卡了 [`GATEWAY_STALL_PENDING_MS`] 以上 → 连接可疑，一律不发；
/// - critical（心跳 / ACE）自身不参与判定，它们有独立保留槽位。
#[must_use]
pub fn is_gateway_idle_for_low_priority(load: &GatewayLoadSnapshot) -> bool {
    if load.blocking_queued > 0 || load.business_pending > 0 || load.background_pending > 0 {
        return false;
    }
    if load.heartbeat_misses > 0 {
        return false;
    }
    if load.oldest_pending_age_ms >= GATEWAY_STALL_PENDING_MS {
        return false;
    }
    true
}

/// farm / friend 定时任务的健康度闸门。判据比 background 宽松得多：
/// 不要求网关空闲（定时任务本来就该和前台操作抢槽位），只要求「连接还在回包」
/// （对齐 bot `isGatewayHealthyForBusiness`）。
#[must_use]
pub fn is_gateway_healthy_for_business(load: &GatewayLoadSnapshot) -> bool {
    if load.heartbeat_misses > 0 {
        return false;
    }
    if load.oldest_pending_age_ms >= GATEWAY_STALL_PENDING_MS {
        return false;
    }
    true
}

/// 闸门关着时定时任务的下一次退避时长：首次 30 秒，之后翻倍并封顶 60 秒
/// （对齐 bot `nextBusinessBackoffMs`）。
#[must_use]
pub fn next_business_backoff_ms(previous_backoff_ms: u64) -> u64 {
    if previous_backoff_ms == 0 {
        return BUSINESS_BACKOFF_MIN_MS;
    }
    (previous_backoff_ms * 2).min(BUSINESS_BACKOFF_MAX_MS)
}

// =====================================================================
// 容量记账调度器（gateway 发送路径接入点）
// =====================================================================

/// 进程级单调毫秒时钟（不受 utils::time 服务器时间同步跳变影响；
/// 排队顺序 / 饥饿提升 / 在途年龄只关心相对时长）。
fn monotonic_ms() -> i64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as i64
}

/// 排队条目：持有授权发送端，被选中时把在途额度 guard 递给排队的调用方。
struct QueueSlot {
    class: RequestClass,
    lane: Option<CriticalLane>,
    enqueued_at_ms: i64,
    /// 授权通道：drain 选中本条目时把 [`InFlightGuard`] 递给排队的 `send_rpc`。
    /// 调用方放弃等待（超时 / 取消）后 receiver 被 drop，`is_closed()` 即可回收。
    grant: oneshot::Sender<InFlightGuard>,
}

/// 在途条目台账（对齐 go pending 里每条请求的 class/lane/enqueuedAt 记账）。
struct FlightSlot {
    id: u64,
    class: RequestClass,
    lane: Option<CriticalLane>,
    enqueued_at_ms: i64,
}

/// 在途额度 guard：持有即占用该班次的一个在途槽位，Drop 时归还并触发重新调度。
///
/// scheduler 引用用 `Weak`：授权 payload 可能在「调用方已被取消」的窗口里被
/// 丢弃，guard 必须自回收；`Weak` 保证 guard 不反过来把调度器钉在内存里。
pub struct InFlightGuard {
    scheduler: Weak<RpcScheduler>,
    id: u64,
}

impl std::fmt::Debug for InFlightGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InFlightGuard").field("id", &self.id).finish()
    }
}

impl InFlightGuard {
    /// 主动归还（等价 Drop，可显式调用表达意图）
    pub fn release(self) {}
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        if let Some(scheduler) = self.scheduler.upgrade() {
            scheduler.release_flight(self.id);
        }
    }
}

/// 排队凭据：`try_enqueue` 成功后发给调用方，等待调度授权。
pub struct DispatchTicket {
    rx: oneshot::Receiver<InFlightGuard>,
}

impl DispatchTicket {
    /// 等待调度授权。`None` 表示队列已被清空（连接断开 / 调度器关闭），
    /// 调用方应按「连接未打开」处理（对齐 go `连接未打开: %s`）。
    pub async fn granted(self) -> Option<InFlightGuard> {
        self.rx.await.ok()
    }
}

/// 队列已满信息（按班次配额拒绝，对齐 go「请求等待队列已满」）。
#[derive(Debug, Clone, Copy)]
pub struct QueueFullInfo {
    /// 该班次当前排队数
    pub queued_for_class: usize,
    /// 该班次排队上限
    pub limit: usize,
    /// 全队列排队总数
    pub queued_total: usize,
    /// 在途总数
    pub pending: usize,
}

/// 五班次容量记账调度器。
///
/// 台账 + 授权都在 `parking_lot::Mutex` 里同步完成（drain 不跨 await）：
/// 选中条目 = 从队列移除 + 记入在途台账 + `grant.send(guard)` 唤醒排队的
/// `send_rpc`；它醒来后自己去抢 `send_order` 锁完成 seq 分配 → 加密 → 上 wire。
/// 「选中即占槽」复现 go 在写帧前先登记 pending 的记账时机，也和 bot
/// 「drain 队列时把请求移出队列再发送」一致。
pub struct RpcScheduler {
    inner: Mutex<SchedState>,
    /// 弱引用自身：guard Drop 时靠它归还额度（见 [`InFlightGuard`]）。
    self_weak: Weak<RpcScheduler>,
}

struct SchedState {
    queue: Vec<QueueSlot>,
    in_flight: Vec<FlightSlot>,
    next_flight_id: u64,
    last_pressure_log_ms: Option<i64>,
}

impl RpcScheduler {
    /// 创建（返回 `Arc`：guard 的自回收需要弱引用回链）。
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|weak| Self {
            inner: Mutex::new(SchedState {
                queue: Vec::new(),
                in_flight: Vec::new(),
                next_flight_id: 1,
                last_pressure_log_ms: None,
            }),
            self_weak: weak.clone(),
        })
    }

    /// 入队一个请求并立刻尝试调度。
    ///
    /// 该班次排队配额已满时返回 [`QueueFullInfo`]（对齐 bot `isClassQueueFull`：
    /// 在入队前直接拒绝，而不是让请求熬到超时）。
    pub fn try_enqueue(
        &self,
        class: RequestClass,
        lane: Option<CriticalLane>,
    ) -> Result<DispatchTicket, QueueFullInfo> {
        let mut state = self.inner.lock();
        // 先回收已放弃的排队条目，配额按存活条目计算
        state.queue.retain(|slot| !slot.grant.is_closed());
        let queued_for_class = state.queue.iter().filter(|slot| slot.class == class).count();
        if queued_for_class >= max_queued_for_class(class) {
            return Err(QueueFullInfo {
                queued_for_class,
                limit: max_queued_for_class(class),
                queued_total: state.queue.len(),
                pending: state.in_flight.len(),
            });
        }
        let (tx, rx) = oneshot::channel();
        state.queue.push(QueueSlot { class, lane, enqueued_at_ms: monotonic_ms(), grant: tx });
        self.drain_locked(&mut state);
        self.maybe_log_pressure(&mut state);
        Ok(DispatchTicket { rx })
    }

    /// 归还一个在途额度并重新调度（guard Drop / 请求完成时调用）。
    fn release_flight(&self, id: u64) {
        let mut state = self.inner.lock();
        state.in_flight.retain(|slot| slot.id != id);
        self.drain_locked(&mut state);
    }

    /// 主动触发一次调度（连接断开清理等场景的兜底）。
    pub fn drain(&self) {
        let mut state = self.inner.lock();
        self.drain_locked(&mut state);
    }

    /// 清空排队（对齐 go rejectAll：连接断开时排队的请求一并失败）。
    /// 掉 drop 各授权发送端，等待方从 [`DispatchTicket::granted`] 拿到 `None`。
    pub fn reject_all_queued(&self) -> usize {
        let mut state = self.inner.lock();
        let n = state.queue.len();
        state.queue.clear();
        n
    }

    /// 当前负载快照。`heartbeat_misses` 由调用方（gateway 用入站静默近似）填充。
    #[must_use]
    pub fn load(&self) -> GatewayLoadSnapshot {
        let state = self.inner.lock();
        let now = monotonic_ms();
        let mut load = GatewayLoadSnapshot {
            pending: state.in_flight.len(),
            queued: state.queue.len(),
            ..GatewayLoadSnapshot::default()
        };
        for slot in &state.queue {
            if slot.class != RequestClass::Background {
                load.blocking_queued += 1;
            }
        }
        for flight in &state.in_flight {
            match flight.class {
                RequestClass::Critical => load.critical_pending += 1,
                RequestClass::Background => load.background_pending += 1,
                RequestClass::Foreground => {
                    load.business_pending += 1;
                    load.foreground_pending += 1;
                }
                RequestClass::Farm | RequestClass::Friend => load.business_pending += 1,
            }
            let age = (now - flight.enqueued_at_ms).max(0);
            if age > load.oldest_pending_age_ms {
                load.oldest_pending_age_ms = age;
            }
        }
        load
    }

    /// 当前排队总数（诊断用）
    #[must_use]
    pub fn queued_count(&self) -> usize {
        self.inner.lock().queue.len()
    }

    /// 当前在途总数（诊断用）
    #[must_use]
    pub fn in_flight_count(&self) -> usize {
        self.inner.lock().in_flight.len()
    }

    /// 核心调度循环：按纯函数 [`select_dispatch_index`] 逐条授权，直到无可发。
    ///
    /// 调用方持有 `state` 锁；授权是同步 oneshot send，不跨 await。
    fn drain_locked(&self, state: &mut SchedState) {
        loop {
            // 回收已放弃的排队条目（超时 / 取消的调用方不会再消费授权）
            state.queue.retain(|slot| !slot.grant.is_closed());
            if state.queue.is_empty() {
                return;
            }
            let now = monotonic_ms();
            let queue_cands: Vec<Candidate> = state
                .queue
                .iter()
                .map(|slot| Candidate::new(slot.class, slot.lane, slot.enqueued_at_ms))
                .collect();
            let flight_cands: Vec<Candidate> = state
                .in_flight
                .iter()
                .map(|slot| Candidate::new(slot.class, slot.lane, slot.enqueued_at_ms))
                .collect();
            let Some(idx) = select_dispatch_index(&queue_cands, &flight_cands, now) else {
                return;
            };
            let slot = state.queue.remove(idx);
            let id = state.next_flight_id;
            state.next_flight_id += 1;
            state.in_flight.push(FlightSlot {
                id,
                class: slot.class,
                lane: slot.lane,
                enqueued_at_ms: slot.enqueued_at_ms,
            });
            let guard = InFlightGuard { scheduler: Weak::clone(&self.self_weak), id };
            if slot.grant.send(guard).is_err() {
                // 调用方在授权瞬间恰好消失：立刻释放这条额度，继续调度
                state.in_flight.retain(|flight| flight.id != id);
            }
        }
    }

    /// 队列压力日志：有业务请求在排队时节流告警（对齐 go logRequestPressureLocked，
    /// 5s 一次；纯 background 排队是常态，不告警）。
    fn maybe_log_pressure(&self, state: &mut SchedState) {
        let blocking = state
            .queue
            .iter()
            .filter(|slot| slot.class != RequestClass::Background)
            .count();
        if blocking == 0 {
            return;
        }
        let now = monotonic_ms();
        if let Some(last) = state.last_pressure_log_ms {
            if now - last < REQUEST_PRESSURE_LOG_INTERVAL_MS {
                return;
            }
        }
        state.last_pressure_log_ms = Some(now);
        let active: Vec<String> = state
            .in_flight
            .iter()
            .take(6)
            .map(|f| format!("{}:{}ms", f.class.marker(), (now - f.enqueued_at_ms).max(0)))
            .collect();
        let queued_marks: Vec<&str> =
            state.queue.iter().take(8).map(|slot| slot.class.marker()).collect();
        tracing::warn!(
            pending = state.in_flight.len(),
            queued = state.queue.len(),
            active = active.join(","),
            queued_marks = queued_marks.join(""),
            "Gateway 请求压力"
        );
    }
}

// =====================================================================
// 单元测试：移植 bot request-priority.test.js 与 low-priority-gate.test.js 核心用例
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// bot queued(requestClass, extra)：入队时刻统一 1_000_000
    fn queued(class: RequestClass) -> Candidate {
        Candidate::new(class, None, 1_000_000)
    }

    fn queued_lane(class: RequestClass, lane: CriticalLane) -> Candidate {
        Candidate::new(class, Some(lane), 1_000_000)
    }

    fn flying(class: RequestClass) -> Candidate {
        Candidate::new(class, None, 1_000_000)
    }

    fn flying_lane(class: RequestClass, lane: CriticalLane) -> Candidate {
        Candidate::new(class, Some(lane), 1_000_000)
    }

    // ----- 班次解析（bot「班次解析」用例的 rust 对应版） -----

    #[test]
    fn resolve_heartbeat_and_ace_always_critical_lanes() {
        // 心跳 / ACE 按方法名走 critical 的两条保留通道
        let (class, lane) = resolve_request_class("Heartbeat", Some(RequestClass::Background));
        assert_eq!(class, RequestClass::Critical);
        assert_eq!(lane, Some(CriticalLane::Heartbeat));
        let (class, lane) = resolve_request_class("antidata", None);
        assert_eq!(class, RequestClass::Critical);
        assert_eq!(lane, Some(CriticalLane::Ace));
    }

    #[test]
    fn resolve_ambient_inherited_and_default_foreground() {
        // 有环境班次就继承（调度器注入；api 层默认 normal 在 rust 里不存在）
        assert_eq!(resolve_request_class("GetAll", Some(RequestClass::Friend)).0, RequestClass::Friend);
        assert_eq!(resolve_request_class("CheckFarm", Some(RequestClass::Farm)).0, RequestClass::Farm);
        // 没有环境班次（面板 HTTP / IPC 调用链）默认前台：那边确实有人在等结果
        assert_eq!(resolve_request_class("Purchase", None).0, RequestClass::Foreground);
    }

    #[test]
    fn scheduler_namespace_mapping_matches_bot() {
        assert_eq!(class_for_scheduler_namespace("ace"), None);
        assert_eq!(class_for_scheduler_namespace("farm-service"), Some(RequestClass::Farm));
        assert_eq!(class_for_scheduler_namespace("friend-service"), Some(RequestClass::Friend));
        assert_eq!(class_for_scheduler_namespace("worker:g1"), Some(RequestClass::Farm));
        assert_eq!(class_for_scheduler_namespace(""), None);
    }

    // ----- critical 保留通道（bot「心跳与 ACE 各有独立保留槽位」） -----

    #[test]
    fn heartbeat_and_ace_have_reserved_lanes_business_cannot_take() {
        let queue = [queued(RequestClass::Farm), queued_lane(RequestClass::Critical, CriticalLane::Ace), queued_lane(RequestClass::Critical, CriticalLane::Heartbeat)];
        let busy_business = [
            flying(RequestClass::Farm),
            flying(RequestClass::Friend),
            flying(RequestClass::Foreground),
        ];
        // 业务在途已满，心跳仍然先走
        assert_eq!(select_dispatch_index(&queue, &busy_business, 1_000_000), Some(2));
        // 心跳已经在飞，ACE 用自己的槽位
        let busy_with_hb = [
            busy_business.as_slice(),
            &[flying_lane(RequestClass::Critical, CriticalLane::Heartbeat)],
        ]
        .concat();
        assert_eq!(select_dispatch_index(&queue, &busy_with_hb, 1_000_000), Some(1));
        // 两条通道都在飞就轮到业务，但业务预算已满 → 什么都发不出去
        let both_lanes = [
            flying_lane(RequestClass::Critical, CriticalLane::Heartbeat),
            flying_lane(RequestClass::Critical, CriticalLane::Ace),
        ];
        let all_busy = [busy_business.as_slice(), both_lanes.as_slice()].concat();
        assert_eq!(select_dispatch_index(&queue, &all_busy, 1_000_000), None);
        // 业务在途清空后（只剩 critical），farm 可以发
        assert_eq!(select_dispatch_index(&queue, &both_lanes, 1_000_000), Some(0));
    }

    // ----- 前台保留槽位（bot「前台至少保留两个业务槽位」） -----

    #[test]
    fn foreground_keeps_two_business_slots_from_automation() {
        let queue = [queued(RequestClass::Farm), queued(RequestClass::Foreground)];
        let non_foreground = [flying(RequestClass::Farm)];

        assert_eq!(non_foreground.len(), MAX_NON_FOREGROUND_BUSINESS_IN_FLIGHT);
        // farm 已占满非前台额度：队首的 farm 被跳过，前台请求直接插到前面
        assert_eq!(select_dispatch_index(&queue, &non_foreground, 1_000_000), Some(1));
        // 业务总预算被占满后连前台也得等（此时在飞的都会很快回来）
        let full = [
            flying(RequestClass::Farm),
            flying(RequestClass::Foreground),
            flying(RequestClass::Foreground),
        ];
        assert_eq!(full.len(), MAX_BUSINESS_IN_FLIGHT);
        assert_eq!(select_dispatch_index(&queue, &full, 1_000_000), None);
    }

    // ----- 业务班次排序（bot「前台 > 自己农场 > 好友农场，同班次 FIFO」） -----

    #[test]
    fn business_classes_ordered_foreground_farm_friend_fifo() {
        let queue = [
            Candidate::new(RequestClass::Friend, None, 1_000_000),
            Candidate::new(RequestClass::Farm, None, 1_000_000),
            Candidate::new(RequestClass::Foreground, None, 1_000_000),
            Candidate::new(RequestClass::Farm, None, 1_000_000),
        ];
        // 前台请求排在队尾也先走
        assert_eq!(select_dispatch_index(&queue, &[], 1_000_000), Some(2));
        // 前台已发完（不在队列里）时轮到自己农场，同班次内取更早入队的 farmA
        let without_foreground = [queue[0], queue[1], queue[3]];
        assert_eq!(
            select_dispatch_index(&without_foreground, &[flying(RequestClass::Foreground)], 1_000_000),
            Some(1)
        );
        // 自己农场还有活要干时（farm 每班次上限 1），好友农场就得等（4 秒后靠饥饿提升）
        let farm_in_flight = [flying(RequestClass::Farm)];
        assert_eq!(
            select_dispatch_index(&[queued(RequestClass::Friend)], &farm_in_flight, 1_000_000),
            None
        );
        // 后台业务总在途上限为 1，好友要等农场请求返回
        assert_eq!(select_dispatch_index(&[queued(RequestClass::Friend)], &[], 1_000_000), Some(0));
    }

    // ----- 饥饿提升（bot「前台请求排队时，后台业务即使排队超时也让路」） -----

    #[test]
    fn starvation_promotes_only_when_no_queued_foreground() {
        let now = 1_000_000;
        let queue = [
            Candidate::new(RequestClass::Friend, None, now - CLASS_STARVATION_MS),
            Candidate::new(RequestClass::Foreground, None, now),
        ];
        // 好友请求已经等了阈值时长，但只要前台还在排队，它就不能抢占
        assert_eq!(select_dispatch_index(&queue, &[], now), Some(1));
        // 前台队列清空后，超时的低优先班次才会被提升
        let without_foreground = [queue[0]];
        assert_eq!(select_dispatch_index(&without_foreground, &[], now), Some(0));
        // 没等到阈值的照常按班次优先级来
        let fresh = [
            Candidate::new(RequestClass::Friend, None, now - 500),
            Candidate::new(RequestClass::Foreground, None, now),
        ];
        assert_eq!(select_dispatch_index(&fresh, &[], now), Some(1));
    }

    // ----- background 空闲门（bot「background 只在连接彻底空闲时才发」） -----

    #[test]
    fn background_flies_only_when_gateway_fully_idle() {
        let now = 1_000_000;
        assert_eq!(select_dispatch_index(&[queued(RequestClass::Background)], &[], now), Some(0));
        // 有任何在途请求（哪怕只是心跳）都不发后台请求
        assert_eq!(
            select_dispatch_index(
                &[queued(RequestClass::Background)],
                &[flying_lane(RequestClass::Critical, CriticalLane::Heartbeat)],
                now
            ),
            None
        );
        // 队列里还有业务请求在等，也不能插队
        assert_eq!(
            select_dispatch_index(
                &[queued(RequestClass::Background), queued(RequestClass::Friend)],
                &[],
                now
            ),
            Some(1)
        );
        // 已经有一个后台请求在飞就不再叠加
        assert_eq!(
            select_dispatch_index(
                &[queued(RequestClass::Background)],
                &[flying(RequestClass::Background)],
                now
            ),
            None
        );
    }

    // ----- 排队配额（bot「排队配额按班次独立计算」） -----

    #[test]
    fn queue_quota_is_per_class() {
        let background_full: Vec<Candidate> = std::iter::repeat_n(
            queued(RequestClass::Background),
            max_queued_for_class(RequestClass::Background),
        )
        .collect();
        // 配额按班次各自计算：background 排满不影响 critical / foreground 的名额
        assert_eq!(background_full.len(), max_queued_for_class(RequestClass::Background));
        assert!(max_queued_for_class(RequestClass::Background) < max_queued_for_class(RequestClass::Foreground));
        assert!(max_queued_for_class(RequestClass::Friend) <= max_queued_for_class(RequestClass::Farm));
        // 常量表与 bot 完全一致
        assert_eq!(max_queued_for_class(RequestClass::Critical), 8);
        assert_eq!(max_queued_for_class(RequestClass::Foreground), 60);
        assert_eq!(max_queued_for_class(RequestClass::Farm), 40);
        assert_eq!(max_queued_for_class(RequestClass::Friend), 30);
        assert_eq!(max_queued_for_class(RequestClass::Background), 10);
        assert_eq!(max_in_flight_for_class(RequestClass::Critical), 2);
        assert_eq!(max_in_flight_for_class(RequestClass::Foreground), 3);
        assert_eq!(max_in_flight_for_class(RequestClass::Farm), 1);
        assert_eq!(max_in_flight_for_class(RequestClass::Friend), 1);
        assert_eq!(max_in_flight_for_class(RequestClass::Background), 1);
        // 让路参数是有限正数，避免后台任务无限等待
        const {
            assert!(BUSINESS_BACKOFF_MIN_MS <= BUSINESS_BACKOFF_MAX_MS);
            assert!(LOW_PRIORITY_QUEUE_WAIT_MS > 0);
            assert!(LOW_PRIORITY_IDLE_WAIT_MAX_MS > 0);
            assert!(LOW_PRIORITY_IDLE_POLL_MS > 0);
        }
    }

    // ----- 压力标记（bot「压力日志标记能区分心跳/ACE 与各业务班次」） -----

    #[test]
    fn class_markers_distinguish_lanes() {
        assert_eq!(RequestClass::Critical.marker(), "!");
        assert_eq!(RequestClass::Foreground.marker(), "");
        assert_eq!(RequestClass::Farm.marker(), "#");
        assert_eq!(RequestClass::Friend.marker(), "&");
        assert_eq!(RequestClass::Background.marker(), "~");
    }

    // ----- 低优先闸门（low-priority-gate.test.js） -----

    #[test]
    fn gateway_idle_requires_zero_main_flow_traffic() {
        // 心跳 / ACE 有独立保留槽位，不影响后台请求（criticalPending=2 依旧空闲）
        let load = GatewayLoadSnapshot { critical_pending: 2, ..Default::default() };
        assert!(is_gateway_idle_for_low_priority(&load));

        assert!(!is_gateway_idle_for_low_priority(&GatewayLoadSnapshot {
            blocking_queued: 1,
            ..Default::default()
        }));
        assert!(!is_gateway_idle_for_low_priority(&GatewayLoadSnapshot {
            business_pending: 1,
            ..Default::default()
        }));
        // 已经有后台扫描占着唯一的 background 槽位时不再叠加
        assert!(!is_gateway_idle_for_low_priority(&GatewayLoadSnapshot {
            background_pending: 1,
            ..Default::default()
        }));
        // 心跳漏过说明连接本身可疑，后台请求一律不发
        assert!(!is_gateway_idle_for_low_priority(&GatewayLoadSnapshot {
            heartbeat_misses: 1,
            ..Default::default()
        }));
    }

    #[test]
    fn stalled_pending_counts_as_gateway_silence() {
        // 服务端静默时主流程请求会挂十几秒，这种连接上一个后台请求都不该再加
        let stalled = GatewayLoadSnapshot { oldest_pending_age_ms: GATEWAY_STALL_PENDING_MS, ..Default::default() };
        assert!(!is_gateway_idle_for_low_priority(&stalled));
        assert!(!is_gateway_idle_for_low_priority(&GatewayLoadSnapshot {
            oldest_pending_age_ms: 18_136,
            ..Default::default()
        }));
        // 刚发出去的心跳还在正常等回包，不算静默
        assert!(is_gateway_idle_for_low_priority(&GatewayLoadSnapshot {
            oldest_pending_age_ms: 200,
            ..Default::default()
        }));
        // 健康度闸门同样卡静默
        assert!(!is_gateway_healthy_for_business(&stalled));
    }

    #[test]
    fn business_gate_allows_normal_contention() {
        // 前台操作和自己农场的请求在飞、队列里有活，都属于正常竞争，定时任务照常跑
        assert!(is_gateway_healthy_for_business(&GatewayLoadSnapshot {
            blocking_queued: 3,
            business_pending: 3,
            oldest_pending_age_ms: 1_200,
            ..Default::default()
        }));
        assert!(is_gateway_healthy_for_business(&GatewayLoadSnapshot {
            background_pending: 1,
            oldest_pending_age_ms: 0,
            ..Default::default()
        }));
        // 心跳漏拍说明服务端静默，本轮必须让路给心跳和 ACE
        assert!(!is_gateway_healthy_for_business(&GatewayLoadSnapshot {
            heartbeat_misses: 1,
            ..Default::default()
        }));
    }

    #[test]
    fn business_backoff_first_30s_then_double_capped_60s() {
        assert_eq!(next_business_backoff_ms(0), BUSINESS_BACKOFF_MIN_MS);
        assert_eq!(next_business_backoff_ms(BUSINESS_BACKOFF_MIN_MS), BUSINESS_BACKOFF_MAX_MS);
        assert_eq!(next_business_backoff_ms(BUSINESS_BACKOFF_MAX_MS), BUSINESS_BACKOFF_MAX_MS);
    }

    // ----- RpcScheduler 容量记账（tokio 异步集成） -----

    mod scheduler {
        use super::*;

        async fn grant_of(scheduler: &Arc<RpcScheduler>, class: RequestClass) -> InFlightGuard {
            scheduler
                .try_enqueue(class, None)
                .expect("enqueue")
                .granted()
                .await
                .expect("grant")
        }

        #[tokio::test]
        async fn grants_immediately_when_idle() {
            let scheduler = RpcScheduler::new();
            let guard = grant_of(&scheduler, RequestClass::Foreground).await;
            assert_eq!(scheduler.in_flight_count(), 1);
            assert_eq!(scheduler.load().foreground_pending, 1);
            drop(guard);
            assert_eq!(scheduler.in_flight_count(), 0);
        }

        #[tokio::test]
        async fn farm_waits_while_business_slots_saturated() {
            let scheduler = RpcScheduler::new();
            // 前台可以并发 3 个
            let f1 = grant_of(&scheduler, RequestClass::Foreground).await;
            let f2 = grant_of(&scheduler, RequestClass::Foreground).await;
            let f3 = grant_of(&scheduler, RequestClass::Foreground).await;
            assert_eq!(scheduler.in_flight_count(), 3);
            // farm 只能在前台等待时排队（非前台额度为 0）
            let ticket = scheduler.try_enqueue(RequestClass::Farm, None).expect("farm queued");
            let farm_grant = ticket.granted();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            // farm 仍在排队（被前台占满 + 前台在排队时不让路）
            assert_eq!(scheduler.queued_count(), 1);
            drop((f1, f2, f3));
            // 全部释放后 farm 立即拿到授权
            let guard = tokio::time::timeout(std::time::Duration::from_millis(500), farm_grant)
                .await
                .expect("farm granted after release")
                .expect("guard");
            assert_eq!(scheduler.in_flight_count(), 1);
            drop(guard);
        }

        #[tokio::test]
        async fn queue_full_is_per_class() {
            let scheduler = RpcScheduler::new();
            // 先占住唯一的 background 在途槽，后续 background 只能排队
            let bg = grant_of(&scheduler, RequestClass::Background).await;
            // 把 background 的排队配额填满
            let mut tickets = Vec::new();
            for _ in 0..max_queued_for_class(RequestClass::Background) {
                tickets
                    .push(scheduler.try_enqueue(RequestClass::Background, None).expect("bg queued"));
            }
            // background 配额满 → 拒绝；其它班次的名额不受影响
            assert!(scheduler.try_enqueue(RequestClass::Background, None).is_err());
            assert!(scheduler.try_enqueue(RequestClass::Critical, None).is_ok());
            assert!(scheduler.try_enqueue(RequestClass::Foreground, None).is_ok());
            // critical / foreground 排队后都会被授权（空闲门只拦 background）
            let _g = grant_of(&scheduler, RequestClass::Critical).await;
            let _f = grant_of(&scheduler, RequestClass::Foreground).await;
            drop((bg, tickets));
        }

        #[tokio::test]
        async fn dropped_ticket_releases_queue_slot() {
            let scheduler = RpcScheduler::new();
            // 占住 background 在途槽，让后续 background 排队
            let bg = grant_of(&scheduler, RequestClass::Background).await;
            let ticket = scheduler.try_enqueue(RequestClass::Background, None).expect("queued");
            drop(ticket);
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            // 触发一次调度做回收
            scheduler.drain();
            assert_eq!(scheduler.queued_count(), 0);
            drop(bg);
        }

        #[tokio::test]
        async fn reject_all_wakes_queued_with_none() {
            let scheduler = RpcScheduler::new();
            // 占住 farm 班次的在途槽，让第二条 farm 排队（farm 每班次上限 1）
            let _farm1 = grant_of(&scheduler, RequestClass::Farm).await;
            let ticket = scheduler.try_enqueue(RequestClass::Farm, None).expect("queued");
            let wait = ticket.granted();
            assert_eq!(scheduler.queued_count(), 1);
            assert_eq!(scheduler.reject_all_queued(), 1);
            let granted =
                tokio::time::timeout(std::time::Duration::from_millis(100), wait).await.expect("woken");
            assert!(granted.is_none(), "reject_all 应让排队方拿到 None");
        }

        #[tokio::test]
        async fn heartbeat_lane_granted_while_business_saturated() {
            let scheduler = RpcScheduler::new();
            let _f1 = grant_of(&scheduler, RequestClass::Foreground).await;
            let _f2 = grant_of(&scheduler, RequestClass::Foreground).await;
            let _f3 = grant_of(&scheduler, RequestClass::Foreground).await;
            // 业务全满时心跳仍立即拿到保留通道
            let (class, lane) = resolve_request_class("Heartbeat", None);
            let ticket = scheduler.try_enqueue(class, lane).expect("hb queued");
            let guard = tokio::time::timeout(std::time::Duration::from_millis(100), ticket.granted())
                .await
                .expect("heartbeat granted immediately")
                .expect("guard");
            assert_eq!(scheduler.load().critical_pending, 1);
            drop(guard);
        }
    }
}
