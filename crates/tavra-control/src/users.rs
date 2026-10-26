use crate::*;
use serde::Deserialize;
use uuid::Uuid;
pub fn validate_contact(
    first: &str,
    last: &str,
    phone: &str,
    email: &str,
) -> Result<(), &'static str> {
    if first.chars().count() > 120
        || last.chars().count() > 120
        || phone.chars().count() > 64
        || email.chars().count() > 254
        || [first, last, phone, email]
            .iter()
            .any(|s| s.chars().any(char::is_control))
    {
        Err("Contact field exceeds limits or contains control characters")
    } else {
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateUser {
    first_name: String,
    #[serde(default)]
    last_name: String,
    #[serde(default)]
    phone: String,
    #[serde(default)]
    email: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateUser {
    first_name: Option<String>,
    last_name: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    status: Option<String>,
}
pub async fn list(State(s): State<AppState>) -> ApiResult<Vec<User>> {
    Ok(Json(
        sqlx::query_as("SELECT * FROM users ORDER BY created_at,id LIMIT 10000")
            .fetch_all(&s.pool)
            .await?,
    ))
}
pub async fn create(
    State(s): State<AppState>,
    Extension(VerifiedTime(now, _)): Extension<VerifiedTime>,
    ApiJson(i): ApiJson<CreateUser>,
) -> ApiResult<User> {
    validate_contact(&i.first_name, &i.last_name, &i.phone, &i.email).map_err(ApiError::bad)?;
    let u=sqlx::query_as("INSERT INTO users(id,first_name,last_name,phone,email,status,created_at) VALUES($1,$2,$3,$4,$5,'active',$6) RETURNING *").bind(Uuid::new_v4()).bind(i.first_name).bind(i.last_name).bind(i.phone).bind(i.email).bind(now).fetch_one(&s.pool).await?;
    Ok(Json(u))
}
pub async fn update(
    State(s): State<AppState>,
    ApiId(id): ApiId,
    ApiJson(i): ApiJson<UpdateUser>,
) -> ApiResult<User> {
    let mut tx = s.pool.begin().await?;
    let mut u: User = sqlx::query_as("SELECT * FROM users WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    if let Some(v) = i.first_name {
        u.first_name = v;
    }
    if let Some(v) = i.last_name {
        u.last_name = v;
    }
    if let Some(v) = i.phone {
        u.phone = v;
    }
    if let Some(v) = i.email {
        u.email = v;
    }
    if let Some(v) = i.status {
        if v != "active" && v != "blocked" {
            return Err(ApiError::bad("Invalid user status"));
        }
        u.status = v;
    }
    validate_contact(&u.first_name, &u.last_name, &u.phone, &u.email).map_err(ApiError::bad)?;
    let updated=sqlx::query_as("UPDATE users SET first_name=$2,last_name=$3,phone=$4,email=$5,status=$6 WHERE id=$1 RETURNING *").bind(id).bind(u.first_name).bind(u.last_name).bind(u.phone).bind(u.email).bind(&u.status).fetch_one(&mut *tx).await?;
    if u.status == "blocked" {
        sqlx::query("DELETE FROM challenges WHERE subscription_id IN (SELECT id FROM subscriptions WHERE user_id=$1)").bind(id).execute(&mut *tx).await?;
        sqlx::query("UPDATE devices SET status='revoked' WHERE subscription_id IN (SELECT id FROM subscriptions WHERE user_id=$1)").bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Json(updated))
}
pub async fn remove(State(s): State<AppState>, ApiId(id): ApiId) -> ApiResult<Value> {
    let result = sqlx::query("DELETE FROM users WHERE id=$1")
        .bind(id)
        .execute(&s.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::missing());
    }
    Ok(Json(json!({"deleted":true})))
}
