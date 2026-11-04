CREATE TABLE IF NOT EXISTS sqb_warehouse_consumption (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at {time} NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    execution_id TEXT NOT NULL,
    phase TEXT NOT NULL,
    run_id TEXT NOT NULL,
    node_name TEXT NOT NULL,
    target_database TEXT,
    target_schema TEXT NOT NULL,
    target_name TEXT NOT NULL,
    version_hash TEXT NOT NULL,
    input_count BIGINT NOT NULL,
    input_kind TEXT,
    input_name TEXT,
    input_version TEXT,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL
);
CREATE INDEX IF NOT EXISTS sqb_consumption_reconciliation ON sqb_warehouse_consumption (location_id,target,state_database,state_schema,observed_at,event_id);
CREATE INDEX IF NOT EXISTS sqb_consumption_relation ON sqb_warehouse_consumption (location_id,target_schema,target_name,observed_at);
