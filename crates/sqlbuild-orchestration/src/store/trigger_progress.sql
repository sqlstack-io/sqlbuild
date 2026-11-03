CREATE TABLE sqb_trigger_progress (
    storage_id BIGINT PRIMARY KEY,
    trigger_id {uuid} NOT NULL,
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    schema_version BIGINT NOT NULL,
    projection_json {json} NOT NULL
);
CREATE INDEX sqb_trigger_progress_latest ON sqb_trigger_progress(trigger_id, storage_id);
