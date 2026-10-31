CREATE TABLE sqb_location_instances (
    instance_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    connected_at {time} NOT NULL,
    last_seen_at {time} NOT NULL,
    health_json {json} NOT NULL
);
CREATE INDEX sqb_location_instances_location_seen ON sqb_location_instances(location_id, last_seen_at);
