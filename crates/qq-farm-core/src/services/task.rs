//! 任务系统 — 任务 / 活跃度 / 图鉴奖励自动领取。
//!
//! 1:1 翻译原 `core/src/services/task.ts`（451 行）。
//!
//! ## 协议
//!
//! - `gamepb.taskpb.TaskService.TaskInfo` — 拉取任务信息
//! - `gamepb.taskpb.TaskService.ClaimTaskReward` — 领取单个任务奖励
//! - `gamepb.taskpb.TaskService.ClaimDailyReward` — 领取活跃度奖励
//! - `gamepb.illustratedpb.IllustratedService.ClaimAllRewardsV2` — 领取图鉴奖励
//!
//! ## 业务
//!
//! - 任务分类：成长（task_type=1）/ 每日（task_type=2）/ 其他
//! - `is_automation_on("task")` 关闭时不执行
//! - 同一进程串行化：`checking` 标志防止重入
//! - 任务可分享翻倍（`share_multiple > 1`）：自动启用 `do_shared`
//! - 活跃度奖励：每条 active 找出 `status == DONE`（=2）的 reward
//! - 图鉴奖励：领取前后比点券差，< 200 视为"没有奖励"（避免假阳性）
//!
//! ## 与原 TS 的差异
//!
//! - 原 TS 用 `networkEvents.on('taskInfoNotify', ...)` 订阅推送
//!   本实现提供 `on_task_info_notify` 公开方法，由 runtime engine 在收到 WS
//!   推送时手动调用
//! - 原 TS 用 `createScheduler` 触发 debounce
//!   本实现提供 `trigger_check_and_claim` 直接入口（runtime 负责去重 / 调度）
//! - 自动化开关默认开启（`is_automation_on` 默认返回 `true`），runtime 可
//!   通过 `services::automation::set_automation_flag` 覆盖

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use prost::Message;
use serde::Serialize;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item;
use crate::proto::generated::gamepb::illustratedpb::{
    ClaimAllRewardsV2Reply, ClaimAllRewardsV2Request,
};
use crate::proto::generated::gamepb::taskpb::{
    ClaimDailyRewardReply, ClaimDailyRewardRequest, ClaimTaskRewardReply, ClaimTaskRewardRequest,
    Task, TaskInfo, TaskInfoReply, TaskInfoRequest,
};

use super::automation::{category, is_automation_on};
use super::stats::record_operation_for;
use super::warehouse::{get_bag_items, WarehouseService};

const TASK_SERVICE: &str = "gamepb.taskpb.TaskService";
const ILLUSTRATED_SERVICE: &str = "gamepb.illustratedpb.IllustratedService";

/// 单条任务 DTO
#[derive(Debug, Clone, Serialize)]
pub struct TaskDto {
    pub id: i64,
    pub desc: String,
    /// `daily` / `growth` / `main`
    pub category: &'static str,
    pub progress: i64,
    pub total_progress: i64,
    pub is_claimed: bool,
    pub is_unlocked: bool,
    pub share_multiple: i64,
    pub rewards: Vec<RewardItemDto>,
    pub can_claim: bool,
}

/// 简化奖励
#[derive(Debug, Clone, Serialize)]
pub struct RewardItemDto {
    pub id: i64,
    pub count: i64,
}

/// 成长任务状态（带明细）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrowthTaskItem {
    pub id: i64,
    pub desc: String,
    pub progress: i64,
    pub total_progress: i64,
    pub is_claimed: bool,
    pub is_unlocked: bool,
    pub is_completed: bool,
}

/// 每日任务状态（用于 app 展示）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDailyStateLikeApp {
    pub key: &'static str,
    pub done_today: bool,
    pub last_claim_at: i64,
    pub claimable_count: i32,
    pub pending_count: i32,
    pub completed_count: i32,
    pub total_count: i32,
}

