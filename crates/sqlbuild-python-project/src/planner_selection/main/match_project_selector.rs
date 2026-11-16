//! The keys one parsed selector matches before `+` graph expansion.

use crate::planner_selection::_helpers::selectors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyFrozenSet;

/// The keys one parsed selector matches before `+` graph expansion.
#[inline]
pub fn match_project_selector<'py>(
    parsed: &Bound<'py, PyAny>,
    all_keys: &Bound<'py, PyAny>,
    tag_index: &Bound<'py, PyAny>,
    path_index: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    selectors::match_project_selector(parsed, all_keys, tag_index, path_index)
}
