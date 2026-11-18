CREATE TABLE IF NOT EXISTS "recall_meta" (
    "component" TEXT NOT NULL,
    "key" TEXT NOT NULL,
    "value" TEXT NOT NULL,
    "recorded_at" TEXT NOT NULL,
    "invocation_id" TEXT NOT NULL,
    "invocation_started_at" TEXT NOT NULL,
    "schema_version" INTEGER NOT NULL,
    "producer_version" TEXT NOT NULL,
    "ingested_at" TEXT NOT NULL,
    "content_hash" TEXT NOT NULL,
    UNIQUE ("component", "key", "recorded_at")
);

CREATE TABLE IF NOT EXISTS "recall_invocations" (
    "command" TEXT,
    "subcommand" TEXT,
    "selection" JSON,
    "exclusion" JSON,
    "project_name" TEXT,
    "project_dir_hash" TEXT,
    "target" TEXT,
    "adapter" TEXT,
    "destination_database" TEXT,
    "destination_schema" TEXT,
    "destination_fingerprint" TEXT,
    "sqlbuild_version" TEXT,
    "git_revision" TEXT,
    "git_dirty" INTEGER,
    "host_token" TEXT,
    "pid" INTEGER,
    "external_context" JSON,
    "recall_filter_active" INTEGER,
    "argv_digest" TEXT,
    "origin" TEXT,
    "command_mode" TEXT,
    "selection_count" INTEGER,
    "selection_truncated" INTEGER,
    "selector_files" JSON,
    "invocation_id" TEXT NOT NULL,
    "invocation_started_at" TEXT NOT NULL,
    "schema_version" INTEGER NOT NULL,
    "producer_version" TEXT NOT NULL,
    "ingested_at" TEXT NOT NULL,
    "content_hash" TEXT NOT NULL,
    UNIQUE ("invocation_id")
);

CREATE INDEX IF NOT EXISTS "recall_invocations_started" ON "recall_invocations" ("invocation_started_at");

CREATE TABLE IF NOT EXISTS "recall_output" (
    "record_id" TEXT NOT NULL,
    "sequence" INTEGER NOT NULL,
    "stream" TEXT NOT NULL,
    "occurred_at" TEXT NOT NULL,
    "data" BLOB NOT NULL,
    "byte_offset" INTEGER NOT NULL,
    "byte_count" INTEGER NOT NULL,
    "priority" TEXT,
    "dropped_before" INTEGER NOT NULL,
    "omitted_bytes" INTEGER,
    "omitted_sequences" INTEGER,
    "invocation_id" TEXT NOT NULL,
    "invocation_started_at" TEXT NOT NULL,
    "schema_version" INTEGER NOT NULL,
    "producer_version" TEXT NOT NULL,
    "ingested_at" TEXT NOT NULL,
    "content_hash" TEXT NOT NULL,
    UNIQUE ("invocation_id", "sequence")
);

CREATE INDEX IF NOT EXISTS "recall_output_expiry" ON "recall_output" ("invocation_started_at");

CREATE TABLE IF NOT EXISTS "recall_events" (
    "event_id" TEXT NOT NULL,
    "invocation_sequence" INTEGER,
    "occurred_at" TEXT NOT NULL,
    "event_type" TEXT NOT NULL,
    "event_kind" TEXT NOT NULL,
    "severity" TEXT NOT NULL,
    "producer" TEXT NOT NULL,
    "event_schema_version" INTEGER NOT NULL,
    "run_id" TEXT,
    "resource_id" TEXT,
    "resource_attempt_id" TEXT,
    "operation_id" TEXT,
    "statement_id" TEXT,
    "command" TEXT,
    "resource_kind" TEXT,
    "resource_name" TEXT,
    "error_code" TEXT,
    "error_type" TEXT,
    "duration_ms" REAL,
    "exit_code" INTEGER,
    "query_id" TEXT,
    "sql_digest" TEXT,
    "intent" TEXT,
    "statement_kind" TEXT,
    "purpose" TEXT,
    "outcome" TEXT,
    "row_count" INTEGER,
    "affected_rows" INTEGER,
    "payload" JSON NOT NULL,
    "started_at" TEXT,
    "invocation_id" TEXT NOT NULL,
    "invocation_started_at" TEXT NOT NULL,
    "schema_version" INTEGER NOT NULL,
    "producer_version" TEXT NOT NULL,
    "ingested_at" TEXT NOT NULL,
    "content_hash" TEXT NOT NULL,
    storage_id INTEGER PRIMARY KEY AUTOINCREMENT,
    UNIQUE ("event_id")
);

CREATE INDEX IF NOT EXISTS "recall_events_invocation" ON "recall_events" ("invocation_id", "invocation_sequence");

CREATE INDEX IF NOT EXISTS "recall_events_type" ON "recall_events" ("event_type", "occurred_at");

CREATE INDEX IF NOT EXISTS "recall_events_statement" ON "recall_events" ("statement_id");

CREATE INDEX IF NOT EXISTS "recall_events_expiry" ON "recall_events" ("invocation_started_at");

CREATE TABLE IF NOT EXISTS "recall_diagnostics" (
    "record_id" TEXT NOT NULL,
    "sequence" INTEGER NOT NULL,
    "occurred_at" TEXT NOT NULL,
    "severity" TEXT NOT NULL,
    "logger" TEXT NOT NULL,
    "source" TEXT NOT NULL,
    "message" TEXT NOT NULL,
    "fields" JSON NOT NULL,
    "log_stream_id" TEXT,
    "run_id" TEXT,
    "resource_id" TEXT,
    "resource_attempt_id" TEXT,
    "operation_id" TEXT,
    "statement_id" TEXT,
    "invocation_id" TEXT NOT NULL,
    "invocation_started_at" TEXT NOT NULL,
    "schema_version" INTEGER NOT NULL,
    "producer_version" TEXT NOT NULL,
    "ingested_at" TEXT NOT NULL,
    "content_hash" TEXT NOT NULL,
    UNIQUE ("invocation_id", "sequence")
);

CREATE INDEX IF NOT EXISTS "recall_diagnostics_severity" ON "recall_diagnostics" ("severity");

CREATE INDEX IF NOT EXISTS "recall_diagnostics_expiry" ON "recall_diagnostics" ("invocation_started_at");

CREATE TABLE IF NOT EXISTS "recall_artifacts" (
    "digest" TEXT NOT NULL,
    "kind" TEXT NOT NULL,
    "media_type" TEXT NOT NULL,
    "size" INTEGER NOT NULL,
    "content" BLOB NOT NULL,
    "seen_at" TEXT NOT NULL,
    "invocation_id" TEXT NOT NULL,
    "invocation_started_at" TEXT NOT NULL,
    "schema_version" INTEGER NOT NULL,
    "producer_version" TEXT NOT NULL,
    "ingested_at" TEXT NOT NULL,
    "content_hash" TEXT NOT NULL,
    UNIQUE ("digest")
);

