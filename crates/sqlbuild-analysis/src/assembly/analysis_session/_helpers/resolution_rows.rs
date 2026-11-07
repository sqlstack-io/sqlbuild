//! A model's diagnostics with SQLQuack's name findings in place of polyglot's.

use crate::assembly::analysis_session::constants::{
    SPELLING_SUGGESTION, SQLQUACK_NAME_CODES, TYPE_CHECK_CODE_RANGES, TYPE_CHECK_CODES,
    WARNING_SEVERITY,
};
use crate::semantic_validation::models::DiagnosticRow;

/// Polyglot's `diagnostics` with SQLQuack's name `findings` in place of its name rows.
///
/// A finding polyglot also reports (same code, line and column) keeps polyglot's severity; any
/// other is a warning, so a new name error ships as a warning first. A polyglot name row SQLQuack
/// does not report is dropped. Every other polyglot row stays, its type rows after the names.
pub(crate) fn with_name_findings(
    diagnostics: Vec<DiagnosticRow>,
    findings: Vec<DiagnosticRow>,
) -> Vec<DiagnosticRow> {
    let (names, others): (Vec<DiagnosticRow>, Vec<DiagnosticRow>) = diagnostics
        .into_iter()
        .partition(|row| name_code(&row.code));
    let (types, mut rows): (Vec<DiagnosticRow>, Vec<DiagnosticRow>) =
        others.into_iter().partition(type_check);
    rows.extend(
        findings
            .into_iter()
            .filter(|row| name_code(&row.code))
            .map(|mut row| {
                let reported: bool = names.iter().any(|name| {
                    name.code == row.code && name.line == row.line && name.column == row.column
                });
                if !reported {
                    WARNING_SEVERITY.clone_into(&mut row.severity);
                }
                row
            }),
    );
    rows.extend(types);
    rows
}

fn name_code(code: &str) -> bool {
    SQLQUACK_NAME_CODES.contains(&code)
}

/// Whether polyglot reports `row` after names: a type, column-count or function check, but not
/// SQLBuild's own spelling check of a built-in.
fn type_check(row: &DiagnosticRow) -> bool {
    let code: &str = &row.code;
    !row.message.contains(SPELLING_SUGGESTION)
        && (TYPE_CHECK_CODES.contains(&code)
            || code
                .strip_prefix('B')
                .and_then(|number| number.parse::<u16>().ok())
                .is_some_and(|number| {
                    TYPE_CHECK_CODE_RANGES
                        .iter()
                        .any(|range| range.contains(&number))
                }))
}
