//! 萌宠成长日记门面。写操作使用该账号当前 worker 上的同一服务。

use std::sync::Arc;

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

fn require_account(account_id: &str) -> AppResult<()> {
    qq_farm_core::services::pet_diary::require_pet_account(account_id).map_err(AppError::from_core)
}

pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    require_account(account_id)?;
    require_worker_loop(ctx, account_id)?.pet_diary().snapshot().await.map_err(AppError::from_core)
}

pub async fn feed(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.feed_if(still).await }).await
}

pub async fn draw(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.draw_if(still).await }).await
}

pub async fn initialize(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.initialize_if(still).await }).await
}

pub async fn claim_dog(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.claim_dog_if(still).await }).await
}

pub async fn claim_story(ctx: &AppContext, account_id: &str, order: i64) -> AppResult<Value> {
    call(ctx, account_id, move |service, still| async move {
        service.claim_story_if(order, still).await
    })
    .await
}

pub async fn claim_seeds(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.claim_seeds_if(still).await }).await
}

pub async fn refresh_charm(
    ctx: &AppContext,
    account_id: &str,
    payment: &str,
    expected_paid: Option<i64>,
) -> AppResult<Value> {
    let payment = payment.to_owned();
    call(ctx, account_id, move |service, still| async move {
        service.refresh_charm_if(&payment, expected_paid, still).await
    })
    .await
}

pub async fn equip_charm(ctx: &AppContext, account_id: &str, charm_id: i64) -> AppResult<Value> {
    call(ctx, account_id, move |service, still| async move {
        service.equip_charm_if(charm_id, still).await
    })
    .await
}

pub async fn open_treasure(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(ctx, account_id, |service, still| async move { service.open_treasure_if(still).await })
        .await
}

pub async fn claim_compensation(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    call(
        ctx,
        account_id,
        |service, still| async move { service.claim_compensation_if(still).await },
    )
    .await
}

pub async fn exchange(
    ctx: &AppContext,
    account_id: &str,
    goods_id: i64,
    count: i64,
) -> AppResult<Value> {
    call(ctx, account_id, move |service, still| async move {
        service.exchange_if(goods_id, count, still).await
    })
    .await
}

pub async fn claim_solar(ctx: &AppContext, account_id: &str, term_id: i64) -> AppResult<Value> {
    call(ctx, account_id, move |service, still| async move {
        service.claim_solar_if(term_id, still).await
    })
    .await
}

pub async fn battle(
    ctx: &AppContext,
    account_id: &str,
    gid: i64,
    treasure_id: &str,
    challenge_id: i64,
) -> AppResult<Value> {
    let treasure_id = treasure_id.to_owned();
    call(ctx, account_id, move |service, still| async move {
        service.battle_if(gid, &treasure_id, challenge_id, still).await
    })
    .await
}

async fn call<F, Fut>(ctx: &AppContext, account_id: &str, invoke: F) -> AppResult<Value>
where
    F: FnOnce(
        Arc<qq_farm_core::services::pet_diary::PetDiaryService>,
        Box<dyn Fn() -> bool + Send>,
    ) -> Fut,
    Fut: std::future::Future<Output = qq_farm_core::error::Result<Value>>,
{
    require_account(account_id)?;
    let worker = require_worker_loop(ctx, account_id)?;
    let service = worker.pet_diary();
    let engine = Arc::clone(&ctx.engine);
    let account = account_id.to_owned();
    let worker = Arc::clone(&worker);
    invoke(
        service,
        Box::new(move || {
            engine.worker_loop(&account).is_some_and(|found| Arc::ptr_eq(&found, &worker))
        }),
    )
    .await
    .map_err(AppError::from_core)
}
