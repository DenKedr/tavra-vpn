# TAVRA VPN Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the approved TAVRA Admin/Client VPN platform, own Core and Link, server management, protocol adapters and signed platform releases.

**Architecture:** Flutter clients call a Rust core through a versioned boundary and native VPN services. A Rust control plane with PostgreSQL owns subscriptions and server state; node agents reconcile signed configuration and protocol lifecycles. TAVRA Link provides authenticated IP packet framing over QUIC/TLS, with a separate tested camouflage layer.

**Tech Stack:** Rust, Flutter/Dart, Kotlin, Swift/Network Extension, Windows service/WFP, PostgreSQL, GitHub Actions.

**Spec:** [TAVRA-design.md](TAVRA-design.md), approved by Denis on 27 October 2026: «все ок тогда продолжай работу».

## Global Constraints

- Windows 10 22H2/Windows 11 x64; macOS 13+ arm64/x86_64; Android 10+ arm64.
- Ubuntu 22.04/24.04/26.04 × amd64/arm64; DEB for control/node, EXE/APK/PKG/DMG for applications.
- `expires=0` means never expires; `device_limit=0` means unlimited registered devices. Authentication and finite leases remain mandatory.
- Own Core and own Link are mandatory; reviewed cryptographic transports, no custom cipher.
- Anti-DPI is mandatory, plain QUIC/custom ALPN is not camouflage; regional support requires dated real-network tests.
- All 131 feature groups remain in the denominator; partial delivery is labelled partial.
- One live-host writer, inspect before mutation, preserve SSH and prepared rollback.
- No inherited server access assumed; credentials and local profiles never enter repository/release.
- Heavy Mac builds run serially; stage native runtime feasibility early.

## Review Focus

- Concurrent enrolment at the last device slot: transaction admits one unique device, retry is idempotent (task 2).
- Unreachable primary or clock rollback: cached credentials cannot extend access, restore must not resurrect revocation (tasks 2, 9).
- Partial install/crash/network change: fail closed, retain independently verified management path (tasks 3, 5).
- Cross-platform adapters: a working desktop subprocess does not prove NE/VpnService support (tasks 4, 6).
- Public releases: unsigned/misidentified/downgraded artifacts never execute under a valid-looking checksum alone (task 10).

## Repository and ownership

Product root: `outputs/tavra-vpn/`. Research remains outside it. `crates/tavra-core` owns schema, policies, state machine and capabilities. `crates/tavra-control` owns HTTP/auth/database. `crates/tavra-link` owns codec/authenticated transport. `crates/tavra-node` owns reconciliation and measurements. `apps/tavra` owns shared Flutter UI and native app runners. `platform/` owns privileged runtime probes and shims. `packaging/`, `.github/`, `docs/` own distribution and contracts.

Parallel authors own separate directories and coordinate dependency contracts through the primary. Root Cargo/lockfile/integration are primary-owned; agents do not change them concurrently.

### Task 1: Contracts and native feasibility

**Files:** `docs/contracts.md`, `docs/platform-feasibility.md`, `crates/tavra-core/src/{lib,model,policy,capability,connection}.rs`, `crates/tavra-core/tests/policies.rs`, `platform/macos-probe/`.

**Interfaces:** `SubscriptionPolicy::validate_at(now_unix_s:i64)->Result<(),PolicyError>`; `Capabilities::evaluate(&EndpointPrerequisites)->Availability`; `ConnectionMachine::transition(ConnectionEvent)->Result<ConnectionState,TransitionError>`. JSON contract uses snake_case, stable enum strings and Unix seconds, no secret-bearing Debug output.

- [ ] Write failing policy tests for expiry 0, device_limit 0, invalid negative/overflow, blocked policy, unknown prerequisites and forbidden direct fallback.
- [ ] Implement typed policy/capabilities and finite lease defaults. Verify `cargo test -p tavra-core`.
- [ ] Compile an isolated macOS Network Extension API probe without installing profiles or altering routes; record entitlement/runtime limitations and per-app feasibility. Inspect installed Flutter/SDK before installing project toolchains.
- [ ] Commit only source/docs and manifest; evidence remains in project work directory.

