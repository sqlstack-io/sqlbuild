//! Resource attempt rows: starts, terminals, and the terminals a result callback claims.

use std::collections::VecDeque;

use crate::cli_text::models::CliStyle;
use crate::progress::constants::{
    AUDIT_OUTCOME_STATUSES, AUDIT_RESOURCE_KIND, AUDIT_RESOURCE_PREFIX, FAILED_SUFFIX, KIND_WIDTH,
    SCENARIO_RESOURCE_PREFIX, SKIPPED_SUFFIX,
};
use crate::progress::models::{ProgressEvent, ProgressProjection};
use crate::terminal_events::models::TerminalRequest;

/// The resource name, prefixed by its scenario when the resource runs inside one.
fn display_resource_name(resource_id: Option<&str>, resource_name: &str) -> String {
    let Some(scenario) = resource_id.and_then(|id| id.strip_prefix(SCENARIO_RESOURCE_PREFIX))
    else {
        return resource_name.to_owned();
    };
    let scenario_name: &str = scenario.split(':').next().unwrap_or(scenario);
    format!("{scenario_name}/{resource_name}")
}

/// `name (target.column)` from an audit identifier `audit:<name>:<kind>[:<target>[:<column>]]`.
fn audit_display_name(resource_id: Option<&str>, resource_name: &str) -> String {
    let Some(identity) = resource_id.and_then(|id| id.strip_prefix(AUDIT_RESOURCE_PREFIX)) else {
        return resource_name.to_owned();
    };
    let attached: Vec<&str> = identity.split(':').skip(2).collect();
    if attached.is_empty() {
        return resource_name.to_owned();
    }
    format!("{resource_name} ({})", attached.join("."))
}

/// The row name of one resource: audits name their target, scenario resources their scenario.
fn resource_display_name(kind: &str, resource_id: Option<&str>, resource_name: &str) -> String {
    if kind == AUDIT_RESOURCE_KIND {
        audit_display_name(resource_id, resource_name)
    } else {
        display_resource_name(resource_id, resource_name)
    }
}

/// Remember an attempt, keeping the position of its first sighting.
fn store_attempt(
    mut projection: ProgressProjection,
    attempt_id: &str,
    event: &ProgressEvent,
) -> ProgressProjection {
    if projection
        .attempts_by_id
        .insert(attempt_id.to_owned(), event.clone())
        .is_none()
    {
        projection.attempt_order.push(attempt_id.to_owned());
    }
    projection
}

fn remember_resource(
    mut projection: ProgressProjection,
    resource_name: &str,
    resource_id: &str,
) -> ProgressProjection {
    projection
        .resource_ids_by_name
        .entry(resource_name.to_owned())
        .or_default()
        .insert(resource_id.to_owned());
    projection
}

/// Render the start row of one resource attempt.
pub(crate) fn consume_resource_start(
    projection: ProgressProjection,
    event: &ProgressEvent,
) -> (ProgressProjection, Vec<String>) {
    let (Some(attempt_id), Some(resource_id), Some(resource_name), Some(resource_kind)) = (
        &event.resource_attempt_id,
        &event.resource_id,
        &event.payload.resource_name,
        &event.payload.resource_kind,
    ) else {
        return (projection, Vec::new());
    };
    let mut projection: ProgressProjection =
        remember_resource(projection, resource_name, resource_id);
    projection
        .resource_names
        .insert(resource_id.clone(), resource_name.clone());
    let (projection, mut lines) = render_prior_attempts(projection, (resource_id, resource_name));
    let projection: ProgressProjection = store_attempt(projection, attempt_id, event);
    if projection.unstarted_resource_names.contains(resource_name)
        || projection.quiet(Some(resource_id.as_str()), resource_name)
    {
        return (projection, lines);
    }
    if projection.settings.is_tty
        && (projection.enriched_resource_names.contains(resource_name)
            || !projection.settings.debug)
    {
        return (projection, lines);
    }
    let counter: String = match projection.resource_ordinals.get(resource_name) {
        Some(ordinal) if projection.resource_total > 0 => {
            format!("{ordinal}/{}  ", projection.resource_total)
        }
        _ => String::new(),
    };
    let status: String = CliStyle {
        use_color: projection.settings.use_color,
    }
    .status("START");
    let display_name: String =
        resource_display_name(resource_kind, Some(resource_id), resource_name);
    lines.push(format!(
        "  {counter}{resource_kind:<KIND_WIDTH$}{display_name} {status}"
    ));
    (projection, lines)
}

/// Record one resource terminal, rendering it unless a result callback enriches it.
pub(crate) fn consume_resource_terminal(
    projection: ProgressProjection,
    event: &ProgressEvent,
) -> (ProgressProjection, Vec<String>) {
    let (Some(attempt_id), Some(resource_id), Some(resource_name)) = (
        &event.resource_attempt_id,
        &event.resource_id,
        &event.payload.resource_name,
    ) else {
        return (projection, Vec::new());
    };
    let projection: ProgressProjection = remember_resource(projection, resource_name, resource_id);
    let mut projection: ProgressProjection = store_attempt(projection, attempt_id, event);
    projection
        .pending_terminal_ids_by_resource
        .entry(resource_id.clone())
        .or_default()
        .push_back(attempt_id.clone());
    if projection.enriched_resource_names.contains(resource_name)
        || projection.quiet(Some(resource_id.as_str()), resource_name)
    {
        return (projection, Vec::new());
    }
    let (projection, line) = render_resource_terminal(projection, event, resource_name);
    (projection, line.into_iter().collect())
}

