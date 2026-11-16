//! `len(facts)`.

use crate::lineage::_helpers::lineage_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// `len(facts)`.
#[inline]
pub fn facts_len(
    instance: &Bound<'_, PyAny>,
    _args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    lineage_facts::facts_len(instance, _args, _kwargs)
}
