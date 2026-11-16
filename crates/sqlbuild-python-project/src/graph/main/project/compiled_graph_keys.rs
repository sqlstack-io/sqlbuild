//! `CompiledObjectKey`s for native graph key pairs.

use crate::graph::models::KeyCache;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyFrozenSet;
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// `CompiledObjectKey`s for native graph key pairs.
#[pyfunction]
pub fn compiled_graph_keys<'py>(
    py: Python<'py>,
    keys: Vec<ObjectKey>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let cache: KeyCache<'py> = KeyCache::new(py)?;
    PyFrozenSet::new(py, cache.keys(&keys)?)
}
