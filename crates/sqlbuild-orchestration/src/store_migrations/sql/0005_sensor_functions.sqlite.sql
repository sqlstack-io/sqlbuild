-- Sensors are decorated functions that choose their own jobs: a sensor trigger stores no job,
-- ticks record the event they were called with, and source events a sensor handled are kept.
-- Sensors published before this version stay hidden until their location publishes again,
-- and their earlier tick history and pending location work, in the old shape, are dropped.
CREATE TABLE sqb_triggers_v2 (
    trigger_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('schedule', 'sensor')),
    name TEXT NOT NULL,
    job_key TEXT,
    definition_json {json} NOT NULL,
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    removed_in_snapshot_id {uuid} REFERENCES sqb_snapshots(snapshot_id),
    UNIQUE (location_id, target, kind, name)
);

INSERT INTO sqb_triggers_v2 (trigger_id, location_id, target, kind, name, job_key, definition_json, snapshot_id, removed_in_snapshot_id) SELECT trigger_id, location_id, target, kind, name, job_key, definition_json, snapshot_id, removed_in_snapshot_id FROM sqb_triggers;

DROP TABLE sqb_triggers;

ALTER TABLE sqb_triggers_v2 RENAME TO sqb_triggers;

CREATE INDEX IF NOT EXISTS sqb_triggers_snapshot ON sqb_triggers(snapshot_id);

CREATE INDEX IF NOT EXISTS sqb_triggers_removed ON sqb_triggers(removed_in_snapshot_id) WHERE removed_in_snapshot_id IS NOT NULL;

UPDATE sqb_triggers SET job_key = NULL, removed_in_snapshot_id = COALESCE(removed_in_snapshot_id, snapshot_id), definition_json = json_set(json_remove(definition_json, '$.job_key', '$.when'), '$.function', name, '$.code_hash', 'unpublished', '$.timeout_seconds', 60) WHERE kind = 'sensor';

DELETE FROM sqb_location_work WHERE json_extract(request_json, '$.work.kind') = 'evaluate_sensor';

DROP TABLE sqb_sensor_evaluation_events;

CREATE TABLE sqb_sensor_evaluation_events (
    event_id {uuid} PRIMARY KEY,
    evaluation_id {uuid} NOT NULL,
    trigger_id {uuid} NOT NULL REFERENCES sqb_triggers(trigger_id),
    phase TEXT NOT NULL CHECK (phase IN ('requested', 'completed', 'failed', 'timed_out', 'abandoned')),
    occurred_at {time} NOT NULL,
    fact_json {json} NOT NULL,
    subject_event_id TEXT,
    UNIQUE (evaluation_id, phase)
);

CREATE INDEX sqb_sensor_evaluation_history ON sqb_sensor_evaluation_events(trigger_id, phase, occurred_at, evaluation_id);

CREATE INDEX sqb_sensor_evaluation_expiry ON sqb_sensor_evaluation_events(occurred_at);

CREATE INDEX sqb_sensor_evaluation_subject ON sqb_sensor_evaluation_events(trigger_id, subject_event_id) WHERE subject_event_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS sqb_sensor_deliveries (
    trigger_id {uuid} NOT NULL REFERENCES sqb_triggers(trigger_id),
    event_id TEXT NOT NULL,
    PRIMARY KEY (trigger_id, event_id)
);
