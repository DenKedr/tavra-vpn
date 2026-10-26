use crate::*;
use serde::Deserialize;
use uuid::Uuid;
#[derive(Serialize, sqlx::FromRow)]
pub struct Server {
    id: Uuid,
    name: String,
    host: String,
    ssh_port: i32,
    username: String,
    status: String,
    created_at: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    name: String,
    host: String,
    ssh_port: i32,
    username: String,
}
pub async fn list(State(s): State<AppState>) -> ApiResult<Vec<Server>> {
    Ok(Json(
        sqlx::query_as("SELECT * FROM servers ORDER BY created_at,id LIMIT 10000")
            .fetch_all(&s.pool)
            .await?,
    ))
}
pub async fn create(
    State(s): State<AppState>,
    Extension(VerifiedTime(now, _)): Extension<VerifiedTime>,
    ApiJson(i): ApiJson<Create>,
) -> ApiResult<Server> {
    if i.name.trim().is_empty()
        || i.name.chars().count() > 120
        || i.host.is_empty()
        || i.host.len() > 253
        || !i
            .host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-:[]".contains(&b))
        || i.username.is_empty()
        || i.username.len() > 64
        || !i
            .username
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        || !(1..=65535).contains(&i.ssh_port)
    {
        return Err(ApiError::bad("Invalid server inventory fields"));
    }
    Ok(Json(sqlx::query_as("INSERT INTO servers(id,name,host,ssh_port,username,created_at) VALUES($1,$2,$3,$4,$5,$6) RETURNING *").bind(Uuid::new_v4()).bind(i.name).bind(i.host).bind(i.ssh_port).bind(i.username).bind(now).fetch_one(&s.pool).await?))
}
