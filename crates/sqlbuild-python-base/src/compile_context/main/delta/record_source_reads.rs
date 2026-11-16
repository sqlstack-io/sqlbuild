//! Record sources a resource other than a model reads, when a capture is running.

use crate::compile_context::_helpers::delta;

/// Record sources a resource other than a model reads, when a capture is running.
#[inline]
pub fn record_source_reads<'a>(names: impl IntoIterator<Item = &'a str>) {
    delta::record_source_reads(names)
}
