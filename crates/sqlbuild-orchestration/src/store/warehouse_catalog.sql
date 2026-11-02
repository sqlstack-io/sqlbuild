CREATE TABLE sqb_warehouse_catalog (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    observed_at {time} NOT NULL,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL
);
CREATE INDEX sqb_warehouse_catalog_namespace ON sqb_warehouse_catalog
    (location_id, target, state_schema, state_database, observed_at, event_id);
