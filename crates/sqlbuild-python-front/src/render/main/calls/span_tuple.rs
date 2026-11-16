//! A span as the tuple Python reads.

use crate::render::_helpers::calls::macro_call_rows;
use crate::render::models::SpanTuple;
use crate::render::types::SpanRow;

/// A span as the tuple Python reads.
#[inline]
pub fn span_tuple(span: &SpanRow) -> SpanTuple {
    macro_call_rows::span_tuple(span)
}
