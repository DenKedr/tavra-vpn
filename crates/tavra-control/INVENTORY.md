# Inventory CRUD implementation report

Scope: server inventory update/delete and domain CRUD only; no live hosts, remote SSH/uninstall, DNS/TLS verification, Flutter/Core changes, commits or pushes.

## Implemented

- Admin PATCH/DELETE `/v1/servers/{id}`. Sparse PATCH validates every effective field and rejects explicit nulls. Host canonicalizes IP/IDNA; no URLs/credentials/ports/wildcards accepted. Delete is inventory-only and returns `remote_uninstall_performed:false` plus unassigned domain IDs.
- Admin GET/POST `/v1/domains`, PATCH/DELETE `/v1/domains/{id}`. Typed Domain response `{id,name,server_id,verification_status,created_at,updated_at}`. Optional/null assignment, explicit null PATCH unassigns, omitted preserves. IDNA canonical ASCII lowercase, root-dot removal, DNS label/length rules and unique canonical name. IP/numeric IPv4, URL, credentials, wildcard, whitespace, invalid labels/IDNA rejected. All saves remain `unverified`.
- Migration2026102704 adds domains FK to servers ON DELETE SET NULL and retained inventory_audit before/after events. No API mutates/deletes audit. History has no FK to deleted entity IDs. Inventory writes serialize using one transaction-scoped advisory lock; server delete atomically unassigns retained domains and appends their audit events. Failed validation/FK/uniqueness leaves no partial audit or domain edits.
- Exact public contract appended in docs/contracts.md, tranche2 section. Contract sent to UI and primary before implementation.

## Verification actually run

- TDD RED: missing domain endpoint returned404 instead of required authenticated401. Initial inventory workflow test then passed with implementation. Additional explicit-null PATCH regression returned200 instead of400; present-field deserializer fixed this and test passed.
- `cargo test -p tavra-control` GREEN: settings2unit tests plus api/clock/inventory/validation integrations on actual isolated tavra_test PostgreSQL. Inventory workflow covers server edit validation, IDNA create, duplicate409, invalid assignment400, invalid name cases, null unassign/reassign, inventory delete retaining domain, reconnect persistence, missing delete404, retained before/after audit order, simultaneous server deletion/reassignment and duplicate equivalent-domain create (one success/one409).
- `cargo clippy -p tavra-control --all-targets -- -D warnings` GREEN. Formatting applied.
- Native UI QA agent notified test DB released and latest target/debug/tavra-control ready. Its real-process/UI checks are independently owned and not claimed here.

No production node provisioning/domain DNS/TLS validation/readiness claim. Audit records are DB-stored for later audit UI/API; lists cap10000 without pagination in this tranche. Existing pre-migration server history is not invented retroactively.
