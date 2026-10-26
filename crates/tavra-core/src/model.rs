use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolProfile {
    AmneziaWgLegacy,
    AmneziaWg2,
    WireGuard,
    OpenVpn,
    OpenVpnCloak,
    OpenVpnShadowsocks,
    Ikev2,
    XrayReality,
    SsXray,
    NaiveProxy,
    Hysteria2,
    TrustTunnel,
    Shadowsocks,
    Dnstt,
    TavraLinkQuic,
    TavraLinkHttps,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Requirement {
    PeerIdentityVerified,
    ClientRuntime,
    NodeRuntime,
    Credentials,
    TrustedManifest,
    ProtocolHealth,
    TcpReachable,
    UdpReachable,
    Domain,
    DnsResolution,
    VerifiedTls,
    HttpsFrontend,
    RealityTarget,
    RealityKeys,
    CloakFrontend,
    DnsDelegation,
    DnsTransportReachable,
    DnsPublicKey,
    IpPacketFraming,
    CamouflageAdapter,
    LicenseClearance,
}