/// 成长任务状态（用于 app 展示）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrowthTaskStateLikeApp {
    pub key: &'static str,
    pub done_today: bool,
    pub completed_count: i32,
    pub total_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_task: Option<GrowthTaskItem>,
    pub tasks: Vec<GrowthTaskItem>,
}

/// 通用每日状态
#[derive(Debug, Clone, Serialize)]
pub struct TaskClaimDailyState {
    pub key: &'static str,
    pub done_today: bool,
    pub last_claim_at: i64,
}

/// 活跃度领取结果
#[derive(Debug, Clone, Default)]
pub struct ActiveClaimResult {
    pub scanned: i32,
    pub claimed: i32,
    pub errors: i32,
}

/// 任务服务
pub struct TaskService {
    gateway: Arc<Gateway>,

    checking: Mutex<bool>,
    task_claim_done_date_key: Mutex<String>,
    task_claim_last_at: Mutex<i64>,
    account_id: Mutex<String>,
}

impl TaskService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            checking: Mutex::new(false),
            task_claim_done_date_key: Mutex::new(String::new()),
            task_claim_last_at: Mutex::new(0),
            account_id: Mutex::new(String::new()),
        }
    }

    /// 绑定账号（登录后调用，隔离 taskClaim 计数）
    pub fn set_account_id(&self, account_id: &str) {
        *self.account_id.lock() = account_id.to_string();
    }

    /// 拉取任务信息
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn get_task_info(&self) -> Result<TaskInfoReply> {
        let body = self
            .gateway
            .request(
                TASK_SERVICE,
                "TaskInfo",
                &TaskInfoRequest {}.encode_to_vec(),
                10_000,
            )
            .await?;
        Ok(TaskInfoReply::decode(&body[..])?)
    }

    /// 领取单个任务奖励
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn claim_task_reward(
        &self,
        task_id: i64,
        do_shared: bool,
    ) -> Result<ClaimTaskRewardReply> {
        let req = ClaimTaskRewardRequest {
            id: task_id,
            do_shared,
        };
        let body = self
            .gateway
            .request(TASK_SERVICE, "ClaimTaskReward", &req.encode_to_vec(), 10_000)
            .await?;
        Ok(ClaimTaskRewardReply::decode(&body[..])?)
    }

    /// 领取活跃度奖励
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn claim_daily_reward(
        &self,
        active_type: i32,
        point_ids: Vec<i64>,
    ) -> Result<ClaimDailyRewardReply> {
        let req = ClaimDailyRewardRequest {
            r#type: active_type,
            point_ids,
        };
        let body = self
            .gateway
            .request(TASK_SERVICE, "ClaimDailyReward", &req.encode_to_vec(), 10_000)
            .await?;
        Ok(ClaimDailyRewardReply::decode(&body[..])?)
    }

    /// 领取图鉴所有奖励
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn claim_all_illustrated_rewards(&self) -> Result<ClaimAllRewardsV2Reply> {
        let req = ClaimAllRewardsV2Request {
            only_claimable: true,
        };
        let body = self
            .gateway
            .request(
                ILLUSTRATED_SERVICE,
                "ClaimAllRewardsV2",
                &req.encode_to_vec(),
                10_000,
            )
            .await?;
        Ok(ClaimAllRewardsV2Reply::decode(&body[..])?)
    }

    /// 主入口：检查并自动领取所有任务 / 活跃度 / 图鉴奖励
    pub async fn check_and_claim_tasks(&self) {
        if *self.checking.lock() {
            return;
        }
        if !is_automation_on(category::TASK) {
            return;
        }
        *self.checking.lock() = true;

        let result = self.do_check_and_claim().await;
        if let Err(e) = result {
            tracing::warn!("[任务] 检查任务失败: {}", e);
        }
        *self.checking.lock() = false;
    }

    async fn do_check_and_claim(&self) -> Result<()> {
        let reply = self.get_task_info().await?;
        let task_info = match reply.task_info {
            Some(ti) => ti,
            None => return Ok(()),
        };
        let normalized = normalize_task_info(&task_info);

        let daily_claimable = analyze_task_list(&normalized.daily_tasks, "daily");
        let growth_claimable = analyze_task_list(&normalized.growth_tasks, "growth");
        let main_claimable = analyze_task_list(&normalized.other_tasks, "main");
        let claimable: Vec<TaskDto> = daily_claimable
            .iter()
            .chain(growth_claimable.iter())
            .chain(main_claimable.iter())
            .cloned()
            .collect();
        if !claimable.is_empty() {
            tracing::info!("[任务] 发现 {} 个可领取任务", claimable.len());
            crate::services::panel_log::log(
                &self.account_id.lock(),
                "任务",
                format!("发现 {} 个可领取任务", claimable.len()),
                Some(serde_json::json!({ "module": "task", "event": "检查任务", "count": claimable.len() })),
            );
            if !daily_claimable.is_empty() {
                let descs: Vec<&str> = daily_claimable.iter().map(|t| t.desc.as_str()).collect();
                tracing::info!("[任务] 每日任务可领取: {}", descs.join("，"));
                crate::services::panel_log::log(
                    &self.account_id.lock(),
                    "任务",
                    format!("每日任务可领取: {}", descs.join("，")),
                    Some(serde_json::json!({ "module": "task", "event": "每日任务" })),
                );
            }
            let mut daily_claim_success: i32 = 0;
            for task in &claimable {
                let ok = self.do_claim(task).await;
                if task.category == "daily" && ok {
                    daily_claim_success += 1;
                }
            }
            if !daily_claimable.is_empty() && daily_claim_success == 0 {
                tracing::info!("[任务] 每日任务本次未领取成功");
            }
        }
        self.check_and_claim_actives(&normalized.actives).await;
        self.check_and_claim_illustrated_rewards().await;
        Ok(())
    }

    /// 领取单个任务（公开入口，供手动调用）
    pub async fn do_claim(&self, task: &TaskDto) -> bool {
        let use_share = task.share_multiple > 1;
        let multiple_str = if use_share {
            format!(" ({}倍)", task.share_multiple)
        } else {
            String::new()
        };
        match self.claim_task_reward(task.id, use_share).await {
            Ok(reply) => {
                let reward = get_reward_summary(&reply.items);
                let reward_str = if reply.items.is_empty() {
                    "无".to_string()
                } else if reward.is_empty() {
                    "无".to_string()
                } else {
                    reward
                };
                let category_name = match task.category {
                    "daily" => "每日任务",
                    "growth" => "成长任务",
                    _ => "任务",
                };
                tracing::info!(
                    "[任务] 领取({}): {}{} → {}",
                    category_name,
                    task.desc,
                    multiple_str,
                    reward_str
                );
                crate::services::panel_log::log(
                    &self.account_id.lock(),
                    "任务",
                    format!("领取({category_name}): {}{multiple_str} → {reward_str}", task.desc),
                    Some(serde_json::json!({ "module": "task", "event": "领取任务" })),
                );
                *self.task_claim_done_date_key.lock() = get_date_key();
                *self.task_claim_last_at.lock() = now_ms();
                record_operation_for(&self.account_id.lock(), "taskClaim", 1);
                tokio::time::sleep(Duration::from_millis(300)).await;
                if task.category == "growth" {
                    // 服务端一次只暴露一条成长链任务；领取后再拉一次才能立刻看到下一条。
                    let _ = self.get_task_info().await;
                }
                true
            }
            Err(_) => false,
        }
    }

    /// 扫描并领取活跃度奖励
    pub async fn check_and_claim_actives(&self, actives: &[crate::proto::generated::gamepb::taskpb::Active]) -> ActiveClaimResult {
        let mut result = ActiveClaimResult::default();
        for active in actives {
            let active_type = active.r#type;
            let claimable: Vec<_> = active
                .rewards
                .iter()
                .filter(|r| r.status == 2) // ActiveStatus::DONE
                .collect();
            if claimable.is_empty() {
                continue;
            }
            result.scanned += claimable.len() as i32;
            let point_ids: Vec<i64> = claimable
                .iter()
                .map(|r| r.point_id)
                .filter(|id| *id > 0)
                .collect();
            if point_ids.is_empty() {
                continue;
            }
            let type_name = match active_type {
                1 => "日活跃",
                2 => "周活跃",
                _ => &format!("活跃{}", active_type),
            };
            tracing::info!(
                "[活跃] {} 发现 {} 个可领取奖励",
                type_name,
                point_ids.len()
            );
            crate::services::panel_log::log(
                &self.account_id.lock(),
                "活跃",
                format!("{type_name} 发现 {} 个可领取奖励", point_ids.len()),
                Some(serde_json::json!({ "module": "task", "event": "活跃度" })),
            );
            match self.claim_daily_reward(active_type, point_ids.clone()).await {
                Ok(reply) => {
                    if !reply.items.is_empty() {
                        let reward = get_reward_summary(&reply.items);
                        if !reward.is_empty() {
                            tracing::info!("[活跃] {} 领取: {}", type_name, reward);
                            crate::services::panel_log::log(
                                &self.account_id.lock(),
                                "活跃",
                                format!("{type_name} 领取: {reward}"),
                                Some(serde_json::json!({ "module": "task", "event": "活跃度" })),
                            );
                        }
                    }
                    result.claimed += point_ids.len() as i32;
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
                Err(e) => {
                    result.errors += 1;
                    tracing::warn!("[活跃] {} 领取失败: {}", type_name, e);
                    crate::services::panel_log::log_warn(
                        &self.account_id.lock(),
                        "活跃",
                        format!("{type_name} 领取失败: {e}"),
                        Some(serde_json::json!({ "module": "task", "event": "活跃度" })),
                    );
                }
            }
        }
        result
    }

    /// 领取图鉴奖励（点券变化 < 200 视为"无奖励"）
    pub async fn check_and_claim_illustrated_rewards(&self) -> bool {
        let before = get_ticket_balance_from_bag(&self.gateway).await;
        let reply = match self.claim_all_illustrated_rewards().await {
            Ok(r) => r,
            Err(_) => return false,
        };
        let after = get_ticket_balance_from_bag(&self.gateway).await;
        let gain = (after - before).max(0);
        if gain < 200 {
            return false;
        }
        let total_items = reply.items.len() + reply.bonus_items.len();
        tracing::info!("[任务] 领取成功: 点券{}", gain);
        crate::services::panel_log::log(
            &self.account_id.lock(),
            "任务",
            format!("领取成功: 点券{gain}"),
            Some(serde_json::json!({ "module": "task", "event": "图鉴" })),
        );
        *self.task_claim_done_date_key.lock() = get_date_key();
        *self.task_claim_last_at.lock() = now_ms();
        record_operation_for(&self.account_id.lock(), "taskClaim", 1);
        let _ = total_items;
        true
    }

    /// 处理来自 WS 推送的 `TaskInfoNotify`
    pub async fn on_task_info_notify(&self, task_info: &TaskInfo) {
        if !is_automation_on(category::TASK) {
            return;
        }
        let normalized = normalize_task_info(task_info);
        let daily_claimable = analyze_task_list(&normalized.daily_tasks, "daily");
        let growth_claimable = analyze_task_list(&normalized.growth_tasks, "growth");
        let main_claimable = analyze_task_list(&normalized.other_tasks, "main");
        let claimable: Vec<TaskDto> = daily_claimable
            .iter()
            .chain(growth_claimable.iter())
            .chain(main_claimable.iter())
            .cloned()
            .collect();
        let has_claimable = !claimable.is_empty();
        let actives = normalized.actives.clone();
        if !has_claimable && actives.is_empty() {
            return;
        }
        if has_claimable {
            tracing::info!(
                "[任务] 有 {} 个任务可领取，准备自动领取...",
                claimable.len()
            );
        }
        if has_claimable {
            for task in &claimable {
                self.do_claim(task).await;
            }
        }
        self.check_and_claim_actives(&actives).await;
        self.check_and_claim_illustrated_rewards().await;
    }

    /// 简单每日状态
    #[must_use]
    pub fn get_task_claim_daily_state(&self) -> TaskClaimDailyState {
        TaskClaimDailyState {
            key: "task_claim",
            done_today: *self.task_claim_done_date_key.lock() == get_date_key(),
            last_claim_at: *self.task_claim_last_at.lock(),
        }
    }

    /// App 风格每日任务状态（含完成 / 待领取 / 可领取数）
    ///
    /// # Errors
    /// - 网络 / 网关错误
    pub async fn get_task_daily_state_like_app(&self) -> TaskDailyStateLikeApp {
        match self.get_task_info().await {
            Ok(reply) => {
                let ti = match reply.task_info {
                    Some(t) => t,
                    None => {
                        return TaskDailyStateLikeApp {
                            key: "task_claim",
                            done_today: false,
                            last_claim_at: *self.task_claim_last_at.lock(),
                            claimable_count: 0,
                            pending_count: 0,
                            completed_count: 0,
                            total_count: 3,
                        };
                    }
                };
                let normalized = normalize_task_info(&ti);
                let daily_all = &normalized.daily_tasks;
                let completed_daily: Vec<_> = daily_all
                    .iter()
                    .filter(|t| t.total_progress > 0 && t.progress >= t.total_progress)
                    .collect();
                let completed_count = completed_daily.len().min(3) as i32;
                let pending_daily: Vec<_> = daily_all
                    .iter()
                    .filter(|t| t.is_unlocked && !t.is_claimed && t.total_progress > 0)
                    .collect();
                let daily_claimable = analyze_task_list(daily_all, "daily");
                TaskDailyStateLikeApp {
                    key: "task_claim",
                    done_today: completed_count >= 3,
                    last_claim_at: *self.task_claim_last_at.lock(),
                    claimable_count: daily_claimable.len() as i32,
                    pending_count: pending_daily.len() as i32,
                    completed_count,
                    total_count: 3,
                }
            }
            Err(_) => TaskDailyStateLikeApp {
                key: "task_claim",
                done_today: false,
                last_claim_at: *self.task_claim_last_at.lock(),
                claimable_count: 0,
                pending_count: 0,
                completed_count: 0,
                total_count: 3,
            },
        }
    }

    /// App 风格成长任务状态（对齐 bot `getGrowthTaskStateLikeApp`）
    ///
    /// # Errors
    /// - 网络 / 网关错误
    pub async fn get_growth_task_state_like_app(&self) -> GrowthTaskStateLikeApp {
        match self.get_task_info().await {
            Ok(reply) => {
                let ti = match reply.task_info {
                    Some(t) => t,
                    None => {
                        return GrowthTaskStateLikeApp {
                            key: "growth_task",
                            done_today: false,
                            completed_count: 0,
                            total_count: 0,
                            current_task: None,
                            tasks: vec![],
                        };
                    }
                };
                let normalized = normalize_task_info(&ti);
                let tasks: Vec<GrowthTaskItem> = normalized
                    .growth_tasks
                    .iter()
                    .map(|t| {
                        let progress = t.progress.max(0);
                        let total_progress = t.total_progress.max(0);
                        GrowthTaskItem {
                            id: t.id,
                            desc: if t.desc.is_empty() {
                                format!("成长任务#{}", t.id)
                            } else {
                                t.desc.clone()
                            },
                            progress,
                            total_progress,
                            is_claimed: t.is_claimed,
                            is_unlocked: t.is_unlocked,
                            is_completed: total_progress > 0 && progress >= total_progress,
                        }
                    })
                    .collect();
                let current_task = tasks
                    .iter()
                    .find(|t| t.is_unlocked && !t.is_claimed)
                    .cloned()
                    .or_else(|| tasks.first().cloned());
                GrowthTaskStateLikeApp {
                    key: "growth_task",
                    // 成长任务是链，不是每日清单；空列表表示链已完成。
                    done_today: false,
                    completed_count: current_task
                        .as_ref()
                        .map(|t| t.progress.min(t.total_progress) as i32)
                        .unwrap_or(0),
                    total_count: current_task
                        .as_ref()
                        .map(|t| t.total_progress as i32)
                        .unwrap_or(0),
                    current_task,
                    tasks,
                }
            }
            Err(_) => GrowthTaskStateLikeApp {
                key: "growth_task",
                done_today: false,
                completed_count: 0,
                total_count: 0,
                current_task: None,
                tasks: vec![],
            },
        }
    }
}

