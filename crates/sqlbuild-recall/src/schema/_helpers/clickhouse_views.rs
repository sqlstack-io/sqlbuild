//! The bodies of the supported views in ClickHouse: each key counted once, within retention.

use crate::schema::_helpers::rendering::clickhouse_table;
use crate::schema::constants::{OUTPUT_TAIL_RECORDS, REPLACING_ENGINE};
use crate::schema::main::table_spec::table_spec;
use crate::schema::models::{DdlOptions, Table};

/// The columns the `invocations` pass reads from its three sources, NULL (the terminal flag 0) where a source does not give them.
const INVOCATION_SOURCE_COLUMNS: [(&str, &str, &str); 35] = [
    ("ctx_command", "Nullable(String)", "NULL"),
    ("ctx_subcommand", "Nullable(String)", "NULL"),
    ("ctx_target", "Nullable(String)", "NULL"),
    ("ctx_adapter", "Nullable(String)", "NULL"),
    ("ctx_project_name", "Nullable(String)", "NULL"),
    ("ctx_selection", "Nullable(String)", "NULL"),
    ("ctx_exclusion", "Nullable(String)", "NULL"),
    ("ctx_sqlbuild_version", "Nullable(String)", "NULL"),
    ("ctx_git_revision", "Nullable(String)", "NULL"),
    ("ctx_git_dirty", "Nullable(UInt8)", "NULL"),
    ("ctx_host_token", "Nullable(String)", "NULL"),
    ("ctx_pid", "Nullable(Int64)", "NULL"),
    ("ctx_external_context", "Nullable(String)", "NULL"),
    ("ctx_recall_filter_active", "Nullable(UInt8)", "NULL"),
    ("ctx_origin", "Nullable(String)", "NULL"),
    ("ctx_command_mode", "Nullable(String)", "NULL"),
    ("ctx_selection_count", "Nullable(Int64)", "NULL"),
    ("ctx_selection_truncated", "Nullable(UInt8)", "NULL"),
    ("ctx_selector_files", "Nullable(String)", "NULL"),
    ("heartbeat_at", "Nullable(DateTime64(6, 'UTC'))", "NULL"),
    ("beat_stale_at", "Nullable(DateTime64(6, 'UTC'))", "NULL"),
    ("beat_command", "Nullable(String)", "NULL"),
    ("beat_subcommand", "Nullable(String)", "NULL"),
    ("lifecycle_event", "Nullable(String)", "NULL"),
    (
        "event_occurred_at",
        "Nullable(DateTime64(6, 'UTC'))",
        "NULL",
    ),
    ("event_sequence", "Nullable(Int64)", "NULL"),
    ("event_command", "Nullable(String)", "NULL"),
    ("event_exit_code", "Nullable(Int64)", "NULL"),
    ("event_duration_ms", "Nullable(Float64)", "NULL"),
    ("event_error_type", "Nullable(String)", "NULL"),
    ("event_error_code", "Nullable(String)", "NULL"),
    ("event_error_message", "Nullable(String)", "NULL"),
    ("event_signal_number", "Nullable(Int64)", "NULL"),
    ("event_cost_status", "Nullable(String)", "NULL"),
    ("event_is_terminal", "UInt8", "0"),
];

/// The statement columns each statement event carries into the second fold, in tuple order.
const STATEMENT_FACT_COLUMNS: [&str; 19] = [
    "statement_id",
    "event_type",
    "occurred_at",
    "invocation_sequence",
    "run_id",
    "resource_attempt_id",
    "operation_id",
    "intent",
    "statement_kind",
    "purpose",
    "sql_digest",
    "query_id",
    "error_type",
    "error_code",
    "duration_ms",
    "row_count",
    "affected_rows",
    "outcome",
    "started_at",
];

