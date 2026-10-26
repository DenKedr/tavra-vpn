# TAVRA protection and routing extension

Approved additions from Denis, 27 October 2026: AdGuard-like functions inside Settings, AdGuard VPN protocol in selector; bypass LAN, website exceptions, individual IP/CIDR exceptions. These extend the existing full scope and do not replace VPN, own Link or installers.

## Sources and interpretation

- [AdGuard welcome](https://adguard.info/ru/welcome.html): product reference for ad/tracker and browsing protection.
- [SecurityLab article](https://www.securitylab.ru/blog/personal/Bitshield/360762.php): third-party commentary about AdGuard VPN/privacy, not another executable product or protocol specification. Its marketing, legal and audit claims are not implementation requirements or independently verified facts.
- [AdGuard official TrustTunnel announcement](https://adguard-vpn.com/en/blog/adguard-vpn-protocol-goes-open-source-meet-trusttunnel.html): confirms that the protocol powering AdGuard VPN is now open-source TrustTunnel. Selector label: **TrustTunnel (протокол AdGuard VPN)**; one adapter, not two different protocols.
- [Official Android feature inventory](https://adguard.com/kb/adguard-for-android/features/): protection, per-app management, statistics, filters, DNS, HTTPS options/userscripts and integration with VPN. These groups are references; platform support must be established separately.

## Settings / Protection

Own module named TAVRA Shield, own interface/assets; no AdGuard account, commercial subscription, proprietary binaries or brand cloning. Settings cover ad blocking, tracking protection, annoyance/cookie/pop-up blocking, malicious/phishing domains, optional adult-content/safe-search policy, selected DNS with encrypted transport, filter list versions/update/errors, user allow/block rules, per-app/per-site exceptions, temporary pause and local counters. DNS request journal is local, off by default, with explicit retention/clear; no domain/payload logs in push/cloud stats.

DNS blocking, URL/network rules and cosmetic page filtering are separate capabilities. DNS cannot remove same-domain ads, page elements, first-party sponsored content or all video ads. Browser cosmetic rules/element picker need an explicit browser integration; HTTPS URL/content filtering needs its own consent and platform-compatible implementation. Optional local CA/profile installation is a separate on-device action; no CA, system trust, browser extension or HTTPS interception is installed on the developer Mac by this task. Certificate-pinned apps and Android trust limitations must be shown. DNS privacy and filter behavior cannot be inferred from a checkbox.

First implementation: strict bounded domain-rule engine and persistent policy; accepted rule syntax and unsupported rules reported. Later: runtime DNS interception inside our existing VPN service, encrypted upstream, platform/app identity hooks, content/cosmetic rules and per-platform acceptance. Android uses one VpnService for tunnel+filter, not a second competing VPN. Before runtime hookup, UI reports configured policy and `not_connected`; never fabricated blocked counts or protection-active status.

## Settings / Routing

Fields: `bypass_lan` default true, `excluded_sites` default empty, `excluded_networks` default empty. Site accepts hostname or HTTP(S) URL converted to canonical host; no wildcard catch-all or credentials, exact hostname and its subdomains only. `example.com` matches `www.example.com` but not `example.com.evil.test` or `evil-example.com`. IP accepts IPv4/IPv6 literal or CIDR; invalid ranges or prefixes show errors. Rules are bounded and validated before save. No route changes on invalid save.

Bypass LAN includes RFC1918, loopback and link-local IPv4, ULA/link-local/loopback IPv6 plus validated directly connected interface prefixes discovered by the native backend. CGNAT100.64/10 and arbitrary global IPv6 are not automatically LAN. Multicast discovery policy is a separate capability rather than a global Internet bypass. Exact endpoint/control-plane bypass is privileged and not user-editable as arbitrary process exceptions.

Routing precedence: authenticated management/transport safety restrictions; application-selection policy; explicit site/IP/LAN exclusions direct; otherwise protected tunnel. Excluded traffic intentionally bypasses VPN and UI says so. Domain-IP caches carry TTL and per-request attribution; a shared IP alone must not silently bypass other hosted domains. DNS for protected domains remains inside the VPN; exempt LAN names may use a separately scoped local resolver. Remote DoH in third-party apps and shared OS resolvers require platform tests, not fake hostname knowledge.

Client settings are device-local; Admin may store deployment defaults, which do not silently override local policies or the mandatory organisation policy. Persistent config contains no credentials; admin/device tokens remain in appropriate secure storage. Apply action is transactional and waits for native confirmation. With no runtime attached policy can be saved but UI explicitly reports pending application.

Acceptance: add/edit/remove/persist/reload; malformed IDNA/URL/userinfo/CIDR; IPv4 and IPv6 LAN/public/CGNAT; suffix boundary; shared hosting and DNS TTL; app helper identity; sleep/network switch; only-excluded direct packet capture; kill-switch behavior for protected vs intentionally excluded flows; filters cannot create a second VPN or disable tunnel TLS trust.

## Scope tracking

New rows are appended to the original feature matrix. No implemented core/config setting alone is marked as a completed system-wide AdGuard replacement. Full filter/content/browser/runtime capabilities and real traffic tests remain tracked until delivered.

## Background lifecycle (new explicit requirement)

Closing a desktop window hides the UI and keeps the application, VPN backend, monitors and notifications running. macOS menu-bar item/Windows tray allows reopening and explicit exit. Separate Disconnect stops VPN without quitting UI/background observers; explicit Exit and disconnect stops owned services cleanly. Default is continue on window close. No autostart is silently enabled on the developer computer; product startup setting is separate.

Android leaving/swiping the UI does not stop the foreground VpnService; it has the required persistent system notification and reconnect policy. Android OS force-stop, denied permissions and platform termination are separate states: the application cannot promise to evade force-stop. Native helpers/extensions may outlive UI only with ownership, finite lease enforcement and fail-closed checks; a minimized window alone does not prove persistence of the VPN data path.

Acceptance: close/reopen and tray/menu actions, UI crash vs provider crash, sleep/network changes, background lease expiry/revoke/push, explicit Exit, restart and uninstall without orphan services. Desktop window hiding can be delivered before VPN runtime, with honest disconnected state.
