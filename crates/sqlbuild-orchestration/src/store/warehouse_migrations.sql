CREATE TABLE sqb_warehouse_migrations (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at {time} NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    target_name TEXT,
    destination_model TEXT NOT NULL,
    decision TEXT NOT NULL,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL
);
CREATE INDEX sqb_migrations_reconciliation ON sqb_warehouse_migrations (location_id,target,state_database,state_schema,observed_at,event_id);
CREATE TABLE sqb_warehouse_old_name_views (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at {time} NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    target_name TEXT,
    migration_event_id TEXT NOT NULL,
    destination_model TEXT NOT NULL,
    event_type TEXT NOT NULL,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL
);
CREATE INDEX sqb_old_name_views_reconciliation ON sqb_warehouse_old_name_views (location_id,target,state_database,state_schema,observed_at,event_id);
