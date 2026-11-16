//! Span rows.

use crate::render::types::SpanRow;
use sqlbuild_analysis::compiled_project::models::SplicedSql;

/// Span rows.
pub fn span_rows(spliced: &SplicedSql) -> Vec<SpanRow> {
    spliced
        .spans
        .iter()
        .map(|span| SpanRow {
            source_start: span.source_start,
            source_end: span.source_end,
            output_start: span.output_start,
            output_end: span.output_end,
        })
        .collect()
}
