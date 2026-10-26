use std::net::IpAddr;
use tavra_core::*;
fn ip(s: &str) -> IpAddr {
    s.parse().unwrap()
}
fn policy(sites: &[&str], networks: &[&str]) -> RoutingPolicy {
    RoutingPolicy {
        bypass_lan: false,
        excluded_sites: sites.iter().map(|s| s.to_string()).collect(),
        excluded_networks: networks.iter().map(|s| s.to_string()).collect(),
    }
}
#[test]
fn domains_are_canonical_and_label_bounded() {
    let p = policy(
        &["HTTPS://ПРИМЕР.РФ/private?secret=not_saved", "Example.COM."],
        &[],
    )
    .validate()
    .unwrap();
    assert_eq!(
        p.excluded_sites,
        vec!["xn--e1afmkfd.xn--p1ai", "example.com"]
    );
    for host in [
        "example.com",
        "www.example.com",
        "EXAMPLE.COM.",
        "пример.рф",
    ] {
        assert!(matches!(
            p.evaluate(Some(host), ip("8.8.8.8")).unwrap(),
            RouteDecision::Direct { .. }
        ));
    }
    for host in [
        "evil-example.com",
        "example.com.evil.org",
        "evilxn--e1afmkfd.xn--p1ai",
    ] {
        assert_eq!(
            p.evaluate(Some(host), ip("8.8.8.8")).unwrap(),
            RouteDecision::Tunnel
        );
    }
}
#[test]
fn cidrs_match_both_families_and_literals() {
    let p = policy(&[], &["203.0.113.45/24", "2001:db8::/32", "8.8.8.8", "::1"])
        .validate()
        .unwrap();
    assert_eq!(
        p.excluded_networks,
        vec!["203.0.113.0/24", "2001:db8::/32", "8.8.8.8/32", "::1/128"]
    );
    for address in ["203.0.113.200", "2001:db8::12", "8.8.8.8", "::1"] {
        assert!(matches!(
            p.evaluate(None, ip(address)).unwrap(),
            RouteDecision::Direct { .. }
        ));
    }
    for address in ["203.0.114.1", "2001:db9::12", "8.8.4.4"] {
        assert_eq!(
            p.evaluate(None, ip(address)).unwrap(),
            RouteDecision::Tunnel
        );
    }
}
#[test]
fn lan_defaults_on_but_does_not_include_cgnat_or_public_ipv6() {
    let p = RoutingPolicy::default();
    for address in [
        "10.1.1.1",
        "172.16.1.1",
        "172.31.255.255",
        "192.168.1.1",
        "127.0.0.1",
        "169.254.1.2",
        "::1",
        "fc00::1",
        "fdff::123",
        "fe80::1",
    ] {
        assert_eq!(
            p.evaluate(None, ip(address)).unwrap(),
            RouteDecision::Direct {
                reason: DirectReason::Lan
            }
        );
    }
    for address in [
        "100.64.0.1",
        "172.32.0.1",
        "192.169.1.1",
        "8.8.8.8",
        "2001:4860:4860::8888",
        "ff02::1",
        "::",
    ] {
        assert_eq!(
            p.evaluate(None, ip(address)).unwrap(),
            RouteDecision::Tunnel
        );
    }
    assert_eq!(
        policy(&[], &[]).evaluate(None, ip("192.168.1.1")).unwrap(),
        RouteDecision::Tunnel
    );
}
#[test]
fn invalid_configuration_cannot_bypass_and_errors_do_not_echo_secrets() {
    for site in [
        "*",
        "*.com",
        "https://user:password@example.com/",
        "ftp://example.com",
        "example.com/path",
        "bad host",
        "-bad.com",
        "example..com",
        "https:///",
        "https://example.com:bad",
    ] {
        let p = policy(&[site], &[]);
        let err = p.validate().unwrap_err();
        assert!(!err.to_string().contains("password"));
    }
    for network in ["0.0.0.0/33", "::/129", "10.0.0.1/no", "not-an-ip"] {
        assert!(policy(&[], &[network]).validate().is_err());
    }
    let mut p = policy(&["example.com"], &[]);
    p.excluded_sites = vec!["example.com".into(); 257];
    assert!(p.validate().is_err());
    let p = policy(&[&"a".repeat(2049)], &[]);
    assert!(p.validate().is_err());
    assert!(
        policy(&["*"], &[])
            .evaluate(Some("example.com"), ip("1.1.1.1"))
            .is_err()
    );
    assert!(
        serde_json::from_str::<RoutingPolicy>(
            r#"{"bypass_lan":true,"excluded_sites":["*"],"excluded_networks":[]}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<RoutingPolicy>(
            r#"{"bypass_lan":true,"excluded_sites":[],"excluded_networks":[],"unknown":true}"#
        )
        .is_err()
    );
}
#[test]
fn empty_userinfo_is_rejected_and_defaults_and_reasons_are_stable() {
    assert!(policy(&["https://@example.com"], &[]).validate().is_err());
    assert!(policy(&["http://:@example.com"], &[]).validate().is_err());
    let p: RoutingPolicy = serde_json::from_str("{}").unwrap();
    assert!(p.bypass_lan);
    let mut p = policy(
        &["https://example.com:443/path?q=private#private"],
        &["8.8.8.8"],
    )
    .validate()
    .unwrap();
    p.bypass_lan = true;
    assert_eq!(
        p.evaluate(Some("sub.example.com"), ip("8.8.8.8")).unwrap(),
        RouteDecision::Direct {
            reason: DirectReason::Site {
                site: "example.com".into()
            }
        }
    );
    assert_eq!(
        p.evaluate(Some("other.test"), ip("8.8.8.8")).unwrap(),
        RouteDecision::Direct {
            reason: DirectReason::Network {
                network: "8.8.8.8/32".into()
            }
        }
    );
    assert_eq!(
        p.evaluate(None, ip("192.168.1.1")).unwrap(),
        RouteDecision::Direct {
            reason: DirectReason::Lan
        }
    );
    assert!(p.evaluate(Some("*"), ip("192.168.1.1")).is_err());
    let serialized = serde_json::to_string(&p).unwrap();
    assert!(!serialized.contains("private"));
}
#[test]
fn unvalidated_policy_debug_does_not_log_url_secrets() {
    let p = policy(&["https://user:password@example.com/?token=secret"], &[]);
    let debug = format!("{p:?}");
    assert!(!debug.contains("password"));
    assert!(!debug.contains("secret"));
}
