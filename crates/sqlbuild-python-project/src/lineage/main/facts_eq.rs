//! `facts == other`: item-wise equality with any sequence.

use crate::lineage::_helpers::lineage_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// `facts == other`: item-wise equality with any sequence.
#[inline]
pub fn facts_eq(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    lineage_facts::facts_eq(instance, args, _kwargs)
}
