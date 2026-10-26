# macOS native feasibility — compile-only gate, 2026-10-27

**Result: SDK/API compilation passes; runtime per-app feasibility is NOT established.**
Target: macOS 13+, arm64. Host toolchain: Xcode 26.3, Apple Swift 6.2.4. No installed profile, saved preferences, system-extension activation, network route/PF change or new OS permission. Run `platform/macos-probe/verify.sh`: temporary executable constructs configuration objects and tests pure identity decisions, then deletes itself. This is neither an app nor a signed system-extension bundle.

## What the probe establishes

`NEPacketTunnelProvider` and `NETransparentProxyProvider` subclasses compile against deployment target 13.0. Both start callbacks deliberately return errors. Packet configuration has placeholder provider ID and `.invalid` server. Candidate transparent settings contain outbound TCP and UDP rules with nil address criteria; acceptance by the OS is **untested**. Avoid wildcard-address + port-zero rules: SDK documents restrictions; `plutil` and compilation do not validate a network policy.

Policy fixtures test exact signing-ID membership, explicit helper IDs, unknown-identity blocking, both empty-list modes and rejection of blank IDs. This demonstrates policy generation only. Signing ID is not bundle display name or security identity: production must verify audit-token code identity, designated requirement/team, PID reuse behavior, unsigned binaries and update/helper ownership. Prefix matching is intentionally absent. App picker and helper discovery are not implemented.

The transparent provider's `false` return bypasses the proxy to the ultimate destination. Protected/unknown flows are claimed and closed because forwarding is unimplemented. The provider cannot start, so this is not an operational kill switch. There is no packet transport, TCP pump, UDP association, DNS solution, IPC, persisted state or reconnect/sleep implementation.

## Architecture decision still required

PacketTunnel is the candidate IP tunnel for Rust in-process transport. IP packets do not automatically provide source application identity. macOS transparent proxy can inspect application metadata per TCP/UDP flow and choose direct versus handled flows. It is a plausible unmanaged-desktop candidate, not proof of complete VPN routing. It provides socket flows rather than arbitrary IP packets; Rust needs a flow-capable transport adapter or carefully validated conversion layer. An ordinary Rust TUN reader cannot consume NEAppProxyFlow directly.

Do not conflate managed per-app profiles with this consumer requirement. TN3134 restricts some platform/provider per-app cases to MDM-installed applications; the macOS system-extension rows permit direct Developer ID distribution and do not carry that blanket restriction. That does not establish that a chosen PacketTunnel app-rule configuration meets unmanaged desktop requirements. The proposed transparent mechanism must be tested without MDM.

Transparent settings ignore NEDNSSettings and NEProxySettings; name-based APIs still resolve DNS normally. Therefore protecting selected TCP/UDP payload does not establish DNS privacy. Resolver attribution, shared mDNSResponder, QUIC, IPv6, helper/browser subprocess identities, loopback proxies and non-TCP/UDP flows remain explicit risks. Excluding an app must not accidentally exempt unrelated traffic from a shared helper. Simultaneous PacketTunnel/proxy deployment, recursion avoidance and exact endpoint bypass need a runtime design and capture evidence. No claim of all-IP per-app support is made.

Network extensions must use embedded libraries with supported APIs for each engine. Do not rely on `Process`, shell, downloaded executables, arbitrary upstream CLI binaries, or a GUI subprocess as evidence of NE compatibility. Each adapter needs an in-process Rust/C ABI build for arm64/x86_64, bounded memory, cancellation and a transport/socket design that avoids proxy recursion. A separate privileged helper would require independently designed authenticated typed IPC and deployment validation; this spike supplies none.

## Packaging/signing integration requirements

Templates are placeholders, not production signing assets. Host requires system-extension installation entitlement and appropriate Network Extension capability/profile. Provider requires `packet-tunnel-provider-systemextension` and/or `app-proxy-provider-systemextension` authorized in its signing profile; transparent proxy uses app-proxy extension point. Remove unused capability when splitting targets. Replace org.example IDs, team prefix and module names consistently with NETunnelProviderProtocol identifiers. Shared Admin/Client ownership, upgrade and uninstall must be decided before packaging.

Build a real system-extension executable entry point calling `NEProvider.startSystemExtensionMode()` and its run loop; this compile harness intentionally has no such entry point. Embed signed `.systemextension` in host `Contents/Library/SystemExtensions`. Info mapping uses `NetworkExtension/NEProviderClasses`, not an iOS NSExtension principal-class plist. Sign nested code inside-out with Developer ID Application, correct approved provisioning profiles and entitlements; verify `codesign --verify --deep --strict`, entitlement/profile consistency and architectures. Developer ID Installer is separate for PKG. Notarize/staple and test Gatekeeper on a clean supported Mac.

Host must be installed in an appropriate Applications location and implement OSSystemExtensionRequest lifecycle including user approval, replacement/reboot-required, cancellation and denial; only then configure managers/save preferences and start a session with explicit user action. None of these calls occurs here. User approval/signing identities and a dedicated test machine are missing runtime dependencies, not reasons to activate on the current machine.

## Required next gate (not run)

Signed release-like bundles on unmanaged macOS 13 and current macOS, arm64 and x86_64: install/activate, only-selected/all-except app + helper TCP/UDP/QUIC, IPv4/IPv6 and DNS packet captures; direct/protected destinations, unsigned/missing identity, resolver/shared helper, tunnel endpoint exclusion, start failure, provider crash, GUI exit, reconnect/network switch, sleep/wake and explicit disconnect. Verify no protected direct packets before or during each transition; test both providers together and individually. Observe CPU/memory/backpressure, revoked/expired credentials, uninstall/upgrade and reboot. Until evidence exists, design §15 feasibility gate remains OPEN. Windows WFP/service and Android VpnService require their own target runners/devices; this Mac probe verifies neither.

## Primary sources

- [Apple TN3134: deployment and MDM restrictions](https://developer.apple.com/documentation/technotes/tn3134-network-extension-provider-deployment)
- [Apple transparent provider: bypass and DNS behavior](https://developer.apple.com/documentation/networkextension/netransparentproxyprovider)
- [Apple handling flow copying: application-based decisions](https://developer.apple.com/documentation/networkextension/handling-flow-copying)
- [Apple NEProvider: system extension entry point](https://developer.apple.com/documentation/networkextension/neprovider/startsystemextensionmode())
- Local installed macOS SDK headers: NEProvider.h (NEProviderClasses), NETransparentProxyNetworkSettings.h (rule restrictions), NEAppProxyProvider.h (flow ownership), NEFlowMetaData.h (signing ID/audit token). Headers were read directly; SDK compile is stronger API spelling evidence than an uncompiled sketch.
