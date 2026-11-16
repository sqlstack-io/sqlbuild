//! Span row.

use crate::render::_helpers::calls::macro_call_rows;
use crate::render::types::SpanRow;
use sqlbuild_render::macro_calls::models::MacroSpliceSpan;

/// Span row.
#[inline]
pub fn span_row(span: MacroSpliceSpan) -> SpanRow {
    macro_call_rows::span_row(span)
}