/// The columns the `output_invocations` pass reads: the chunk's own values default to what adds nothing to a sum, and the delivery and context values to NULL.
const OUTPUT_SOURCE_COLUMNS: [(&str, &str, &str); 14] = [
    ("chunk_stream", "String", "''"),
    ("chunk_bytes", "Int64", "0"),
    ("chunk_sequence", "Nullable(Int64)", "NULL"),
    ("chunk_dropped_before", "Int64", "0"),
    ("chunk_spans", "Int64", "0"),
    ("chunk_omitted_bytes", "Int64", "0"),
    ("delivery_accepted", "Nullable(Int64)", "NULL"),
    ("delivery_delivered", "Nullable(Int64)", "NULL"),
    ("delivery_filtered", "Nullable(Int64)", "NULL"),
    ("delivery_dropped", "Nullable(Int64)", "NULL"),
    ("delivery_failed", "Nullable(Int64)", "NULL"),
    ("delivery_flush_complete", "Nullable(UInt8)", "NULL"),
    ("context_filter_active", "Nullable(UInt8)", "NULL"),
    ("is_listed", "UInt8", "0"),
];

/// `table`'s rows within its TTL, one copy per key: `FINAL` for replacing tables, so filters on the sorting key reach the read, else the first ingested copy. SQL text outlives retention like its TTL, so every reference still within retention finds it.
fn deduplicated(table: Table, options: &DdlOptions) -> String {
    let spec = table_spec(table);
    let days: u32 = options.retention_days.saturating_add(spec.ttl_extra_days);
    if spec.clickhouse_engine == REPLACING_ENGINE {
        return format!(
            "(SELECT * FROM {name} FINAL WHERE {expiry} >= now64(6) - INTERVAL {days} DAY)",
            name = clickhouse_table(spec.name, options),
            expiry = spec.expiry_column,
        );
    }
    format!(
        "(SELECT * FROM {name} WHERE {expiry} >= now64(6) - INTERVAL {days} DAY \
         ORDER BY ingested_at ASC LIMIT 1 BY {key})",
        name = clickhouse_table(spec.name, options),
        expiry = spec.expiry_column,
        key = spec.key.join(", "),
    )
}

/// `table`'s rows within its TTL that match `filter`, every stored copy included. Only for folds whose result is the same however often a row repeats (`any`, `min`, `max`, `argMin`, `argMax`, a count compared with zero): copies of one key hold the same content, since a write with other content is refused as a conflict.
fn retained(table: Table, options: &DdlOptions, filter: &str) -> String {
    let spec = table_spec(table);
    let days: u32 = options.retention_days.saturating_add(spec.ttl_extra_days);
    let filter: String = if filter.is_empty() {
        String::new()
    } else {
        format!(" AND {filter}")
    };
    format!(
        "(SELECT * FROM {name} WHERE {expiry} >= now64(6) - INTERVAL {days} DAY{filter})",
        name = clickhouse_table(spec.name, options),
        expiry = spec.expiry_column,
    )
}

/// The value `values` gives column `name`, if any.
fn branch_value<'a>(values: &[(&str, &'a str)], name: &str) -> Option<&'a str> {
    values
        .iter()
        .find(|(column, _)| *column == name)
        .map(|(_, value)| *value)
}

/// One branch of a single-pass `UNION ALL` reading `from`: `invocation_id`, `invocation_started_at`, the `source` label, then every column of `columns` (name, type, default), taken from `values` where given and the default elsewhere. Column names differ from the tables' own, so no alias hides a table column.
fn union_branch(
    source: &str,
    columns: &[(&str, &str, &str)],
    values: &[(&str, &str)],
    from: &str,
) -> String {
    let selected: Vec<String> = columns
        .iter()
        .map(|(name, column_type, default)| {
            let value: &str = branch_value(values, name).unwrap_or(*default);
            format!("        CAST({value} AS {column_type}) AS {name}")
        })
        .collect();
    format!(
        "    SELECT invocation_id, invocation_started_at, '{source}' AS source,\n{}\n    FROM {from}",
        selected.join(",\n")
    )
}

