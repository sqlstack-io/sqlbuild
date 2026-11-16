//! Span row.

use crate::render::types::SpanRow;
use sqlbuild_render::macro_calls::models::MacroSpliceSpan;

/// Span row.
pub fn span_row(span: MacroSpliceSpan) -> SpanRow {
    SpanRow {
        source_start: span.source_start,
        source_end: span.source_end,
        output_start: span.output_start,
        output_end: span.output_end,
    }
}