CREATE INDEX IF NOT EXISTS "recall_artifacts_seen" ON "recall_artifacts" ("seen_at");

CREATE TABLE IF NOT EXISTS "recall_artifact_refs" (
    "digest" TEXT NOT NULL,
    "ref_id" TEXT NOT NULL,
    "statement_id" TEXT,
    "resource_id" TEXT,
    "role" TEXT NOT NULL,
    "referenced_at" TEXT NOT NULL,
    "invocation_id" TEXT NOT NULL,
    "invocation_started_at" TEXT NOT NULL,
    "schema_version" INTEGER NOT NULL,
    "producer_version" TEXT NOT NULL,
    "ingested_at" TEXT NOT NULL,
    "content_hash" TEXT NOT NULL,
    UNIQUE ("invocation_id", "digest", "ref_id")
);

CREATE INDEX IF NOT EXISTS "recall_artifact_refs_digest" ON "recall_artifact_refs" ("digest");

CREATE TABLE IF NOT EXISTS "recall_heartbeats" (
    "beat_at" TEXT NOT NULL,
    "stale_at" TEXT NOT NULL,
    "host_token" TEXT,
    "pid" INTEGER,
    "command" TEXT,
    "subcommand" TEXT,
    "invocation_id" TEXT NOT NULL,
    "invocation_started_at" TEXT NOT NULL,
    "schema_version" INTEGER NOT NULL,
    "producer_version" TEXT NOT NULL,
    "ingested_at" TEXT NOT NULL,
    "content_hash" TEXT NOT NULL,
    UNIQUE ("invocation_id", "beat_at")
);

CREATE TABLE IF NOT EXISTS "recall_delivery" (
    "stream" TEXT NOT NULL,
    "accepted" INTEGER NOT NULL,
    "delivered" INTEGER NOT NULL,
    "filtered" INTEGER NOT NULL,
    "dropped" INTEGER NOT NULL,
    "failed" INTEGER NOT NULL,
    "conflicts" INTEGER NOT NULL,
    "flush_complete" INTEGER NOT NULL,
    "written_at" TEXT NOT NULL,
    "invocation_id" TEXT NOT NULL,
    "invocation_started_at" TEXT NOT NULL,
    "schema_version" INTEGER NOT NULL,
    "producer_version" TEXT NOT NULL,
    "ingested_at" TEXT NOT NULL,
    "content_hash" TEXT NOT NULL,
    UNIQUE ("invocation_id", "stream")
);

CREATE TABLE IF NOT EXISTS "recall_conflicts" (
    "dataset" TEXT NOT NULL,
    "record_key" TEXT NOT NULL,
    "existing_sha256" TEXT NOT NULL,
    "rejected_sha256" TEXT NOT NULL,
    "detected_at" TEXT NOT NULL,
    "invocation_id" TEXT NOT NULL,
    "invocation_started_at" TEXT NOT NULL,
    "schema_version" INTEGER NOT NULL,
    "producer_version" TEXT NOT NULL,
    "ingested_at" TEXT NOT NULL,
    "content_hash" TEXT NOT NULL,
    UNIQUE ("dataset", "record_key", "rejected_sha256")
);

CREATE INDEX IF NOT EXISTS "recall_conflicts_invocation" ON "recall_conflicts" ("invocation_id");

CREATE VIEW IF NOT EXISTS "invocations" AS
WITH ids AS (
    SELECT invocation_id, MIN(invocation_started_at) AS invocation_started_at
    FROM (
        SELECT invocation_id, invocation_started_at FROM recall_invocations
        UNION ALL SELECT invocation_id, invocation_started_at FROM recall_heartbeats
        UNION ALL SELECT invocation_id, invocation_started_at FROM recall_events
            WHERE event_type IN ('invocation_started', 'invocation_completed', 'invocation_failed')
    ) AS source_rows
    GROUP BY invocation_id
),
started AS (
    SELECT invocation_id, MIN(occurred_at) AS started_at, MIN(command) AS command
    FROM recall_events
    WHERE event_type = 'invocation_started'
    GROUP BY invocation_id
),
terminals AS (
    SELECT invocation_id, event_type, occurred_at, exit_code, duration_ms, error_type, error_code,
        json_extract(payload, '$.error_message') AS error_message,
        json_extract(payload, '$.signal_number') AS signal_number,
        json_extract(payload, '$.cost_status') AS cost_status,
        ROW_NUMBER() OVER (
            PARTITION BY invocation_id
            ORDER BY invocation_sequence DESC, occurred_at DESC, event_id DESC
        ) AS position
    FROM recall_events
    WHERE event_type IN ('invocation_completed', 'invocation_failed')
),
beats AS (
    SELECT invocation_id, MAX(beat_at) AS last_heartbeat_at, MAX(stale_at) AS stale_at,
        MIN(command) AS command, MIN(subcommand) AS subcommand
    FROM recall_heartbeats
    GROUP BY invocation_id
),
projected AS (
    SELECT
        ids.invocation_id,
        ids.invocation_started_at,
        COALESCE(started.started_at, ids.invocation_started_at) AS started_at,
        CASE
            WHEN terminals.event_type = 'invocation_completed' THEN 'completed'
            WHEN terminals.event_type = 'invocation_failed'
                AND terminals.signal_number IS NOT NULL THEN 'cancelled'
            WHEN terminals.event_type = 'invocation_failed' THEN 'failed'
            WHEN beats.stale_at IS NULL THEN 'unknown'
            WHEN beats.stale_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') THEN 'running'
            ELSE 'abandoned'
        END AS status,
        terminals.event_type AS terminal_event,
        terminals.occurred_at AS terminal_at,
        terminals.duration_ms AS terminal_duration_ms,
        terminals.exit_code,
        terminals.error_type,
        terminals.error_code,
        terminals.error_message,
        terminals.signal_number,
        terminals.cost_status,
        COALESCE(context.command, started.command, beats.command) AS command,
        COALESCE(context.subcommand, beats.subcommand) AS subcommand,
        context.origin,
        context.command_mode,
        context.target,
        context.adapter,
        context.project_name,
        context.selection,
        COALESCE(context.selection_count, json_array_length(context.selection))
            AS selection_count,
        context.selection_truncated,
        context.selector_files,
        context.exclusion,
        context.sqlbuild_version,
        context.git_revision,
        context.git_dirty,
        context.host_token,
        context.pid,
        context.external_context,
        CASE WHEN json_valid(context.external_context)
            THEN json_extract(context.external_context, '$.integration.name') END
            AS integration,
        CASE WHEN json_valid(context.external_context)
            THEN json_extract(context.external_context, '$.integration.job_name') END
            AS integration_job,
        CASE WHEN json_valid(context.external_context)
            THEN json_extract(context.external_context, '$.integration.run_id') END
            AS integration_run_id,
        CASE WHEN json_valid(context.external_context)
            THEN json_extract(context.external_context, '$.integration.retry_number') END
            AS integration_retry,
        context.recall_filter_active,
        beats.last_heartbeat_at
    FROM ids
    LEFT JOIN recall_invocations AS context ON context.invocation_id = ids.invocation_id
    LEFT JOIN started ON started.invocation_id = ids.invocation_id
    LEFT JOIN terminals
        ON terminals.invocation_id = ids.invocation_id AND terminals.position = 1
    LEFT JOIN beats ON beats.invocation_id = ids.invocation_id
)
SELECT
    invocation_id,
    invocation_started_at,
    started_at,
    status,
    CASE
        WHEN terminal_event IS NOT NULL THEN terminal_at
        WHEN status = 'abandoned' THEN last_heartbeat_at
    END AS ended_at,
    CASE
        WHEN terminal_event IS NOT NULL THEN terminal_duration_ms
        WHEN status = 'abandoned' THEN ROUND((unixepoch(last_heartbeat_at, 'subsec') - unixepoch(started_at, 'subsec')) * 1000.0, 3)
    END AS duration_ms,
    CASE
        WHEN terminal_event IS NOT NULL THEN 0
        WHEN status = 'abandoned' THEN 1
    END AS ended_at_estimated,
    exit_code,
    error_type,
    error_code,
    error_message,
    signal_number,
    cost_status,
    command,
    subcommand,
    origin,
    command_mode,
    target,
    adapter,
    project_name,
    selection,
    selection_count,
    selection_truncated,
    selector_files,
    exclusion,
    sqlbuild_version,
    git_revision,
    git_dirty,
    host_token,
    pid,
    external_context,
    integration,
    integration_job,
    integration_run_id,
    integration_retry,
    recall_filter_active,
    last_heartbeat_at
