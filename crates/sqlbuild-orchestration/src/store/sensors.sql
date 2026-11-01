CREATE TABLE sqb_sensor_evaluation_events (
    event_id {uuid} PRIMARY KEY,
    storage_id BIGINT NOT NULL UNIQUE,
    evaluation_id {uuid} NOT NULL,
    trigger_id {uuid} NOT NULL REFERENCES sqb_triggers(trigger_id),
    phase TEXT NOT NULL CHECK (phase IN ('requested', 'completed', 'failed', 'timed_out', 'abandoned')),
    occurred_at {time} NOT NULL,
    fact_json {json} NOT NULL,
    UNIQUE (evaluation_id, phase)
);
CREATE INDEX sqb_sensor_evaluation_history ON sqb_sensor_evaluation_events(trigger_id, storage_id);
CREATE TABLE sqb_sensor_cursor_commits (
    event_id {uuid} PRIMARY KEY,
    storage_id BIGINT NOT NULL UNIQUE,
    trigger_id {uuid} NOT NULL REFERENCES sqb_triggers(trigger_id),
    cursor_json {json} NOT NULL
);
CREATE INDEX sqb_sensor_cursor_history ON sqb_sensor_cursor_commits(trigger_id, storage_id);
CREATE TABLE sqb_run_requests (
    trigger_id {uuid} NOT NULL REFERENCES sqb_triggers(trigger_id),
    run_key TEXT NOT NULL,
    evaluation_id {uuid} NOT NULL,
    run_id {uuid} NOT NULL REFERENCES sqb_runs(run_id),
    PRIMARY KEY (trigger_id, run_key)
);
