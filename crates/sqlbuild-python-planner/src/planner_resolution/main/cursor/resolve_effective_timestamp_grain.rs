//! Resolve effective timestamp grain.

use crate::planner_resolution::_helpers::cursor::cursor_windows;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Resolve effective timestamp grain.
#[inline]
pub fn resolve_effective_timestamp_grain<'py>(
    cursor_type: &Bound<'py, PyAny>,
    downstream_grain: &Bound<'py, PyAny>,
    cursor_input_grains: &Bound<'py, PyAny>,
    microbatch_strategy: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    cursor_windows::resolve_effective_timestamp_grain(
        cursor_type,
        downstream_grain,
        cursor_input_grains,
        microbatch_strategy,
    )
}