/// One pass over the context rows, heartbeats and lifecycle events, folded by `invocation_id` and `invocation_started_at`, which an invocation's rows share: each table is read once, a filter on either column reaches every read, and no join needs `join_use_nulls`.
fn invocations(options: &DdlOptions) -> String {
    let context = retained(Table::Invocations, options, "");
    let events = retained(
        Table::Events,
        options,
        "event_type IN ('invocation_started', 'invocation_completed', 'invocation_failed')",
    );
    let beats = retained(Table::Heartbeats, options, "");
    let columns = &INVOCATION_SOURCE_COLUMNS;
    let context_rows = union_branch(
        "context",
        columns,
        &[
            ("ctx_command", "command"),
            ("ctx_subcommand", "subcommand"),
            ("ctx_target", "target"),
            ("ctx_adapter", "adapter"),
            ("ctx_project_name", "project_name"),
            ("ctx_selection", "selection"),
            ("ctx_exclusion", "exclusion"),
            ("ctx_sqlbuild_version", "sqlbuild_version"),
            ("ctx_git_revision", "git_revision"),
            ("ctx_git_dirty", "git_dirty"),
            ("ctx_host_token", "host_token"),
            ("ctx_pid", "pid"),
            ("ctx_external_context", "external_context"),
            ("ctx_recall_filter_active", "recall_filter_active"),
            ("ctx_origin", "origin"),
            ("ctx_command_mode", "command_mode"),
            ("ctx_selection_count", "selection_count"),
            ("ctx_selection_truncated", "selection_truncated"),
            ("ctx_selector_files", "selector_files"),
        ],
        &context,
    );
    let beat_rows = union_branch(
        "beat",
        columns,
        &[
            ("heartbeat_at", "beat_at"),
            ("beat_stale_at", "stale_at"),
            ("beat_command", "command"),
            ("beat_subcommand", "subcommand"),
        ],
        &beats,
    );
    let event_rows = union_branch(
        "event",
        columns,
        &[
            ("lifecycle_event", "event_type"),
            ("event_occurred_at", "occurred_at"),
            ("event_sequence", "invocation_sequence"),
            ("event_command", "command"),
            ("event_exit_code", "exit_code"),
            ("event_duration_ms", "duration_ms"),
            ("event_error_type", "error_type"),
            ("event_error_code", "error_code"),
            (
                "event_error_message",
                "JSONExtract(payload, 'error_message', 'Nullable(String)')",
            ),
            (
                "event_signal_number",
                "JSONExtract(payload, 'signal_number', 'Nullable(Int64)')",
            ),
            (
                "event_cost_status",
                "JSONExtract(payload, 'cost_status', 'Nullable(String)')",
            ),
            (
                "event_is_terminal",
                "event_type IN ('invocation_completed', 'invocation_failed')",
            ),
        ],
        &events,
    );
    let order = "tuple(event_sequence, event_occurred_at)";
    let context_fields: String = INVOCATION_SOURCE_COLUMNS
        .iter()
        .filter_map(|(name, _, _)| name.strip_prefix("ctx_"))
        .map(|field| format!("        anyIf(ctx_{field}, source = 'context') AS context_{field},"))
        .collect::<Vec<_>>()
        .join("\n");
    let terminal = |column: &str, alias: &str| {
        format!("        argMaxIf({column}, {order}, event_is_terminal = 1) AS {alias},")
    };
    format!(
        "WITH
source_rows AS (
{context_rows}
    UNION ALL
{beat_rows}
    UNION ALL
{event_rows}
),
folded AS (
    SELECT invocation_id, invocation_started_at,
{context_fields}
        minIf(event_occurred_at, lifecycle_event = 'invocation_started') AS started_occurred_at,
        anyIf(event_command, lifecycle_event = 'invocation_started') AS started_command,
{terminal_type}
{terminal_at}
{terminal_exit_code}
{terminal_duration_ms}
{terminal_error_type}
{terminal_error_code}
{terminal_error_message}
{terminal_signal_number}
{terminal_cost_status}
        maxIf(heartbeat_at, source = 'beat') AS last_heartbeat_at,
        maxIf(beat_stale_at, source = 'beat') AS stale_at,
        minIf(beat_command, source = 'beat') AS first_beat_command,
        minIf(beat_subcommand, source = 'beat') AS first_beat_subcommand
    FROM source_rows
    GROUP BY invocation_id, invocation_started_at
)
SELECT
    invocation_id,
    invocation_started_at,
    coalesce(started_occurred_at, invocation_started_at) AS started_at,
    multiIf(
        terminal_type = 'invocation_completed', 'completed',
        terminal_type = 'invocation_failed' AND terminal_signal_number IS NOT NULL, 'cancelled',
        terminal_type = 'invocation_failed', 'failed',
        stale_at IS NULL, 'unknown',
        stale_at >= now64(6), 'running',
        'abandoned'
    ) AS status,
    if(terminal_type IS NOT NULL, terminal_at,
        if(stale_at < now64(6), last_heartbeat_at, NULL)) AS ended_at,
    if(terminal_type IS NOT NULL, terminal_duration_ms,
        if(stale_at < now64(6),
            toFloat64(dateDiff('millisecond', started_at, last_heartbeat_at)), NULL))
        AS duration_ms,
    multiIf(terminal_type IS NOT NULL, toUInt8(0), stale_at < now64(6), toUInt8(1), NULL)
        AS ended_at_estimated,
    terminal_exit_code AS exit_code,
    terminal_error_type AS error_type,
    terminal_error_code AS error_code,
    terminal_error_message AS error_message,
    terminal_signal_number AS signal_number,
    terminal_cost_status AS cost_status,
    coalesce(context_command, started_command, first_beat_command) AS command,
    coalesce(context_subcommand, first_beat_subcommand) AS subcommand,
    context_origin AS origin,
    context_command_mode AS command_mode,
    context_target AS target,
    context_adapter AS adapter,
    context_project_name AS project_name,
    context_selection AS selection,
    coalesce(context_selection_count,
        if(isValidJSON(context_selection), toInt64(JSONLength(context_selection)), NULL))
        AS selection_count,
    context_selection_truncated AS selection_truncated,
    context_selector_files AS selector_files,
    context_exclusion AS exclusion,
    context_sqlbuild_version AS sqlbuild_version,
    context_git_revision AS git_revision,
    context_git_dirty AS git_dirty,
    context_host_token AS host_token,
    context_pid AS pid,
    context_external_context AS external_context,
    if(isValidJSON(context_external_context),
        JSONExtract(context_external_context, 'integration', 'name', 'Nullable(String)'), NULL)
        AS integration,
    if(isValidJSON(context_external_context),
        JSONExtract(context_external_context, 'integration', 'job_name', 'Nullable(String)'), NULL)
        AS integration_job,
    if(isValidJSON(context_external_context),
        JSONExtract(context_external_context, 'integration', 'run_id', 'Nullable(String)'), NULL)
        AS integration_run_id,
    if(isValidJSON(context_external_context),
        JSONExtract(context_external_context, 'integration', 'retry_number', 'Nullable(Int64)'),
        NULL) AS integration_retry,
    context_recall_filter_active AS recall_filter_active,
    last_heartbeat_at
FROM folded",
        terminal_type = terminal("lifecycle_event", "terminal_type"),
        terminal_at = terminal("event_occurred_at", "terminal_at"),
        terminal_exit_code = terminal("event_exit_code", "terminal_exit_code"),
        terminal_duration_ms = terminal("event_duration_ms", "terminal_duration_ms"),
        terminal_error_type = terminal("event_error_type", "terminal_error_type"),
        terminal_error_code = terminal("event_error_code", "terminal_error_code"),
        terminal_error_message = terminal("event_error_message", "terminal_error_message"),
        terminal_signal_number = terminal("event_signal_number", "terminal_signal_number"),
        terminal_cost_status = terminal("event_cost_status", "terminal_cost_status"),
    )
}

