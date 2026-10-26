# Fix 1 — fail-closed authorization time and user-block boundary

Scope: independent review P2 clock rollback / stale enrollment after user block. Only tavra-control files changed. Root settings files are owned separately.

## Changes

- Migration0002 adds durable singleton `security_clock.high_water`; migration0003 adds credential generation to clock/sessions/challenges.
- Outer request middleware samples one trusted Unix-second time per request, after locking the persisted watermark. All authorization deadlines and handler timestamps use that request snapshot. Health remains available independently.
- Wall time below durable high_water, invalid/pre-epoch wall time, or wall time falling behind the process monotonic Instant baseline fails closed with503 `{error:{code:"clock_untrusted",message:"Authorization paused: correct server clock, then sign in again"}}`.
- Detection advances the durable boundary to at least the monotonic floor, increments generation and deletes sessions/challenges atomically. Restart does not clear the boundary. It does not use max(now,high_water) as an apparently valid frozen clock.
- Credential issue/consume transactions hold a SHARE lock on security_clock and verify request generation before taking user/subscription/challenge locks. Clock invalidation uses the exclusive clock lock. This prevents issuance concurrently slipping past invalidation. Authentication also checks persisted current generation.
- User block deletes pending challenges for every owned subscription in the same user-locked transaction that revokes devices. Unblock does not restore them. Subscription block/revoke/rotate already uses transactional invalidation and remains unchanged.

## Recovery

Ordinary recovery: correct/synchronize the OS wall clock to a trustworthy value at or later than persisted `security_clock.high_water`. Login again using the existing persistent admin key; old sessions and old challenges stay invalid, new challenges must be requested. No credentials should be pasted into logs or SQL. Application restart is optional and cannot bypass the durable boundary.

If an accidentally far-future clock made the persisted boundary impractical, do not lower it while the API is serving. Operator maintenance recovery requires stopping every control-plane process using this DB, verifying the correct UTC time from a trusted source, backing up the DB, then in one DB transaction incrementing security_clock.generation, deleting sessions and challenges, revoking all existing subscriptions/invitations and devices, and replacing high_water with that verified Unix time. Revoking subscriptions is deliberately destructive and needs explicit operator authorization; access must be reissued after recovery so an earlier observed-expired subscription cannot revive after the manual reset. Restart only with the corrected OS clock and perform fresh admin login. Retain the admin credential hash. Never restore an older sessions/challenges table, lower only the watermark on a live API, or reset generation: those operations can resurrect credentials. This exceptional DB procedure is documentation, not an operation executed here.

## Verification

Actual PostgreSQL regression tests observed RED for block/unblock stale proof (200 vs401), rollback revival (200 vs503) and stale-generation in-flight issuance (200 vs401), then GREEN after fixes. Test coverage includes challenge/session forward-expiry then backward retry, durable boundary across AppState reconnect, correct-clock recovery, stale-generation session denial, monotonic frozen-clock denial, pre-epoch denial, and health availability while auth closed. Existing integration suite adjusted to move its injected test clock forward rather than silently rolling back between unrelated scenarios. Isolated test DB only.

No credentials or live host changes. No signed/node-enforced VPN lease claim added.

Final verification: `cargo test -p tavra-control` passed (3 test functions, actual isolated PostgreSQL), `cargo clippy -p tavra-control --all-targets -- -D warnings` passed, formatting applied. This result predates primary settings-module integration, which remains owned by primary.
