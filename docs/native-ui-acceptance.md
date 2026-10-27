# Native Flutter acceptance

Run from repository root:

```sh
python3 apps/tavra/integration_test/run_native_fixture.py
```

Requires the local Flutter SDK at `../../work/toolchains/flutter`, an existing debug `target/debug/tavra-control`, PostgreSQL and the private `../../work/runtime/dev-env.json`. Credentials are loaded without printing. The fixture refuses any database whose URL path is not exactly `/tavra_test`. It resets only that dedicated database's entities, policies and synthetic security-clock watermark. Never run concurrently with backend tests sharing this test database.

The fixture starts a separate API on loopback 8790, uses public test-only bootstrap/admin keys, runs the native macOS Flutter engine with `integration_test`, then stops its API in `finally`. No mock repository, production admin credentials, SSH, VPN activation or OS route changes are involved. The app performs real HTTP requests. API read-back checks persistence after UI actions.

Verified 2026-10-27: latest native Debug build and expanded smoke passed (1 test, 48 seconds test body, exit 0). Flutter logged `Failed to foreground app; open returned 1`, but the native driver connected and every assertion completed. This warning does not establish successful OS foreground/window management.

Server/domain acceptance uses only reserved server address `198.51.100.10` and `.example` domain names. It verifies server create/edit persistence and `not_provisioned` status; canonical domain create and assignment; rename plus explicit unassignment; reassignment; server delete cancellation and confirmation; retained domain with null server and `unverified` status; domain delete cancellation and confirmation. Each state is independently read back using the actual API. A separate disposable API-created server checks the actual DELETE response has `remote_uninstall_performed: false`; UI deletion also checks the explicit no-uninstall notice. No host is contacted or provisioned and no DNS/TLS verification is claimed.

The fixture also clears test-only domains and inventory audit records, preventing stale inventory between runs.

Coverage: narrow 320×700 rendering and dark theme; login and real empty overview; create user; confirmed block; cancel and confirm deletion; LAN/site/CIDR policy save and API read-back; invalid client-local CIDR rejected in the form. Native window pixels, OS close/background behavior, mobile packaging and actual VPN traffic require separate platform acceptance. Widget driver interaction does not establish OS window-close behavior.