FROM projected;

CREATE VIEW IF NOT EXISTS "statements" AS
WITH facts AS (
    SELECT *
    FROM recall_events
    WHERE statement_id IS NOT NULL AND event_type LIKE 'statement!_%' ESCAPE '!'
),
firsts AS (
    SELECT invocation_id, invocation_started_at, statement_id, run_id, resource_id, resource_attempt_id, operation_id,
        intent, statement_kind, purpose, sql_digest,
        ROW_NUMBER() OVER (
            PARTITION BY statement_id ORDER BY invocation_sequence, occurred_at, event_id
        ) AS position
    FROM facts
),
started AS (
    SELECT statement_id, MIN(started_at) AS started_at
    FROM (
        SELECT statement_id, occurred_at AS started_at FROM facts WHERE event_type = 'statement_started'
        UNION ALL
        SELECT statement_id, started_at FROM facts WHERE started_at IS NOT NULL
    ) AS starts
    GROUP BY statement_id
),
submitted AS (
    SELECT statement_id, MIN(occurred_at) AS submitted_at
    FROM facts WHERE event_type = 'statement_submitted' GROUP BY statement_id
),
query_ids AS (
    SELECT statement_id, MAX(query_id) AS query_id FROM facts GROUP BY statement_id
),
terminals AS (
    SELECT statement_id, event_type, occurred_at, query_id, error_type, error_code, duration_ms,
        row_count, affected_rows, outcome,
        ROW_NUMBER() OVER (
            PARTITION BY statement_id
            ORDER BY invocation_sequence DESC, occurred_at DESC, event_id DESC
        ) AS position
    FROM facts
    WHERE event_type IN ('statement_completed', 'statement_failed')
),
captures AS (
    SELECT firsts.statement_id, (
        SELECT MAX(artifact.digest IS NOT NULL)
        FROM recall_artifact_refs AS ref
        LEFT JOIN recall_artifacts AS artifact ON artifact.digest = ref.digest
        WHERE ref.invocation_id = firsts.invocation_id
            AND ref.digest = 'sha256:' || firsts.sql_digest
    ) AS stored
    FROM firsts
    WHERE firsts.position = 1
),
resources AS (
    SELECT invocation_id, resource_id, MIN(resource_name) AS resource_name,
        MIN(resource_kind) AS resource_kind
    FROM recall_events
    WHERE resource_id IS NOT NULL AND resource_name IS NOT NULL
    GROUP BY invocation_id, resource_id
)
SELECT
    firsts.invocation_id,
    firsts.invocation_started_at,
    firsts.statement_id,
    firsts.run_id,
    firsts.resource_id,
    resources.resource_name,
    resources.resource_kind,
    firsts.resource_attempt_id,
    firsts.operation_id,
    firsts.intent,
    firsts.statement_kind,
    firsts.purpose,
    COALESCE(terminals.query_id, query_ids.query_id) AS query_id,
    started.started_at,
    submitted.submitted_at,
    terminals.occurred_at AS ended_at,
    CASE terminals.event_type
        WHEN 'statement_completed' THEN 'completed'
        WHEN 'statement_failed' THEN
            CASE WHEN terminals.outcome = 'handled' THEN 'handled' ELSE 'failed' END
        ELSE 'no_terminal'
    END AS outcome,
    terminals.error_type,
    terminals.error_code,
    terminals.duration_ms,
    terminals.row_count,
    terminals.affected_rows,
    firsts.sql_digest,
    COALESCE(captures.stored, 0) AS sql_available,
    CASE captures.stored
        WHEN 1 THEN NULL
        WHEN 0 THEN 'expired_or_not_captured'
        ELSE 'not_captured'
    END AS sql_unavailable_reason
FROM firsts
LEFT JOIN started ON started.statement_id = firsts.statement_id
LEFT JOIN submitted ON submitted.statement_id = firsts.statement_id
LEFT JOIN query_ids ON query_ids.statement_id = firsts.statement_id
LEFT JOIN terminals
    ON terminals.statement_id = firsts.statement_id AND terminals.position = 1
LEFT JOIN captures ON captures.statement_id = firsts.statement_id
LEFT JOIN resources
    ON resources.invocation_id = firsts.invocation_id
    AND resources.resource_id = firsts.resource_id
WHERE firsts.position = 1;

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
    json_extract(payload, '$.sample_digest') AS sample_digest
FROM recall_events
WHERE event_type = 'audit_completed';

