use hickory_proto::{
    op::{Message, MessageType, Query, ResponseCode},
    rr::{Name, RecordType},
};
use std::{sync::Arc, time::Duration};
use tavra_shield::*;
use tokio::net::UdpSocket;
fn query(host: &str) -> Vec<u8> {
    let mut q = Message::new();
    q.set_id(42)
        .add_query(Query::query(Name::from_ascii(host).unwrap(), RecordType::A));
    q.to_vec().unwrap()
}
#[tokio::test]
async fn real_loopback_udp_blocks_and_forwards_to_local_upstream() {
    let e = Arc::new(Engine::new(
        CompiledRules::compile(&[Source {
            category: Category::Ads,
            text: "||ads.test^",
        }])
        .unwrap(),
        Policy::default(),
    ));
    let upstream = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream.local_addr().unwrap();
    let upstream_task = tokio::spawn(async move {
        let mut buf = [0; 4096];
        let (n, peer) = upstream.recv_from(&mut buf).await.unwrap();
        let mut response = Message::from_vec(&buf[..n]).unwrap();
        response.set_message_type(MessageType::Response);
        upstream
            .send_to(&response.to_vec().unwrap(), peer)
            .await
            .unwrap();
    });
    let server = LoopbackDns::bind(e.clone(), Duration::from_millis(100))
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    assert!(addr.ip().is_loopback());
    let task = tokio::spawn(async move {
        for _ in 0..2 {
            server
                .serve_once(|request| async move {
                    let socket = UdpSocket::bind("127.0.0.1:0").await?;
                    socket.connect(upstream_addr).await?;
                    socket.send(&request).await?;
                    let mut buf = vec![0; 4096];
                    let n = socket.recv(&mut buf).await?;
                    buf.truncate(n);
                    Ok(buf)
                })
                .await
                .unwrap();
        }
    });
    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    for (host, code) in [
        ("ads.test", ResponseCode::NXDomain),
        ("good.test", ResponseCode::NoError),
    ] {
        client.send_to(&query(host), addr).await.unwrap();
        let mut b = [0; 4096];
        let (n, _) = tokio::time::timeout(Duration::from_secs(2), client.recv_from(&mut b))
            .await
            .unwrap()
            .unwrap();
        let response = Message::from_vec(&b[..n]).unwrap();
        assert_eq!(response.response_code(), code);
        assert_eq!(response.id(), 42);
    }
    task.await.unwrap();
    upstream_task.await.unwrap();
    assert_eq!(e.counters().blocked, 1);
    assert_eq!(e.counters().allowed, 1);
}
#[tokio::test]
async fn upstream_timeout_returns_servfail() {
    let engine = Arc::new(Engine::new(
        CompiledRules::compile(&[]).unwrap(),
        Policy::default(),
    ));
    let server = LoopbackDns::bind(engine, Duration::from_millis(10))
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let task = tokio::spawn(async move {
        server
            .serve_once(|_| async { std::future::pending::<std::io::Result<Vec<u8>>>().await })
            .await
            .unwrap();
    });
    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    client.send_to(&query("good.test"), addr).await.unwrap();
    let mut b = [0; 4096];
    let (n, _) = tokio::time::timeout(Duration::from_secs(1), client.recv_from(&mut b))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        Message::from_vec(&b[..n]).unwrap().response_code(),
        ResponseCode::ServFail
    );
    task.await.unwrap();
}
#[tokio::test]
async fn mismatched_upstream_id_is_not_forwarded() {
    let engine = Arc::new(Engine::new(
        CompiledRules::compile(&[]).unwrap(),
        Policy::default(),
    ));
    let server = LoopbackDns::bind(engine, Duration::from_millis(50))
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let task = tokio::spawn(async move {
        server
            .serve_once(|raw| async move {
                let mut response = Message::from_vec(&raw).unwrap();
                response.set_id(99).set_message_type(MessageType::Response);
                Ok(response.to_vec().unwrap())
            })
            .await
            .unwrap();
    });
    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    client.send_to(&query("good.test"), addr).await.unwrap();
    let mut b = [0; 4096];
    let (n, _) = tokio::time::timeout(Duration::from_secs(1), client.recv_from(&mut b))
        .await
        .unwrap()
        .unwrap();
    let response = Message::from_vec(&b[..n]).unwrap();
    assert_eq!(response.id(), 42);
    assert_eq!(response.response_code(), ResponseCode::ServFail);
    task.await.unwrap();
}
#[tokio::test]
async fn malformed_and_multiquestion_queries_never_reach_upstream() {
    for raw in [
        vec![0, 1, 2],
        {
            let mut m = Message::from_vec(&query("one.test")).unwrap();
            m.add_query(Query::query(
                Name::from_ascii("two.test").unwrap(),
                RecordType::A,
            ));
            m.to_vec().unwrap()
        },
        vec![0; 4097],
    ] {
        let engine = Arc::new(Engine::new(
            CompiledRules::compile(&[]).unwrap(),
            Policy::default(),
        ));
        let server = LoopbackDns::bind(engine.clone(), Duration::from_millis(50))
            .await
            .unwrap();
        let addr = server.local_addr().unwrap();
        let task = tokio::spawn(async move {
            server
                .serve_once(|_| async {
                    panic!("invalid request forwarded");
                    #[allow(unreachable_code)]
                    Ok(Vec::new())
                })
                .await
                .unwrap();
        });
        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        client.send_to(&raw, addr).await.unwrap();
        task.await.unwrap();
        assert_eq!(engine.counters(), Counters::default());
    }
}
