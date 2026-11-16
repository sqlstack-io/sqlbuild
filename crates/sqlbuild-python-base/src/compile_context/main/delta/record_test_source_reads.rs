//! Record sources an SQL test file reads, when a capture is running.

use crate::compile_context::_helpers::delta;

/// Record sources an SQL test file reads, when a capture is running.
#[inline]
pub fn record_test_source_reads<'a>(relative_path: &str, names: impl IntoIterator<Item = &'a str>) {
    delta::record_test_source_reads(relative_path, names)
}