/// Two folds over one read of the events: by resource, which names the resource and gathers its statement events, then by statement. Both group by `invocation_id` and `invocation_started_at`, so a filter on either reaches the read; `sql_available` needs a reference to the digest in the statement's own invocation and the stored SQL, which is shared across invocations. The references and the stored SQL are each read once.
fn statements(options: &DdlOptions) -> String {
    let events = retained(Table::Events, options, "");
    let artifacts = clickhouse_table(table_spec(Table::Artifacts).name, options);
    let refs = retained(Table::ArtifactRefs, options, "");
    let order = "tuple(invocation_sequence, occurred_at)";
    let terminal = "event_type IN ('statement_completed', 'statement_failed')";
    let statement_event = "statement_id IS NOT NULL AND startsWith(event_type, 'statement_')";
    let facts: String = STATEMENT_FACT_COLUMNS.join(", ");
    let unpacked: String = STATEMENT_FACT_COLUMNS
        .iter()
        .enumerate()
        .map(|(index, column)| format!("fact.{} AS {column}", index + 1))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "WITH
by_resource AS (
    SELECT invocation_id, invocation_started_at, resource_id,
        min(resource_name) AS group_resource_name,
        minIf(resource_kind, resource_name IS NOT NULL) AS group_resource_kind,
        groupArrayIf(tuple({facts}), {statement_event}) AS facts
    FROM {events}
    WHERE ({statement_event}) OR (resource_id IS NOT NULL AND resource_name IS NOT NULL)
    GROUP BY invocation_id, invocation_started_at, resource_id
),
statement_events AS (
    SELECT invocation_id, invocation_started_at, resource_id, group_resource_name, group_resource_kind,
        {unpacked}
    FROM by_resource ARRAY JOIN facts AS fact
),
folded AS (
    SELECT invocation_id, invocation_started_at, statement_id,
        argMin(run_id, {order}) AS first_run_id,
        argMin(resource_id, {order}) AS first_resource_id,
        argMinIf(tuple(group_resource_name, group_resource_kind), {order}, resource_id IS NOT NULL)
            AS first_resource,
        argMin(resource_attempt_id, {order}) AS first_resource_attempt_id,
        argMin(operation_id, {order}) AS first_operation_id,
        argMin(intent, {order}) AS first_intent,
        argMin(statement_kind, {order}) AS first_statement_kind,
        argMin(purpose, {order}) AS first_purpose,
        argMin(sql_digest, {order}) AS first_sql_digest,
        coalesce(min(started_at), minIf(occurred_at, event_type = 'statement_started'))
            AS statement_started_at,
        minIf(occurred_at, event_type = 'statement_submitted') AS submitted_at,
        max(query_id) AS any_query_id,
        countIf({terminal}) > 0 AS has_terminal,
        argMaxIf(event_type, {order}, {terminal}) AS last_event_type,
        argMaxIf(occurred_at, {order}, {terminal}) AS last_ended_at,
        argMaxIf(query_id, {order}, {terminal}) AS last_query_id,
        argMaxIf(error_type, {order}, {terminal}) AS last_error_type,
        argMaxIf(error_code, {order}, {terminal}) AS last_error_code,
        argMaxIf(duration_ms, {order}, {terminal}) AS last_duration_ms,
        argMaxIf(row_count, {order}, {terminal}) AS last_row_count,
        argMaxIf(affected_rows, {order}, {terminal}) AS last_affected_rows,
        argMaxIf(outcome, {order}, {terminal}) AS last_outcome
    FROM statement_events GROUP BY invocation_id, invocation_started_at, statement_id
)
SELECT
    invocation_id,
    invocation_started_at,
    statement_id,
    first_run_id AS run_id,
    first_resource_id AS resource_id,
    tupleElement(first_resource, 1) AS resource_name,
    tupleElement(first_resource, 2) AS resource_kind,
    first_resource_attempt_id AS resource_attempt_id,
    first_operation_id AS operation_id,
    first_intent AS intent,
    first_statement_kind AS statement_kind,
    first_purpose AS purpose,
    if(has_terminal, coalesce(last_query_id, any_query_id), any_query_id) AS query_id,
    statement_started_at AS started_at,
    submitted_at,
    if(has_terminal, last_ended_at, NULL) AS ended_at,
    multiIf(
        NOT has_terminal, 'no_terminal',
        last_event_type = 'statement_completed', 'completed',
        last_outcome = 'handled', 'handled',
        'failed'
    ) AS outcome,
    if(has_terminal, last_error_type, NULL) AS error_type,
    if(has_terminal, last_error_code, NULL) AS error_code,
    if(has_terminal, last_duration_ms, NULL) AS duration_ms,
    if(has_terminal, last_row_count, NULL) AS row_count,
    if(has_terminal, last_affected_rows, NULL) AS affected_rows,
    first_sql_digest AS sql_digest,
    sql_referenced
        AND concat('sha256:', first_sql_digest) IN (SELECT digest FROM {artifacts})
        AS sql_available,
    multiIf(
        sql_available, NULL,
        sql_referenced, 'expired_or_not_captured',
        'not_captured'
    ) AS sql_unavailable_reason
FROM (
    SELECT *,
        (invocation_id, concat('sha256:', first_sql_digest))
            IN (SELECT invocation_id, digest FROM {refs}) AS sql_referenced
    FROM folded
)"
    )
}