// =====================================================================
// 辅助 / 纯函数
// =====================================================================

/// 归一化任务信息
///
/// 1:1 对齐原 TS `normalizeTaskInfo`：
/// - 旧版 task_info 含 `tasks`（混合）/`growth_tasks`/`daily_tasks`
/// - 把 `tasks` 按 `task_type` 拆分为成长 / 每日 / 其他
/// - 去重（按 id）
/// - `actives` 原样返回
#[must_use]
pub fn normalize_task_info(task_info: &TaskInfo) -> NormalizedTaskInfo {
    let mut growth_tasks: Vec<Task> = Vec::new();
    let mut daily_tasks: Vec<Task> = Vec::new();
    let mut other_tasks: Vec<Task> = Vec::new();
    let mut seen_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();

    let mut append = |task: Task, target: &mut Vec<Task>| {
        let id = task.id;
        if id > 0 {
            if !seen_ids.insert(id) {
                return;
            }
        }
        target.push(task);
    };

    for task in &task_info.tasks {
        let task_type = task.task_type;
        if task_type == 1 {
            append(task.clone(), &mut growth_tasks);
        } else if task_type == 2 {
            append(task.clone(), &mut daily_tasks);
        } else {
            append(task.clone(), &mut other_tasks);
        }
    }
    for task in &task_info.growth_tasks {
        append(task.clone(), &mut growth_tasks);
    }
    for task in &task_info.daily_tasks {
        append(task.clone(), &mut daily_tasks);
    }

    NormalizedTaskInfo {
        growth_tasks,
        daily_tasks,
        other_tasks,
        actives: task_info.actives.clone(),
    }
}

