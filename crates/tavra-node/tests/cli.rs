use std::process::Command;
#[test]
fn inspect_emits_actual_typed_json_and_does_not_claim_vpn_readiness() {
    let out = Command::new(env!("CARGO_BIN_EXE_tavra-node"))
        .arg("inspect")
        .output()
        .unwrap();
    assert!(out.status.success());
    let metrics: tavra_node::Metrics = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(metrics.architecture, std::env::consts::ARCH);
    assert!(metrics.capabilities.iter().all(|cap| !cap.supported));
    assert!(
        !String::from_utf8(out.stdout)
            .unwrap()
            .contains("PRIVATE KEY")
    );
}
#[test]
fn missing_config_and_unknown_secret_field_fail_without_echoing_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    std::fs::write(&path,r#"{"server_cert":"a","server_key":"b","management_client_ca":"c","token":"DO_NOT_ECHO_THIS"}"#).unwrap();
    for args in [
        vec!["--config", path.to_str().unwrap()],
        vec!["serve", "--config", "/nonexistent/tavra-node.json"],
        vec![],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_tavra-node"))
            .args(args)
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
        assert!(
            !String::from_utf8(out.stderr)
                .unwrap()
                .contains("DO_NOT_ECHO_THIS")
        );
    }
}
