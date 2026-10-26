# Linux TUN isolated test (experimental)

`tavra-link-cli CONFIG.json` runs one admitted session then exits. Linux only,
CAP_NET_ADMIN/root required. It creates a nonpersistent, exclusive TUN after signed
lease admission; closing the fd removes that interface. Never attaches an existing
interface. No address, route, NAT, forwarding, firewall or persistent system changes
are made by the binary. IPv6 unsupported. This is an own-protocol IP tunnel test,
not DPI camouflage, full client integration or production deployment.

Server JSON:
```json
{"interface":"tavra0","own_ip":"10.250.0.1","peer_ip":"10.250.0.2","mtu":1100,"bind":"0.0.0.0:44443","mode":"server","cert_file":"/run/tavra/cert.pem","key_file":"/run/tavra/key.pem","authority_key":"ED25519_PUBLIC_HEX_32_BYTES","node_id":"isolated-test","epoch":1}
```
Client JSON:
```json
{"interface":"tavra0","own_ip":"10.250.0.2","peer_ip":"10.250.0.1","mtu":1100,"bind":"0.0.0.0:0","mode":"client","server_addr":"SERVER_IPV4:44443","server_name":"link.test","root_cert_file":"/run/tavra/root.pem","device_key_file":"/run/tavra/device.hex","signed_lease_file":"/run/tavra/lease.json"}
```

Use CSPRNG-generated Ed25519 seeds. Device file is 64 hex characters (32-byte seed),
optional trailing newline. Lease file uses tavra-link `SignedLease` JSON, signed by
the matching authority through `admission::issue_lease`. Its audience, epoch and
public device key must match the config/device. Lifetime is 1..900 seconds. Store
private key files/config in a root-owned 0700 directory with 0600 files; never log
contents. TLS server key is PEM, certificate chain PEM; client root PEM must be
explicitly trusted and the certificate SAN must match `server_name`. There is no
insecure verifier, key log, or early data.

One live-host executor must first inspect existing management routes, interfaces,
service ports and firewall. Record SSH baseline and keep the original session open.
Only an authorized test UDP port may be made reachable. No default routes, NAT or
Internet forwarding belong in this test. Own/peer addresses must be distinct private
IPv4 addresses. Interface names are ASCII alphanumeric/underscore/hyphen, 1..15 bytes.
MTU config range 576..1100; parent must set the same actual interface MTU.

After both processes print readiness, the executor configures only the new interfaces:
```sh
# server
ip address add 10.250.0.1 peer 10.250.0.2/32 dev tavra0
ip link set dev tavra0 mtu 1100 up
# client
ip address add 10.250.0.2 peer 10.250.0.1/32 dev tavra0
ip link set dev tavra0 mtu 1100 up
```

Check ping in both directions, bounded packet sizes (`ping -c 3 -s 1000 PEER`),
and independent fresh SSH connections to both hosts. Record interface counters.
Drop inbound source != peer or destination != own; outbound reverse policy is exact.
Malformed/truncated/oversized/IPv6 packets are dropped. QUIC framing adds 8 bytes;
actual DATAGRAM capacity is checked by transport. No fragmentation/reassembly.

Negative acceptance: wrong TLS root/name must fail before TUN creation; expired,
wrong-node, wrong-epoch and wrong-device leases must fail admission. Wait for lease
expiry while idle: receive timeout exits, closes transport and removes TUN. Restart
requires a fresh valid finite lease. SIGTERM and CTRL-C remove the owned interface.
SIGKILL/normal process exit closes fd too. Confirm `ip link show tavra0` fails after
shutdown and that private test routes disappeared; verify baseline SSH/services.
Any host firewall test rule must have its own exact rollback managed by the executor.

Bounds: 1 admitted connection, 5s connect/admission timeout, 60s initial accept wait,
15s QUIC idle timeout, 64KiB datagram buffers, 64KiB single TUN read buffer, 1s TUN
write timeout, no task per packet. No TUN allocation before admission. The small
runtime is not a multi-user daemon: no reconnect, lease renewal, control-plane
revocation, clock high-watermark, metrics or production recovery integration yet.

Mac unit checks do not exercise Linux ioctl/TUN. Linux compile and actual two-host
packet/expiry/trust tests are separate release requirements; document actual results
rather than claiming this file proves them.

Offline lease fixture helper (no network, never writes the private seed):
```sh
issue-test-lease /run/tavra/authority.hex /run/tavra/claims.json > /run/tavra/lease.json
```
Claims JSON keys: `device_key` (public hex32), `node_id`, `epoch`, `issued_at`
(Unix seconds), `expires_at` (Unix seconds, at most issued_at + 900).
Authority/device seeds should be generated from the OS CSPRNG by the test executor;
the helper only signs provided claims, and does not enroll a production device.
