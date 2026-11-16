//! A typed SQL value as the plain value native consumers read.

use crate::compiled_views::_helpers::project::records::COLLECTION_KINDS;
use crate::compiled_views::_helpers::project::records::DECIMAL_KIND;
use crate::compiled_views::_helpers::project::records::SCALAR_KINDS;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyList};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// A typed SQL value as the plain value native consumers read.
#[pyfunction]
pub fn plain_typed_value<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let kind: String = native_field(value, "kind")?.getattr("value")?.extract()?;
    let inner: Bound<'py, PyAny> = native_field(value, "value")?;
    if kind == DECIMAL_KIND {
        return Ok(inner.str()?.into_any());
    }
    if SCALAR_KINDS.contains(&kind.as_str()) {
        return Ok(inner);
    }
    if COLLECTION_KINDS.contains(&kind.as_str()) {
        let items: Vec<Bound<'py, PyAny>> = inner
            .try_iter()?
            .map(|item| plain_typed_value(&item?))
            .collect::<PyResult<_>>()?;
        return Ok(PyList::new(py, items)?.into_any());
    }
    let mapping: Bound<'py, PyDict> = PyDict::new(py);
    for pair in inner.try_iter()? {
        let (key, item): (Bound<'py, PyAny>, Bound<'py, PyAny>) = pair?.extract()?;
        mapping.set_item(key, plain_typed_value(&item)?)?;
    }
    Ok(mapping.into_any())
}
