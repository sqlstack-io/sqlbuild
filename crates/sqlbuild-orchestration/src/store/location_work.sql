CREATE TABLE sqb_location_work (
    request_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    generation BIGINT NOT NULL,
    request_json {json} NOT NULL,
    requested_by TEXT NOT NULL,
    requested_at {time} NOT NULL,
    expires_at {time} NOT NULL,
    claim_id {uuid},
    session_id {uuid} REFERENCES sqb_location_instances(instance_id),
    claimed_until {time}
);
CREATE INDEX sqb_location_work_pending ON sqb_location_work(location_id, target, requested_at);
CREATE TABLE sqb_location_work_results (
    request_id {uuid} PRIMARY KEY REFERENCES sqb_location_work(request_id),
    result_json {json} NOT NULL,
    completed_at {time} NOT NULL
);
