//! Python's syntax validation: Polyglot's `validate` or `parse_one` of the cleaned SQL.

use std::collections::HashMap;

use polyglot_sql::{Dialect, DialectType, ParseOptions, ValidationOptions, validate_with_dialect};

use crate::assembly::project::constants::INVALID_SQL;
use crate::parse_guard::main::polyglot_complexity_guard::polyglot_complexity_guard;
use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;

use crate::assembly::project::models::{SyntaxCheck, SyntaxFailure, SyntaxMode};
use crate::lineage::main::parser_dialect::parser_dialect;
use crate::semantic_validation::main::normalize::normalize_analysis_sql;
use crate::semantic_validation::models::NormalizationInput;

/// Whether Polyglot's `validate` accepts the cleaned SQL; Err is the `ValueError` Python raises.
pub(crate) fn syntax_valid(check: &SyntaxCheck, dialect: &str) -> Result<bool, SyntaxFailure> {
    syntax_error(check, dialect, SyntaxMode::Validate).map(|error| error.is_none())
}

/// Python's syntax error message for one SQL string, None where Polyglot accepts it.
pub(crate) fn syntax_error(
    check: &SyntaxCheck,
    dialect: &str,
    mode: SyntaxMode,
) -> Result<Option<String>, SyntaxFailure> {
    let cleaned: String = normalize_analysis_sql(NormalizationInput {
        sql: check.sql.clone(),
        dialect: dialect.to_owned(),
        stubs: HashMap::new(),
        placeholders: HashMap::new(),
    })
    .map_err(SyntaxFailure::Normalization)?;
    let cleaned: String = if check.placeholders.is_empty() {
        cleaned
    } else {
        placeholder_defaults(&cleaned, &check.placeholders)
    };
    let dialect_type: DialectType = parser_dialect(Some(dialect))
        .map_err(|unknown| SyntaxFailure::UnknownDialect(unknown.to_owned()))?;
    let complexity_guard = polyglot_complexity_guard();
    let polyglot: Dialect = Dialect::get(dialect_type);
    Ok(match mode {
        SyntaxMode::Validate => {
            let options = ValidationOptions {
                complexity_guard: Some(complexity_guard),
                strict_syntax: false,
                semantic: false,
            };
            let result = validate_with_dialect(&cleaned, &polyglot, &options);
            (!result.valid).then(|| {
                result
                    .errors
                    .first()
                    .map_or_else(|| INVALID_SQL.to_owned(), |error| error.message.clone())
            })
        }
        SyntaxMode::Parse => {
            let options = ParseOptions {
                complexity_guard: Some(complexity_guard),
            };
            match polyglot.parse_with_options(&cleaned, &options) {
                Ok(statements) if statements.len() == 1 => None,
                Ok(statements) => Some(format!("Expected 1 statement, found {}", statements.len())),
                Err(error) => Some(error.to_string()),
            }
        }
    })
}
