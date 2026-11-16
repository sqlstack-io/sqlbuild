//! `facts[index]`: one fact, or a tuple of facts for a slice.

use crate::lineage::_helpers::lineage_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// `facts[index]`: one fact, or a tuple of facts for a slice.
#[inline]
pub fn facts_getitem(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    lineage_facts::facts_getitem(instance, args, _kwargs)
}
