use crate::*;
use axum::{extract::State, middleware::Next};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub fn hash(secret: &str) -> Vec<u8> {
    Sha256::digest(secret.as_bytes()).to_vec()
}
pub fn random_secret() -> Result<String, ApiError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| ApiError::internal())?;
    Ok(hex::encode(bytes))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bootstrap {
    bootstrap_secret: String,
    admin_secret: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
    admin_secret: String,
}
fn validate_key(s: &str) -> Result<(), ApiError> {
    if s.len() < 32 || s.len() > 256 {
        Err(ApiError::bad("Admin key must have 32 to 256 bytes"))
    } else {
        Ok(())
    }
}
pub async fn bootstrap(
    State(s): State<AppState>,
    Extension(VerifiedTime(now, generation)): Extension<VerifiedTime>,
    ApiJson(input): ApiJson<Bootstrap>,
) -> ApiResult<Value> {
    validate_key(&input.admin_secret)?;
    let mut tx = s.pool.begin().await?;
    db::verify_generation(&mut tx, generation).await?;
    // Serialize singleton activation, including an empty admins table.
    sqlx::query("SELECT pg_advisory_xact_lock(729840012)")
        .execute(&mut *tx)
        .await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM admins)")
        .fetch_one(&mut *tx)
        .await?;
    if exists {
        return Err(ApiError::conflict("Owner already activated"));
    }
    if s.bootstrap_secret.len() < 32 || hash(&input.bootstrap_secret) != hash(&s.bootstrap_secret) {
        return Err(ApiError::unauthorized());
    }
    sqlx::query("INSERT INTO admins(credential_hash) VALUES ($1)")
        .bind(hash(&input.admin_secret))
        .execute(&mut *tx)
        .await?;
    let token = random_secret()?;
    let expires = now + 8 * 3600;
    sqlx::query("INSERT INTO sessions(token_hash,expires_at,generation) VALUES ($1,$2,$3)")
        .bind(hash(&token))
        .bind(expires)
        .bind(generation)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"token":token,"expires_at":expires})))
}
pub async fn session(
    State(s): State<AppState>,
    Extension(VerifiedTime(now, generation)): Extension<VerifiedTime>,
    ApiJson(input): ApiJson<Login>,
) -> ApiResult<Value> {
    validate_key(&input.admin_secret)?;
    let valid: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM admins WHERE credential_hash=$1)")
            .bind(hash(&input.admin_secret))
            .fetch_one(&s.pool)
            .await?;
    if !valid {
        return Err(ApiError::unauthorized());
    }
    let mut tx = s.pool.begin().await?;
    db::verify_generation(&mut tx, generation).await?;
    let token = random_secret()?;
    let expires = now + 8 * 3600;
    sqlx::query("DELETE FROM sessions WHERE expires_at <= $1")
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO sessions(token_hash,expires_at,generation) VALUES ($1,$2,$3)")
        .bind(hash(&token))
        .bind(expires)
        .bind(generation)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"token":token,"expires_at":expires})))
}
pub async fn require_admin(
    State(s): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .filter(|s| s.len() == 64)
        .ok_or_else(ApiError::unauthorized)?;
    let VerifiedTime(now, generation) = *req
        .extensions()
        .get::<VerifiedTime>()
        .ok_or_else(ApiError::clock_untrusted)?;
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sessions WHERE token_hash=$1 AND expires_at>$2 AND generation=$3 AND generation=(SELECT generation FROM security_clock WHERE singleton=true))",
    )
    .bind(hash(token))
    .bind(now)
    .bind(generation)
    .fetch_one(&s.pool)
    .await?;
    if !valid {
        return Err(ApiError::unauthorized());
    }
    Ok(next.run(req).await)
}
