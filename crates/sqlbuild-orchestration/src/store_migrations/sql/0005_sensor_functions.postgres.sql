-- Sensors are decorated functions that choose their own jobs: a sensor trigger stores no job,
-- ticks record the event they were called with, and source events a sensor handled are kept.
-- Sensors published before this version stay hidden until their location publishes again,
-- and their earlier tick history and pending location work, in the old shape, are dropped.
ALTER TABLE sqb_triggers ALTER COLUMN job_key DROP NOT NULL;

UPDATE sqb_triggers SET job_key = NULL, removed_in_snapshot_id = COALESCE(removed_in_snapshot_id, snapshot_id), definition_json = (definition_json - 'job_key' - 'when') || jsonb_build_object('function', name, 'code_hash', 'unpublished', 'timeout_seconds', 60) WHERE kind = 'sensor';

DELETE FROM sqb_location_work WHERE request_json -> 'work' ->> 'kind' = 'evaluate_sensor';

DELETE FROM sqb_sensor_evaluation_events;

ALTER TABLE sqb_sensor_evaluation_events ADD COLUMN IF NOT EXISTS subject_event_id TEXT;

CREATE INDEX IF NOT EXISTS sqb_sensor_evaluation_subject ON sqb_sensor_evaluation_events(trigger_id, subject_event_id) WHERE subject_event_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS sqb_sensor_deliveries (
    trigger_id {uuid} NOT NULL REFERENCES sqb_triggers(trigger_id),
    event_id TEXT NOT NULL,
    evaluation_id {uuid} NOT NULL,
    delivered_at {time} NOT NULL,
    PRIMARY KEY (trigger_id, event_id)
);
