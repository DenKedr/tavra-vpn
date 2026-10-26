//! Routing intent only: platform adapters must enforce it for DNS and packet flows.
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use url::{Host, Url};

pub const MAX_ROUTING_RULES: usize = 256;
pub const MAX_ROUTING_ENTRY_BYTES: usize = 2048;

#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "RoutingPolicyWire")]
pub struct RoutingPolicy {
    pub bypass_lan: bool,
    pub excluded_sites: Vec<String>,
    pub excluded_networks: Vec<String>,
}
impl std::fmt::Debug for RoutingPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RoutingPolicy")
            .field("bypass_lan", &self.bypass_lan)
            .field("excluded_sites_count", &self.excluded_sites.len())
            .field("excluded_networks_count", &self.excluded_networks.len())
            .finish()
    }
}
impl Default for RoutingPolicy {
    fn default() -> Self {
        Self {
            bypass_lan: true,
            excluded_sites: vec![],
            excluded_networks: vec![],
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, default)]
struct RoutingPolicyWire {
    bypass_lan: bool,
    excluded_sites: Vec<String>,
    excluded_networks: Vec<String>,
}
impl Default for RoutingPolicyWire {
    fn default() -> Self {
        Self {
            bypass_lan: true,
            excluded_sites: vec![],
            excluded_networks: vec![],
        }
    }
}
impl TryFrom<RoutingPolicyWire> for RoutingPolicy {
    type Error = RoutingError;
    fn try_from(v: RoutingPolicyWire) -> Result<Self, Self::Error> {
        Self {
            bypass_lan: v.bypass_lan,
            excluded_sites: v.excluded_sites,
            excluded_networks: v.excluded_networks,
        }
        .validate()
    }
}
/// Deliberately excludes raw user input, which may contain URL credentials/query data.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RoutingError {
    #[error("too many routing exceptions (maximum 256 combined)")]
    TooManyRules,
    #[error("invalid site exception or destination host")]
    InvalidSite,
    #[error("invalid IP address or CIDR exception")]
    InvalidNetwork,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DirectReason {
    Site { site: String },
    Network { network: String },
    Lan,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "route", rename_all = "snake_case")]
pub enum RouteDecision {
    Tunnel,
    Direct { reason: DirectReason },
}

impl RoutingPolicy {
    /// HTTP(S) URL paths, ports, queries and fragments are intentionally discarded:
    /// a site exception always applies to the whole host and its subdomains.
    pub fn validate(mut self) -> Result<Self, RoutingError> {
        if self
            .excluded_sites
            .len()
            .saturating_add(self.excluded_networks.len())
            > MAX_ROUTING_RULES
        {
            return Err(RoutingError::TooManyRules);
        }
        self.excluded_sites = self
            .excluded_sites
            .iter()
            .map(|s| canonical_site(s))
            .collect::<Result<_, _>>()?;
        self.excluded_networks = self
            .excluded_networks
            .iter()
            .map(|s| parse_network(s).map(|n| n.to_string()))
            .collect::<Result<_, _>>()?;
        Ok(self)
    }
    /// The adapter supplies the actual destination IP and, if known, its verified
    /// host association. Never infer one from an arbitrary reverse-DNS answer.
    /// Explicit site, explicit IP and built-in LAN rules are ORed, in that order.
    pub fn evaluate(
        &self,
        host: Option<&str>,
        destination: IpAddr,
    ) -> Result<RouteDecision, RoutingError> {
        // Public fields allow construction/mutation by Rust consumers; never trust
        // these callers to have previously validated their configuration.
        let policy = self.clone().validate()?;
        let host = host.map(canonical_site).transpose()?;
        if let Some(host) = host {
            for site in &policy.excluded_sites {
                if host == *site
                    || (site.parse::<IpAddr>().is_err()
                        && host
                            .strip_suffix(site)
                            .is_some_and(|prefix| prefix.ends_with('.')))
                {
                    return Ok(RouteDecision::Direct {
                        reason: DirectReason::Site { site: site.clone() },
                    });
                }
            }
        }
        for network in &policy.excluded_networks {
            if parse_network(network)?.contains(&destination) {
                return Ok(RouteDecision::Direct {
                    reason: DirectReason::Network {
                        network: network.clone(),
                    },
                });
            }
        }
        if policy.bypass_lan && is_builtin_lan(destination) {
            return Ok(RouteDecision::Direct {
                reason: DirectReason::Lan,
            });
        }
        Ok(RouteDecision::Tunnel)
    }
}
fn canonical_site(input: &str) -> Result<String, RoutingError> {
    if input.is_empty()
        || input.len() > MAX_ROUTING_ENTRY_BYTES
        || input.chars().any(char::is_whitespace)
    {
        return Err(RoutingError::InvalidSite);
    }
    if let Ok(ip) = input.parse::<IpAddr>() {
        return Ok(ip.to_string());
    }
    let host = if input.contains("://") {
        // URL serialization erases empty userinfo; reject it before parsing too.
        let authority = input
            .split_once("://")
            .ok_or(RoutingError::InvalidSite)?
            .1
            .split(['/', '?', '#'])
            .next()
            .ok_or(RoutingError::InvalidSite)?;
        if authority.contains('@') {
            return Err(RoutingError::InvalidSite);
        }
        let url = Url::parse(input).map_err(|_| RoutingError::InvalidSite)?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(RoutingError::InvalidSite);
        }
        url.host().ok_or(RoutingError::InvalidSite)?.to_owned()
    } else {
        Host::parse(input).map_err(|_| RoutingError::InvalidSite)?
    };
    match host {
        Host::Ipv4(v) => Ok(v.to_string()),
        Host::Ipv6(v) => Ok(v.to_string()),
        Host::Domain(domain) => {
            let domain = domain
                .strip_suffix('.')
                .unwrap_or(&domain)
                .to_ascii_lowercase();
            if domain.len() > 253
                || domain.split('.').any(|label| {
                    label.is_empty()
                        || label.len() > 63
                        || label.starts_with('-')
                        || label.ends_with('-')
                        || !label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
            {
                return Err(RoutingError::InvalidSite);
            }
            Ok(domain)
        }
    }
}
fn parse_network(input: &str) -> Result<IpNet, RoutingError> {
    if input.is_empty() || input.len() > MAX_ROUTING_ENTRY_BYTES {
        return Err(RoutingError::InvalidNetwork);
    }
    if let Ok(ip) = input.parse::<IpAddr>() {
        return Ok(IpNet::from(ip));
    }
    input
        .parse::<IpNet>()
        .map(|n| n.trunc())
        .map_err(|_| RoutingError::InvalidNetwork)
}
fn is_builtin_lan(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_private() || v.is_loopback() || v.is_link_local(),
        IpAddr::V6(v) => {
            v.is_loopback()
                || (v.segments()[0] & 0xfe00 == 0xfc00)
                || (v.segments()[0] & 0xffc0 == 0xfe80)
        }
    }
}
