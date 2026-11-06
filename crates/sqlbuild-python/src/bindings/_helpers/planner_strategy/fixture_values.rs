//! Fixture boundary records and adapter callbacks.

use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::planner_fixtures::models::FixtureColumn;
use sqlbuild_core::text::main::python_casefold::python_casefold;
use sqlbuild_sqltext::resolution::main::unquoted_column_name::unquoted_column_name;

pub(crate) fn folded(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(folded_name(&value.extract::<String>()?))
}

/// The case-folded column name, without the quotes of a quoted identifier such as `"listingId"`.
pub(crate) fn folded_name(name: &str) -> String {
    python_casefold(unquoted_column_name(name))
}

pub(crate) fn columns(values: &Bound<'_, PyAny>, inferred: bool) -> PyResult<Vec<FixtureColumn>> {
    values
        .try_iter()?
        .map(|value| {
            let value = value?;
            let name = native_field(&value, "name")?;
            Ok(FixtureColumn {
                name: name.extract()?,
                folded_name: folded(&name)?,
                sql_type: native_field(&value, "type")?.extract()?,
                nullable: if inferred {
                    None
                } else {
                    native_field(&value, "nullable")?.extract()?
                },
            })
        })
        .collect()
}

pub(crate) fn record<'py>(
    py: Python<'py>,
    name: RecordId,
    kwargs: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    new_record(py, name, (), Some(kwargs))
}

pub(crate) fn label(key: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(format!(
        "mock {} '{}'",
        native_field(&key.get_item(0)?, "value")?.extract::<String>()?,
        key.get_item(1)?.extract::<String>()?
    ))
}

pub(crate) fn diagnostic<'py>(
    key: &Bound<'py, PyAny>,
    message: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let kwargs = PyDict::new(key.py());
    kwargs.set_item("key", key)?;
    kwargs.set_item("message", message)?;
    record(key.py(), RecordId::RelationFixtureDiagnostic, &kwargs)
}
