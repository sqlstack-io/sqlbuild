use polyglot_sql::{DialectType, ValidationResult};

pub(crate) fn map_diagnostics(
    sql: &str,
    dialect: DialectType,
    result: ValidationResult,
) -> Result<ValidationResult, String> {
    crate::semantic_validation::_helpers::diagnostics::map_diagnostics(sql, dialect, result)
}
