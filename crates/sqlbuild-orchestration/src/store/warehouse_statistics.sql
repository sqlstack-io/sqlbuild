CREATE TABLE sqb_warehouse_statistics (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    relation_name TEXT NOT NULL,
    observed_at {time} NOT NULL,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL
);
CREATE INDEX sqb_statistics_relation_time ON sqb_warehouse_statistics
    (location_id,target,state_schema,state_database,relation_name,observed_at,event_id);
CREATE INDEX sqb_statistics_expiration ON sqb_warehouse_statistics (observed_at,event_id);
CREATE INDEX sqb_catalog_statistics_scan ON sqb_warehouse_catalog
    (location_id,target,state_schema,state_database,event_id);
