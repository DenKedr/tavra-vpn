# TAVRA VPN

Native Admin/Client applications, an own Rust control core and experimental TAVRA Link VPN protocol, server management and filtering. **Active development — this preview is not yet a working system VPN.**

[Download development Preview1](https://github.com/DenKedr/tavra-vpn/releases/tag/v0.1.0-preview.1). Newer source includes server/domain inventory CRUD, tested with the native macOS app and real API; that continuation is not included in Preview1 installers.

## Implemented foundation

- Rust subscription/finite-lease policy, protocol prerequisites and attempt-correlated connection state machine.
- PostgreSQL HTTP control API: administrator authentication, users/subscriptions, Ed25519 device enrolment, concurrent device limits, revocation, server inventory, persisted routing/protection defaults.
- Server editing/deletion and domain CRUD/assignment with IDNA validation and retained audit. Domain records remain unverified until actual DNS/TLS checks exist; inventory deletion does not remotely uninstall software.
- Flutter macOS/Windows/Android development shell with real API data, client registration and device-local settings. It never reports a VPN connection before a native tunnel exists.
- LAN/site/IP/CIDR routing policy; DNS filtering rules and a local UDP test harness, with actual counters.
- TAVRA Link bounded IP framing, signed device admission bound to TLS exporter/nonce, encrypted QUIC transport and finite session deadline. A real isolated Linux TUN packet/negative/expiry/SIGTERM test passed on two authorized hosts; see [live acceptance](docs/live-acceptance.md).
- mTLS node metrics agent; independent management-client CA, no arbitrary remote command execution.
- Desktop background window lifecycle: menu/tray reopening and explicit exit. Windows runtime verification is pending.

## Run development API

Use Rust1.99.1+, PostgreSQL and environment variables; **generate your own high-entropy keys**, never use example credentials for a deployment.

```sh
export TAVRA_DATABASE_URL='postgres://USER:PASSWORD@127.0.0.1:5432/tavra'
# Set TAVRA_BOOTSTRAP_SECRET to your freshly generated administrator bootstrap secret.
cargo run -p tavra-control
```

Default API is `http://127.0.0.1:8787`; do not expose it directly to the Internet. See [API contract](docs/contracts.md). Bootstrap is one-use; subsequent login uses the administrator key chosen at activation. API configuration/secrets must be kept outside this repository.

```sh
cd apps/tavra
flutter pub get --enforce-lockfile
flutter run -d macos
```

Windows/Android use matching target runners. Current preview combines Admin/Client modes in one app; separate production IDs and signing are pending. Device private keys in this early shell are currently memory-only, so client enrolment is a development test until secure persistent storage is connected.

## Verify

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
# TAVRA_TEST_DATABASE_URL must name an isolated disposable PostgreSQL test database.
cargo test --workspace --locked
cd apps/tavra
flutter analyze
flutter test
```

Control integration tests truncate their dedicated test tables. Node and Link tests use loopback ephemeral ports, not live servers or OS route changes. [Node agent](docs/node-agent.md), [native feasibility](docs/platform-feasibility.md), [own Link wire specification](docs/tavra-link-v0.md), [routing/protection](docs/protection-and-routing.md).

## Full scope and limitations

[Design](docs/design.md), [implementation plan](docs/implementation-plan.md) and [feature matrix](docs/features.csv) retain the full requested scope. Native VPN/TUN routing, protocol engines, multi-hop/WARP, DPI camouflage and region-specific verification, push delivery, secure device storage and final signed installers are still being integrated. DNS rules alone are not a complete content/cosmetic AdGuard replacement. Plain QUIC is not DPI camouflage and is not marketed as undetectable.

The matrix retains all156 groups: `yes` means the specific group was implemented and accepted within its stated scope; `partial` records a tested subset and its remaining work; `no` remains pending. Partial groups do not count as completed, and successful installer builds do not establish a complete VPN product.

Installer workflows build explicitly labelled development previews; successful build is not a production security or network acceptance result. Research binaries, credentials, private deployment configurations and original product assets are excluded from this source tree.
