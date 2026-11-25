//! The result row of one finished scenario run.

use crate::cli_text::models::CliStyle;
use crate::run_on::main::run_on_column::run_on_column;
use crate::scenario_command::_helpers::expectations::{expectation_failures, expectation_rows};
use crate::scenario_command::_helpers::rows::{heading, indented_error_lines};
use crate::scenario_command::constants::RETAINED_KIND_WIDTH;
use crate::scenario_command::models::ScenarioResultRow;

/// The status row, its error, expectations, retained artifacts and failure details.
#[must_use]
pub fn render_scenario_result(row: &ScenarioResultRow, style: CliStyle) -> String {
    let duration: String = row
        .duration_ms
        .map(|milliseconds| format!("  {:.2}s", milliseconds / 1000.0))
        .unwrap_or_default();
    let run_on: &str = if row.local { "local" } else { "warehouse" };
    let status: String = format!(
        "{}  {}",
        style.status(&row.status),
        run_on_column(run_on).trim_end()
    );
    let mut text: String = heading(&row.name, &status, &duration);
    if let Some(error) = row.error.as_ref().filter(|error| !error.message.is_empty()) {
        text.push_str(&indented_error_lines(error, style));
        if !row.retained && !row.local {
            text.push_str("    Rerun with --retain to inspect scenario-owned artifacts.\n");
        }
    }
    text.push_str(&expectation_rows(row, style));
    if row.retained {
        if let Some(relations) = &row.retained_relations {
            text.push_str("    Retained relations:\n");
            for relation in relations {
                text.push_str(&format!(
                    "      {:<RETAINED_KIND_WIDTH$} {} -> {}\n",
                    relation.kind, relation.logical_name, relation.physical_name
                ));
            }
        }
        if let Some(path) = &row.local_duckdb_path {
            text.push_str(&format!("    Retained local DuckDB: {path}\n"));
        }
    }
    text.push_str(&expectation_failures(row));
    text
}
