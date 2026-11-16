//! Span rows.

use crate::render::_helpers::declarations::declaration_expansion;
use crate::render::types::SpanRow;
use sqlbuild_analysis::compiled_project::models::SplicedSql;

/// Span rows.
#[inline]
pub fn span_rows(spliced: &SplicedSql) -> Vec<SpanRow> {
    declaration_expansion::span_rows(spliced)
}
