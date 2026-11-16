//! A span as the expansion semantic validation maps offsets through.

use crate::render::types::SpanRow;
use sqlbuild_analysis::semantic_validation::models::Expansion;

/// A span as the expansion semantic validation maps offsets through.
pub fn span_expansion(span: SpanRow) -> Expansion {
    Expansion {
        source_start: span.source_start,
        source_end: span.source_end,
        output_start: span.output_start,
        output_end: span.output_end,
    }
}
