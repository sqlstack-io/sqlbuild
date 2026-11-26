//! Warehouse statement rows of resource attempts: debug starts, heartbeats and failures.

use crate::progress::constants::{NESTED_PREFIX, STATEMENT_FAILED, UNKNOWN_RESOURCE_NAME};
use crate::progress::models::{ProgressEvent, ProgressProjection};

/// The phase a statement ran in and its leading keyword.
fn statement_phase_kind(
    projection: &ProgressProjection,
    event: &ProgressEvent,
) -> (String, String) {
    let phase: &str = projection
        .operation_starts
        .get(event.operation_id.as_deref().unwrap_or_default())
        .and_then(|start| {
            start
                .payload
                .phase
                .as_deref()
                .or(start.payload.operation_name.as_deref())
        })
        .unwrap_or(&event.payload.intent);
    (phase.to_owned(), event.payload.statement_kind.clone())
}

/// `model=...  phase=...  kind=...` for one statement.
fn statement_context(projection: &ProgressProjection, event: &ProgressEvent) -> String {
    let resource_name: &str = projection
        .resource_names
        .get(event.resource_id.as_deref().unwrap_or_default())
        .map_or(UNKNOWN_RESOURCE_NAME, String::as_str);
    let operation: Option<&ProgressEvent> = projection
        .operation_starts
        .get(event.operation_id.as_deref().unwrap_or_default());
    let phase: &str = operation
        .and_then(|start| {
            start
                .payload
                .phase
                .as_deref()
                .or(start.payload.operation_name.as_deref())
        })
        .unwrap_or(&event.payload.intent);
    format!(
        "model={resource_name}  phase={phase}  kind={}",
        event.payload.statement_kind
    )
}

/// The query identifier the event carries, or the one its statement submitted.
fn statement_query_id(projection: &ProgressProjection, event: &ProgressEvent) -> Option<String> {
    if let Some(query_id) = &event.payload.query_id {
        return Some(query_id.clone());
    }
    projection
        .statement_query_ids
        .get(event.statement_id.as_deref()?)
        .cloned()
}

fn query_suffix(query_id: Option<&String>) -> String {
    query_id
        .map(|query_id| format!("  query_id={query_id}"))
        .unwrap_or_default()
}

/// Track a statement of a known resource, rendering its start in debug mode.
pub(crate) fn consume_statement_start(
    mut projection: ProgressProjection,
    event: &ProgressEvent,
) -> (ProgressProjection, Option<String>) {
    let Some(statement_id) = &event.statement_id else {
        return (projection, None);
    };
    if !event
        .resource_id
        .as_ref()
        .is_some_and(|resource_id| projection.resource_names.contains_key(resource_id))
    {
        return (projection, None);
    }
    projection
        .statement_starts
        .insert(statement_id.clone(), event.clone());
    if !projection.settings.debug {
        return (projection, None);
    }
    let context: String = statement_context(&projection, event);
    (
        projection,
        Some(format!("{NESTED_PREFIX}statement  {context}  START")),
    )
}

/// Remember a submitted query identifier, rendering it in debug mode.
pub(crate) fn consume_statement_submitted(
    mut projection: ProgressProjection,
    event: &ProgressEvent,
) -> (ProgressProjection, Option<String>) {
    let (Some(statement_id), Some(query_id)) = (&event.statement_id, &event.payload.query_id)
    else {
        return (projection, None);
    };
    let Some(start) = projection.statement_starts.get(statement_id) else {
        return (projection, None);
    };
    if projection.statement_query_ids.get(statement_id) == Some(query_id) {
        return (projection, None);
    }
    let context: String = statement_context(&projection, start);
    projection
        .statement_query_ids
        .insert(statement_id.clone(), query_id.clone());
    if !projection.settings.debug {
        return (projection, None);
    }
    (
        projection,
        Some(format!(
            "{NESTED_PREFIX}statement  {context}  query_id={query_id}"
        )),
    )
}

/// Render a long-running statement heartbeat.
pub(crate) fn statement_heartbeat_line(
    projection: &ProgressProjection,
    event: &ProgressEvent,
) -> Option<String> {
    if !event
        .statement_id
        .as_ref()
        .is_some_and(|statement_id| projection.statement_starts.contains_key(statement_id))
    {
        return None;
    }
    let duration: f64 = event.payload.duration_ms?;
    let suffix: String = query_suffix(statement_query_id(projection, event).as_ref());
    let context: String = statement_context(projection, event);
    Some(format!(
        "{NESTED_PREFIX}statement  {context}  RUNNING {:.1}s{suffix}",
        duration / 1000.0
    ))
}

/// Forget a finished statement, rendering it when it failed.
pub(crate) fn consume_statement_terminal(
    mut projection: ProgressProjection,
    event: &ProgressEvent,
) -> (ProgressProjection, Option<String>) {
    let Some(statement_id) = &event.statement_id else {
        return (projection, None);
    };
    let Some(start) = projection.statement_starts.remove(statement_id) else {
        return (projection, None);
    };
    let resource_name: String = projection
        .resource_names
        .get(start.resource_id.as_deref().unwrap_or_default())
        .cloned()
        .unwrap_or_default();
    if event.event_type == STATEMENT_FAILED {
        let (phase, kind) = statement_phase_kind(&projection, &start);
        projection.failed_statements.insert(
            ProgressProjection::statement_owner(start.resource_id.as_deref(), &resource_name),
            (phase, kind),
        );
    }
    let line: Option<String> = (event.event_type == STATEMENT_FAILED
        && !projection.quiet(start.resource_id.as_deref(), &resource_name))
    .then(|| {
        let suffix: String = query_suffix(statement_query_id(&projection, event).as_ref());
        let context: String = statement_context(&projection, &start);
        format!("{NESTED_PREFIX}statement  {context}  FAIL{suffix}")
    });
    projection.statement_query_ids.remove(statement_id);
    (projection, line)
}

/// Count a finished statement by kind, keeping its query identifier.
pub(crate) fn count_statement(
    mut projection: ProgressProjection,
    event: &ProgressEvent,
) -> ProgressProjection {
    let query_id: Option<String> = statement_query_id(&projection, event);
    *projection
        .statements
        .by_kind
        .entry(event.payload.statement_kind.clone())
        .or_default() += 1;
    projection.statements.query_ids.extend(query_id);
    projection
}
