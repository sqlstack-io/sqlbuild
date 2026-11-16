//! Whether the parsed command writes machine output to stdout, so progress must use stderr.

use crate::build_progress::_helpers::progress;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Whether the parsed command writes machine output to stdout, so progress must use stderr.
#[inline]
pub fn machine_output_mode(args: &Bound<'_, PyAny>) -> PyResult<bool> {
    progress::machine_output_mode(args)
}