CREATE VIEW IF NOT EXISTS "output_invocations" AS
WITH received AS (
    SELECT invocation_id,
        MIN(invocation_started_at) AS invocation_started_at,
        COUNT(*) AS chunks,
        SUM(byte_count) AS bytes,
        SUM(CASE WHEN stream = 'stdout' THEN byte_count ELSE 0 END) AS stdout_bytes,
        SUM(CASE WHEN stream = 'stderr' THEN byte_count ELSE 0 END) AS stderr_bytes,
        MAX(sequence) AS max_sequence,
        SUM(dropped_before) AS queue_drops,
        SUM(COALESCE(omitted_sequences, 1)) AS spans,
        SUM(COALESCE(omitted_bytes, 0)) AS truncated_bytes
    FROM recall_output
    GROUP BY invocation_id
),
delivery AS (
    SELECT * FROM recall_delivery WHERE stream = 'output'
),
ids AS (
    SELECT invocation_id, invocation_started_at FROM received
    UNION SELECT invocation_id, invocation_started_at FROM delivery
),
counted AS (
    SELECT
        ids.invocation_id,
        ids.invocation_started_at,
        COALESCE(received.chunks, 0) AS chunks,
        COALESCE(received.bytes, 0) AS bytes,
        COALESCE(received.stdout_bytes, 0) AS stdout_bytes,
        COALESCE(received.stderr_bytes, 0) AS stderr_bytes,
        COALESCE(received.truncated_bytes, 0) AS truncated_bytes,
        received.max_sequence,
        MAX(
            COALESCE(received.max_sequence + 1 - received.spans, 0),
            COALESCE(delivery.accepted, 0) - COALESCE(received.chunks, 0)
        ) AS missing_sequences,
        MAX(COALESCE(received.queue_drops, 0), COALESCE(delivery.dropped, 0)) AS reported_drops,
        COALESCE(delivery.filtered, 0) AS filtered,
        COALESCE(delivery.failed, 0) AS failed,
        COALESCE(context.recall_filter_active, 0) AS filter_active,
        COALESCE(delivery.flush_complete, 0) AS flush_complete,
        delivery.invocation_id IS NOT NULL AS delivery_reported,
        COALESCE(delivery.delivered, 0) AS delivered
    FROM ids
    LEFT JOIN received ON received.invocation_id = ids.invocation_id
    LEFT JOIN delivery ON delivery.invocation_id = ids.invocation_id
    LEFT JOIN recall_invocations AS context ON context.invocation_id = ids.invocation_id
)
SELECT
    invocation_id,
    invocation_started_at,
    chunks,
    bytes,
    stdout_bytes,
    stderr_bytes,
    truncated_bytes,
    max_sequence,
    missing_sequences,
    reported_drops,
    filtered,
    failed,
    filter_active,
    flush_complete,
    CASE
        WHEN missing_sequences = 0 AND truncated_bytes > 0 THEN 'truncated'
        WHEN missing_sequences = 0 THEN NULL
        WHEN filtered >= missing_sequences THEN 'filtered'
        WHEN filtered + reported_drops + failed >= missing_sequences THEN 'dropped'
        ELSE 'unexplained'
    END AS gap_reason,
    CASE
        WHEN delivery_reported AND flush_complete = 1 AND missing_sequences = 0
            AND reported_drops = 0 AND filtered = 0 AND failed = 0 AND truncated_bytes = 0
            AND chunks >= delivered
        THEN 1 ELSE 0
    END AS complete
FROM counted;

CREATE VIEW IF NOT EXISTS "output_tail" AS
SELECT invocation_id, invocation_started_at, sequence, stream, occurred_at, byte_count,
    CAST(data AS TEXT) AS text
FROM (
    SELECT *, ROW_NUMBER() OVER (PARTITION BY invocation_id ORDER BY sequence DESC) AS position
    FROM recall_output
) AS source_rows
WHERE position <= 200;

CREATE VIEW IF NOT EXISTS "output_transcripts" AS
SELECT invocation_id, MIN(invocation_started_at) AS invocation_started_at,
    CAST(COALESCE(group_concat(CASE WHEN stream = 'stdout' THEN data END, x'' ORDER BY sequence), x'') AS BLOB) AS stdout,
    CAST(COALESCE(group_concat(CASE WHEN stream = 'stderr' THEN data END, x'' ORDER BY sequence), x'') AS BLOB) AS stderr,
    CAST(COALESCE(group_concat(data, x'' ORDER BY sequence), x'') AS BLOB) AS combined
FROM recall_output GROUP BY invocation_id;

CREATE VIEW IF NOT EXISTS "events" AS
SELECT
    event_id,
    invocation_sequence,
    occurred_at,
    event_type,
    event_kind,
    severity,
    producer,
    event_schema_version,
    run_id,
    resource_id,
    resource_attempt_id,
    operation_id,
    statement_id,
    command,
    resource_kind,
    resource_name,
    error_code,
    error_type,
    duration_ms,
    exit_code,
    query_id,
    sql_digest,
    intent,
    statement_kind,
    purpose,
    outcome,
    row_count,
    affected_rows,
    CASE WHEN producer = 'sqlbuild' THEN json_patch(payload, json_remove(json_object('command', command, 'resource_kind', resource_kind, 'resource_name', resource_name, 'error_code', error_code, 'error_type', error_type, 'query_id', query_id, 'sql_digest', sql_digest, 'intent', intent, 'statement_kind', statement_kind, 'purpose', purpose, 'outcome', outcome, 'exit_code', exit_code, 'row_count', row_count, 'affected_rows', affected_rows), CASE WHEN command IS NULL THEN '$.command' ELSE '$.payload' END, CASE WHEN resource_kind IS NULL THEN '$.resource_kind' ELSE '$.payload' END, CASE WHEN resource_name IS NULL THEN '$.resource_name' ELSE '$.payload' END, CASE WHEN error_code IS NULL THEN '$.error_code' ELSE '$.payload' END, CASE WHEN error_type IS NULL THEN '$.error_type' ELSE '$.payload' END, CASE WHEN query_id IS NULL THEN '$.query_id' ELSE '$.payload' END, CASE WHEN sql_digest IS NULL THEN '$.sql_digest' ELSE '$.payload' END, CASE WHEN intent IS NULL THEN '$.intent' ELSE '$.payload' END, CASE WHEN statement_kind IS NULL THEN '$.statement_kind' ELSE '$.payload' END, CASE WHEN purpose IS NULL THEN '$.purpose' ELSE '$.payload' END, CASE WHEN outcome IS NULL THEN '$.outcome' ELSE '$.payload' END, CASE WHEN exit_code IS NULL THEN '$.exit_code' ELSE '$.payload' END, CASE WHEN row_count IS NULL THEN '$.row_count' ELSE '$.payload' END, CASE WHEN affected_rows IS NULL THEN '$.affected_rows' ELSE '$.payload' END)) ELSE payload END AS payload,
    started_at,
    invocation_id,
    invocation_started_at,
    schema_version,
    producer_version,
    ingested_at,
    content_hash,
    storage_id
