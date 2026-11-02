CREATE TABLE sqb_warehouse_microbatches (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at {time} NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    model_name TEXT NOT NULL,
    target_name TEXT NOT NULL,
    scope_kind TEXT NOT NULL,
    scope_key TEXT NOT NULL,
    physical_generation_id TEXT NOT NULL,
    record_type TEXT NOT NULL,
    origin_run_id TEXT NOT NULL,
    execution_run_id TEXT NOT NULL,
    replay_requirement_id TEXT,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL
);
CREATE INDEX sqb_microbatches_reconciliation ON sqb_warehouse_microbatches (location_id,target,state_database,state_schema,observed_at,event_id);
