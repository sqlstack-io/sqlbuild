//! Format command and execution context for a progress header.

use pyo3::pyfunction;
use sqlbuild_core::progress::main::format_execution_header::format_execution_header as format_header;

/// Format command and execution context for a progress header.
#[pyfunction]
#[pyo3(signature = (*, command, target, concurrency))]
pub fn format_execution_header(command: &str, target: Option<&str>, concurrency: i64) -> String {
    format_header(command, target, concurrency)
}