FROM recall_events;

CREATE VIEW IF NOT EXISTS "diagnostics" AS
SELECT * FROM recall_diagnostics;

CREATE VIEW IF NOT EXISTS "artifacts" AS
SELECT
    refs.invocation_id,
    refs.invocation_started_at,
    refs.digest,
    refs.ref_id,
    refs.role,
    refs.statement_id,
    refs.resource_id,
    artifact.kind,
    artifact.media_type,
    artifact.size,
    artifact.digest IS NOT NULL AS available,
    CASE WHEN artifact.digest IS NULL THEN 'expired_or_not_captured' END AS unavailable_reason,
    CASE WHEN artifact.media_type LIKE 'text/%' OR artifact.media_type = 'application/json'
        THEN CAST(artifact.content AS TEXT) END AS text,
    artifact.content
FROM recall_artifact_refs AS refs
LEFT JOIN recall_artifacts AS artifact ON artifact.digest = refs.digest;

CREATE VIEW IF NOT EXISTS "conflicts" AS
SELECT * FROM recall_conflicts;

CREATE TABLE sqb_installation (
    singleton BIGINT PRIMARY KEY CHECK (singleton = 1),
    installation_id TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE sqb_locations (
    location_id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    archived_at TEXT
);

CREATE TABLE sqb_source_texts (
    text_hash TEXT PRIMARY KEY,
    body BLOB NOT NULL
);

CREATE TABLE sqb_node_definitions (
    node_hash TEXT PRIMARY KEY,
    body BLOB NOT NULL
);

CREATE TABLE sqb_documents (
    document_hash TEXT PRIMARY KEY,
    body BLOB NOT NULL
);

CREATE TABLE sqb_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    generation BIGINT NOT NULL CHECK (generation > 0),
    git_revision TEXT,
    git_dirty BOOLEAN NOT NULL,
    git_author TEXT,
    image_id TEXT,
    environment_hash TEXT NOT NULL,
    sqb_version TEXT NOT NULL,
    manifest_digest TEXT NOT NULL,
    published_at TEXT NOT NULL,
    UNIQUE (location_id, target, generation),
    UNIQUE (location_id, target, snapshot_id)
);

CREATE INDEX sqb_snapshots_location_time ON sqb_snapshots(location_id, target, published_at);

CREATE TABLE sqb_snapshot_nodes (
    snapshot_id TEXT NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    node_key TEXT NOT NULL,
    node_kind TEXT NOT NULL,
    node_hash TEXT NOT NULL REFERENCES sqb_node_definitions(node_hash),
    PRIMARY KEY (snapshot_id, node_key)
);

CREATE TABLE sqb_snapshot_sources (
    snapshot_id TEXT NOT NULL,
    node_key TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('raw_sql', 'compiled_sql', 'python_source')),
    text_hash TEXT NOT NULL REFERENCES sqb_source_texts(text_hash),
    PRIMARY KEY (snapshot_id, node_key, kind),
    FOREIGN KEY (snapshot_id, node_key) REFERENCES sqb_snapshot_nodes(snapshot_id, node_key)
);

CREATE TABLE sqb_snapshot_documents (
    snapshot_id TEXT NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    kind TEXT NOT NULL CHECK (kind IN ('jobs', 'groups', 'triggers', 'roles', 'lineage', 'columns', 'providers')),
    document_hash TEXT NOT NULL REFERENCES sqb_documents(document_hash),
    PRIMARY KEY (snapshot_id, kind)
);

CREATE TABLE sqb_current_snapshots (
    location_id TEXT NOT NULL,
    target TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    PRIMARY KEY (location_id, target),
    FOREIGN KEY (location_id, target, snapshot_id)
        REFERENCES sqb_snapshots(location_id, target, snapshot_id)
);

CREATE TABLE sqb_snapshot_changes (
    snapshot_id TEXT NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    change_seq BIGINT NOT NULL,
    change_kind TEXT NOT NULL CHECK (change_kind IN ('added', 'changed', 'removed')),
    subject_key TEXT NOT NULL,
    before_hash TEXT,
    after_hash TEXT,
    PRIMARY KEY (snapshot_id, change_seq)
);

CREATE TABLE sqb_scheduler_lease (
    name TEXT PRIMARY KEY CHECK (name = 'scheduler'),
    holder TEXT NOT NULL,
    epoch BIGINT NOT NULL CHECK (epoch > 0),
    expires_at TEXT NOT NULL
);

CREATE TABLE sqb_jobs (
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    job_key TEXT NOT NULL,
    snapshot_id TEXT NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    kind TEXT NOT NULL CHECK (kind IN ('job', 'group')),
    definition_json JSON NOT NULL,
    PRIMARY KEY (location_id, target, job_key)
);

CREATE TABLE sqb_triggers (
    trigger_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('schedule', 'sensor')),
    name TEXT NOT NULL,
    job_key TEXT NOT NULL,
    definition_json JSON NOT NULL,
    snapshot_id TEXT NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    removed_in_snapshot_id TEXT REFERENCES sqb_snapshots(snapshot_id),
    UNIQUE (location_id, target, kind, name)
);

CREATE TABLE sqb_trigger_state (
    trigger_id TEXT PRIMARY KEY REFERENCES sqb_triggers(trigger_id),
    status TEXT NOT NULL CHECK (status IN ('running', 'paused')),
    status_override TEXT CHECK (status_override IN ('running', 'paused')),
    started_at TEXT NOT NULL,
    cursor_json JSON,
    last_tick_for TEXT,
    next_due_at TEXT
);

CREATE INDEX sqb_trigger_state_due ON sqb_trigger_state(next_due_at) WHERE status = 'running';

CREATE TABLE sqb_ticks (
    tick_id TEXT PRIMARY KEY,
    trigger_id TEXT NOT NULL REFERENCES sqb_triggers(trigger_id),
    scheduled_for TEXT NOT NULL,
    evaluated_at TEXT NOT NULL,
    epoch BIGINT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('requested', 'covered', 'skipped')),
    run_id TEXT,
    covered_by_run_id TEXT,
    active_run_id TEXT,
    UNIQUE (trigger_id, scheduled_for)
);

CREATE INDEX sqb_ticks_trigger_time ON sqb_ticks(trigger_id, scheduled_for);

