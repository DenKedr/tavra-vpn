//! Inventory-only transactions and append-only, credential-free audit snapshots.
use crate::ApiError;
use serde::Serialize;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub async fn begin(pool: &PgPool) -> Result<Transaction<'_, Postgres>, ApiError> {
    let mut tx = pool.begin().await?;
    // Small admin inventory serializes mutations consistently across processes.
    // This keeps server deletion/reassignment/audit atomic without row lock cycles.
    sqlx::query("SELECT pg_advisory_xact_lock(729840013)")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}
pub fn snapshot(value: &impl Serialize) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(|_| ApiError::internal())
}
pub async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    entity_type: &str,
    entity_id: Uuid,
    action: &str,
    before: Option<Value>,
    after: Option<Value>,
    now: i64,
) -> Result<(), ApiError> {
    sqlx::query("INSERT INTO inventory_audit(entity_type,entity_id,action,before_state,after_state,occurred_at) VALUES($1,$2,$3,$4,$5,$6)").bind(entity_type).bind(entity_id).bind(action).bind(before).bind(after).bind(now).execute(&mut **tx).await?;
    Ok(())
}
pub fn domain_error(error: sqlx::Error) -> ApiError {
    if matches!(error,sqlx::Error::Database(ref db) if db.is_unique_violation()) {
        ApiError::conflict("Domain already exists")
    } else {
        error.into()
    }
}

/// A PATCH field may be omitted, but a present nonnullable field cannot be null.
pub fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
