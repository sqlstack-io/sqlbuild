//! The verbose build execution context.

use crate::build_command::_helpers::text::{display_value, sanitize};
use crate::build_command::constants::{CONTEXT_LABEL_WIDTH, NOT_SET};
use crate::build_command::models::RunContextFacts;
use crate::cli_text::models::{CliDocument, CliStyle};
use crate::number_text::main::format_thousands::format_thousands;

/// `true`, the models of the named jobs that rebuild theirs from scratch, or `false`.
fn full_refresh(facts: &RunContextFacts) -> String {
    match facts.full_refresh_jobs.as_slice() {
        _ if facts.full_refresh => true.to_string(),
        [] => false.to_string(),
        [job] => format!("models of job {}", sanitize(job)),
        jobs => format!("models of jobs {}", sanitize(&jobs.join(", "))),
    }
}

fn date_vars(facts: &RunContextFacts) -> String {
    match &facts.date_vars {
        (None, None) => NOT_SET.to_owned(),
        (None, Some(end)) => format!("end_date={}", sanitize(end)),
        (Some(start), None) => format!("start_date={}", sanitize(start)),
        (Some(start), Some(end)) => format!("{} to {}", sanitize(start), sanitize(end)),
    }
}

/// The resolved placement, mode and selection, then the selector files.
#[must_use]
pub fn render_run_context(facts: &RunContextFacts, style: CliStyle) -> String {
    let warehouse: String = facts.warehouse.as_ref().map_or_else(
        || NOT_SET.to_owned(),
        |(warehouse, source)| format!("{} ({source})", sanitize(warehouse)),
    );
    let rows: Vec<(String, String)> = [
        ("command", sanitize(&facts.command)),
        ("run_id", display_value(facts.run_id.as_deref())),
        ("target", display_value(facts.target.as_deref())),
        ("database", display_value(facts.database.as_deref())),
        ("schema", display_value(facts.schema.as_deref())),
        ("warehouse", warehouse),
        (
            "concurrency",
            if facts.concurrency_source.is_empty() {
                format!("{} configured limit", facts.concurrency)
            } else {
                format!(
                    "{} configured limit ({})",
                    facts.concurrency,
                    sanitize(&facts.concurrency_source)
                )
            },
        ),
        ("full_refresh", full_refresh(facts)),
        (
            "selected",
            format!(
                "{} of {} build resources",
                format_thousands(facts.selected_count as i128),
                format_thousands(facts.total_count as i128)
            ),
        ),
        ("date vars", date_vars(facts)),
    ]
    .into_iter()
    .map(|(label, value)| (label.to_owned(), value))
    .collect();
    let mut document: CliDocument = CliDocument::new(style);
    document.header("Execution", None);
    document.fields(&rows, Some(CONTEXT_LABEL_WIDTH));
    if !facts.selector_files.is_empty() {
        document.lines.push(String::new());
        document.header("Selection files", None);
        for file in &facts.selector_files {
            let noun: &str = if file.selector_count == 1 {
                "selector"
            } else {
                "selectors"
            };
            document.fields(
                &[(
                    "selector_file".to_owned(),
                    format!(
                        "{} ({} {noun})",
                        sanitize(&file.path),
                        format_thousands(file.selector_count as i128)
                    ),
                )],
                Some(CONTEXT_LABEL_WIDTH),
            );
        }
    }
    document.lines.push(String::new());
    document.render(true)
}
