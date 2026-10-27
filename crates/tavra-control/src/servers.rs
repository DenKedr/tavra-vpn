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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    #[serde(default, deserialize_with = "inventory::present")]
    name: Option<String>,
    #[serde(default, deserialize_with = "inventory::present")]
    host: Option<String>,
    #[serde(default, deserialize_with = "inventory::present")]
    ssh_port: Option<i32>,
    #[serde(default, deserialize_with = "inventory::present")]
    username: Option<String>,
}
fn validate(name: &str, host: &str, port: i32, user: &str) -> Result<(String, String), ApiError> {
    if name.trim().is_empty()
        || name.chars().count() > 120
        || name.chars().any(char::is_control)
        || user.is_empty()
        || user.len() > 64
        || user.starts_with('-')
        || !user
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        || !(1..=65535).contains(&port)
    {
        return Err(ApiError::bad("Invalid server inventory fields"));
    }
    Ok((name.trim().into(), domains::normalize_server_host(host)?))
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
    let (name, host) = validate(&i.name, &i.host, i.ssh_port, &i.username)?;
    let mut tx = inventory::begin(&s.pool).await?;
    let server:Server=sqlx::query_as("INSERT INTO servers(id,name,host,ssh_port,username,created_at) VALUES($1,$2,$3,$4,$5,$6) RETURNING *").bind(Uuid::new_v4()).bind(name).bind(host).bind(i.ssh_port).bind(i.username).bind(now).fetch_one(&mut *tx).await?;
    inventory::audit(
        &mut tx,
        "server",
        server.id,
        "created",
        None,
        Some(inventory::snapshot(&server)?),
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(server))
}
pub async fn update(
    State(s): State<AppState>,
    Extension(VerifiedTime(now, _)): Extension<VerifiedTime>,
    ApiId(id): ApiId,
    ApiJson(i): ApiJson<Update>,
) -> ApiResult<Server> {
    let mut tx = inventory::begin(&s.pool).await?;
    let previous: Server = sqlx::query_as("SELECT * FROM servers WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    let (name, host) = validate(
        i.name.as_deref().unwrap_or(&previous.name),
        i.host.as_deref().unwrap_or(&previous.host),
        i.ssh_port.unwrap_or(previous.ssh_port),
        i.username.as_deref().unwrap_or(&previous.username),
    )?;
    let server: Server = sqlx::query_as(
        "UPDATE servers SET name=$2,host=$3,ssh_port=$4,username=$5 WHERE id=$1 RETURNING *",
    )
    .bind(id)
    .bind(name)
    .bind(host)
    .bind(i.ssh_port.unwrap_or(previous.ssh_port))
    .bind(i.username.as_deref().unwrap_or(&previous.username))
    .fetch_one(&mut *tx)
    .await?;
    inventory::audit(
        &mut tx,
        "server",
        id,
        "updated",
        Some(inventory::snapshot(&previous)?),
        Some(inventory::snapshot(&server)?),
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(server))
}
pub async fn remove(
    State(s): State<AppState>,
    Extension(VerifiedTime(now, _)): Extension<VerifiedTime>,
    ApiId(id): ApiId,
) -> ApiResult<Value> {
    let mut tx = inventory::begin(&s.pool).await?;
    let server: Server = sqlx::query_as("SELECT * FROM servers WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    let assigned: Vec<domains::Domain> =
        sqlx::query_as("SELECT * FROM domains WHERE server_id=$1 ORDER BY id FOR UPDATE")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    let mut unassigned = Vec::with_capacity(assigned.len());
    for previous in assigned {
        let domain:domains::Domain=sqlx::query_as("UPDATE domains SET server_id=NULL,verification_status='unverified',updated_at=$2 WHERE id=$1 RETURNING *").bind(previous.id).bind(now).fetch_one(&mut *tx).await?;
        inventory::audit(
            &mut tx,
            "domain",
            domain.id,
            "server_unassigned",
            Some(inventory::snapshot(&previous)?),
            Some(inventory::snapshot(&domain)?),
            now,
        )
        .await?;
        unassigned.push(domain.id);
    }
    sqlx::query("DELETE FROM servers WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    inventory::audit(
        &mut tx,
        "server",
        id,
        "deleted",
        Some(inventory::snapshot(&server)?),
        None,
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"deleted":true,"remote_uninstall_performed":false,"unassigned_domain_ids":unassigned}),
    ))
}
