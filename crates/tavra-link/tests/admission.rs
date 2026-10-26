use ed25519_dalek::SigningKey;
use tavra_link::admission::*;
fn fixture() -> (SigningKey, SigningKey, SignedLease) {
    let a = SigningKey::from_bytes(&[1; 32]);
    let d = SigningKey::from_bytes(&[2; 32]);
    let l = issue_lease(
        &a,
        LeaseClaims {
            device_key: hex::encode(d.verifying_key().as_bytes()),
            node_id: "node-a".into(),
            epoch: 1,
            issued_at: 100,
            expires_at: 200,
        },
    )
    .unwrap();
    (a, d, l)
}
#[test]
fn admission_binds_authority_device_nonce_session_audience_and_time() {
    let (a, d, l) = fixture();
    let proof = make_auth(l, &d, &[3; 32], &[4; 32]).unwrap();
    let verify = |binding, nonce, node, epoch, now| {
        verify_auth(
            &proof,
            &a.verifying_key(),
            &binding,
            &nonce,
            node,
            epoch,
            now,
        )
    };
    assert!(verify([3; 32], [4; 32], "node-a", 1, 150).is_ok());
    for (binding, nonce, node, epoch, now) in [
        ([8; 32], [4; 32], "node-a", 1, 150),
        ([3; 32], [8; 32], "node-a", 1, 150),
        ([3; 32], [4; 32], "node-b", 1, 150),
        ([3; 32], [4; 32], "node-a", 2, 150),
        ([3; 32], [4; 32], "node-a", 1, 200),
        ([3; 32], [4; 32], "node-a", 1, 99),
    ] {
        assert!(verify(binding, nonce, node, epoch, now).is_err());
    }
    assert!(
        verify_auth(
            &proof,
            &SigningKey::from_bytes(&[7; 32]).verifying_key(),
            &[3; 32],
            &[4; 32],
            "node-a",
            1,
            150
        )
        .is_err()
    );
    let mut bad = proof;
    bad.lease.claims.expires_at = 300;
    assert!(
        verify_auth(
            &bad,
            &a.verifying_key(),
            &[3; 32],
            &[4; 32],
            "node-a",
            1,
            150
        )
        .is_err()
    );
}
#[test]
fn impossible_leases_and_wrong_device_are_denied() {
    let (a, _d, l) = fixture();
    assert!(
        make_auth(
            l.clone(),
            &SigningKey::from_bytes(&[9; 32]),
            &[0; 32],
            &[0; 32]
        )
        .is_err()
    );
    for exp in [0, 100, 1001, i64::MAX] {
        let mut c = l.claims.clone();
        c.expires_at = exp;
        assert!(issue_lease(&a, c).is_err());
    }
    let mut c = l.claims;
    c.epoch = 0;
    assert!(issue_lease(&a, c).is_err());
}
