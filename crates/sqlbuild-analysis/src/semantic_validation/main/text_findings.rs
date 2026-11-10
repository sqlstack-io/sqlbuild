//! SQLBuild's own checks of SQL text that every analysis route adds to the parse's findings.

use crate::lineage::main::parser_dialect::parser_dialect;
use crate::semantic_validation::_helpers::catalog::diagnostic_row;
use crate::semantic_validation::_helpers::function_names::unsupported_function_errors;
use crate::semantic_validation::_helpers::sqlite_temporal_casts::sqlite_temporal_cast_warnings;
use crate::semantic_validation::_helpers::truth_tests::unsupported_truth_test_errors;
use crate::semantic_validation::models::DiagnosticRow;

/// Built-in spellings the dialect lacks (B101), truth tests it rejects (B103) and SQLite's temporal casts (B104).
pub fn text_findings(sql: &str, dialect: &str) -> Vec<DiagnosticRow> {
    let Ok(dialect) = parser_dialect(Some(dialect)) else {
        return Vec::new();
    };
    unsupported_function_errors(sql, dialect)
        .into_iter()
        .chain(unsupported_truth_test_errors(sql, dialect))
        .chain(sqlite_temporal_cast_warnings(sql, dialect))
        .map(diagnostic_row)
        .collect()
}
