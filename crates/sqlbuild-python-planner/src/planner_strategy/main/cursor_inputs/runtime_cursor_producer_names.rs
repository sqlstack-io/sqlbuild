//! Selected models and seeds, and selected sources with loaders, whose relations this invocation produces.

use crate::planner_strategy::_helpers::cursor_inputs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyFrozenSet;

/// Selected models and seeds, and selected sources with loaders, whose relations this invocation produces.
#[inline]
pub fn runtime_cursor_producer_names<'py>(
    scope: &Bound<'py, PyAny>,
    source_map: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    cursor_inputs::runtime_cursor_producer_names(scope, source_map)
}
