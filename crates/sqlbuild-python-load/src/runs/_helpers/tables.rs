//! Which required destination tables a plan leaves out, and the plan lines SQLBuild shows.

use sqlload::loader::models::ProjectPlan;

use crate::runs::constants::{PLAN_SUMMARY_MARKS, SQLLOAD_COMMAND};

/// The destination table of every table `plan` loads or leaves as it is, in plan order.
pub(crate) fn planned_tables(plan: &ProjectPlan) -> Vec<String> {
    plan.tables
        .iter()
        .map(|table| table.spec.destination.table.clone())
        .chain(plan.skipped.iter().map(|skipped| {
            skipped
                .destination
                .rsplit('.')
                .next()
                .unwrap_or_default()
                .to_owned()
        }))
        .collect()
}

/// The names in `required` that `planned` doesn't hold, in `required`'s order.
pub(crate) fn missing_tables(required: &[String], planned: &[String]) -> Vec<String> {
    required
        .iter()
        .filter(|table| !planned.contains(table))
        .cloned()
        .collect()
}

/// `sqlload plan`'s lines as shown under a load: no blank lines, `sqlload` command hints or closing summary.
pub(crate) fn shown_plan_lines(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .filter(|line| !line.trim().is_empty() && !is_summary(line))
        .map(|line| without_command(&line).to_owned())
        .collect()
}

fn is_summary(line: &str) -> bool {
    PLAN_SUMMARY_MARKS.iter().any(|mark| line.starts_with(mark))
}

fn without_command(line: &str) -> &str {
    match line.rfind(SQLLOAD_COMMAND) {
        Some(index)
            if line[index + SQLLOAD_COMMAND.len()..]
                .chars()
                .all(|character| character.is_ascii_lowercase()) =>
        {
            line[..index].trim_end()
        }
        _ => line,
    }
}
