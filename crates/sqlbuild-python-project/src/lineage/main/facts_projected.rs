//! `CompactLineageFacts.facts`: every row projected once, in order.

use crate::lineage::_helpers::lineage_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `CompactLineageFacts.facts`: every row projected once, in order.
#[inline]
pub fn facts_projected(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    lineage_facts::facts_projected(instance)
}