CREATE TABLE sqb_runs (
    run_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    snapshot_id TEXT NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    image_id TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('job', 'group', 'adhoc')),
    origin TEXT NOT NULL CHECK (origin IN ('manual', 'schedule', 'sensor', 'rerun', 'backfill')),
    job_key TEXT,
    parent_run_id TEXT REFERENCES sqb_runs(run_id),
    root_run_id TEXT NOT NULL,
    trigger_id TEXT REFERENCES sqb_triggers(trigger_id),
    tick_id TEXT REFERENCES sqb_ticks(tick_id),
    rerun_of_run_id TEXT REFERENCES sqb_runs(run_id),
    requested_by TEXT NOT NULL,
    priority BIGINT NOT NULL DEFAULT 0,
    selection_json JSON NOT NULL,
    request_json JSON NOT NULL,
    authored_json JSON,
    write_set_digest TEXT NOT NULL,
    rewrite_history BOOLEAN NOT NULL DEFAULT FALSE,
    status TEXT NOT NULL CHECK (status IN ('queued', 'launched', 'running', 'unresponsive', 'success', 'warn', 'partial', 'failed', 'cancelled')),
    status_reason TEXT,
    queued_at TEXT NOT NULL,
    launched_at TEXT,
    started_at TEXT,
    ended_at TEXT,
    cancel_requested_at TEXT,
    node_counts_json JSON NOT NULL,
    last_event_storage_id BIGINT NOT NULL,
    worker_instance TEXT,
    container_id TEXT,
    claimed_at TEXT,
    last_heartbeat_at TEXT
);

CREATE INDEX sqb_runs_location_time ON sqb_runs(location_id, target, queued_at);

CREATE INDEX sqb_runs_location_ended ON sqb_runs(location_id, target, ended_at);

CREATE INDEX sqb_runs_queue ON sqb_runs(priority DESC, queued_at) WHERE status = 'queued';

CREATE INDEX sqb_runs_active ON sqb_runs(location_id, target, job_key) WHERE status IN ('queued', 'launched', 'running', 'unresponsive');

CREATE TABLE sqb_run_nodes (
    run_id TEXT NOT NULL REFERENCES sqb_runs(run_id),
    node_key TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'success', 'warn', 'failed', 'skipped', 'cancelled')),
    writes BOOLEAN NOT NULL,
    started_at TEXT,
    ended_at TEXT,
    rows_affected BIGINT,
    error_code TEXT,
    skipped_because_of TEXT,
    kind TEXT NOT NULL DEFAULT 'node' CHECK (kind IN ('node', 'check')),
    finished_storage_id BIGINT,
    PRIMARY KEY (run_id, node_key)
);

CREATE INDEX sqb_run_nodes_node ON sqb_run_nodes(node_key, run_id);

CREATE INDEX sqb_run_nodes_finished ON sqb_run_nodes(finished_storage_id)
WHERE finished_storage_id IS NOT NULL;

CREATE INDEX sqb_run_nodes_decisive ON sqb_run_nodes(node_key, finished_storage_id)
WHERE finished_storage_id IS NOT NULL AND status IN ('success', 'failed');

CREATE TABLE sqb_write_claims (
    target TEXT NOT NULL,
    node_key TEXT NOT NULL,
    run_id TEXT NOT NULL REFERENCES sqb_runs(run_id),
    PRIMARY KEY (target, node_key)
);

CREATE INDEX sqb_write_claims_run ON sqb_write_claims(run_id);

CREATE TABLE sqb_concurrency_slots (
    group_key TEXT NOT NULL,
    slot BIGINT NOT NULL CHECK (slot > 0),
    run_id TEXT NOT NULL REFERENCES sqb_runs(run_id),
    PRIMARY KEY (group_key, slot),
    UNIQUE (group_key, run_id)
);

CREATE INDEX sqb_concurrency_slots_run ON sqb_concurrency_slots(run_id);

CREATE TABLE sqb_principals (
    principal_id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    disabled_at TEXT
);

CREATE TABLE sqb_access_tokens (
    token_id TEXT PRIMARY KEY,
    secret_hash TEXT NOT NULL,
    location_id TEXT REFERENCES sqb_locations(location_id),
    run_id TEXT REFERENCES sqb_runs(run_id),
    scope_json JSON NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    revoked_at TEXT,
    CHECK (
        (location_id IS NOT NULL AND run_id IS NULL)
        OR (location_id IS NULL AND run_id IS NOT NULL)
    )
);

CREATE INDEX sqb_access_tokens_location ON sqb_access_tokens(location_id);

CREATE INDEX sqb_access_tokens_run ON sqb_access_tokens(run_id);

CREATE TABLE sqb_security_audit (
    event_id TEXT PRIMARY KEY,
    occurred_at TEXT NOT NULL,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    granting_role TEXT NOT NULL,
    request_id TEXT NOT NULL,
    ip_address TEXT,
    scope_json JSON NOT NULL,
    before_json JSON NOT NULL,
    after_json JSON NOT NULL,
    storage_id BIGINT
);

CREATE INDEX sqb_security_audit_time ON sqb_security_audit(occurred_at, event_id);

CREATE INDEX sqb_security_audit_trigger ON sqb_security_audit((scope_json ->> 'trigger_id'), occurred_at, event_id)
WHERE action = 'trigger.changed';

CREATE TABLE sqb_location_instances (
    instance_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    health_json JSON NOT NULL
);

CREATE INDEX sqb_location_instances_location_seen ON sqb_location_instances(location_id, last_seen_at);

CREATE TABLE sqb_location_work (
    request_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    snapshot_id TEXT NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    generation BIGINT NOT NULL,
    request_json JSON NOT NULL,
    requested_by TEXT NOT NULL,
    requested_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    claim_id TEXT,
    session_id TEXT REFERENCES sqb_location_instances(instance_id),
    claimed_until TEXT,
    result_json JSON,
    completed_at TEXT
);

CREATE INDEX sqb_location_work_pending ON sqb_location_work(location_id, target, requested_at);

CREATE TABLE sqb_platform_sink_deliveries (
    sink_id TEXT NOT NULL,
    storage_id BIGINT NOT NULL,
    event_id TEXT NOT NULL,
    delivered_at TEXT NOT NULL,
    PRIMARY KEY (sink_id, storage_id)
);

CREATE TABLE sqb_sensor_evaluation_events (
    event_id TEXT PRIMARY KEY,
    evaluation_id TEXT NOT NULL,
    trigger_id TEXT NOT NULL REFERENCES sqb_triggers(trigger_id),
    phase TEXT NOT NULL CHECK (phase IN ('requested', 'completed', 'failed', 'timed_out', 'abandoned')),
    occurred_at TEXT NOT NULL,
    fact_json JSON NOT NULL,
    UNIQUE (evaluation_id, phase)
);

CREATE INDEX sqb_sensor_evaluation_history ON sqb_sensor_evaluation_events(trigger_id, phase, occurred_at, evaluation_id);

