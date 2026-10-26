use crate::*;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use uuid::Uuid;
#[derive(Serialize, sqlx::FromRow)]
pub struct Device {
    id: Uuid,
    subscription_id: Uuid,
    public_key: String,
    status: String,
    created_at: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChallengeRequest {
    invitation: String,
    public_key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proof {
    challenge_id: Uuid,
    public_key: String,
    signature: String,
}
#[derive(sqlx::FromRow)]
struct Challenge {
    subscription_id: Uuid,
    public_key: String,
    nonce: String,
    expires_at: i64,
    consumed: bool,
    generation: i64,
}
fn public_key(value: &str) -> Result<VerifyingKey, ApiError> {
    if value.len() != 64 || value != value.to_lowercase() {
        return Err(ApiError::bad("Public key must be 32 lowercase hex bytes"));
    }
    let bytes: [u8; 32] = hex::decode(value)
        .map_err(|_| ApiError::bad("Invalid public key"))?
        .try_into()
        .map_err(|_| ApiError::bad("Invalid public key"))?;
    let key = VerifyingKey::from_bytes(&bytes).map_err(|_| ApiError::bad("Invalid public key"))?;
    if key.is_weak() {
        return Err(ApiError::bad("Weak public key"));
    }
    Ok(key)
}
async fn user_active(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
) -> Result<(), ApiError> {
    let status: String = sqlx::query_scalar("SELECT status FROM users WHERE id=$1 FOR SHARE")
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
    if status != "active" {
        return Err(ApiError::unauthorized());
    }
    Ok(())
}
fn active(sub: &Subscription, now: i64) -> Result<(), ApiError> {
    if sub.status != "active" || (sub.expires_at != 0 && sub.expires_at <= now) {
        Err(ApiError::unauthorized())
    } else {
        Ok(())
    }
}
pub async fn challenge(
    State(s): State<AppState>,
    Extension(VerifiedTime(now, generation)): Extension<VerifiedTime>,
    ApiJson(i): ApiJson<ChallengeRequest>,
) -> ApiResult<Value> {
    public_key(&i.public_key)?;
    if i.invitation.len() != 64 {
        return Err(ApiError::unauthorized());
    }
    let mut tx = s.pool.begin().await?;
    db::verify_generation(&mut tx, generation).await?;
    let ids: Option<(Uuid, Uuid)> =
        sqlx::query_as("SELECT id,user_id FROM subscriptions WHERE invitation_hash=$1")
            .bind(auth::hash(&i.invitation))
            .fetch_optional(&mut *tx)
            .await?;
    let (id, user_id) = ids.ok_or_else(ApiError::unauthorized)?;
    user_active(&mut tx, user_id).await?;
    let sub = db::lock_subscription(&mut tx, id).await?;
    active(&sub, now)?;
    // Recheck under the lock; rotate/revoke may have raced the initial lookup.
    let valid: bool =
        sqlx::query_scalar("SELECT invitation_hash=$2 FROM subscriptions WHERE id=$1")
            .bind(id)
            .bind(auth::hash(&i.invitation))
            .fetch_one(&mut *tx)
            .await?;
    if !valid {
        return Err(ApiError::unauthorized());
    }
    sqlx::query("DELETE FROM challenges WHERE expires_at <= $1 OR consumed")
        .bind(now)
        .execute(&mut *tx)
        .await?;
    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM challenges WHERE subscription_id=$1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    if pending >= 32 {
        return Err(ApiError::conflict("Too many pending challenges"));
    }
    let challenge_id = Uuid::new_v4();
    let nonce = auth::random_secret()?;
    let expires_at = now + 120;
    sqlx::query("INSERT INTO challenges(id,subscription_id,public_key,nonce,expires_at,generation) VALUES($1,$2,$3,$4,$5,$6)").bind(challenge_id).bind(id).bind(&i.public_key).bind(&nonce).bind(expires_at).bind(generation).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"challenge_id":challenge_id,"nonce":nonce,"expires_at":expires_at}),
    ))
}
pub async fn enroll(
    State(s): State<AppState>,
    Extension(VerifiedTime(now, generation)): Extension<VerifiedTime>,
    ApiJson(i): ApiJson<Proof>,
) -> ApiResult<Value> {
    let key = public_key(&i.public_key)?;
    if i.signature.len() != 128 {
        return Err(ApiError::bad("Invalid signature"));
    }
    let bytes = hex::decode(&i.signature).map_err(|_| ApiError::bad("Invalid signature"))?;
    let signature =
        Signature::from_slice(&bytes).map_err(|_| ApiError::bad("Invalid signature"))?;
    let mut tx = s.pool.begin().await?;
    db::verify_generation(&mut tx, generation).await?;
    let ids:Option<(Uuid,Uuid)>=sqlx::query_as("SELECT c.subscription_id,s.user_id FROM challenges c JOIN subscriptions s ON s.id=c.subscription_id WHERE c.id=$1").bind(i.challenge_id).fetch_optional(&mut *tx).await?;
    let (sub_id, user_id) = ids.ok_or_else(ApiError::unauthorized)?;
    user_active(&mut tx, user_id).await?;
    let sub = db::lock_subscription(&mut tx, sub_id).await?;
    active(&sub, now)?;
    let challenge:Option<Challenge>=sqlx::query_as("SELECT subscription_id,public_key,nonce,expires_at,consumed,generation FROM challenges WHERE id=$1 FOR UPDATE").bind(i.challenge_id).fetch_optional(&mut *tx).await?;
    let ch = challenge.ok_or_else(ApiError::unauthorized)?;
    if ch.generation != generation
        || ch.consumed
        || ch.expires_at <= now
        || ch.public_key != i.public_key
        || ch.subscription_id != sub_id
    {
        return Err(ApiError::unauthorized());
    }
    let payload = format!(
        "tavra-enroll-v1\n{}\n{}\n{}",
        i.challenge_id, ch.nonce, i.public_key
    );
    let valid = key.verify_strict(payload.as_bytes(), &signature).is_ok();
    // Even an invalid proof consumes this challenge. Commit before rejecting.
    sqlx::query("UPDATE challenges SET consumed=true WHERE id=$1")
        .bind(i.challenge_id)
        .execute(&mut *tx)
        .await?;
    if !valid {
        tx.commit().await?;
        return Err(ApiError::unauthorized());
    }
    let existing: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id,status FROM devices WHERE subscription_id=$1 AND public_key=$2")
            .bind(sub_id)
            .bind(&i.public_key)
            .fetch_optional(&mut *tx)
            .await?;
    let device_id = if let Some((id, status)) = existing {
        if status != "active" {
            tx.commit().await?;
            return Err(ApiError::unauthorized());
        }
        id
    } else {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM devices WHERE subscription_id=$1 AND status='active'",
        )
        .bind(sub_id)
        .fetch_one(&mut *tx)
        .await?;
        if sub.device_limit != 0 && count >= sub.device_limit {
            tx.commit().await?;
            return Err(ApiError::conflict("Device limit reached"));
        }
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO devices(id,subscription_id,public_key,status,created_at) VALUES($1,$2,$3,'active',$4)").bind(id).bind(sub_id).bind(&i.public_key).bind(now).execute(&mut *tx).await?;
        id
    };
    tx.commit().await?;
    let until = if sub.expires_at == 0 {
        now + 300
    } else {
        sub.expires_at.min(now + 300)
    };
    Ok(Json(
        json!({"device_id":device_id,"subscription_id":sub_id,"registered_until":until,"status":"registered","vpn_lease":null}),
    ))
}
pub async fn list(State(s): State<AppState>) -> ApiResult<Vec<Device>> {
    Ok(Json(
        sqlx::query_as("SELECT * FROM devices ORDER BY created_at,id LIMIT 10000")
            .fetch_all(&s.pool)
            .await?,
    ))
}
pub async fn revoke(State(s): State<AppState>, ApiId(id): ApiId) -> ApiResult<Value> {
    let mut tx = s.pool.begin().await?;
    let sub_id: Uuid = sqlx::query_scalar("SELECT subscription_id FROM devices WHERE id=$1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    db::lock_subscription(&mut tx, sub_id).await?;
    sqlx::query("UPDATE devices SET status='revoked' WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"revoked":true})))
}
