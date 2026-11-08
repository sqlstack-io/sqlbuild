//! A SQL scenario classified natively, as the compile attachment reads it.

use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::analysis_route::types::NativeDialect;
use crate::compiler::_helpers;
use crate::compiler::models::SqlScenarioCtes;

/// The scenario's classified CTEs, or its error; `parser` is the project's native parser.
pub fn extract_scenario(
    sql: &str,
    file_label: &str,
    (syntax, parser): (&LexicalSyntax, Option<&dyn NativeDialect>),
) -> Result<SqlScenarioCtes, String> {
    _helpers::sql_tests::scenario_extraction::extract_scenario(sql, file_label, (syntax, parser))
}