fn audit_results(options: &DdlOptions) -> String {
    let events = deduplicated(Table::Events, options);
    format!(
        "SELECT
    invocation_id,
    invocation_started_at,
    event_id,
    occurred_at,
    run_id,
    resource_id,
    resource_attempt_id,
    JSONExtract(payload, 'audit_name', 'Nullable(String)') AS audit_name,
    JSONExtract(payload, 'audit_definition_name', 'Nullable(String)') AS audit_definition_name,
    if(JSONHas(payload, 'target_name'), concat(
        if(JSONHas(payload, 'target_database'), concat(JSONExtractString(payload, 'target_database'), '.'), ''),
        if(JSONHas(payload, 'target_schema'), concat(JSONExtractString(payload, 'target_schema'), '.'), ''),
        JSONExtractString(payload, 'target_name')), NULL) AS target_relation,
    JSONExtract(payload, 'attached_target_kind', 'Nullable(String)') AS attached_target_kind,
    JSONExtract(payload, 'attached_target_name', 'Nullable(String)') AS attached_target_name,
    JSONExtract(payload, 'attached_column_name', 'Nullable(String)') AS column,
    outcome,
    JSONExtract(payload, 'severity', 'Nullable(String)') AS severity,
    JSONExtract(payload, 'violation_count', 'Nullable(Int64)') AS violation_count,
    JSONExtract(payload, 'measured_value', 'Nullable(Float64)') AS measured_value,
    JSONExtract(payload, 'evaluation_mode', 'Nullable(String)') AS evaluation_mode,
    JSONExtract(payload, 'execution_error', 'Nullable(String)') AS execution_error,
    sql_digest,
    JSONExtract(payload, 'evidence_count', 'Nullable(Int64)') AS evidence_count,
    if(JSONHas(payload, 'evidence_truncated'),
        toUInt8(JSONExtractBool(payload, 'evidence_truncated')), NULL) AS evidence_truncated,
    JSONExtract(payload, 'sample_digest', 'Nullable(String)') AS sample_digest
FROM {events}
WHERE event_type = 'audit_completed'"
    )
}

