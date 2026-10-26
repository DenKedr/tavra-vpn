CREATE TABLE app_settings (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
 policy JSONB NOT NULL,
 revision BIGINT NOT NULL DEFAULT 1 CHECK(revision > 0)
);