### Task 2: Real subscriptions, users, device registration and revocation

**Files:** `crates/tavra-control/src/{main,lib,config,auth,db,users,subscriptions,devices,servers}.rs`, `crates/tavra-control/migrations/`, `crates/tavra-control/tests/api.rs`.

**Interfaces:** REST `/v1/health`, `/v1/admin/bootstrap`, `/v1/admin/session`, `/v1/users`, `/v1/subscriptions`, `/v1/enroll`, `/v1/devices`, `/v1/subscriptions/:id/revoke`. Authenticated admin mutations; invite bootstrap stores hash only. `POST /v1/enroll` input contains invitation, public key and possession proof; output is device id and finite lease/manifest.

- [ ] Write integration tests for unauthorized admin CRUD, persistence across restart, business validation, independent credentials, transactional last-slot race/idempotent retry, blocked/expired invites and revoke.
- [ ] Implement PostgreSQL schema/migrations and repository with dependency-injected clock; local demo storage, if needed, is explicitly a development adapter with the same transaction invariants.
- [ ] Verify actual HTTP create/read/revoke and no secret in list/diagnostic responses; `cargo test -p tavra-control` and smoke with clean DB.
- [ ] Commit schema/API. Mark only corresponding verified subfeatures; never mark full subscription security complete before proof/leases work.

### Task 3: Own Link protocol

**Files:** `docs/tavra-link-v0.md`, `crates/tavra-link/src/{lib,codec,session,transport}.rs`, `crates/tavra-link/tests/{codec,transport}.rs`, fuzz targets.

**Interfaces:** `Frame::encode()->Result<Vec<u8>,CodecError>`, `Frame::decode(&[u8])->Result<Frame,CodecError>`; messages Hello/Auth/Config/Packet/Ping/Close with bounded sizes. QUIC stream admission and DATAGRAM IP transport; no data before verified finite device lease and channel-bound proof; no0RTT.

- [ ] Specify wire bytes, length/MTU/reassembly caps, admission audience/nonce/session binding and fail rules before implementation.
- [ ] Write negative codec/auth tests: truncation, extra/unknown data, oversize, malformed IP, replay, expiry, wrong node/protocol/device.
- [ ] Implement codec and real loopback QUIC with standard TLS, certificate verification/pin and no0RTT; test encrypted bidirectional packet roundtrip, rejections and clean shutdown.
- [ ] Add bounded fragmentation/IPv6 policy and TLS/TCP fallback, then fuzz/loss/reorder/DoS tests. Mark plain transport experimental until separate camouflage and security gates pass.

### Task 4: Flutter Admin and Client with real API data

**Files:** `apps/tavra/pubspec.yaml`, `lib/{main,app,api,models}.dart`, `lib/screens/{dashboard,servers,users,subscription,client,settings}.dart`, `test/`.

**Interfaces:** `/v1` HTTP contract from task 2; UI never simulates a successful VPN connection. Stable disabled reason for unsupported runtime/prerequisites.

- [ ] Add model/API tests for malformed/unauthorized/offline responses and credential redaction.
- [ ] Build shared navy/ivory/turquoise design system and Admin/Client modes with separate production IDs. Connect user/subscription/server forms to persistence; all buttons perform named actions or show explicit unavailable reason.
- [ ] Run `flutter analyze`, widget/API tests and a native macOS build; visually verify actual forms, list persistence, errors, keyboard and narrow layout.
- [ ] Pair actual VPN actions only after native runtime integration; client state stays disconnected until core event confirms tunnel.

### Task 5: SSH installer, node agent and first real standard VPN path

**Files:** `crates/tavra-node/`, `crates/tavra-provision/`, `packaging/deb/`, installer tests/VM scripts.

**Interfaces:** `inspect->plan->apply->verify->commit/rollback`, installation resource manifest, typed node reconcile jobs and applied revision. SSH password/key/user cert and strict known-host verification. Adapter contract from spec §6.

