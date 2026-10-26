use crate::*;
use serde::Deserialize;
use uuid::Uuid;
const COLUMNS: &str = "id,user_id,status,expires_at,device_limit,created_at";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    user_id: Uuid,
    expires_at: i64,
    device_limit: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    status: Option<String>,
    expires_at: Option<i64>,
    device_limit: Option<i64>,
}
fn validate(expiry: i64, limit: i64) -> Result<(), ApiError> {
    if expiry < 0 || !(0..=1000000).contains(&limit) {
        Err(ApiError::bad("Invalid expiry or device limit"))
    } else {
        Ok(())
    }
}
pub async fn list(State(s): State<AppState>) -> ApiResult<Vec<Subscription>> {
    Ok(Json(
        sqlx::query_as(&format!(
            "SELECT {COLUMNS} FROM subscriptions ORDER BY created_at,id LIMIT 10000"
        ))
        .fetch_all(&s.pool)
        .await?,
    ))
}
pub async fn create(
    State(s): State<AppState>,
    Extension(VerifiedTime(now, _)): Extension<VerifiedTime>,
    ApiJson(i): ApiJson<Create>,
) -> ApiResult<Value> {
    validate(i.expires_at, i.device_limit)?;
    let invitation = auth::random_secret()?;
    let sub:Subscription=sqlx::query_as(&format!("INSERT INTO subscriptions(id,user_id,status,expires_at,device_limit,invitation_hash,created_at) VALUES($1,$2,'active',$3,$4,$5,$6) RETURNING {COLUMNS}")).bind(Uuid::new_v4()).bind(i.user_id).bind(i.expires_at).bind(i.device_limit).bind(auth::hash(&invitation)).bind(now).fetch_one(&s.pool).await?;
    let mut value = serde_json::to_value(sub).map_err(|_| ApiError::internal())?;
    value["invitation"] = json!(invitation);
    Ok(Json(value))
}
pub async fn update(
    State(s): State<AppState>,
    Extension(VerifiedTime(now, _)): Extension<VerifiedTime>,
    ApiId(id): ApiId,
    ApiJson(i): ApiJson<Update>,
) -> ApiResult<Subscription> {
    let mut tx = s.pool.begin().await?;
    let sub = db::lock_subscription(&mut tx, id).await?;
    let status = i.status.unwrap_or(sub.status);
    if status != "active" && status != "blocked" {
        return Err(ApiError::bad("Use revoke endpoint for revocation"));
    }
    // A revoked subscription is never resurrected, even with an active status update.
    let revoked: bool =
        sqlx::query_scalar("SELECT status='revoked' FROM subscriptions WHERE id=$1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    if revoked {
        return Err(ApiError::conflict("Subscription revoked"));
    }
    let expiry = i.expires_at.unwrap_or(sub.expires_at);
    let limit = i.device_limit.unwrap_or(sub.device_limit);
    validate(expiry, limit)?;
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM devices WHERE subscription_id=$1 AND status='active'",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if limit != 0 && limit < count {
        return Err(ApiError::conflict("Revoke devices before lowering limit"));
    }
    let updated=sqlx::query_as(&format!("UPDATE subscriptions SET status=$2,expires_at=$3,device_limit=$4 WHERE id=$1 RETURNING {COLUMNS}")).bind(id).bind(&status).bind(expiry).bind(limit).fetch_one(&mut *tx).await?;
    if status == "blocked" || (expiry != 0 && expiry <= now) {
        db::invalidate(&mut tx, id).await?;
    }
    tx.commit().await?;
    Ok(Json(updated))
}
pub async fn revoke(State(s): State<AppState>, ApiId(id): ApiId) -> ApiResult<Value> {
    let mut tx = s.pool.begin().await?;
    db::lock_subscription(&mut tx, id).await?;
    sqlx::query("UPDATE subscriptions SET status='revoked',invitation_hash=NULL WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    db::invalidate(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(json!({"revoked":true})))
}
pub async fn rotate(State(s): State<AppState>, ApiId(id): ApiId) -> ApiResult<Value> {
    let mut tx = s.pool.begin().await?;
    let sub = db::lock_subscription(&mut tx, id).await?;
    if sub.status == "revoked" {
        return Err(ApiError::conflict("Subscription revoked"));
    }
    let invitation = auth::random_secret()?;
    sqlx::query("UPDATE subscriptions SET invitation_hash=$2 WHERE id=$1")
        .bind(id)
        .bind(auth::hash(&invitation))
        .execute(&mut *tx)
        .await?;
    db::invalidate(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id,"invitation":invitation})))
}
