//! `facts[index]`: one fact, or a tuple of facts for a slice.

use pyo3::exceptions::PyIndexError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PySlice, PyTuple, PyTupleMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `facts[index]`: one fact, or a tuple of facts for a slice.
pub fn facts_getitem(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let index: Bound<'_, PyAny> = args.get_item(0)?;
    let projected: Bound<'_, PyAny> = native_field(instance, "facts")?;
    if index.is_instance_of::<PySlice>() {
        return Ok(projected.get_item(index)?.unbind());
    }
    let length: isize = isize::try_from(projected.len()?)?;
    let requested: isize = index.extract()?;
    let normalized: isize = if requested >= 0 {
        requested
    } else {
        length + requested
    };
    if normalized < 0 || normalized >= length {
        return Err(PyIndexError::new_err(index.unbind()));
    }
    Ok(projected.get_item(normalized)?.unbind())
}
