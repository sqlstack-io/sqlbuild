//! `select_latest_fingerprints`: an unfiltered latest read narrowed as a filtered read would be.

use crate::fingerprints::_helpers::latest_rows;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `select_latest_fingerprints`: an unfiltered latest read narrowed as a filtered read would be.
#[inline]
pub fn select_latest_fingerprints<'py>(
    py: Python<'py>,
    fingerprint_set: &Bound<'py, PyAny>,
    node_names: Vec<String>,
    filtered_node_types: Vec<String>,
) -> PyResult<Bound<'py, PyAny>> {
    latest_rows::select_latest_fingerprints(py, fingerprint_set, node_names, filtered_node_types)
}
