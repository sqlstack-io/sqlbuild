//! Reading planner values by attribute and building the JSON documents' dictionaries.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `str(value)`.
pub(crate) fn text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.str()?.to_string())
}

/// `str(value)`, or `None`.
pub(crate) fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    text(value).map(Some)
}

/// `str(item)` of every item.
pub(crate) fn texts(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    value.try_iter()?.map(|item| text(&item?)).collect()
}

/// Every item of an iterable.
pub(crate) fn items<'py>(value: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    value.try_iter()?.collect()
}

/// `value.<name>.value`, the value of an enum attribute.
pub(crate) fn enum_value<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    native_field(&native_field(value, name)?, "value")
}

/// `value.value` when `value` is set, else `None`.
pub(crate) fn optional_enum_value<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.is_none() {
        return Ok(value.clone());
    }
    native_field(value, "value")
}

/// A dictionary of the entries, in order.
pub(crate) fn dict<'py>(
    py: Python<'py>,
    entries: Vec<(&str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyDict>> {
    let built: Bound<'py, PyDict> = PyDict::new(py);
    for (key, value) in entries {
        built.set_item(key, value)?;
    }
    Ok(built)
}

/// Any Rust value as a Python object.
pub(crate) fn any<'py, T: IntoPyObjectExt<'py>>(
    py: Python<'py>,
    value: T,
) -> PyResult<Bound<'py, PyAny>> {
    value.into_bound_py_any(py)
}

/// `list(value)`.
pub(crate) fn list<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyList::new(value.py(), items(value)?)?.into_any())
}

/// `[build(item) for item in value]`.
pub(crate) fn mapped<'py>(
    value: &Bound<'py, PyAny>,
    build: impl Fn(&Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let built: Vec<Bound<'py, PyAny>> = items(value)?.iter().map(build).collect::<PyResult<_>>()?;
    Ok(PyList::new(value.py(), built)?.into_any())
}

/// `value.qualified_name or value.name`.
pub(crate) fn relation_name<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let qualified: Bound<'py, PyAny> = native_field(value, "qualified_name")?;
    if qualified.is_truthy()? {
        return Ok(qualified);
    }
    native_field(value, "name")
}

/// `value.isoformat()` when set, else `None`.
pub(crate) fn iso<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.is_none() {
        return Ok(value.clone());
    }
    value.call_method0("isoformat")
}
