-- Skipped runs, ignored audit failures, connection documents, pooled concurrency slots,
-- changed-build reports and plan diffs. Releases before this version never recorded an
-- ignored audit failure, so the new columns' defaults are exact for existing rows.
ALTER TABLE sqb_runs ADD COLUMN ignored_audit_failures BIGINT NOT NULL DEFAULT 0;

ALTER TABLE sqb_runs DROP CONSTRAINT sqb_runs_status_check;

ALTER TABLE sqb_runs ADD CONSTRAINT sqb_runs_status_check CHECK (status IN ('queued', 'launched', 'running', 'unresponsive', 'success', 'warn', 'partial', 'failed', 'cancelled', 'skipped'));

DROP INDEX sqb_runs_terminal_cursor;

CREATE INDEX sqb_runs_terminal_cursor ON sqb_runs(location_id, target, job_key, last_event_storage_id)
WHERE status IN ('success', 'warn', 'partial', 'failed', 'cancelled', 'skipped');

ALTER TABLE sqb_snapshot_documents DROP CONSTRAINT sqb_snapshot_documents_kind_check;

ALTER TABLE sqb_snapshot_documents ADD CONSTRAINT sqb_snapshot_documents_kind_check CHECK (kind IN ('jobs', 'groups', 'triggers', 'roles', 'lineage', 'columns', 'providers', 'connection'));

ALTER TABLE sqb_concurrency_slots DROP CONSTRAINT sqb_concurrency_slots_group_key_run_id_key;

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

CREATE OR REPLACE VIEW "audit_results" AS
SELECT
    invocation_id,
    invocation_started_at,
    event_id,
    occurred_at,
    run_id,
    resource_id,
    resource_attempt_id,
    (payload #>> '{audit_name}') AS audit_name,
    (payload #>> '{audit_definition_name}') AS audit_definition_name,
    CASE WHEN (payload #>> '{target_name}') IS NULL THEN NULL ELSE
        COALESCE((payload #>> '{target_database}') || '.', '')
        || COALESCE((payload #>> '{target_schema}') || '.', '')
        || (payload #>> '{target_name}')
    END AS target_relation,
    (payload #>> '{attached_target_kind}') AS attached_target_kind,
    (payload #>> '{attached_target_name}') AS attached_target_name,
    (payload #>> '{attached_column_name}') AS "column",
    outcome,
    (payload #>> '{severity}') AS severity,
    (payload #>> '{violation_count}')::BIGINT AS violation_count,
    (payload #>> '{measured_value}')::DOUBLE PRECISION AS measured_value,
    (payload #>> '{evaluation_mode}') AS evaluation_mode,
    (payload #>> '{execution_error}') AS execution_error,
    sql_digest,
    (payload #>> '{evidence_count}')::BIGINT AS evidence_count,
    (payload #>> '{evidence_truncated}')::BOOLEAN::INTEGER AS evidence_truncated,
    (payload #>> '{sample_digest}') AS sample_digest,
    COALESCE((payload #>> '{ignored}')::BOOLEAN::INTEGER, 0) AS ignored
FROM recall_events
WHERE event_type = 'audit_completed';
