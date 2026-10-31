//! PostgreSQL expressions for the shared relational Recall view definitions.

use crate::schema::_helpers::relational_views::relational_view_bodies;

fn json_fields(mut body: String) -> String {
    for (field, cast) in [
        ("audit_name", ""),
        ("audit_definition_name", ""),
        ("target_name", ""),
        ("target_database", ""),
        ("target_schema", ""),
        ("attached_target_kind", ""),
        ("attached_target_name", ""),
        ("attached_column_name", ""),
        ("severity", ""),
        ("violation_count", "::BIGINT"),
        ("measured_value", "::DOUBLE PRECISION"),
        ("evaluation_mode", ""),
        ("execution_error", ""),
        ("evidence_count", "::BIGINT"),
        ("evidence_truncated", "::BOOLEAN::INTEGER"),
        ("sample_digest", ""),
        ("error_message", ""),
        ("signal_number", "::BIGINT"),
        ("cost_status", ""),
    ] {
        body = body.replace(
            &format!("json_extract(payload, '$.{field}')"),
            &format!("(payload ->> '{field}'){cast}"),
        );
    }
    for (field, cast) in [
        ("name", ""),
        ("job_name", ""),
        ("run_id", ""),
        ("retry_number", "::BIGINT"),
    ] {
        body = body.replace(
            &format!("json_extract(context.external_context, '$.integration.{field}')"),
            &format!("(context.external_context #>> '{{integration,{field}}}'){cast}"),
        );
    }
    body.replace(
        "json_valid(context.external_context)",
        "context.external_context IS NOT NULL",
    )
    .replace(
        "json_array_length(context.selection)",
        "jsonb_array_length(context.selection)",
    )
}

/// Keep display bytes until readers decode them: PostgreSQL text rejects NUL and malformed UTF-8.
fn postgres_body(body: String) -> String {
    let mut body = json_fields(body)
        .replace("strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", "CURRENT_TIMESTAMP")
        .replace("MAX(artifact.digest IS NOT NULL)", "MAX(CAST(artifact.digest IS NOT NULL AS INTEGER))")
        .replace("artifact.digest IS NOT NULL AS available", "CAST(artifact.digest IS NOT NULL AS INTEGER) AS available")
        .replace("MAX(\n            COALESCE(received.max_sequence", "GREATEST(\n            COALESCE(received.max_sequence")
        .replace("MAX(COALESCE(received.queue_drops", "GREATEST(COALESCE(received.queue_drops")
        .replace("COALESCE(context.recall_filter_active, 0)", "COALESCE(CAST(context.recall_filter_active AS INTEGER), 0)")
        .replace("COALESCE(delivery.flush_complete, 0)", "COALESCE(CAST(delivery.flush_complete AS INTEGER), 0)")
        .replace("CAST(data AS TEXT)", "data")
        .replace("CAST(artifact.content AS TEXT)", "artifact.content")
        .replace(")\nGROUP BY", ") AS source_rows\nGROUP BY")
        .replace(")\n    GROUP BY", ") AS source_rows\n    GROUP BY")
        .replace(")\nWHERE position", ") AS source_rows\nWHERE position")
        .replace("ROUND(\n            (unixepoch(last_heartbeat_at, 'subsec') - unixepoch(started_at, 'subsec')) * 1000.0, 3\n        )",
            "CAST(ROUND(CAST(EXTRACT(EPOCH FROM (last_heartbeat_at - started_at)) * 1000 AS NUMERIC), 3) AS DOUBLE PRECISION)");
    for expression in [
        "byte_count",
        "CASE WHEN stream = 'stdout' THEN byte_count ELSE 0 END",
        "CASE WHEN stream = 'stderr' THEN byte_count ELSE 0 END",
        "dropped_before",
        "COALESCE(omitted_sequences, 1)",
        "COALESCE(omitted_bytes, 0)",
    ] {
        body = body.replace(
            &format!("SUM({expression})"),
            &format!("CAST(SUM({expression}) AS BIGINT)"),
        );
    }
    body
}

pub(crate) fn postgres_view_bodies() -> Vec<(&'static str, String)> {
    relational_view_bodies(true)
        .into_iter()
        .map(|(name, body)| (name, postgres_body(body)))
        .collect()
}
