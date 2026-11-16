//! `CompiledObjectKey`s for native graph key pairs.

use crate::graph::_helpers::project::views;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyFrozenSet;
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// `CompiledObjectKey`s for native graph key pairs.
#[inline]
pub fn compiled_graph_keys<'py>(
    py: Python<'py>,
    keys: Vec<ObjectKey>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    views::compiled_graph_keys(py, keys)
}
