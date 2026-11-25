-- 'partial' is no longer a run outcome: a run that built some nodes and not others ended `failed`.
-- Stored runs, their terminal facts, run_failed events and sensor ticks say `failed` instead, and
-- the data step makes sensors watching `partial` runs watch `failed` ones.
CREATE TABLE sqb_runs_v2 (
    run_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    image_id TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('job', 'group', 'adhoc')),
    origin TEXT NOT NULL CHECK (origin IN ('manual', 'schedule', 'sensor', 'rerun', 'backfill')),
    job_key TEXT,
    parent_run_id {uuid} REFERENCES sqb_runs(run_id),
    root_run_id {uuid} NOT NULL,
    trigger_id {uuid} REFERENCES sqb_triggers(trigger_id),
    tick_id {uuid} REFERENCES sqb_ticks(tick_id),
    rerun_of_run_id {uuid} REFERENCES sqb_runs(run_id),
    requested_by TEXT NOT NULL,
    priority BIGINT NOT NULL DEFAULT 0,
    selection_json {json} NOT NULL,
    request_json {json} NOT NULL,
    authored_json {json},
    write_set_digest TEXT NOT NULL,
    rewrite_history BOOLEAN NOT NULL DEFAULT FALSE,
    status TEXT NOT NULL CHECK (status IN ('queued', 'launched', 'running', 'unresponsive', 'success', 'warn', 'failed', 'cancelled', 'skipped')),
    status_reason TEXT,
    queued_at {time} NOT NULL,
    launched_at {time},
    started_at {time},
    ended_at {time},
    cancel_requested_at {time},
    node_counts_json {json} NOT NULL,
    ignored_audit_failures BIGINT NOT NULL DEFAULT 0,
    last_event_storage_id BIGINT NOT NULL,
    worker_instance {uuid},
    container_id TEXT,
    claimed_at {time},
    last_heartbeat_at {time}
);

INSERT INTO sqb_runs_v2 (run_id, location_id, target, snapshot_id, image_id, kind, origin, job_key, parent_run_id, root_run_id, trigger_id, tick_id, rerun_of_run_id, requested_by, priority, selection_json, request_json, authored_json, write_set_digest, rewrite_history, status, status_reason, queued_at, launched_at, started_at, ended_at, cancel_requested_at, node_counts_json, ignored_audit_failures, last_event_storage_id, worker_instance, container_id, claimed_at, last_heartbeat_at) SELECT run_id, location_id, target, snapshot_id, image_id, kind, origin, job_key, parent_run_id, root_run_id, trigger_id, tick_id, rerun_of_run_id, requested_by, priority, selection_json, request_json, authored_json, write_set_digest, rewrite_history, CASE WHEN status = 'partial' THEN 'failed' ELSE status END, status_reason, queued_at, launched_at, started_at, ended_at, cancel_requested_at, node_counts_json, ignored_audit_failures, last_event_storage_id, worker_instance, container_id, claimed_at, last_heartbeat_at FROM sqb_runs;

DROP TABLE sqb_runs;

ALTER TABLE sqb_runs_v2 RENAME TO sqb_runs;

CREATE INDEX sqb_runs_location_time ON sqb_runs(location_id, target, queued_at);

CREATE INDEX sqb_runs_location_ended ON sqb_runs(location_id, target, ended_at);

CREATE INDEX sqb_runs_queue ON sqb_runs(priority DESC, queued_at) WHERE status = 'queued';

CREATE INDEX sqb_runs_active ON sqb_runs(location_id, target, job_key) WHERE status IN ('queued', 'launched', 'running', 'unresponsive');

CREATE INDEX sqb_runs_terminal_cursor ON sqb_runs(location_id, target, job_key, last_event_storage_id)
WHERE status IN ('success', 'warn', 'failed', 'cancelled', 'skipped');

CREATE INDEX sqb_runs_parent ON sqb_runs(parent_run_id) WHERE parent_run_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS sqb_runs_tick ON sqb_runs(tick_id);

CREATE INDEX IF NOT EXISTS sqb_runs_snapshot ON sqb_runs(snapshot_id);

UPDATE recall_events SET payload = json_set(payload, '$.change.change.outcome', 'failed')
WHERE event_type = 'run_terminated' AND json_extract(payload, '$.change.change.outcome') = 'partial';

UPDATE recall_events SET payload = json_set(payload, '$.platform.outcome', 'failed')
WHERE event_type = 'platform_run_failed' AND json_extract(payload, '$.platform.outcome') = 'partial';

UPDATE sqb_sensor_evaluation_events SET fact_json = json_set(fact_json, '$.request.event.status', 'failed')
WHERE json_extract(fact_json, '$.request.event.status') = 'partial';

UPDATE sqb_sensor_evaluation_events
SET fact_json = json_set(fact_json, '$.request.event.details.outcome', 'failed')
WHERE json_extract(fact_json, '$.request.event.details.outcome') = 'partial';
