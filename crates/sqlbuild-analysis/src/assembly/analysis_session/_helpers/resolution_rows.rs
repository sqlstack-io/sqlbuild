//! Name-resolution diagnostics for routed models: SQLQuack's findings in polyglot's place.

use crate::assembly::analysis_session::constants::{RESOLUTION_CODES, WARNING_SEVERITY};
use crate::semantic_validation::models::DiagnosticRow;

/// `diagnostics` with its name-resolution rows replaced by SQLQuack's `resolved`, in the place of
/// the first one, else after the rest.
///
/// A SQLQuack row polyglot does not also report (same code, line and column) is a warning: new
/// errors ship as warnings first (D30). A polyglot row SQLQuack does not report is dropped.
pub(crate) fn with_resolution_rows(
    diagnostics: Vec<DiagnosticRow>,
    resolved: Vec<DiagnosticRow>,
) -> Vec<DiagnosticRow> {
    let resolution = |row: &DiagnosticRow| RESOLUTION_CODES.contains(&row.code.as_str());
    let first: Option<usize> = diagnostics.iter().position(resolution);
    let (known, mut rows): (Vec<DiagnosticRow>, Vec<DiagnosticRow>) =
        diagnostics.into_iter().partition(resolution);
    let resolved: Vec<DiagnosticRow> = resolved
        .into_iter()
        .map(|row| {
            let reported: bool = known.iter().any(|known| {
                known.code == row.code && known.line == row.line && known.column == row.column
            });
            match reported {
                true => row,
                false => DiagnosticRow {
                    severity: WARNING_SEVERITY.to_owned(),
                    ..row
                },
            }
        })
        .collect();
    let at: usize = first.unwrap_or(rows.len()).min(rows.len());
    rows.splice(at..at, resolved);
    rows
}
