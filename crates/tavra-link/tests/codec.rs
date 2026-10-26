use tavra_link::codec::Frame;
fn packet() -> Vec<u8> {
    let mut p = vec![0; 24];
    p[0] = 0x45;
    p[2..4].copy_from_slice(&24u16.to_be_bytes());
    p[8] = 64;
    p[9] = 17;
    p[12..16].copy_from_slice(&[10, 8, 0, 2]);
    p[16..20].copy_from_slice(&[1, 1, 1, 1]);
    p[20..].copy_from_slice(b"TAVR");
    p
}
#[test]
fn canonical_header_and_roundtrip_all_frame_kinds() {
    let cases = vec![
        Frame::Hello(vec![7; 32]),
        Frame::Auth(b"{}".to_vec()),
        Frame::Config(b"{}".to_vec()),
        Frame::Packet(packet()),
        Frame::Ping(42),
        Frame::Close {
            code: 1,
            reason: "finish".into(),
        },
    ];
    for f in cases {
        let bytes = f.encode().unwrap();
        assert_eq!(&bytes[..3], &[0x54, 0x56, 0]);
        assert_eq!(Frame::decode(&bytes).unwrap(), f);
        for n in 0..bytes.len() {
            assert!(Frame::decode(&bytes[..n]).is_err());
        }
        let mut extra = bytes;
        extra.push(0);
        assert!(Frame::decode(&extra).is_err());
    }
}
#[test]
fn rejects_unsupported_versions_lengths_and_payloads() {
    let mut encoded = Frame::Ping(1).encode().unwrap();
    encoded[2] = 1;
    assert!(Frame::decode(&encoded).is_err());
    encoded[2] = 0;
    encoded[3] = 255;
    assert!(Frame::decode(&encoded).is_err());
    assert!(Frame::Hello(vec![0; 31]).encode().is_err());
    assert!(Frame::Auth(vec![0; 4097]).encode().is_err());
    assert!(Frame::Config(vec![0; 4097]).encode().is_err());
    assert!(
        Frame::Close {
            code: 0,
            reason: "x".repeat(257)
        }
        .encode()
        .is_err()
    );
    let mut bad = packet();
    bad[2..4].copy_from_slice(&200u16.to_be_bytes());
    assert!(Frame::Packet(bad).encode().is_err());
    assert!(Frame::Packet(vec![0; 20]).encode().is_err());
    assert!(Frame::Packet(vec![0; 65536]).encode().is_err());
}
#[test]
fn validates_ipv6_and_non_utf8_close() {
    let mut p = vec![0; 40];
    p[0] = 0x60;
    assert!(Frame::Packet(p.clone()).encode().is_ok());
    p.push(1);
    assert!(Frame::Packet(p).encode().is_err());
    let bad = [0x54, 0x56, 0, 6, 0, 0, 0, 3, 0, 0, 0xff];
    assert!(Frame::decode(&bad).is_err());
}
