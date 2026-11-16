//! Upstream loader dependency names.

use crate::planner_scope::_helpers::loaders;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Upstream loader dependency names.
#[inline]
pub fn upstream_loader_dependency_names<'py>(
    loader_function: &Bound<'py, PyAny>,
    loader_functions: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    loaders::upstream_loader_dependency_names(loader_function, loader_functions)
}