/// One pass over the output chunks, the output delivery row and the context row, folded by invocation without joins. Chunks are counted and summed, so they keep `FINAL`; the delivery and context rows are only picked with `any`.
fn output_invocations(options: &DdlOptions) -> String {
    let output = deduplicated(Table::Output, options);
    let delivery = retained(Table::Delivery, options, "stream = 'output'");
    let context = retained(Table::Invocations, options, "");
    let columns = &OUTPUT_SOURCE_COLUMNS;
    let chunk_rows = union_branch(
        "output",
        columns,
        &[
            ("chunk_stream", "stream"),
            ("chunk_bytes", "byte_count"),
            ("chunk_sequence", "sequence"),
            ("chunk_dropped_before", "dropped_before"),
            ("chunk_spans", "coalesce(omitted_sequences, 1)"),
            ("chunk_omitted_bytes", "coalesce(omitted_bytes, 0)"),
            ("is_listed", "1"),
        ],
        &output,
    );
    let delivery_rows = union_branch(
        "delivery",
        columns,
        &[
            ("delivery_accepted", "accepted"),
            ("delivery_delivered", "delivered"),
            ("delivery_filtered", "filtered"),
            ("delivery_dropped", "dropped"),
            ("delivery_failed", "failed"),
            ("delivery_flush_complete", "flush_complete"),
            ("is_listed", "1"),
        ],
        &delivery,
    );
    let context_rows = union_branch(
        "context",
        columns,
        &[("context_filter_active", "recall_filter_active")],
        &context,
    );
    format!(
        "WITH
source_rows AS (
{chunk_rows}
    UNION ALL
{delivery_rows}
    UNION ALL
{context_rows}
),
folded AS (
    SELECT invocation_id, invocation_started_at,
        countIf(source = 'output') AS chunks,
        sumIf(chunk_bytes, source = 'output') AS bytes,
        sumIf(chunk_bytes, source = 'output' AND chunk_stream = 'stdout') AS stdout_bytes,
        sumIf(chunk_bytes, source = 'output' AND chunk_stream = 'stderr') AS stderr_bytes,
        maxIf(chunk_sequence, source = 'output') AS max_sequence,
        sumIf(chunk_dropped_before, source = 'output') AS queue_drops,
        sumIf(chunk_spans, source = 'output') AS spans,
        sumIf(chunk_omitted_bytes, source = 'output') AS truncated_bytes,
        anyIf(delivery_accepted, source = 'delivery') AS accepted,
        anyIf(delivery_delivered, source = 'delivery') AS delivered,
        anyIf(delivery_filtered, source = 'delivery') AS filtered_count,
        anyIf(delivery_dropped, source = 'delivery') AS dropped,
        anyIf(delivery_failed, source = 'delivery') AS failed_count,
        anyIf(delivery_flush_complete, source = 'delivery') AS flush_flag,
        countIf(source = 'delivery') > 0 AS delivery_reported,
        anyIf(context_filter_active, source = 'context') AS filter_flag
    FROM source_rows
    GROUP BY invocation_id, invocation_started_at
    HAVING max(is_listed) = 1
),
counted AS (
    SELECT
        invocation_id,
        invocation_started_at,
        chunks,
        bytes,
        stdout_bytes,
        stderr_bytes,
        truncated_bytes,
        max_sequence,
        greatest(
            coalesce(toInt64(max_sequence) + 1 - toInt64(spans), 0),
            coalesce(toInt64(accepted), 0) - coalesce(toInt64(chunks), 0)
        ) AS missing_sequences,
        greatest(coalesce(queue_drops, 0), coalesce(dropped, 0)) AS reported_drops,
        coalesce(filtered_count, 0) AS filtered,
        coalesce(failed_count, 0) AS failed,
        coalesce(filter_flag, 0) AS filter_active,
        coalesce(flush_flag, 0) AS flush_complete,
        delivery_reported,
        coalesce(delivered, 0) AS delivered
    FROM folded
)
SELECT
    invocation_id, invocation_started_at, chunks, bytes, stdout_bytes, stderr_bytes,
    truncated_bytes, max_sequence, missing_sequences, reported_drops, filtered, failed,
    filter_active, flush_complete,
    multiIf(
        missing_sequences = 0 AND truncated_bytes > 0, 'truncated',
        missing_sequences = 0, NULL,
        filtered >= missing_sequences, 'filtered',
        filtered + reported_drops + failed >= missing_sequences, 'dropped',
        'unexplained'
    ) AS gap_reason,
    toUInt8(delivery_reported AND flush_complete = 1 AND missing_sequences = 0
        AND reported_drops = 0 AND filtered = 0 AND failed = 0 AND truncated_bytes = 0
        AND chunks >= delivered)
        AS complete
FROM counted"
    )
}

