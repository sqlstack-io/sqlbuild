//! Operation start and terminal rows, and scheduled retries.

use crate::cli_text::models::CliStyle;
use crate::progress::constants::{
    FAILED_SUFFIX, FRESHNESS_METADATA_OPERATION, FRESHNESS_QUERY_OPERATION, NESTED_PREFIX,
    TTY_RESOURCE_OWNER_OPERATIONS, TTY_TRANSIENT_OWNER_OPERATIONS, VISIBLE_OPERATION_LABELS,
};
use crate::progress::models::{ProgressEvent, ProgressProjection};

fn operation_label(operation_name: &str) -> Option<&'static str> {
    VISIBLE_OPERATION_LABELS
        .iter()
        .find(|(name, _)| *name == operation_name)
        .map(|(_, label)| *label)
}

/// ` {attempt}` when the operation metadata carries an attempt number.
fn operation_attempt_label(event: &ProgressEvent) -> String {
    event
        .payload
        .metadata_attempt_number
        .map(|attempt| format!(" {attempt}"))
        .unwrap_or_default()
}

fn nested_prefix(event: &ProgressEvent) -> &'static str {
    if event.resource_attempt_id.is_some() {
        NESTED_PREFIX
    } else {
        ""
    }
}

/// Whether a live terminal owner shows this operation instead of a persistent row.
fn owned_by_live_row(projection: &ProgressProjection, event: &ProgressEvent, name: &str) -> bool {
    projection.settings.is_tty
        && if event.resource_attempt_id.is_none() {
            TTY_TRANSIENT_OWNER_OPERATIONS.contains(&name)
        } else {
            TTY_RESOURCE_OWNER_OPERATIONS.contains(&name)
        }
}

/// Render the start row of a visible operation.
pub(crate) fn consume_operation_start(
    mut projection: ProgressProjection,
    event: &ProgressEvent,
) -> (ProgressProjection, Option<String>) {
    let (Some(operation_id), Some(operation_name)) =
        (&event.operation_id, &event.payload.operation_name)
    else {
        return (projection, None);
    };
    let Some(label) = operation_label(operation_name) else {
        return (projection, None);
    };
    if owned_by_live_row(&projection, event, operation_name) {
        return (projection, None);
    }
    projection
        .operation_starts
        .insert(operation_id.clone(), event.clone());
    if event.resource_attempt_id.is_some() && !projection.settings.debug {
        projection.quiet_operation_ids.insert(operation_id.clone());
        return (projection, None);
    }
    let status: String = CliStyle {
        use_color: projection.settings.use_color,
    }
    .status("START");
    let line: String = format!(
        "{}{label}{}  {status}",
        nested_prefix(event),
        operation_attempt_label(event)
    );
    (projection, Some(line))
}

/// Render the terminal row of a started visible operation.
pub(crate) fn consume_operation_terminal(
    mut projection: ProgressProjection,
    event: &ProgressEvent,
) -> (ProgressProjection, Option<String>) {
    let Some(operation_id) = &event.operation_id else {
        return (projection, None);
    };
    let Some(start) = projection.operation_starts.remove(operation_id) else {
        return (projection, None);
    };
    let failed: bool = event.event_type.ends_with(FAILED_SUFFIX);
    let quiet_resource: bool = projection
        .resource_names
        .get(start.resource_id.as_deref().unwrap_or_default())
        .is_some_and(|name| projection.quiet(start.resource_id.as_deref(), name));
    if projection.quiet_operation_ids.remove(operation_id) && (!failed || quiet_resource) {
        return (projection, None);
    }
    let Some(label) = start
        .payload
        .operation_name
        .as_deref()
        .and_then(operation_label)
    else {
        return (projection, None);
    };
    let status: String = CliStyle {
        use_color: projection.settings.use_color,
    }
    .status(if failed { "FAIL" } else { "OK" });
    let elapsed: String = event
        .payload
        .duration_ms
        .map(|duration| format!("  ({:.2}s)", duration / 1000.0))
        .unwrap_or_default();
    let detail: String = operation_terminal_detail(
        start.payload.operation_name.as_deref().unwrap_or_default(),
        event,
    );
    let line: String = format!(
        "{}{label}{}  {status}{detail}{elapsed}",
        nested_prefix(event),
        operation_attempt_label(&start)
    );
    (projection, Some(line))
}

/// The freshness summary a freshness observation terminal adds.
fn operation_terminal_detail(operation_name: &str, event: &ProgressEvent) -> String {
    let failed: bool = event.event_type.ends_with(FAILED_SUFFIX);
    if operation_name == FRESHNESS_QUERY_OPERATION {
        return if failed {
            " (source freshness unknown)".to_owned()
        } else {
            String::new()
        };
    }
    if operation_name != FRESHNESS_METADATA_OPERATION {
        return String::new();
    }
    let Some(item_count) = event.payload.metadata_item_count else {
        return String::new();
    };
    let noun: &str = if item_count == 1 { "source" } else { "sources" };
    if failed {
        let reason: String = event
            .payload
            .error_type
            .as_ref()
            .map(|error_type| format!(": {error_type}"))
            .unwrap_or_default();
        return format!(" ({item_count} {noun} unknown, metadata lookup failed{reason})");
    }
    let unknown: i64 = event.payload.metadata_unknown_count.unwrap_or(0);
    format!(" ({item_count} {noun}, {unknown} unknown)")
}

/// Render one scheduled retry.
pub(crate) fn retry_scheduled_line(event: &ProgressEvent) -> Option<String> {
    let (Some(failed_attempt), Some(next_attempt), Some(delay_ms)) = (
        event.payload.failed_attempt_number,
        event.payload.next_attempt_number,
        event.payload.delay_ms,
    ) else {
        return None;
    };
    let delay_seconds: f64 = delay_ms / 1000.0;
    Some(format!(
        "{NESTED_PREFIX}retry {failed_attempt}->{next_attempt} in {delay_seconds:.2}s"
    ))
}
