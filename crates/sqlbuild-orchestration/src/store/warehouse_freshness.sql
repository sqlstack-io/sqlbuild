CREATE TABLE sqb_warehouse_freshness (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at {time} NOT NULL,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL
);
CREATE INDEX sqb_warehouse_freshness_scope ON sqb_warehouse_freshness (location_id, target, observed_at, event_id);
