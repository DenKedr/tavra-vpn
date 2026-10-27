CREATE TABLE domains (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE CHECK(length(name) BETWEEN 3 AND 253),
    server_id UUID REFERENCES servers(id) ON DELETE SET NULL,
    verification_status TEXT NOT NULL DEFAULT 'unverified' CHECK(verification_status='unverified'),
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE INDEX domains_server ON domains(server_id);
-- No FK to deletable inventory: before/after snapshots retain deleted entity history.
-- Application exposes no mutation or deletion route for these append-only records.
CREATE TABLE inventory_audit (
    sequence BIGSERIAL PRIMARY KEY,
    entity_type TEXT NOT NULL CHECK(entity_type IN ('server','domain')),
    entity_id UUID NOT NULL,
    action TEXT NOT NULL CHECK(action IN ('created','updated','deleted','server_unassigned')),
    before_state JSONB,
    after_state JSONB,
    actor TEXT NOT NULL DEFAULT 'owner' CHECK(actor='owner'),
    occurred_at BIGINT NOT NULL,
    CHECK((action='created' AND before_state IS NULL AND after_state IS NOT NULL)
       OR (action='deleted' AND before_state IS NOT NULL AND after_state IS NULL)
       OR (action IN ('updated','server_unassigned') AND before_state IS NOT NULL AND after_state IS NOT NULL)),
    CHECK(action!='server_unassigned' OR entity_type='domain')
);
CREATE INDEX inventory_audit_entity ON inventory_audit(entity_type,entity_id,sequence);
