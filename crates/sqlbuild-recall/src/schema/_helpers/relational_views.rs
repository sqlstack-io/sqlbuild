//! Shared relational view definitions, with explicit dialect expressions for binary transcripts.

use crate::schema::constants::OUTPUT_TAIL_RECORDS;

/// The current time as stored timestamps spell it.
const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

const STATEMENTS: &str = "WITH facts AS (
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
    SELECT statement_id, MIN(occurred_at) AS started_at
    FROM facts WHERE event_type = 'statement_started' GROUP BY statement_id
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
WHERE firsts.position = 1";

const AUDIT_RESULTS: &str = "SELECT
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
    json_extract(payload, '$.attached_column_name') AS \"column\",
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
WHERE event_type = 'audit_completed'";

const OUTPUT_INVOCATIONS: &str = "WITH received AS (
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
FROM counted";

const EVENTS: &str = "SELECT * FROM recall_events";

const DIAGNOSTICS: &str = "SELECT * FROM recall_diagnostics";

const ARTIFACTS: &str = "SELECT
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
LEFT JOIN recall_artifacts AS artifact ON artifact.digest = refs.digest";

const CONFLICTS: &str = "SELECT * FROM recall_conflicts";

fn output_transcripts(postgres: bool) -> String {
    let columns: Vec<String> = [
        ("CASE WHEN stream = 'stdout' THEN data END", "stdout"),
        ("CASE WHEN stream = 'stderr' THEN data END", "stderr"),
        ("data", "combined"),
    ]
    .into_iter()
    .map(|(expression, alias)| {
        let value = if postgres {
            format!("COALESCE(string_agg({expression}, ''::BYTEA ORDER BY sequence), ''::BYTEA)")
        } else {
            format!(
                "CAST(COALESCE(group_concat({expression}, x'' ORDER BY sequence), x'') AS BLOB)"
            )
        };
        format!("    {value} AS {alias}")
    })
    .collect();
    format!(
        "SELECT invocation_id, MIN(invocation_started_at) AS invocation_started_at,\n{}\nFROM recall_output GROUP BY invocation_id",
        columns.join(",\n")
    )
}

fn invocations() -> String {
    format!(
        "WITH ids AS (
    SELECT invocation_id, MIN(invocation_started_at) AS invocation_started_at
    FROM (
        SELECT invocation_id, invocation_started_at FROM recall_invocations
        UNION ALL SELECT invocation_id, invocation_started_at FROM recall_heartbeats
        UNION ALL SELECT invocation_id, invocation_started_at FROM recall_events
            WHERE event_type IN ('invocation_started', 'invocation_completed', 'invocation_failed')
    )
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
            WHEN beats.stale_at >= {NOW} THEN 'running'
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
        WHEN status = 'abandoned' THEN ROUND(
            (unixepoch(last_heartbeat_at, 'subsec') - unixepoch(started_at, 'subsec')) * 1000.0, 3
        )
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
FROM projected"
    )
}

fn output_tail() -> String {
    format!(
        "SELECT invocation_id, invocation_started_at, sequence, stream, occurred_at, byte_count,
    CAST(data AS TEXT) AS text
FROM (
    SELECT *, ROW_NUMBER() OVER (PARTITION BY invocation_id ORDER BY sequence DESC) AS position
    FROM recall_output
)
WHERE position <= {OUTPUT_TAIL_RECORDS}"
    )
}

/// Each supported view's name and relational body, in creation order.
pub(crate) fn relational_view_bodies(postgres: bool) -> Vec<(&'static str, String)> {
    vec![
        ("invocations", invocations()),
        ("statements", STATEMENTS.to_owned()),
        ("audit_results", AUDIT_RESULTS.to_owned()),
        ("output_invocations", OUTPUT_INVOCATIONS.to_owned()),
        ("output_tail", output_tail()),
        ("output_transcripts", output_transcripts(postgres)),
        ("events", EVENTS.to_owned()),
        ("diagnostics", DIAGNOSTICS.to_owned()),
        ("artifacts", ARTIFACTS.to_owned()),
        ("conflicts", CONFLICTS.to_owned()),
    ]
}