fn output_tail(options: &DdlOptions) -> String {
    let output = deduplicated(Table::Output, options);
    format!(
        "SELECT invocation_id, invocation_started_at, sequence, stream, occurred_at, byte_count,
    toValidUTF8(data) AS text
FROM {output}
ORDER BY invocation_id, sequence DESC
LIMIT {OUTPUT_TAIL_RECORDS} BY invocation_id"
    )
}

fn output_transcripts(options: &DdlOptions) -> String {
    let output = deduplicated(Table::Output, options);
    format!(
        "SELECT
    invocation_id,
    min(invocation_started_at) AS invocation_started_at,
    arrayStringConcat(arrayMap(chunk -> chunk.2, arraySort(chunk -> chunk.1,
        groupArrayIf(tuple(sequence, data), stream = 'stdout')))) AS stdout,
    arrayStringConcat(arrayMap(chunk -> chunk.2, arraySort(chunk -> chunk.1,
        groupArrayIf(tuple(sequence, data), stream = 'stderr')))) AS stderr,
    arrayStringConcat(arrayMap(chunk -> chunk.2, arraySort(chunk -> chunk.1,
        groupArray(tuple(sequence, data))))) AS combined
FROM {output}
GROUP BY invocation_id"
    )
}

fn passthrough(table: Table, options: &DdlOptions) -> String {
    format!("SELECT * FROM {}", deduplicated(table, options))
}

