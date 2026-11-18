-- Skipped runs, ignored audit failures, connection documents, pooled concurrency slots,
-- changed-build reports and plan diffs. Releases before this version never recorded an
-- ignored audit failure, so the new columns' defaults are exact for existing rows.
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
    status TEXT NOT NULL CHECK (status IN ('queued', 'launched', 'running', 'unresponsive', 'success', 'warn', 'partial', 'failed', 'cancelled', 'skipped')),
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

INSERT INTO sqb_runs_v2 (run_id, location_id, target, snapshot_id, image_id, kind, origin, job_key, parent_run_id, root_run_id, trigger_id, tick_id, rerun_of_run_id, requested_by, priority, selection_json, request_json, authored_json, write_set_digest, rewrite_history, status, status_reason, queued_at, launched_at, started_at, ended_at, cancel_requested_at, node_counts_json, last_event_storage_id, worker_instance, container_id, claimed_at, last_heartbeat_at) SELECT run_id, location_id, target, snapshot_id, image_id, kind, origin, job_key, parent_run_id, root_run_id, trigger_id, tick_id, rerun_of_run_id, requested_by, priority, selection_json, request_json, authored_json, write_set_digest, rewrite_history, status, status_reason, queued_at, launched_at, started_at, ended_at, cancel_requested_at, node_counts_json, last_event_storage_id, worker_instance, container_id, claimed_at, last_heartbeat_at FROM sqb_runs;

DROP TABLE sqb_runs;

ALTER TABLE sqb_runs_v2 RENAME TO sqb_runs;

CREATE INDEX sqb_runs_location_time ON sqb_runs(location_id, target, queued_at);

CREATE INDEX sqb_runs_location_ended ON sqb_runs(location_id, target, ended_at);

CREATE INDEX sqb_runs_queue ON sqb_runs(priority DESC, queued_at) WHERE status = 'queued';

CREATE INDEX sqb_runs_active ON sqb_runs(location_id, target, job_key) WHERE status IN ('queued', 'launched', 'running', 'unresponsive');

CREATE INDEX sqb_runs_terminal_cursor ON sqb_runs(location_id, target, job_key, last_event_storage_id)
WHERE status IN ('success', 'warn', 'partial', 'failed', 'cancelled', 'skipped');

CREATE INDEX sqb_runs_parent ON sqb_runs(parent_run_id) WHERE parent_run_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS sqb_runs_tick ON sqb_runs(tick_id);

CREATE INDEX IF NOT EXISTS sqb_runs_snapshot ON sqb_runs(snapshot_id);

CREATE TABLE sqb_snapshot_documents_v2 (
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    kind TEXT NOT NULL CHECK (kind IN ('jobs', 'groups', 'triggers', 'roles', 'lineage', 'columns', 'providers', 'connection')),
    document_hash TEXT NOT NULL REFERENCES sqb_documents(document_hash),
    PRIMARY KEY (snapshot_id, kind)
);

INSERT INTO sqb_snapshot_documents_v2 (snapshot_id, kind, document_hash) SELECT snapshot_id, kind, document_hash FROM sqb_snapshot_documents;

DROP TABLE sqb_snapshot_documents;

ALTER TABLE sqb_snapshot_documents_v2 RENAME TO sqb_snapshot_documents;

CREATE INDEX IF NOT EXISTS sqb_snapshot_documents_hash ON sqb_snapshot_documents(document_hash);

CREATE TABLE sqb_concurrency_slots_v2 (
    group_key TEXT NOT NULL,
    slot BIGINT NOT NULL CHECK (slot > 0),
    run_id {uuid} NOT NULL REFERENCES sqb_runs(run_id),
    PRIMARY KEY (group_key, slot)
);

INSERT INTO sqb_concurrency_slots_v2 (group_key, slot, run_id) SELECT group_key, slot, run_id FROM sqb_concurrency_slots;

DROP TABLE sqb_concurrency_slots;

ALTER TABLE sqb_concurrency_slots_v2 RENAME TO sqb_concurrency_slots;

CREATE INDEX sqb_concurrency_slots_run ON sqb_concurrency_slots(run_id);

ALTER TABLE sqb_warehouse_audit_results ADD COLUMN ignored BOOLEAN NOT NULL DEFAULT FALSE;

CREATE TABLE sqb_changed_builds (
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    generation BIGINT NOT NULL,
    reported_at {time} NOT NULL,
    report_json {json} NOT NULL,
    PRIMARY KEY (location_id, target)
);

CREATE TABLE sqb_plan_diffs (
    request_id {uuid} NOT NULL REFERENCES sqb_location_work(request_id),
    model TEXT NOT NULL,
    diff_json {json} NOT NULL,
    PRIMARY KEY (request_id, model)
);

DROP VIEW "audit_results";

CREATE VIEW IF NOT EXISTS "audit_results" AS
SELECT
    invocation_id,
    invocation_started_at,
    event_id,
    occurred_at,
    run_id,
    resource_id,
    resource_attempt_id,
    json_extract(payload, '$.audit_name') AS audit_name,
    json_extract(payload, '$.audit_definition_name') AS audit_definition_name,
    CASE WHEN json_extract(payload, '$.target_name') IS NULL THEN NULL ELSE
        COALESCE(json_extract(payload, '$.target_database') || '.', '')
        || COALESCE(json_extract(payload, '$.target_schema') || '.', '')
        || json_extract(payload, '$.target_name')
    END AS target_relation,
    json_extract(payload, '$.attached_target_kind') AS attached_target_kind,
    json_extract(payload, '$.attached_target_name') AS attached_target_name,
    json_extract(payload, '$.attached_column_name') AS "column",
    outcome,
    json_extract(payload, '$.severity') AS severity,
    json_extract(payload, '$.violation_count') AS violation_count,
    json_extract(payload, '$.measured_value') AS measured_value,
    json_extract(payload, '$.evaluation_mode') AS evaluation_mode,
    json_extract(payload, '$.execution_error') AS execution_error,
    sql_digest,
    json_extract(payload, '$.evidence_count') AS evidence_count,
    json_extract(payload, '$.evidence_truncated') AS evidence_truncated,
    json_extract(payload, '$.sample_digest') AS sample_digest,
    COALESCE(json_extract(payload, '$.ignored'), 0) AS ignored
FROM recall_events
WHERE event_type = 'audit_completed';
