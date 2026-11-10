//! Places for native cast and set-operation findings that carry no span, as Legacy placed them.

use polyglot_sql::ValidationError;

use crate::lineage::main::parser_dialect::parser_dialect;
use crate::semantic_validation::_helpers::type_locations::locate_type_findings;
use crate::semantic_validation::types::SpannedFinding;

/// Each `(code, message, span)` finding's span, a span-less type finding placed at its expression.
pub fn located_type_spans(
    sql: &str,
    dialect: &str,
    findings: &[SpannedFinding<'_>],
) -> Vec<Option<(usize, usize)>> {
    let mut errors: Vec<ValidationError> = findings.iter().map(validation_error).collect();
    let unplaced: Vec<Option<(usize, usize)>> = findings.iter().map(|(_, _, span)| *span).collect();
    match parser_dialect(Some(dialect))
        .map_err(str::to_owned)
        .and_then(|dialect| locate_type_findings(sql, dialect, &mut errors))
    {
        Ok(()) => errors
            .iter()
            .map(|error| error.start.zip(error.end))
            .collect(),
        Err(_) => unplaced,
    }
}

/// A finding as the validation error type placement fills in, with any span it already has.
fn validation_error((code, message, span): &SpannedFinding<'_>) -> ValidationError {
    ValidationError {
        start: span.map(|(start, _)| start),
        end: span.map(|(_, end)| end),
        ..ValidationError::error(*message, *code)
    }
}
