//! Diagnostics for routed models: SQLQuack's findings in polyglot's place, for the codes it owns.

use crate::assembly::analysis_session::constants::{
    SCOPE_RULE_PHASE, SPELLING_SUGGESTION, SQLQUACK_CODE_RANGES, SQLQUACK_CODES, WARNING_SEVERITY,
};
use crate::semantic_validation::models::DiagnosticRow;

/// A SQLQuack row with the phase polyglot would report it in.
type PhasedRow = (u8, DiagnosticRow);

/// Whether SQLQuack reports the diagnostic `row` for routed models; SQLBuild's own spelling
/// check of built-ins stays.
pub(crate) fn sqlquack_owns(row: &DiagnosticRow) -> bool {
    let code: &str = &row.code;
    if row.message.contains(SPELLING_SUGGESTION) {
        return false;
    }
    SQLQUACK_CODES.contains(&code)
        || code
            .strip_prefix('B')
            .and_then(|number| number.parse::<u16>().ok())
            .is_some_and(|number| {
                SQLQUACK_CODE_RANGES
                    .iter()
                    .any(|range| range.contains(&number))
            })
}

/// `diagnostics` with the rows of the codes SQLQuack owns replaced by its `findings`, in
/// polyglot's order: scope rules, then polyglot's other rows, then names and types.
///
/// A SQLQuack row polyglot does not also report (same code, line and column) is a warning: new
/// errors ship as warnings first (D30). A polyglot row SQLQuack does not report is dropped.
pub(crate) fn with_resolution_rows(
    diagnostics: Vec<DiagnosticRow>,
    findings: Vec<(u8, DiagnosticRow)>,
) -> Vec<DiagnosticRow> {
    let (known, others): (Vec<DiagnosticRow>, Vec<DiagnosticRow>) =
        diagnostics.into_iter().partition(sqlquack_owns);
    let checked = |row: DiagnosticRow| {
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
    };
    let (rules, rest): (Vec<PhasedRow>, Vec<PhasedRow>) = findings
        .into_iter()
        .partition(|(phase, _)| *phase == SCOPE_RULE_PHASE);
    let mut rows: Vec<DiagnosticRow> = rules.into_iter().map(|(_, row)| checked(row)).collect();
    rows.extend(others);
    rows.extend(rest.into_iter().map(|(_, row)| checked(row)));
    rows
}
