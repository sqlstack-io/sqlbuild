//! Log how long the delta has run, for profiling.

use crate::compile_command::main::invocation::reuse_debug::reuse_debug;
use pyo3::prelude::{PyResult, Python};

/// Log how long the delta has run, for profiling.
pub fn trace(py: Python<'_>, phase: &str, started: std::time::Instant) -> PyResult<()> {
    let milliseconds: u128 = started.elapsed().as_millis();
    reuse_debug(
        py,
        &format!("Delta compile {phase} after {milliseconds} ms"),
    )
}
