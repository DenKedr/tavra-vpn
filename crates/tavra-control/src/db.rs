//! Small shared database operations; all policy changes serialize on subscription rows.
use crate::{ApiError, Subscription};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;
pub async fn lock_subscription(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Subscription, ApiError> {
    Ok(sqlx::query_as("SELECT id,user_id,status,expires_at,device_limit,created_at FROM subscriptions WHERE id=$1 FOR UPDATE").bind(id).fetch_one(&mut **tx).await?)
}
pub async fn invalidate(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<(), ApiError> {
    sqlx::query("UPDATE devices SET status='revoked' WHERE subscription_id=$1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM challenges WHERE subscription_id=$1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Hold the generation shared lock until credential issuance/consumption commits.
/// Clock rollback takes the exclusive lock first, so it cannot race the final write.
pub async fn verify_generation(
    tx: &mut Transaction<'_, Postgres>,
    expected: i64,
) -> Result<(), ApiError> {
    let current: i64 =
        sqlx::query_scalar("SELECT generation FROM security_clock WHERE singleton=true FOR SHARE")
            .fetch_one(&mut **tx)
            .await?;
    if current != expected {
        return Err(ApiError::unauthorized());
    }
    Ok(())
}
