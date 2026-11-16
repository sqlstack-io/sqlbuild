//! Edge dict.

use crate::graph::models::KeyCache;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// Edge dict.
pub fn edge_dict<'py>(
    cache: &KeyCache<'py>,
    edges: &[(ObjectKey, Vec<ObjectKey>)],
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = cache.py();
    let dict: Bound<'py, PyDict> = PyDict::new(py);
    for (key, deps) in edges {
        dict.set_item(cache.key(key)?, PyTuple::new(py, cache.keys(deps)?)?)?;
    }
    Ok(dict)
}
