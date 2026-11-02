CREATE TABLE sqb_warehouse_janitor_events (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at {time} NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    event_type TEXT NOT NULL,
    relation_database TEXT,
    relation_schema TEXT NOT NULL,
    archive_name TEXT NOT NULL,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL
);
CREATE INDEX sqb_janitor_events_reconciliation ON sqb_warehouse_janitor_events (location_id,target,state_database,state_schema,observed_at,event_id);
