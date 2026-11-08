use crate::semantic_validation::models::DiagnosticRow;

pub fn binding_diagnostics(
    sql: &str,
    dialect: &str,
    rows: Vec<DiagnosticRow>,
) -> Result<Vec<DiagnosticRow>, String> {
    crate::semantic_validation::_helpers::diagnostics::binding_diagnostics(sql, dialect, rows)
}

/// SQLBuild's code for an analysis finding: E codes directly, implicit-conversion W codes through
/// their blocking B codes; `None` for a code SQLBuild does not report.
#[must_use]
pub fn sqlbuild_code(code: &str) -> Option<String> {
    crate::semantic_validation::_helpers::diagnostics::sqlbuild_code(code)
}
