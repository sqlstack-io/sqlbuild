//! Reading planner values by attribute and building the JSON documents' dictionaries.

use crate::plan_output::main::items::items;
use crate::plan_output::main::text::text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyList;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

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
