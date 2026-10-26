# Development foundation verification — 27 October 2026

Scope: implemented foundation, not full-product production acceptance. Full feature matrix includes 156 groups; settings/core logic do not establish real VPN or DPI support.

- Rust workspace: core policies/routing, PostgreSQL auth/enrolment/revocation/concurrency/clock rollback, bounded packet codec, channel-bound signed lease and real loopback QUIC, mTLS node and local DNS filter tests.
- Flutter: 19 automated tests and analyzer, covering API contracts, Ed25519 proof, 401/session lifecycle, narrow layout, persistent nonsecret client policies and pre-save DNS/routing limits.
- macOS native: ARM64 debug build, real app opened; background close/reopen and API UI acceptance are being verified separately. Compile-only NE probe is not a working VPN extension.
- Security reviews: stale attempt confirmation, clock rollback grants, block/unblock challenge, handshake TTL reset, post-await expiry and UI policy/session findings fixed; source-level closure recorded. This is not an external production cryptographic audit.
- Packaging workflows pass actionlint and syntax checks; CI builds and platform installer acceptance must be read from actual run results. Android/Windows runtime and Developer ID/notarization are not established by local macOS build.
- Linux TUN runtime has Mac parser/packet tests; Linux compilation and real two-host tests are separate acceptance. It intentionally modifies no default routes, NAT or firewall.

Private test infrastructure and credentials are kept outside the source tree. Only primary operates live test hosts. No certificate trust/profile, macOS VPN route or autostart has been installed on the developer computer.
