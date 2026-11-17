//! `CompactLineageFacts.facts`: every row projected once, in order.

use crate::lineage::_helpers::lineage_facts::project_row;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyTuple;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `CompactLineageFacts.facts`: every row projected once, in order.
pub fn facts_projected(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let projected: Vec<Py<PyAny>> = native_field(instance, "rows")?
        .try_iter()?
        .map(|row| project_row(instance, &row?))
        .collect::<PyResult<_>>()?;
    Ok(PyTuple::new(instance.py(), projected)?.into_any().unbind())
}
