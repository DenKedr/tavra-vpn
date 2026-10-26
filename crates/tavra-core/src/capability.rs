use crate::{ProtocolProfile, Requirement};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrerequisiteStatus {
    Satisfied,
    Missing,
    Unknown,
    Stale,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointPrerequisites {
    pub checks: BTreeMap<Requirement, PrerequisiteStatus>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Availability {
    Available,
    Unavailable { requirements: Vec<Requirement> },
}
#[derive(Clone, Debug, Serialize)]
pub struct Capabilities {
    pub profile: ProtocolProfile,
    pub requirements: Vec<Requirement>,
    pub dpi_camouflage: bool,
}
impl Capabilities {
    /// Catalogue requirements describe validation gates, not implemented engines or regional guarantees.
    pub fn for_profile(profile: ProtocolProfile) -> Self {
        use ProtocolProfile::*;
        use Requirement::*;
        let mut requirements = vec![
            ClientRuntime,
            NodeRuntime,
            Credentials,
            TrustedManifest,
            ProtocolHealth,
        ];
        let extra: &[Requirement] = match profile {
            AmneziaWgLegacy | AmneziaWg2 | WireGuard | Ikev2 => &[UdpReachable],
            OpenVpn | Shadowsocks | SsXray | OpenVpnShadowsocks => &[TcpReachable],
            OpenVpnCloak => &[TcpReachable, DnsResolution, CloakFrontend],
            XrayReality => &[TcpReachable, DnsResolution, RealityTarget, RealityKeys],
            NaiveProxy => &[
                TcpReachable,
                Domain,
                DnsResolution,
                VerifiedTls,
                HttpsFrontend,
                IpPacketFraming,
            ],
            Hysteria2 | TavraLinkQuic => &[UdpReachable, VerifiedTls],
            TrustTunnel => &[TcpReachable, VerifiedTls],
            Dnstt => &[
                Domain,
                DnsResolution,
                DnsDelegation,
                DnsTransportReachable,
                DnsPublicKey,
                IpPacketFraming,
                LicenseClearance,
            ],
            TavraLinkHttps => &[
                TcpReachable,
                Domain,
                DnsResolution,
                VerifiedTls,
                HttpsFrontend,
                IpPacketFraming,
                CamouflageAdapter,
            ],
        };
        requirements.extend_from_slice(extra);
        if matches!(profile, OpenVpn | OpenVpnCloak | OpenVpnShadowsocks | Ikev2) {
            requirements.push(PeerIdentityVerified);
        }
        Self {
            profile,
            requirements,
            dpi_camouflage: matches!(
                profile,
                OpenVpnCloak | XrayReality | NaiveProxy | TavraLinkHttps
            ),
        }
    }
    pub fn evaluate(&self, p: &EndpointPrerequisites) -> Availability {
        let requirements = self
            .requirements
            .iter()
            .filter(|r| p.checks.get(r) != Some(&PrerequisiteStatus::Satisfied))
            .copied()
            .collect::<Vec<_>>();
        if requirements.is_empty() {
            Availability::Available
        } else {
            Availability::Unavailable { requirements }
        }
    }
}
