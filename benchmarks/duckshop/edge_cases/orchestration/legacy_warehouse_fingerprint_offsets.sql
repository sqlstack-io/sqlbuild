-- SQLite predecessor text can carry the writer's offset, across the UTC date boundary.
-- Load after legacy_warehouse_state.sql, before any writer adds event_id.
INSERT INTO main._sqlbuild_fingerprints (
    node_type, node_name, target_database, target_schema, target_name, run_id,
    definition_hash, version_hash, schema_fingerprint, definition_b64, metadata_json_b64, ts
) VALUES (
    'model', 'offset_orders', NULL, 'main', 'offset_orders', 'offset-run',
    'legacy-definition', 'legacy-version', 'legacy-schema',
    'U0VMRUNUIDEgQVMgb3JkZXJfaWQ=', 'e30=', '2025-12-31T23:00:00-11:00'
);
