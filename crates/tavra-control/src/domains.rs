//! Domain inventory: normalization never performs DNS/TLS validation or provisions a protocol.
use crate::*;
use serde::{Deserialize, Deserializer};
use uuid::Uuid;
#[derive(Serialize, sqlx::FromRow)]
pub struct Domain {
    pub id: Uuid,
    pub name: String,
    pub server_id: Option<Uuid>,
    pub verification_status: String,
    pub created_at: i64,
    pub updated_at: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    name: String,
    #[serde(default)]
    server_id: Option<Uuid>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    #[serde(default, deserialize_with = "inventory::present")]
    name: Option<String>,
    #[serde(default, deserialize_with = "assignment")]
    server_id: Option<Option<Uuid>>,
}
fn assignment<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Option<Uuid>>, D::Error> {
    Option::<Uuid>::deserialize(d).map(Some)
}
fn host_input(name: &str) -> Result<(), ApiError> {
    if name.is_empty()
        || name.len() > 1024
        || name
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "/\\@:#?[]*%".contains(c))
    {
        return Err(ApiError::bad(
            "Use a domain name without IP, URL, wildcard or credentials",
        ));
    }
    Ok(())
}
fn labels(ascii: &str, fqdn: bool) -> Result<String, ApiError> {
    let ascii = ascii.strip_suffix('.').unwrap_or(ascii);
    if ascii.len() > 253
        || (fqdn && !ascii.contains('.'))
        || ascii.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return Err(ApiError::bad("Invalid DNS name or label length"));
    }
    Ok(ascii.to_ascii_lowercase())
}
pub fn normalize_domain(name: &str) -> Result<String, ApiError> {
    host_input(name)?;
    match url::Host::parse(name).map_err(|_| ApiError::bad("Invalid IDNA domain name"))? {
        url::Host::Domain(ascii) => labels(&ascii, true),
        _ => Err(ApiError::bad("IP addresses are not domains")),
    }
}
pub(crate) fn normalize_server_host(host: &str) -> Result<String, ApiError> {
    let unbracketed = host
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(ip) = unbracketed.parse::<std::net::IpAddr>() {
        return Ok(ip.to_string());
    }
    host_input(host)?;
    match url::Host::parse(host).map_err(|_| ApiError::bad("Invalid server host"))? {
        url::Host::Domain(ascii) => labels(&ascii, false),
        ip => Ok(ip.to_string()),
    }
}
pub async fn list(State(s): State<AppState>) -> ApiResult<Vec<Domain>> {
    Ok(Json(
        sqlx::query_as("SELECT * FROM domains ORDER BY name,id LIMIT 10000")
            .fetch_all(&s.pool)
            .await?,
    ))
}
pub async fn create(
    State(s): State<AppState>,
    Extension(VerifiedTime(now, _)): Extension<VerifiedTime>,
    ApiJson(i): ApiJson<Create>,
) -> ApiResult<Domain> {
    let name = normalize_domain(&i.name)?;
    let mut tx = inventory::begin(&s.pool).await?;
    let domain:Domain=sqlx::query_as("INSERT INTO domains(id,name,server_id,created_at,updated_at) VALUES($1,$2,$3,$4,$4) RETURNING *").bind(Uuid::new_v4()).bind(name).bind(i.server_id).bind(now).fetch_one(&mut *tx).await.map_err(inventory::domain_error)?;
    inventory::audit(
        &mut tx,
        "domain",
        domain.id,
        "created",
        None,
        Some(inventory::snapshot(&domain)?),
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(domain))
}
pub async fn update(
    State(s): State<AppState>,
    Extension(VerifiedTime(now, _)): Extension<VerifiedTime>,
    ApiId(id): ApiId,
    ApiJson(i): ApiJson<Update>,
) -> ApiResult<Domain> {
    let mut tx = inventory::begin(&s.pool).await?;
    let previous: Domain = sqlx::query_as("SELECT * FROM domains WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    let name = if let Some(name) = i.name {
        normalize_domain(&name)?
    } else {
        previous.name.clone()
    };
    let server_id = i.server_id.unwrap_or(previous.server_id);
    let domain:Domain=sqlx::query_as("UPDATE domains SET name=$2,server_id=$3,verification_status='unverified',updated_at=$4 WHERE id=$1 RETURNING *").bind(id).bind(name).bind(server_id).bind(now).fetch_one(&mut *tx).await.map_err(inventory::domain_error)?;
    inventory::audit(
        &mut tx,
        "domain",
        id,
        "updated",
        Some(inventory::snapshot(&previous)?),
        Some(inventory::snapshot(&domain)?),
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(domain))
}
pub async fn remove(
    State(s): State<AppState>,
    Extension(VerifiedTime(now, _)): Extension<VerifiedTime>,
    ApiId(id): ApiId,
) -> ApiResult<Value> {
    let mut tx = inventory::begin(&s.pool).await?;
    let domain: Domain = sqlx::query_as("DELETE FROM domains WHERE id=$1 RETURNING *")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    inventory::audit(
        &mut tx,
        "domain",
        id,
        "deleted",
        Some(inventory::snapshot(&domain)?),
        None,
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"deleted":true})))
}
