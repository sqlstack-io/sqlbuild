//! Build source load entries.

use crate::planner_scope::_helpers::sources;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::{PyDict, PyTuple};

/// Build source load entries.
#[inline]
pub fn build_source_load_entries<'py>(
    execution_order: &Bound<'py, PyAny>,
    selected_keys: &Bound<'py, PyAny>,
    source_map: &Bound<'py, PyDict>,
    is_reload: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    sources::build_source_load_entries(execution_order, selected_keys, source_map, is_reload)
}
