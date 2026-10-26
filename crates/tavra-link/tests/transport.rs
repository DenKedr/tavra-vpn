use ed25519_dalek::SigningKey;
use rustls::pki_types::PrivatePkcs8KeyDer;
use std::sync::Arc;
use tavra_link::{admission::*, transport};
fn endpoints() -> (quinn::Endpoint, quinn::Endpoint) {
    let c = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert = c.cert.der().clone();
    let key = PrivatePkcs8KeyDer::from(c.signing_key.serialize_der());
    let mut tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert.clone()], key.into())
        .unwrap();
    tls.alpn_protocols = vec![b"tavra-link/0".to_vec()];
    tls.max_early_data_size = 0;
    let server = quinn::Endpoint::server(
        quinn::ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(tls).unwrap(),
        )),
        "127.0.0.1:0".parse().unwrap(),
    )
    .unwrap();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert).unwrap();
    let mut tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    tls.alpn_protocols = vec![b"tavra-link/0".to_vec()];
    tls.enable_early_data = false;
    let mut client = quinn::Endpoint::client("127.0.0.1:0".parse().unwrap()).unwrap();
    client.set_default_client_config(quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(tls).unwrap(),
    )));
    (server, client)
}
fn packet() -> Vec<u8> {
    let mut p = vec![0; 20];
    p[0] = 0x45;
    p[2..4].copy_from_slice(&20u16.to_be_bytes());
    p[8] = 64;
    p
}
#[tokio::test]
async fn encrypted_quic_authenticated_datagram_roundtrip() {
    let (server, client) = endpoints();
    let authority = SigningKey::from_bytes(&[1; 32]);
    let device = SigningKey::from_bytes(&[2; 32]);
    let lease = issue_lease(
        &authority,
        LeaseClaims {
            device_key: hex::encode(device.verifying_key().as_bytes()),
            node_id: "test-node".into(),
            epoch: 1,
            issued_at: 100,
            expires_at: 200,
        },
    )
    .unwrap();
    let accepted = async {
        let conn = server.accept().await.unwrap().await.unwrap();
        let admitted = transport::accept(conn, &authority.verifying_key(), "test-node", 1, 150)
            .await
            .unwrap();
        let data = admitted.receive_packet().await.unwrap();
        assert_eq!(data, packet());
        admitted.send_packet(&data).unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        admitted.close();
    };
    let connected = async {
        let conn = client
            .connect(server.local_addr().unwrap(), "localhost")
            .unwrap()
            .await
            .unwrap();
        let admitted = transport::connect(conn, lease, &device, 150).await.unwrap();
        admitted.send_packet(&packet()).unwrap();
        let data = admitted.receive_packet().await.unwrap();
        assert_eq!(data, packet());
        admitted.close();
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(accepted, connected);
    })
    .await
    .unwrap();
    client.wait_idle().await;
    server.wait_idle().await;
}
#[tokio::test]
async fn expired_admission_never_creates_data_session() {
    let (server, client) = endpoints();
    let a = SigningKey::from_bytes(&[1; 32]);
    let d = SigningKey::from_bytes(&[2; 32]);
    let l = issue_lease(
        &a,
        LeaseClaims {
            device_key: hex::encode(d.verifying_key().as_bytes()),
            node_id: "n".into(),
            epoch: 1,
            issued_at: 100,
            expires_at: 200,
        },
    )
    .unwrap();
    let recv = async {
        let conn = server.accept().await.unwrap().await.unwrap();
        assert!(
            transport::accept(conn, &a.verifying_key(), "n", 1, 200)
                .await
                .is_err()
        );
    };
    let send = async {
        let conn = client
            .connect(server.local_addr().unwrap(), "localhost")
            .unwrap()
            .await
            .unwrap();
        assert!(transport::connect(conn, l, &d, 150).await.is_err());
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(recv, send);
    })
    .await
    .unwrap();
    client.close(0u32.into(), b"done");
    server.close(0u32.into(), b"done");
}

#[tokio::test]
async fn packet_received_before_expiry_cannot_be_forwarded_after_readiness_delay() {
    let (server, client) = endpoints();
    let authority = SigningKey::from_bytes(&[3; 32]);
    let device = SigningKey::from_bytes(&[4; 32]);
    let lease = issue_lease(
        &authority,
        LeaseClaims {
            device_key: hex::encode(device.verifying_key().as_bytes()),
            node_id: "delayed-tun".into(),
            epoch: 1,
            issued_at: 100,
            expires_at: 101,
        },
    )
    .unwrap();
    let receiving = async {
        let conn = server.accept().await.unwrap().await.unwrap();
        let admitted = transport::accept(conn, &authority.verifying_key(), "delayed-tun", 1, 100)
            .await
            .unwrap();
        let queued_packet = admitted.receive_packet().await.unwrap();
        assert_eq!(queued_packet, packet());
        let remaining = admitted.forwarding_allowance().unwrap();
        assert!(remaining <= std::time::Duration::from_secs(1));
        // Model a TUN that becomes writable only after its queued packet's lease
        // expires. The same final permission gate is called before libc::write.
        tokio::time::sleep(remaining + std::time::Duration::from_millis(20)).await;
        assert!(admitted.forwarding_allowance().is_err());
        assert!(admitted.send_packet(&queued_packet).is_err());
        assert!(admitted.forwarding_allowance().is_err());
    };
    let sending = async {
        let conn = client
            .connect(server.local_addr().unwrap(), "localhost")
            .unwrap()
            .await
            .unwrap();
        let admitted = transport::connect(conn, lease, &device, 100).await.unwrap();
        admitted.send_packet(&packet()).unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(1300)).await;
        admitted.close();
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(receiving, sending);
    })
    .await
    .unwrap();
    client.close(0u32.into(), b"done");
    server.close(0u32.into(), b"done");
}
