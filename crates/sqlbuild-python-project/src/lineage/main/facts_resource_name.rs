//! `CompactLineageFacts.resource_name(index)`.

use crate::lineage::_helpers::lineage_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// `CompactLineageFacts.resource_name(index)`.
#[inline]
pub fn facts_resource_name(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    lineage_facts::facts_resource_name(instance, args, kwargs)
}
