//! A span as the expansion semantic validation maps offsets through.

use crate::render::_helpers::calls::macro_call_rows;
use crate::render::types::SpanRow;
use sqlbuild_analysis::semantic_validation::models::Expansion;

/// A span as the expansion semantic validation maps offsets through.
#[inline]
pub fn span_expansion(span: SpanRow) -> Expansion {
    macro_call_rows::span_expansion(span)
}
