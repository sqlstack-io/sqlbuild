//! `Retained (N):`: what each retained scenario kept, before the failed list.

use crate::cli_text::models::CliStyle;
use crate::scenario_command::models::{RetainedRelation, ScenarioResultRow};

fn relation_line(relation: &RetainedRelation, style: CliStyle) -> String {
    format!(
        "    {}{} -> {}",
        style.muted(&format!("{:<10}", relation.kind)),
        relation.logical_name,
        relation.physical_name
    )
}

/// With `--retain` or `--verbose`, each retained scenario's DuckDB file, and with `--retain` its warehouse relations.
#[must_use]
pub fn render_retained_section(
    rows: &[ScenarioResultRow],
    (relations, verbose): (bool, bool),
    style: CliStyle,
) -> String {
    if !relations && !verbose {
        return String::new();
    }
    let retained: Vec<&ScenarioResultRow> = rows
        .iter()
        .filter(|row| row.retained && (row.local_duckdb_path.is_some() || relations))
        .collect();
    if retained.is_empty() {
        return String::new();
    }
    let mut lines: Vec<String> = vec![
        String::new(),
        style.section(&format!("Retained ({}):", retained.len())),
    ];
    for row in retained {
        lines.push(format!("  {}", row.name));
        lines.extend(
            row.local_duckdb_path
                .iter()
                .map(|path| format!("    {}{path}", style.muted("duckdb    "))),
        );
        let kept: &[RetainedRelation] = row.retained_relations.as_deref().unwrap_or_default();
        lines.extend(
            kept.iter()
                .filter(|_| relations)
                .map(|relation| relation_line(relation, style)),
        );
    }
    format!("{}\n", lines.join("\n"))
}