/// 归一化结果
#[derive(Debug, Clone, Default)]
pub struct NormalizedTaskInfo {
    pub growth_tasks: Vec<Task>,
    pub daily_tasks: Vec<Task>,
    pub other_tasks: Vec<Task>,
    pub actives: Vec<crate::proto::generated::gamepb::taskpb::Active>,
}

/// 格式化单条任务为 DTO
#[must_use]
pub fn format_task(t: &Task, category_name: &'static str) -> TaskDto {
    let total = t.total_progress;
    TaskDto {
        id: t.id,
        desc: if t.desc.is_empty() {
            format!("任务#{}", t.id)
        } else {
            t.desc.clone()
        },
        category: category_name,
        progress: t.progress,
        total_progress: total,
        is_claimed: t.is_claimed,
        is_unlocked: t.is_unlocked,
        share_multiple: t.share_multiple,
        rewards: t
            .rewards
            .iter()
            .map(|r| RewardItemDto {
                id: r.id,
                count: r.count,
            })
            .collect(),
        can_claim: t.is_unlocked && !t.is_claimed && t.progress >= total && total > 0,
    }
}

/// 从任务列表中找出可领取的任务
#[must_use]
pub fn analyze_task_list(tasks: &[Task], category_name: &'static str) -> Vec<TaskDto> {
    tasks
        .iter()
        .filter_map(|t| {
            let dto = format_task(t, category_name);
            if dto.id > 0 && dto.can_claim {
                Some(dto)
            } else {
                None
            }
        })
        .collect()
}