- [ ] Write fixture tests for `/etc/os-release`, architecture, occupied TCP/UDP ports, rejected unknown host, interrupted install/idempotency and manifest-limited uninstall.
- [ ] Implement read-only inspect then reversible versioned provisioning, node mTLS enrolment, separate route/firewall ownership and timed rollback/OOB gate.
- [ ] Integrate one standard transport end to end with real TUN and individual device credentials; validate create subscription→enrol→connect→traffic→stats→revoke.
- [ ] Test all six Ubuntu combinations on authorized VM/hosts, fresh independent SSH and preservation of existing services. No arbitrary known host reused.

### Task 6: Remaining protocols and native platform runtimes

**Files:** `crates/tavra-adapters/`, native Kotlin/Swift/Windows service modules, protocol runtime manifests.

- [ ] Add pinned AWG/WG/OpenVPN/Cloak/Shadowsocks/IKEv2/Xray/Naive/Hysteria2/TrustTunnel/dnstt adapters, each with testable platform capabilities and auth/revoke.
- [ ] Verify library/packaged runtime inside Windows service, macOS NE and Android VpnService before enabling UI.
- [ ] Test app-only/all-except on all three OS, helper subprocesses, DNS/IPv6, sleep/crash/network switch; Android protect transport sockets, 4/16KB native ABI.
- [ ] Add original non-VPN services and raw settings/import/export with ownership and licence provenance checks.

### Task 7: Multi-node topology, domains, WARP and load balancing

**Files:** topology module, domain/cert lifecycle module, node policy adapters, UI editors and network acceptance.

- [ ] Test cycles/address overlaps/invalid capability combos and no-domain disabled reasons; add node/domain CRUD and drain/uninstall distinctions.
- [ ] Implement client→entry→exit with source/return-route/NAT policy, WARP isolated egress and sticky health/capacity balancing.
- [ ] Check Exit/WARP failure never leaks via Entry/direct; test manual selection, unknown/stale capacity and reconnection semantics.
- [ ] Add exact read/import/delegated mutation scope providers for Remnawave/Hiddify/3X-UI; reject dual ownership.

### Task 8: Health, usage, speedtest, push and DPI

**Files:** observer, metrics/usage store, alert queue, provider delivery adapters, speedtest service, camouflage profiles and lab matrix.

- [ ] Test counter reset/retry dedup, stale/unknown health, text+colour/antenna load and rate-limited speedtest tickets/directions.
- [ ] Implement independent observer and real FCM/APNs/WNS delivery, including closed-app Windows identity spike and provider errors/retry.
- [ ] Integrate authenticated HTTPS camouflage for own Link and established transport profiles; test UDP/SNI/DNS blocking, no downgrade, fingerprints and bounded own-server active probes.
- [ ] Record dated/operator-specific voluntary network tests for RU/CN/IR/UAE; unsupported regional claims remain unverified.

### Task 9: Admin pairing, backup and primary migration

**Files:** pairing/recovery/migration services, DB backup tools, signed generation/high-watermark logic, UI flows.

- [ ] Test atomic first-owner bootstrap, key-bound single-use pairing, last-owner protection and admin-device revoke.
- [ ] Implement backup/restore rehearsal, write freeze/fencing/promote with signed endpoint/trust transition; no two writable primaries.
- [ ] Test clock/DB rollback, stale credentials, lost old IP without backup, node reconnection and explicit recovery.

### Task 10: Complete source and platform releases

**Files:** `.github/workflows/`, platform installers, signed update metadata, SBOM/notices, user/admin docs.

- [ ] Set up public GitHub repo `DenKedr/tavra-vpn`, deny secret/local-profile/reference-installer inclusion, push reviewed sources.
- [ ] Build EXE/APK/PKG/DMG/DEB on matching runners; include native helper/engines/runtime and proper lifecycle/ownership.
- [ ] Verify install/upgrade/uninstall clean-machine matrix, signature/entitlement/16KB/page-size checks and no unsigned/downgrade update execution.
- [ ] Publish versioned artifacts only with honest prerelease status and actual checksums/provenance; complete full feature audit independently before final production-ready claim.

## Execution decision

Denis has authorized autonomous team execution and approved the full design. Proceed using scoped subagents and primary integration without another approval round. Technical plan decisions are routine within the approved scope. Missing live-host/platform credentials block only their dependent acceptance, not local implementation.