/// Render the terminal row of one attempt once.
pub(crate) fn render_resource_terminal(
    mut projection: ProgressProjection,
    event: &ProgressEvent,
    resource_name: &str,
) -> (ProgressProjection, Option<String>) {
    let Some(attempt_id) = &event.resource_attempt_id else {
        return (projection, None);
    };
    if projection.quiet(event.resource_id.as_deref(), resource_name) {
        return (projection, None);
    }
    if !projection.rendered_terminal_ids.insert(attempt_id.clone()) {
        return (projection, None);
    }
    let Some(resource_kind) = &event.payload.resource_kind else {
        return (projection, None);
    };
    let audit: bool = resource_kind == AUDIT_RESOURCE_KIND;
    let recorded: Option<&str> = projection
        .audit_outcomes
        .get(attempt_id)
        .and_then(|outcome| {
            AUDIT_OUTCOME_STATUSES
                .iter()
                .find(|(value, _)| value == outcome)
                .map(|(_, status)| *status)
        });
    let status_text: &str = if event.event_type.ends_with(FAILED_SUFFIX) {
        "FAIL"
    } else if event.event_type.ends_with(SKIPPED_SUFFIX) {
        "SKIP"
    } else if let Some(status) = recorded {
        status
    } else if audit {
        "PASS"
    } else {
        "OK"
    };
    let status: String = CliStyle {
        use_color: projection.settings.use_color,
    }
    .status(status_text);
    let elapsed: String = event
        .payload
        .duration_ms
        .map(|duration| format!(" {:.2}s", duration / 1000.0))
        .unwrap_or_default();
    let display_name: String =
        resource_display_name(resource_kind, event.resource_id.as_deref(), resource_name);
    let line: String = format!("  {resource_kind:<KIND_WIDTH$}{display_name} {status}{elapsed}");
    (projection, Some(line))
}

/// Render each event's terminal row once, as reported under `resource_name`.
pub(crate) fn render_resource_terminals(
    projection: ProgressProjection,
    events: &[ProgressEvent],
    resource_name: &str,
) -> (ProgressProjection, Vec<String>) {
    let mut projection: ProgressProjection = projection;
    let mut lines: Vec<String> = Vec::new();
    for event in events {
        let (rendered, line) = render_resource_terminal(projection, event, resource_name);
        projection = rendered;
        lines.extend(line);
    }
    (projection, lines)
}

/// Render every unclaimed terminal of earlier attempts before a new attempt starts.
fn render_prior_attempts(
    mut projection: ProgressProjection,
    (resource_id, resource_name): (&str, &str),
) -> (ProgressProjection, Vec<String>) {
    let pending: VecDeque<String> = projection
        .pending_terminal_ids_by_resource
        .remove(resource_id)
        .unwrap_or_default();
    let events: Vec<ProgressEvent> = pending
        .iter()
        .filter_map(|attempt_id| projection.attempts_by_id.get(attempt_id).cloned())
        .collect();
    render_resource_terminals(projection, &events, resource_name)
}

/// The resource identifier a request names, or the only one seen for its name.
fn resolved_resource_id(
    projection: &ProgressProjection,
    request: &TerminalRequest,
) -> Option<String> {
    if let Some(resource_id) = request.resource_id.as_ref().filter(|id| !id.is_empty()) {
        return Some(resource_id.clone());
    }
    let resource_ids = projection
        .resource_ids_by_name
        .get(&request.resource_name)?;
    if resource_ids.len() != 1 {
        return None;
    }
    resource_ids.iter().next().cloned()
}

/// The unrendered pending terminals of the requested resource, and the one the request selects.
pub(crate) fn resolve_resource_terminal(
    projection: &ProgressProjection,
    request: &TerminalRequest,
) -> Option<(String, Vec<ProgressEvent>, usize)> {
    let resource_id: String = resolved_resource_id(projection, request)?;
    let candidates: Vec<ProgressEvent> = projection
        .pending_terminal_ids_by_resource
        .get(&resource_id)
        .into_iter()
        .flatten()
        .filter(|attempt_id| !projection.rendered_terminal_ids.contains(*attempt_id))
        .filter_map(|attempt_id| projection.attempts_by_id.get(attempt_id).cloned())
        .collect();
    let target: usize = if let Some(attempt_id) = &request.resource_attempt_id {
        candidates
            .iter()
            .position(|event| event.resource_attempt_id.as_ref() == Some(attempt_id))?
    } else if let Some(attempt_number) = request.attempt_number {
        candidates
            .iter()
            .rposition(|event| event.payload.attempt_number == Some(attempt_number))?
    } else {
        candidates.len().checked_sub(1)?
    };
    Some((resource_id, candidates, target))
}

/// Drop pending terminals up to and including the claimed one.
pub(crate) fn remove_claimed_terminals(
    mut projection: ProgressProjection,
    resource_id: &str,
    target_attempt_id: &str,
) -> ProgressProjection {
    if let Some(pending) = projection
        .pending_terminal_ids_by_resource
        .get_mut(resource_id)
    {
        while let Some(attempt_id) = pending.pop_front() {
            if attempt_id == target_attempt_id {
                break;
            }
        }
    }
    projection
}
