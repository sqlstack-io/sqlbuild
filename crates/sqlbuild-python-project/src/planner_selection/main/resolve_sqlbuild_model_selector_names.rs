//! One SQLBuild-owned model selector core's model names, and its path selector when translated.

use crate::planner_selection::_helpers::model_selectors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyString;
use pyo3::types::PyTuple;

/// One SQLBuild-owned model selector core's model names, and its path selector when translated.
#[inline]
pub fn resolve_sqlbuild_model_selector_names<'py>(
    project: &Bound<'py, PyAny>,
    term: &Bound<'py, PyString>,
) -> PyResult<Bound<'py, PyTuple>> {
    model_selectors::resolve_sqlbuild_model_selector_names(project, term)
}
