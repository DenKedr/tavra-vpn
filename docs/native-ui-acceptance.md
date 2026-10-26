# Native Flutter acceptance

Run from repository root:

```sh
python3 apps/tavra/integration_test/run_native_fixture.py
```

Requires the local Flutter SDK at `../../work/toolchains/flutter`, an existing debug `target/debug/tavra-control`, PostgreSQL and the private `../../work/runtime/dev-env.json`. Credentials are loaded without printing. The fixture refuses any database whose URL path is not exactly `/tavra_test`. It resets only that dedicated database's entities, policies and synthetic security-clock watermark. Never run concurrently with backend tests sharing this test database.

The fixture starts a separate API on loopback 8790, uses public test-only bootstrap/admin keys, runs the native macOS Flutter engine with `integration_test`, then stops its API in `finally`. No mock repository, production admin credentials, SSH, VPN activation or OS route changes are involved. The app performs real HTTP requests. API read-back checks persistence after UI actions.

Verified 2026-10-27: native Debug build and full smoke passed (1 test, 23 seconds test body).

Coverage: narrow 320×700 rendering and dark theme; login and real empty overview; create user; confirmed block; cancel and confirm deletion; LAN/site/CIDR policy save and API read-back; invalid client-local CIDR rejected in the form. Native window pixels, OS close/background behavior, mobile packaging and actual VPN traffic require separate platform acceptance. Widget driver interaction does not establish OS window-close behavior.
