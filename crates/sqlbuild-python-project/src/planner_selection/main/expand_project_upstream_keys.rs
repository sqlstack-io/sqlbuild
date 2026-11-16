//! The upstream key closure of one compiled graph key.

use crate::planner_selection::_helpers::selectors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyFrozenSet;

/// The upstream key closure of one compiled graph key.
#[inline]
pub fn expand_project_upstream_keys<'py>(
    key: Bound<'py, PyAny>,
    upstream_deps: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    selectors::expand_project_upstream_keys(key, upstream_deps)
}
