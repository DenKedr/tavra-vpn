//! Persisted admin defaults. Saving policy does not attach a packet/DNS runtime.
use crate::*;
use serde::{Deserialize, Serialize};
use tavra_core::RoutingPolicy;
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProtectionSettings {
    pub ad_blocking: bool,
    pub trackers: bool,
    pub malware: bool,
    pub annoyances: bool,
    pub dns_server: String,
    pub filter_lists: Vec<String>,
    pub allow_domains: Vec<String>,
    pub block_domains: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppSettings {
    pub bypass_lan: bool,
    pub excluded_sites: Vec<String>,
    pub excluded_networks: Vec<String>,
    pub protection: ProtectionSettings,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            bypass_lan: true,
            excluded_sites: vec![],
            excluded_networks: vec![],
            protection: ProtectionSettings::default(),
        }
    }
}
impl AppSettings {
    pub fn validated(mut self) -> Result<Self, ApiError> {
        let routing = RoutingPolicy {
            bypass_lan: self.bypass_lan,
            excluded_sites: self.excluded_sites,
            excluded_networks: self.excluded_networks,
        }
        .validate()
        .map_err(|_| ApiError::bad("Invalid routing exception"))?;
        self.excluded_sites = routing.excluded_sites;
        self.excluded_networks = routing.excluded_networks;
        let lists = RoutingPolicy {
            bypass_lan: false,
            excluded_sites: self.protection.allow_domains.clone(),
            excluded_networks: vec![],
        }
        .validate()
        .map_err(|_| ApiError::bad("Invalid allowlist domain"))?;
        self.protection.allow_domains = lists.excluded_sites;
        let lists = RoutingPolicy {
            bypass_lan: false,
            excluded_sites: self.protection.block_domains.clone(),
            excluded_networks: vec![],
        }
        .validate()
        .map_err(|_| ApiError::bad("Invalid blocklist domain"))?;
        self.protection.block_domains = lists.excluded_sites;
        if self.protection.filter_lists.len() > 32 {
            return Err(ApiError::bad("Too many filter lists"));
        }
        for u in &self.protection.filter_lists {
            validate_url(u, &["https"])?;
        }
        if !self.protection.dns_server.is_empty() {
            validate_url(&self.protection.dns_server, &["https", "tls", "quic"])?;
        }
        Ok(self)
    }
}
fn validate_url(s: &str, schemes: &[&str]) -> Result<(), ApiError> {
    if s.len() > 2048
        || s.chars().any(char::is_whitespace)
        || s.split("://")
            .nth(1)
            .is_some_and(|a| a.split('/').next().unwrap_or("").contains('@'))
    {
        return Err(ApiError::bad("Invalid encrypted resolver or filter URL"));
    }
    let u = url::Url::parse(s).map_err(|_| ApiError::bad("Invalid resolver or filter URL"))?;
    if !schemes.contains(&u.scheme())
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
        || u.query().is_some()
        || u.port() == Some(0)
    {
        return Err(ApiError::bad("Invalid encrypted resolver or filter URL"));
    }
    Ok(())
}
fn response(policy: Value, revision: i64) -> Value {
    let mut value = policy;
    value["runtime_status"] = json!("not_attached");
    value["effective"] = json!(false);
    value["revision"] = json!(revision);
    value
}
pub async fn get_settings(State(s): State<AppState>) -> ApiResult<Value> {
    let row: Option<(Value, i64)> =
        sqlx::query_as("SELECT policy,revision FROM app_settings WHERE singleton=TRUE")
            .fetch_optional(&s.pool)
            .await?;
    let (policy, revision) = match row {
        Some(r) => r,
        None => (
            serde_json::to_value(AppSettings::default()).map_err(|_| ApiError::internal())?,
            0,
        ),
    };
    Ok(Json(response(policy, revision)))
}
pub async fn put_settings(
    State(s): State<AppState>,
    ApiJson(v): ApiJson<AppSettings>,
) -> ApiResult<Value> {
    let policy = serde_json::to_value(v.validated()?).map_err(|_| ApiError::internal())?;
    let revision:i64=sqlx::query_scalar("INSERT INTO app_settings(singleton,policy,revision) VALUES(TRUE,$1,1) ON CONFLICT(singleton) DO UPDATE SET policy=EXCLUDED.policy,revision=app_settings.revision+1 RETURNING revision").bind(&policy).fetch_one(&s.pool).await?;
    Ok(Json(response(policy, revision)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_normalize_routing_and_protection() {
        let mut s = AppSettings::default();
        s.excluded_sites.push("https://EXAMPLE.com/a".into());
        s.protection.block_domains.push("ADS.EXAMPLE.COM".into());
        let s = s.validated().unwrap();
        assert_eq!(s.excluded_sites, ["example.com"]);
        assert_eq!(s.protection.block_domains, ["ads.example.com"]);
    }
    #[test]
    fn rejects_invalid_policy_and_unsafe_resolver() {
        let mut s = AppSettings::default();
        s.excluded_networks.push("10.0.0.0/33".into());
        assert!(s.validated().is_err());
        for u in [
            "http://dns.example",
            "https://u:p@dns.example/dns-query",
            "file:///tmp/resolver",
            "tls://dns.example:0",
        ] {
            let mut s = AppSettings::default();
            s.protection.dns_server = u.into();
            assert!(s.validated().is_err());
        }
    }
}