/// Every reference with what is stored for its digest. The stored side is nullable before the join, so a reference without stored SQL reads NULL whether or not the session sets `join_use_nulls`.
fn artifacts(options: &DdlOptions) -> String {
    let refs = deduplicated(Table::ArtifactRefs, options);
    let stored = deduplicated(Table::Artifacts, options);
    format!(
        "SELECT
    refs.invocation_id AS invocation_id,
    refs.invocation_started_at AS invocation_started_at,
    refs.digest AS digest,
    refs.ref_id AS ref_id,
    refs.role AS role,
    refs.statement_id AS statement_id,
    refs.resource_id AS resource_id,
    artifact.stored_kind AS kind,
    artifact.stored_media_type AS media_type,
    artifact.stored_size AS size,
    artifact.stored_digest IS NOT NULL AS available,
    if(artifact.stored_digest IS NULL, 'expired_or_not_captured', NULL) AS unavailable_reason,
    if(startsWith(artifact.stored_media_type, 'text/')
        OR artifact.stored_media_type = 'application/json',
        toValidUTF8(artifact.stored_content), NULL) AS text,
    artifact.stored_content AS content
FROM {refs} AS refs
LEFT JOIN (
    SELECT digest,
        CAST(digest AS Nullable(String)) AS stored_digest,
        CAST(kind AS Nullable(String)) AS stored_kind,
        CAST(media_type AS Nullable(String)) AS stored_media_type,
        CAST(size AS Nullable(Int64)) AS stored_size,
        CAST(content AS Nullable(String)) AS stored_content
    FROM {stored}
) AS artifact ON artifact.digest = refs.digest"
    )
}

/// Each supported view's name and ClickHouse body, in creation order.
pub(crate) fn clickhouse_view_bodies(options: &DdlOptions) -> Vec<(&'static str, String)> {
    vec![
        ("invocations", invocations(options)),
        ("statements", statements(options)),
        ("audit_results", audit_results(options)),
        ("output_invocations", output_invocations(options)),
        ("output_tail", output_tail(options)),
        ("output_transcripts", output_transcripts(options)),
        ("events", passthrough(Table::Events, options)),
        ("diagnostics", passthrough(Table::Diagnostics, options)),
        ("artifacts", artifacts(options)),
        ("conflicts", passthrough(Table::Conflicts, options)),
    ]
}
