//! `CompactLineageFacts.resource_name(index)`.

use crate::lineage::_helpers::lineage_facts::resource_name;
use pyo3::prelude::{Bound, Py, PyAny, PyResult};
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_python_base::boundary::main::call_argument::call_argument;

/// `CompactLineageFacts.resource_name(index)`.
pub fn facts_resource_name(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let index: Bound<'_, PyAny> = call_argument((args, kwargs), "index", 0)?;
    Ok(resource_name(instance, &index)?.unbind())
}
