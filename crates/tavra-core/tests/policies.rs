use tavra_core::*;
#[test]
fn unlimited_business_policy_still_issues_finite_lease() {
    let p = SubscriptionPolicy {
        expires_at: 0,
        device_limit: 0,
        blocked: false,
    };
    assert!(p.validate_at(100).is_ok());
    assert!(p.can_register_device(u64::MAX).is_ok());
    assert_eq!(p.issue_lease(100, 900).unwrap().expires_at(), 1000);
    assert!(p.issue_lease(100, 0).is_err());
    assert!(p.issue_lease(i64::MAX, 900).is_err());
}
#[test]
fn rejects_invalid_expired_blocked_and_device_over_limit() {
    for p in [
        SubscriptionPolicy {
            expires_at: -1,
            device_limit: 1,
            blocked: false,
        },
        SubscriptionPolicy {
            expires_at: 1,
            device_limit: -1,
            blocked: false,
        },
        SubscriptionPolicy {
            expires_at: 100,
            device_limit: 1,
            blocked: false,
        },
        SubscriptionPolicy {
            expires_at: 0,
            device_limit: 1,
            blocked: true,
        },
    ] {
        assert!(p.validate_at(100).is_err());
    }
    let p = SubscriptionPolicy {
        expires_at: 200,
        device_limit: 2,
        blocked: false,
    };
    assert!(p.can_register_device(1).is_ok());
    assert!(p.can_register_device(2).is_err());
    assert_eq!(p.issue_lease(100, 900).unwrap().expires_at(), 200);
    assert!(p.validate_at(-1).is_err());
}
#[test]
fn strict_json_rejects_overflow_unknown_fields_and_invalid_lease() {
    for s in [
        r#"{"expires_at":9223372036854775808,"device_limit":0,"blocked":false}"#,
        r#"{"expires_at":0,"device_limit":0,"blocked":false,"skip_auth":true}"#,
    ] {
        assert!(serde_json::from_str::<SubscriptionPolicy>(s).is_err());
    }
    for s in [
        r#"{"issued_at":100,"expires_at":0}"#,
        r#"{"issued_at":100,"expires_at":100}"#,
    ] {
        assert!(serde_json::from_str::<Lease>(s).is_err());
    }
}
#[test]
fn unknown_stale_or_missing_prerequisites_never_available() {
    let cap = Capabilities::for_profile(ProtocolProfile::NaiveProxy);
    let mut p = EndpointPrerequisites::default();
    assert_ne!(cap.evaluate(&p), Availability::Available);
    for r in cap.requirements.iter() {
        p.checks.insert(*r, PrerequisiteStatus::Satisfied);
    }
    assert_eq!(cap.evaluate(&p), Availability::Available);
    p.checks
        .insert(Requirement::ProtocolHealth, PrerequisiteStatus::Stale);
    assert_ne!(cap.evaluate(&p), Availability::Available);
    assert!(!Capabilities::for_profile(ProtocolProfile::TavraLinkQuic).dpi_camouflage);
    assert!(
        Capabilities::for_profile(ProtocolProfile::Dnstt)
            .requirements
            .contains(&Requirement::DnsDelegation)
    );
}
fn backend(attempt: AttemptId, event: BackendEvent) -> ConnectionEvent {
    ConnectionEvent::Backend { attempt, event }
}
#[test]
fn connection_requires_tunnel_confirmation_and_failure_is_closed() {
    let mut m = ConnectionMachine::default();
    assert!(
        m.transition(ConnectionEvent::Request(ConnectionRequest::Disconnect))
            .is_err()
    );
    assert_eq!(
        m.transition(ConnectionEvent::Request(ConnectionRequest::Connect))
            .unwrap(),
        ConnectionState::Preparing
    );
    let attempt = m.attempt().unwrap();
    assert!(
        m.transition(backend(attempt, BackendEvent::TunnelConfirmed))
            .is_err()
    );
    assert!(m.traffic_blocked());
    assert!(
        m.transition(ConnectionEvent::Request(ConnectionRequest::DirectFallback))
            .is_err()
    );
    assert_eq!(
        m.transition(backend(attempt, BackendEvent::PrerequisitesVerified))
            .unwrap(),
        ConnectionState::Connecting
    );
    assert_eq!(
        m.transition(backend(attempt, BackendEvent::TunnelConfirmed))
            .unwrap(),
        ConnectionState::Connected
    );
    assert!(!m.traffic_blocked());
    assert_eq!(
        m.transition(backend(attempt, BackendEvent::TunnelLost))
            .unwrap(),
        ConnectionState::FailedClosed
    );
    assert!(m.traffic_blocked());
    assert!(
        m.transition(backend(attempt, BackendEvent::TunnelConfirmed))
            .is_err()
    );
    assert!(serde_json::from_str::<ConnectionRequest>(r#""tunnel_confirmed""#).is_err());
}
#[test]
fn finite_lease_enforces_expiry_and_maximum_window() {
    let lease = Lease::new(100, 200).unwrap();
    assert!(lease.validate_at(199).is_ok());
    assert!(lease.validate_at(200).is_err());
    assert!(lease.validate_at(99).is_err());
    assert!(Lease::new(0, 901).is_err());
    let p = SubscriptionPolicy {
        expires_at: 0,
        device_limit: 0,
        blocked: false,
    };
    assert!(p.issue_lease(100, 901).is_err());
}
#[test]
fn catalogue_is_explicit_and_cannot_bypass_required_identity() {
    for profile in [
        ProtocolProfile::OpenVpn,
        ProtocolProfile::OpenVpnShadowsocks,
        ProtocolProfile::OpenVpnCloak,
        ProtocolProfile::Ikev2,
    ] {
        assert!(
            Capabilities::for_profile(profile)
                .requirements
                .contains(&Requirement::PeerIdentityVerified)
        );
    }
}
#[test]
fn negative_business_fields_are_rejected_at_json_boundary() {
    for json in [
        r#"{"expires_at":-1,"device_limit":0,"blocked":false}"#,
        r#"{"expires_at":0,"device_limit":-1,"blocked":false}"#,
    ] {
        assert!(serde_json::from_str::<SubscriptionPolicy>(json).is_err());
    }
}
#[test]
fn delayed_confirmation_from_previous_attempt_cannot_connect_retry() {
    let mut m = ConnectionMachine::default();
    m.transition(ConnectionEvent::Request(ConnectionRequest::Connect))
        .unwrap();
    let a = m.attempt().unwrap();
    m.transition(backend(a, BackendEvent::PrerequisitesVerified))
        .unwrap();
    m.transition(backend(a, BackendEvent::Failure)).unwrap();
    m.transition(ConnectionEvent::Request(ConnectionRequest::Connect))
        .unwrap();
    let b = m.attempt().unwrap();
    assert_ne!(a, b);
    for event in [
        BackendEvent::PrerequisitesVerified,
        BackendEvent::TunnelConfirmed,
        BackendEvent::TunnelLost,
        BackendEvent::Failure,
        BackendEvent::TunnelStopped,
    ] {
        assert!(m.transition(backend(a, event)).is_err());
        assert_eq!(m.state(), ConnectionState::Preparing);
    }
    m.transition(backend(b, BackendEvent::PrerequisitesVerified))
        .unwrap();
    assert!(
        m.transition(backend(a, BackendEvent::TunnelConfirmed))
            .is_err()
    );
    assert_eq!(m.state(), ConnectionState::Connecting);
    assert!(m.traffic_blocked());
    m.transition(backend(b, BackendEvent::TunnelConfirmed))
        .unwrap();
    assert_eq!(m.state(), ConnectionState::Connected);
    m.transition(ConnectionEvent::Request(ConnectionRequest::Disconnect))
        .unwrap();
    assert!(
        m.transition(backend(a, BackendEvent::TunnelStopped))
            .is_err()
    );
    assert_eq!(m.state(), ConnectionState::Disconnecting);
    m.transition(backend(b, BackendEvent::TunnelStopped))
        .unwrap();
    assert_eq!(m.state(), ConnectionState::Disconnected);
    assert!(m.transition(backend(b, BackendEvent::Failure)).is_err());
    assert_eq!(m.state(), ConnectionState::Disconnected);
}
#[test]
fn failure_can_be_followed_by_confirmed_teardown() {
    let mut m = ConnectionMachine::default();
    m.transition(ConnectionEvent::Request(ConnectionRequest::Connect))
        .unwrap();
    let attempt = m.attempt().unwrap();
    m.transition(backend(attempt, BackendEvent::Failure))
        .unwrap();
    m.transition(ConnectionEvent::Request(ConnectionRequest::Disconnect))
        .unwrap();
    assert!(m.attempt().is_some());
    m.transition(backend(attempt, BackendEvent::TunnelStopped))
        .unwrap();
    assert_eq!(m.state(), ConnectionState::Disconnected);
}
