//! Keep one source's described SQL hash and column names when a build armed the capture.

use crate::observations::_helpers::source_capture;

/// Keep one source's described SQL hash and column names when a build armed the capture.
#[inline]
pub fn capture_source_columns(source: &str, sql: &str, columns: &[String]) {
    source_capture::capture_source_columns(source, sql, columns)
}
