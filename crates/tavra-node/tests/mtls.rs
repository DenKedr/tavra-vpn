use rcgen::{BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair};
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::{Duration, timeout},
};
fn ca() -> (rcgen::Certificate, rcgen::CertifiedIssuer<'static, KeyPair>) {
    let mut params = CertificateParams::new(vec![]).unwrap();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let key = KeyPair::generate().unwrap();
    let issuer = rcgen::CertifiedIssuer::self_signed(params, key).unwrap();
    (issuer.as_ref().clone(), issuer)
}
#[tokio::test]
async fn real_tls_requires_dedicated_management_client_identity() {
    let (_, issuer) = ca();
    let (_, server_issuer) = ca();
    let server_key = KeyPair::generate().unwrap();
    let server = CertificateParams::new(vec!["localhost".into()])
        .unwrap()
        .signed_by(&server_key, &server_issuer)
        .unwrap();
    let mut params = CertificateParams::new(vec!["admin".into()]).unwrap();
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    let client_key = KeyPair::generate().unwrap();
    let client = params.signed_by(&client_key, &issuer).unwrap();
    let bad_key = KeyPair::generate().unwrap();
    let bad = CertificateParams::new(vec!["peer".into()])
        .unwrap()
        .signed_by(&bad_key, &server_issuer)
        .unwrap();
    let mut trust = rustls::RootCertStore::empty();
    trust.add(server_issuer.der().clone()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let server_cert = dir.path().join("server.crt");
    let server_key_path = dir.path().join("server.key");
    let management_client_ca = dir.path().join("management.crt");
    std::fs::write(&server_cert, server.pem()).unwrap();
    std::fs::write(&server_key_path, server_key.serialize_pem()).unwrap();
    std::fs::write(&management_client_ca, issuer.pem()).unwrap();
    let node_config = tavra_node::Config {
        bind: "127.0.0.1:0".parse().unwrap(),
        server_cert,
        server_key: server_key_path,
        management_client_ca,
    };
    let server_config = tavra_node::tls_config(&node_config).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(tavra_node::serve(listener, server_config));
    for (identity, success) in [
        (None, false),
        (Some((bad.der().clone(), bad_key.serialize_der())), false),
        (
            Some((client.der().clone(), client_key.serialize_der())),
            true,
        ),
    ] {
        let builder = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(trust.clone());
        let config = if let Some((cert, key)) = identity {
            builder
                .with_client_auth_cert(
                    vec![cert],
                    rustls::pki_types::PrivatePkcs8KeyDer::from(key).into(),
                )
                .unwrap()
        } else {
            builder.with_no_client_auth()
        };
        let result = timeout(Duration::from_secs(3), async {
            let socket = TcpStream::connect(addr).await?;
            let mut stream = tokio_rustls::TlsConnector::from(Arc::new(config))
                .connect(
                    rustls::pki_types::ServerName::try_from("localhost").unwrap(),
                    socket,
                )
                .await?;
            stream
                .write_all(b"GET /v1/metrics HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .await?;
            let mut response = Vec::new();
            match stream.read_to_end(&mut response).await {
                Ok(_) => (),
                Err(e) if response.is_empty() => return Err(e),
                Err(_) => (),
            };
            Ok::<_, std::io::Error>(response)
        })
        .await
        .unwrap();
        if success {
            let bytes = result.unwrap();
            let text = String::from_utf8(bytes).unwrap();
            assert!(text.starts_with("HTTP/1.1 200"));
            let metrics: tavra_node::Metrics =
                serde_json::from_str(text.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(metrics.os, std::env::consts::OS);
            assert!(metrics.capabilities.iter().all(|c| !c.supported));
            assert!(metrics.disks[0].total_bytes > 0);
        } else {
            assert!(result.is_err() || result.unwrap().is_empty());
        }
    }
    task.abort();
}