CREATE INDEX sqb_sensor_evaluation_expiry ON sqb_sensor_evaluation_events(occurred_at);

CREATE TABLE sqb_run_requests (
    trigger_id TEXT NOT NULL REFERENCES sqb_triggers(trigger_id),
    run_key TEXT NOT NULL,
    evaluation_id TEXT NOT NULL,
    run_id TEXT NOT NULL REFERENCES sqb_runs(run_id),
    PRIMARY KEY (trigger_id, run_key)
);

CREATE INDEX sqb_runs_terminal_cursor ON sqb_runs(location_id, target, job_key, last_event_storage_id)
WHERE status IN ('success', 'warn', 'partial', 'failed', 'cancelled');

CREATE INDEX sqb_runs_parent ON sqb_runs(parent_run_id) WHERE parent_run_id IS NOT NULL;

CREATE TABLE sqb_warehouse_freshness (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT,
    scan_id TEXT
);

CREATE INDEX sqb_warehouse_freshness_scope ON sqb_warehouse_freshness (location_id, target, observed_at, event_id);

CREATE TABLE sqb_warehouse_fingerprints (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    definition_text_hash TEXT NOT NULL REFERENCES sqb_source_texts(text_hash),
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT,
    node_type TEXT,
    node_name TEXT,
    target_database TEXT,
    target_schema TEXT,
    target_name TEXT,
    scan_id TEXT
);

CREATE INDEX sqb_warehouse_fingerprints_scope ON sqb_warehouse_fingerprints (location_id, target, observed_at, event_id);

CREATE TABLE sqb_development_locations (
    project_path TEXT NOT NULL,
    target TEXT NOT NULL,
    location_id TEXT NOT NULL UNIQUE REFERENCES sqb_locations(location_id),
    PRIMARY KEY (project_path, target)
);

CREATE TABLE sqb_password_credentials (
    principal_id TEXT PRIMARY KEY REFERENCES sqb_principals(principal_id),
    credential_id TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    changed_at TEXT NOT NULL
);

CREATE TABLE sqb_sessions (
    session_id TEXT PRIMARY KEY,
    principal_id TEXT NOT NULL REFERENCES sqb_principals(principal_id),
    credential_id TEXT,
    secret_hash TEXT NOT NULL,
    csrf_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    revoked_at TEXT
);

CREATE INDEX sqb_sessions_person ON sqb_sessions(principal_id);

CREATE TABLE sqb_password_login_attempts (
    attempt_id TEXT PRIMARY KEY,
    identity_key TEXT NOT NULL,
    peer_address TEXT NOT NULL,
    attempted_at TEXT NOT NULL
);

CREATE INDEX sqb_password_login_user_time ON sqb_password_login_attempts(identity_key, attempted_at);

CREATE INDEX sqb_password_login_peer_time ON sqb_password_login_attempts(peer_address, attempted_at);

CREATE INDEX sqb_password_login_time ON sqb_password_login_attempts(attempted_at);

