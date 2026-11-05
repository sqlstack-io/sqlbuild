-- Populated predecessor schemas for additive event-ID and read-only history-sync checks.
CREATE TABLE main._sqlbuild_fingerprints (
    node_type VARCHAR NOT NULL, node_name VARCHAR NOT NULL,
    target_database VARCHAR, target_schema VARCHAR, target_name VARCHAR,
    run_id VARCHAR NOT NULL, definition_hash VARCHAR NOT NULL, version_hash VARCHAR NOT NULL,
    schema_fingerprint VARCHAR NOT NULL, definition_b64 VARCHAR NOT NULL,
    metadata_json_b64 VARCHAR NOT NULL, ts TIMESTAMP NOT NULL
);
INSERT INTO main._sqlbuild_fingerprints VALUES (
    'model', 'orders', NULL, 'main', 'orders', 'legacy-run', 'legacy-definition',
    'legacy-version', 'legacy-schema', 'U0VMRUNUIDEgQVMgb3JkZXJfaWQ=', 'e30=', '2026-01-01 10:00:00'
);
CREATE TABLE main._sqlbuild_source_freshness (
    source_name VARCHAR NOT NULL, target_database VARCHAR, target_schema VARCHAR,
    target_name VARCHAR, run_id VARCHAR NOT NULL, strategy VARCHAR NOT NULL,
    value_kind VARCHAR NOT NULL, data_version VARCHAR, data_version_hash VARCHAR NOT NULL,
    observed_at TIMESTAMP NOT NULL
);
INSERT INTO main._sqlbuild_source_freshness VALUES (
    'raw.orders', NULL, 'main', 'orders', 'legacy-run', 'max_column', 'integer',
    '7', 'legacy-value', '2026-01-01 10:00:00'
);