/// 汇总奖励为可读字符串
#[must_use]
pub fn get_reward_summary(items: &[Item]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for it in items {
        let id = it.id;
        let count = it.count;
        if count <= 0 {
            continue;
        }
        if id == 1 || id == 1001 {
            parts.push(format!("金币{}", count));
        } else if id == 2 || id == 1101 {
            parts.push(format!("经验{}", count));
        } else if id == 1002 {
            parts.push(format!("点券{}", count));
        } else {
            parts.push(format!("物品#{}x{}", id, count));
        }
    }
    parts.join("/")
}

/// 读取背包里点券（id=1002）的数量
async fn get_ticket_balance_from_bag(gateway: &Arc<Gateway>) -> i64 {
    match WarehouseService::get_bag_via(gateway).await {
        Ok(rep) => {
            for it in get_bag_items(&rep) {
                if it.id == 1002 {
                    return it.count.max(0);
                }
            }
            0
        }
        Err(_) => 0,
    }
}

fn get_date_key() -> String {
    // 对齐 TS `getDateKey`：服务器时间 + 北京时区（UTC+8）
    use chrono::Datelike;
    let server_secs = crate::utils::time::get_server_time_secs();
    let bj_ms = if server_secs > 0 {
        (server_secs as i64) * 1000 + 8 * 3600 * 1000
    } else {
        crate::utils::time::now_ms() + 8 * 3600 * 1000
    };
    let dt = chrono::DateTime::from_timestamp_millis(bj_ms)
        .unwrap_or_else(|| chrono::Utc::now());
    format!("{}-{:02}-{:02}", dt.year(), dt.month(), dt.day())
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::generated::gamepb::taskpb::{Active, ActiveReward};

    fn make_task(id: i64, task_type: i32, progress: i64, total: i64, claimed: bool) -> Task {
        Task {
            id,
            task_type,
            progress,
            total_progress: total,
            is_claimed: claimed,
            is_unlocked: true,
            desc: format!("任务{}", id),
            share_multiple: 0,
            ..Default::default()
        }
    }

    #[test]
    fn service_constant_matches_ts() {
        assert_eq!(TASK_SERVICE, "gamepb.taskpb.TaskService");
        assert_eq!(ILLUSTRATED_SERVICE, "gamepb.illustratedpb.IllustratedService");
    }

    #[test]
    fn reward_summary_empty() {
        assert_eq!(get_reward_summary(&[]), "");
    }

    #[test]
    fn reward_summary_gold() {
        let items = vec![Item { id: 1, count: 100, ..Default::default() }];
        assert_eq!(get_reward_summary(&items), "金币100");
    }

    #[test]
    fn reward_summary_ticket() {
        let items = vec![Item { id: 1002, count: 5, ..Default::default() }];
        assert_eq!(get_reward_summary(&items), "点券5");
    }

    #[test]
    fn reward_summary_unknown() {
        let items = vec![Item { id: 9999, count: 1, ..Default::default() }];
        assert_eq!(get_reward_summary(&items), "物品#9999x1");
    }

    #[test]
    fn format_task_can_claim() {
        let t = make_task(100, 2, 10, 10, false);
        let dto = format_task(&t, "daily");
        assert_eq!(dto.id, 100);
        assert_eq!(dto.category, "daily");
        assert!(dto.can_claim);
    }

    #[test]
    fn format_task_already_claimed() {
        let t = make_task(100, 2, 10, 10, true);
        let dto = format_task(&t, "daily");
        assert!(!dto.can_claim);
    }

    #[test]
    fn format_task_incomplete() {
        let t = make_task(100, 2, 5, 10, false);
        let dto = format_task(&t, "daily");
        assert!(!dto.can_claim);
    }

    #[test]
    fn format_task_zero_total() {
        let t = make_task(100, 2, 0, 0, false);
        let dto = format_task(&t, "daily");
        assert!(!dto.can_claim);
    }

    #[test]
    fn format_task_locked() {
        let mut t = make_task(100, 2, 10, 10, false);
        t.is_unlocked = false;
        let dto = format_task(&t, "daily");
        assert!(!dto.can_claim);
    }

    #[test]
    fn format_task_default_desc() {
        let mut t = make_task(999, 2, 0, 0, false);
        t.desc = String::new();
        let dto = format_task(&t, "daily");
        assert_eq!(dto.desc, "任务#999");
    }

    #[test]
    fn analyze_task_list_filters_unclaimable() {
        let tasks = vec![
            make_task(1, 2, 10, 10, false), // claimable
            make_task(2, 2, 10, 10, true),  // already claimed
            make_task(3, 2, 5, 10, false),  // incomplete
            make_task(0, 2, 0, 0, false),   // id == 0
        ];
        let claimable = analyze_task_list(&tasks, "daily");
        assert_eq!(claimable.len(), 1);
        assert_eq!(claimable[0].id, 1);
    }

    #[test]
    fn normalize_task_info_splits_by_type() {
        let info = TaskInfo {
            tasks: vec![
                make_task(1, 1, 0, 0, false),
                make_task(2, 2, 0, 0, false),
                make_task(3, 0, 0, 0, false),
            ],
            ..Default::default()
        };
        let n = normalize_task_info(&info);
        assert_eq!(n.growth_tasks.len(), 1);
        assert_eq!(n.daily_tasks.len(), 1);
        assert_eq!(n.other_tasks.len(), 1);
    }

    #[test]
    fn normalize_task_info_dedupes() {
        let info = TaskInfo {
            growth_tasks: vec![make_task(1, 1, 0, 0, false)],
            tasks: vec![make_task(1, 1, 0, 0, false)], // 重复
            ..Default::default()
        };
        let n = normalize_task_info(&info);
        assert_eq!(n.growth_tasks.len(), 1);
    }

    #[test]
    fn normalize_task_info_growth_daily_passed_through() {
        let info = TaskInfo {
            growth_tasks: vec![make_task(10, 1, 0, 0, false)],
            daily_tasks: vec![make_task(20, 2, 0, 0, false)],
            ..Default::default()
        };
        let n = normalize_task_info(&info);
        assert_eq!(n.growth_tasks[0].id, 10);
        assert_eq!(n.daily_tasks[0].id, 20);
    }

    #[test]
    fn normalize_task_info_passes_actives() {
        let info = TaskInfo {
            actives: vec![Active {
                r#type: 1,
                progress: 0,
                rewards: vec![ActiveReward {
                    point_id: 1,
                    need_progress: 10,
                    status: 2,
                    rewards: vec![],
                }],
            }],
            ..Default::default()
        };
        let n = normalize_task_info(&info);
        assert_eq!(n.actives.len(), 1);
        assert_eq!(n.actives[0].rewards.len(), 1);
    }

    #[test]
    fn normalize_task_info_default() {
        let info = TaskInfo::default();
        let n = normalize_task_info(&info);
        assert!(n.growth_tasks.is_empty());
        assert!(n.daily_tasks.is_empty());
        assert!(n.other_tasks.is_empty());
        assert!(n.actives.is_empty());
    }

    #[test]
    fn task_dto_includes_rewards() {
        let mut t = make_task(1, 2, 10, 10, false);
        t.rewards = vec![Item { id: 1, count: 100, ..Default::default() }];
        let dto = format_task(&t, "daily");
        assert_eq!(dto.rewards.len(), 1);
        assert_eq!(dto.rewards[0].id, 1);
        assert_eq!(dto.rewards[0].count, 100);
    }

    #[test]
    fn date_key_format() {
        let k = get_date_key();
        assert_eq!(k.len(), 10);
    }
}
