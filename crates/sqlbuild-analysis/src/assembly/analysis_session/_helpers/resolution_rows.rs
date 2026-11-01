//! Diagnostics for routed models: SQLQuack's findings in polyglot's place, for the codes it owns.

use crate::assembly::analysis_session::constants::{
    SCOPE_RULE_PHASE, SPELLING_SUGGESTION, SQLQUACK_CODE_RANGES, SQLQUACK_CODES,
    SQLQUACK_NAME_CODES, WARNING_SEVERITY,
};
use crate::semantic_validation::models::DiagnosticRow;

/// A SQLQuack row with the phase polyglot would report it in.
type PhasedRow = (u8, DiagnosticRow);

/// Whether SQLQuack reports the diagnostic `row` for routed models, only name codes when
/// `names_only`; SQLBuild's own spelling check of built-ins stays.
pub(crate) fn sqlquack_owns(row: &DiagnosticRow, names_only: bool) -> bool {
    let code: &str = &row.code;
    if row.message.contains(SPELLING_SUGGESTION) {
        return false;
    }
    if names_only {
        return SQLQUACK_NAME_CODES.contains(&code);
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
/// With `names_only` (a dialect whose types SQLQuack does not check yet), SQLQuack replaces only
/// name findings and polyglot keeps every other row, its type rows after the names as before.
pub(crate) fn with_resolution_rows(
    diagnostics: Vec<DiagnosticRow>,
    findings: Vec<(u8, DiagnosticRow)>,
    names_only: bool,
) -> Vec<DiagnosticRow> {
    let (known, others): (Vec<DiagnosticRow>, Vec<DiagnosticRow>) = diagnostics
        .into_iter()
        .partition(|row| sqlquack_owns(row, names_only));
    let findings: Vec<(u8, DiagnosticRow)> = findings
        .into_iter()
        .filter(|(_, row)| !names_only || SQLQUACK_NAME_CODES.contains(&row.code.as_str()))
        .collect();
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
    let (kept_types, others): (Vec<DiagnosticRow>, Vec<DiagnosticRow>) = others
        .into_iter()
        .partition(|row| names_only && sqlquack_owns(row, false));
    let mut rows: Vec<DiagnosticRow> = rules.into_iter().map(|(_, row)| checked(row)).collect();
    rows.extend(others);
    rows.extend(rest.into_iter().map(|(_, row)| checked(row)));
    rows.extend(kept_types);
    rows
}
