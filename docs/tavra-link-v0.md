# TAVRA Link v0 — experimental wire format

Implementation stage: bounded codec and authenticated loopback QUIC first; no system TUN, production node integration or DPI camouflage claim until their acceptance tests pass.

## Transport and trust

QUIC v1/TLS1.3, ALPN `tavra-link/0`, 0-RTT disabled. Clients verify a certificate against an explicitly configured trusted root and verify server name; no insecure certificate bypass. A private deployment can import its own root from trusted Admin enrolment. ALPN is recognisable: this transport is not DPI camouflage. Own packet/session layer is independent from optional HTTPS camouflage.

## Frame envelope

All integers big endian. Header: magic `[0x54,0x56]` (TV), version `0`, type `u8`, payload_length `u32`; exactly one frame, trailing bytes rejected. Payload maximum 65,535 bytes; control payload cap 4,096. Unknown version/type, truncation or noncanonical length is fatal. Types: `0x01 Hello`, `0x02 Auth`, `0x03 Config`, `0x04 Packet`, `0x05 Ping`, `0x06 Close`. Control encoding fixed before extending schema; no arbitrary shell/config blob.

Hello is exactly 32 random nonce bytes. Auth and Config use bounded JSON control schema in v0 (deny unknown fields). The finite signed lease carries device key, node audience, epoch, issue and expiry. Auth contains that lease plus device signature over domain label + TLS exporter binding + server nonce + lease digest. Server checks audience/epoch/device/signature/expiry before Config. No data-session allocation before admission. Nonce uniqueness and exporter binding prevent cross-connection replay. The loopback proof fixture is not a control-plane-issued production lease; live user permissions/revocation from control-plane have yet to be attached.

Packet payload is one IPv4/IPv6 packet with exact header-reported length. The library checks IP version and exact length only; source assignment and destination policy are enforced by the separate Linux TUN test runtime. No system forwarding exists in the library itself. No data frames before authenticated Config. Ping is exactly 8 bytes sequence. Close is code u16 plus bounded UTF-8 reason, no credential detail.

## Bounds and lifecycle

Handshake timeout 5 seconds, control cap4096 and frame cap65535 are hard positive bounds. DATAGRAM support uses runtime max_datagram_size; sending beyond it returns typed error. MTU/reassembly extension must be negotiated and bounded before enabling IPv6 on a path unable to carry1280 inner bytes. v0 packet transport initially supports <= negotiated datagram size and does not silently truncate/fragment.

Only one admission control stream per connection; authorization is per connection. Rejected auth closes QUIC with generic application error. Successful loopback test proves authenticated encrypted packet transport only, not forwarding or client lease enforcement on real nodes. Future release must add node-enforced revocation, clock rollback/high-watermark, flow limits, TUN, bounded fragmentation and independent security review.
