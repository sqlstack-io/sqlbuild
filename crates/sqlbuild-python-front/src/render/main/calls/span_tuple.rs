//! A span as the tuple Python reads.

use crate::render::models::SpanTuple;
use crate::render::types::SpanRow;

/// A span as the tuple Python reads.
pub fn span_tuple(span: &SpanRow) -> SpanTuple {
    SpanTuple {
        source_start: span.source_start,
        source_end: span.source_end,
        output_start: span.output_start,
        output_end: span.output_end,
    }
}
