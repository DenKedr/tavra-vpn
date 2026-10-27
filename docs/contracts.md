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

## Server and domain inventory CRUD (tranche 2)

All routes in this section require an administrator Bearer session. Saving inventory does not connect over SSH, install/uninstall software, verify DNS/TLS, or make any VPN protocol ready.

- `PATCH /v1/servers/{id}` accepts sparse `{name?,host?,ssh_port?,username?}` and returns the same server model as POST. Omitted fields remain unchanged; explicit null is invalid. Name is trimmed, nonempty, max120characters and has no control characters. Host is a normalized IP address or IDNA DNS hostname; URLs, credentials, ports, wildcards and whitespace are invalid. SSH port must be1..65535; username max64ASCIIletters/digits/underscore/hyphen and cannot start with a hyphen. Status remains `not_provisioned` in this tranche.
- `DELETE /v1/servers/{id}` removes only the inventory row and returns `{deleted:true,remote_uninstall_performed:false,unassigned_domain_ids:[UUID...]}`. Existing domain records survive: their server_id becomes null, verification_status remains unverified, and updated_at changes in the same transaction. No remote infrastructure is removed. A missing server returns404.
- `GET /v1/domains` returns an array sorted by canonical name/id, capped10000. Domain model: `{id,name,server_id,verification_status,created_at,updated_at}`; server_id is UUID or null, verification_status is always `unverified` until a real verification workflow is implemented.
- `POST /v1/domains {name,server_id?}` creates a record. Omitted/null server_id leaves it unassigned.
- `PATCH /v1/domains/{id} {name?,server_id?}` edits a record. Omitted server_id preserves assignment, explicit null removes assignment. Explicit null name is invalid. Editing always leaves verification_status unverified. Unknown input fields are rejected.
- `DELETE /v1/domains/{id}` removes the domain inventory row and returns `{deleted:true}`. It does not change DNS, certificates, or remote server configuration. Missing records return404.

Domain names normalize through IDNA/UTS46 to lowercase ASCII DNS labels. One final root dot is accepted and removed; at least two labels are required, each1..63bytes, max253bytes total. Only ASCIIletters/digits and internal hyphens are accepted after normalization. IPv4/IPv6 including alternate URL-style numeric IPv4 notation, wildcards, URLs, credentials, percent escapes, whitespace, empty labels, underscores and invalid IDNA are rejected with400. Canonical name uniqueness is global; equivalent Unicode/punycode/case/root-dot names conflict409. Assignment to a nonexistent server returns400 and writes no domain/audit change.

Mutations of servers/domains append an `inventory_audit` event in the same PostgreSQL transaction, containing owner actor, entity type/id, action, timestamp and typed before/after snapshots without credentials. Server deletion adds `server_unassigned` events for retained domains. Audit records deliberately have no FK to deletable inventory, so history survives deletion; there is no API for changing/deleting audit history in this tranche. Domain server_id uses a real FK with ON DELETE SET NULL. Inventory mutations serialize with a transaction-scoped PostgreSQL advisory lock, so deletion/reassignment and duplicate canonical names cannot leave dangling assignments or half-written audit events. List routes do not expose audit internals.
