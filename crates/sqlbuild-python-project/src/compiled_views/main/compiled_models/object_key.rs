//! Object key.

use crate::compiled_views::models::CompiledViewClasses;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};

/// Object key.
pub fn object_key(
    classes: &CompiledViewClasses<'_>,
    kind: &str,
    name: &str,
) -> PyResult<Py<PyAny>> {
    let fields: Bound<'_, PyDict> = PyDict::new(classes.object_key.py());
    fields.set_item("resource_type", kind)?;
    fields.set_item("name", name)?;
    Ok(classes.object_key.call((), Some(&fields))?.unbind())
}
