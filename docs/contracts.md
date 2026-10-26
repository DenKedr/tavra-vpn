# Initial /v1 contract (implementation tranche 1)

All JSON keys snake_case; times integer Unix seconds; identifiers UUID strings. Development API binds loopback by default. HTTP errors `{ "error": { "code": "...", "message": "..." } }`; no tokens/credentials in error bodies. Admin routes require `Authorization: Bearer <session>`.

- GET /v1/health -> `{status,version,storage}`.
- POST /v1/admin/bootstrap `{bootstrap_secret,admin_secret}` -> `{token,expires_at}`; one-use owner activation. `admin_secret` is a locally generated high-entropy administrator key (not a short password), its hash persists; POST /v1/admin/session `{admin_secret}` authenticates after restart without re-enabling bootstrap. Random 256-bit sessions expire after 8h.
- GET/POST /v1/users; user fields `{id,first_name,last_name,phone,email,status,created_at}`. PATCH/DELETE /v1/users/{id}; mutations require admin. Contact fields may be empty, names cannot exceed 120 chars, email max254/phone max64.
- GET/POST /v1/subscriptions; create `{user_id,expires_at,device_limit}` where expiry0=unlimited, limit0=unlimited. Response subscription has `{id,user_id,status,expires_at,device_limit,created_at}`; invitation returned only on creation/rotate.
- POST /v1/subscriptions/{id}/revoke and /rotate; revoke stops enrolment and invalidates credentials.
- GET /v1/devices and DELETE /v1/devices/{id}; list has no private credentials.
- POST /v1/enroll/challenge `{invitation,public_key}` -> finite one-use `{challenge_id,nonce,expires_at}`; POST /v1/enroll `{challenge_id,public_key,signature}` verifies Ed25519 proof for domain-separated challenge payload. Atomic registration slots; idempotent same key; no device private key is uploaded.
- GET/POST /v1/servers for server inventory `{name,host,ssh_port,username}` without credentials; `status=not_provisioned`, health/load unknown. Storing a host is not connecting or installing.
- GET /v1/overview -> actual counts/users/devices/servers, no fabricated throughput.

There is no API that reports real VPN connected until native data path exists. Development registered-device response must not be mistaken for a verified node-enforced lease; production signed manifests/leases are tracked separately.