CREATE TABLE sqb_oidc_login_flows (
    flow_id TEXT PRIMARY KEY,
    configuration_id TEXT NOT NULL,
    state_hash TEXT NOT NULL UNIQUE,
    browser_hash TEXT NOT NULL,
    nonce TEXT NOT NULL,
    pkce_verifier TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE INDEX sqb_oidc_login_flows_expiry ON sqb_oidc_login_flows(expires_at);

CREATE TABLE sqb_role_assignments (
    assignment_id TEXT PRIMARY KEY,
    principal_id TEXT NOT NULL REFERENCES sqb_principals(principal_id),
    role_key TEXT NOT NULL,
    location_id TEXT REFERENCES sqb_locations(location_id),
    target TEXT,
    active_key TEXT NOT NULL UNIQUE
);

CREATE INDEX sqb_role_assignments_principal ON sqb_role_assignments(principal_id);

CREATE TABLE sqb_person_groups (
    principal_id TEXT NOT NULL REFERENCES sqb_principals(principal_id),
    group_name TEXT NOT NULL,
    PRIMARY KEY (principal_id, group_name)
);

CREATE TABLE sqb_group_roles (
    group_name TEXT NOT NULL,
    role_key TEXT NOT NULL,
    location_id TEXT,
    target TEXT
);

CREATE INDEX sqb_fingerprints_namespace ON sqb_warehouse_fingerprints (location_id,target,state_database,state_schema,observed_at,event_id);

CREATE INDEX sqb_freshness_namespace ON sqb_warehouse_freshness (location_id,target,state_database,state_schema,observed_at,event_id);

CREATE UNIQUE INDEX sqb_security_audit_storage_id ON sqb_security_audit(storage_id);

CREATE TABLE sqb_warehouse_audit_results (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    audit_name TEXT NOT NULL,
    outcome TEXT NOT NULL,
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL,
    scan_id TEXT,
    binding_key TEXT
);

CREATE INDEX sqb_audit_results_namespace ON sqb_warehouse_audit_results (location_id,target,state_database,state_schema,observed_at,event_id);

CREATE INDEX sqb_audit_results_binding ON sqb_warehouse_audit_results (location_id,target,binding_key,observed_at,event_id);

CREATE TABLE sqb_warehouse_janitor_events (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    event_type TEXT NOT NULL,
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL,
    scan_id TEXT
);

CREATE INDEX sqb_janitor_events_namespace ON sqb_warehouse_janitor_events (location_id,target,state_database,state_schema,observed_at,event_id);

CREATE TABLE sqb_warehouse_migrations (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    target_name TEXT,
    destination_model TEXT NOT NULL,
    decision TEXT NOT NULL,
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL,
    scan_id TEXT
);

CREATE INDEX sqb_migrations_namespace ON sqb_warehouse_migrations (location_id,target,state_database,state_schema,observed_at,event_id);

CREATE INDEX sqb_migrations_destination ON sqb_warehouse_migrations (location_id,target,destination_model,observed_at,event_id);

CREATE TABLE sqb_warehouse_old_name_views (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    target_name TEXT,
    migration_event_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL,
    scan_id TEXT
);

CREATE INDEX sqb_old_name_views_namespace ON sqb_warehouse_old_name_views (location_id,target,state_database,state_schema,observed_at,event_id);

CREATE INDEX sqb_old_name_views_migration ON sqb_warehouse_old_name_views (location_id,target,migration_event_id,observed_at,event_id);

CREATE TABLE sqb_warehouse_microbatches (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    model_name TEXT NOT NULL,
    target_name TEXT NOT NULL,
    scope_kind TEXT NOT NULL,
    scope_key TEXT NOT NULL,
    record_type TEXT NOT NULL,
    execution_run_id TEXT NOT NULL,
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL,
    scan_id TEXT
);

CREATE INDEX sqb_microbatches_namespace ON sqb_warehouse_microbatches (location_id,target,state_database,state_schema,observed_at,event_id);

CREATE INDEX sqb_microbatches_model ON sqb_warehouse_microbatches (location_id,target,model_name);

CREATE TABLE sqb_warehouse_column_migrations (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL,
    scan_id TEXT
);

CREATE INDEX sqb_column_migrations_namespace ON sqb_warehouse_column_migrations (location_id,target,state_database,state_schema,observed_at,event_id);

CREATE TABLE sqb_warehouse_node_results (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL,
    scan_id TEXT
);

CREATE INDEX sqb_node_results_namespace ON sqb_warehouse_node_results (location_id,target,state_database,state_schema,observed_at,event_id);

CREATE TABLE sqb_warehouse_consumption (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL,
    scan_id TEXT
);

CREATE INDEX sqb_consumption_namespace ON sqb_warehouse_consumption (location_id,target,state_database,state_schema,observed_at,event_id);

CREATE TABLE sqb_warehouse_observations (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    kind TEXT NOT NULL,
    node TEXT NOT NULL,
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL,
    scan_id TEXT
);

CREATE INDEX sqb_observations_namespace ON sqb_warehouse_observations (location_id,target,state_database,state_schema,observed_at,event_id);

CREATE INDEX sqb_observations_latest ON sqb_warehouse_observations (location_id,target,kind,node,observed_at,event_id);

CREATE TABLE sqb_warehouse_resource_costs (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    run_id TEXT NOT NULL,
    orchestration_run_id TEXT,
    attempt BIGINT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_name TEXT NOT NULL,
    run_started_at TEXT NOT NULL,
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL,
    scan_id TEXT
);

CREATE INDEX sqb_resource_costs_namespace ON sqb_warehouse_resource_costs (location_id,target,state_database,state_schema,observed_at,event_id);

CREATE INDEX sqb_resource_costs_run ON sqb_warehouse_resource_costs (location_id,target,run_id,attempt);

CREATE INDEX sqb_resource_costs_orchestration_run ON sqb_warehouse_resource_costs (orchestration_run_id);

CREATE INDEX sqb_resource_costs_started ON sqb_warehouse_resource_costs (location_id,target,run_started_at);

CREATE INDEX sqb_freshness_source_latest ON sqb_warehouse_freshness (
    location_id, target, (record_json ->> 'source_name'),
    (record_json ->> 'target_database'), (record_json ->> 'target_schema'),
    (record_json ->> 'target_name'), observed_at DESC, event_id DESC
);

CREATE INDEX sqb_freshness_source_received ON sqb_warehouse_freshness (
    location_id, target, (record_json ->> 'source_name'),
    (record_json ->> 'target_database'), (record_json ->> 'target_schema'),
    (record_json ->> 'target_name'), received_at DESC, observed_at DESC, event_id DESC
);

CREATE TABLE sqb_security_audit_deliveries (
    sink_id TEXT NOT NULL,
    storage_id BIGINT NOT NULL,
    event_id TEXT NOT NULL,
    delivered_at TEXT NOT NULL,
    PRIMARY KEY (sink_id, storage_id)
);

CREATE TABLE sqb_warehouse_catalog (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL,
    scan_id TEXT
);

CREATE INDEX sqb_warehouse_catalog_namespace ON sqb_warehouse_catalog
    (location_id, target, state_schema, state_database, observed_at, event_id);

CREATE INDEX sqb_fingerprint_node_latest ON sqb_warehouse_fingerprints
    (location_id,target,node_type,node_name,target_database,target_schema,target_name,observed_at DESC,event_id DESC);

CREATE TABLE sqb_warehouse_statistics (
    event_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    relation_name TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    record_json JSON NOT NULL,
    received_at TEXT NOT NULL
);

CREATE INDEX sqb_statistics_relation_time ON sqb_warehouse_statistics
    (location_id,target,state_schema,state_database,relation_name,observed_at,event_id);

CREATE INDEX sqb_statistics_expiration ON sqb_warehouse_statistics (observed_at,event_id);

CREATE INDEX sqb_catalog_statistics_scan ON sqb_warehouse_catalog
    (location_id,target,state_schema,state_database,event_id);

CREATE TABLE sqb_cli_login_flows (
    flow_id TEXT PRIMARY KEY,
    client_hash TEXT NOT NULL,
    browser_hash TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    principal_id TEXT,
    session_id TEXT
);

CREATE INDEX sqb_cli_login_expiry ON sqb_cli_login_flows(expires_at);

CREATE INDEX IF NOT EXISTS sqb_runs_tick ON sqb_runs(tick_id);

CREATE INDEX IF NOT EXISTS sqb_ticks_expiry ON sqb_ticks(scheduled_for) WHERE outcome = 'covered';

CREATE INDEX IF NOT EXISTS sqb_runs_snapshot ON sqb_runs(snapshot_id);

CREATE INDEX IF NOT EXISTS sqb_snapshot_nodes_hash ON sqb_snapshot_nodes(node_hash);

CREATE INDEX IF NOT EXISTS sqb_snapshot_sources_hash ON sqb_snapshot_sources(text_hash);

CREATE INDEX IF NOT EXISTS sqb_snapshot_documents_hash ON sqb_snapshot_documents(document_hash);

CREATE TABLE sqb_effective_roles (
    location_id TEXT NOT NULL,
    target TEXT NOT NULL,
    role_key TEXT NOT NULL,
    resolved_grants_json TEXT NOT NULL,
    PRIMARY KEY (location_id, target, role_key)
);

CREATE INDEX IF NOT EXISTS sqb_jobs_snapshot ON sqb_jobs(snapshot_id);

CREATE INDEX IF NOT EXISTS sqb_triggers_snapshot ON sqb_triggers(snapshot_id);

CREATE INDEX IF NOT EXISTS sqb_triggers_removed ON sqb_triggers(removed_in_snapshot_id) WHERE removed_in_snapshot_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS sqb_runs_trigger ON sqb_runs(trigger_id) WHERE trigger_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS sqb_runs_rerun_of ON sqb_runs(rerun_of_run_id) WHERE rerun_of_run_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS sqb_location_work_session ON sqb_location_work(session_id) WHERE session_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS sqb_location_work_snapshot ON sqb_location_work(snapshot_id);

CREATE INDEX IF NOT EXISTS sqb_run_requests_run ON sqb_run_requests(run_id);

CREATE INDEX IF NOT EXISTS sqb_warehouse_fingerprints_text ON sqb_warehouse_fingerprints(definition_text_hash);

CREATE INDEX IF NOT EXISTS sqb_role_assignments_location ON sqb_role_assignments(location_id);
