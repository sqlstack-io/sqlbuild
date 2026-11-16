//! The Python classes a model's `CompileModelConfig` is built from, and building one from a native build.

use crate::model_config::main::config::config_fields::config_fields;
use crate::model_config::main::config::view::view;
use crate::model_config::models::ConfigClasses;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods};

use crate::model_config::models::BuiltConfig;

const MATERIALIZED_KEY: &str = "materialized";
pub(crate) const VALUES_FIELD: &str = "values";
pub(crate) const PERMANENT_TABLE_TYPE: &str = "permanent";
pub(crate) const TRANSIENT_TABLE_TYPE: &str = "transient";

/// The model's `CompileModelConfig`, once its template reads are recorded.
pub(crate) fn built_config<'py>(
    classes: &ConfigClasses<'py>,
    built: &BuiltConfig<'py>,
    matched: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let fields: Bound<'py, PyDict> = config_fields(classes, built, matched)?;
    view(classes, &built.values, &fields, built.nodes.as_ref())
}

/// The materialization whose inherited storage policy applies, or `None` when not table-backed.
pub(crate) fn storage_key<'py>(
    classes: &ConfigClasses<'py>,
    values: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = values.py();
    if let Some(materialized) = values.get_item(MATERIALIZED_KEY)?
        && let Ok(text) = materialized.downcast::<PyString>()
        && let Ok(text) = text.to_str()
        && classes.table_backed.iter().any(|name| name == text)
    {
        return Ok(materialized);
    }
    Ok(py.None().into_bound(py))
}

pub(crate) fn resolved_retention<'py>(
    classes: &ConfigClasses<'py>,
    desired_days: Bound<'py, PyAny>,
    unmanaged: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let fields: Bound<'py, PyDict> = PyDict::new(desired_days.py());
    fields.set_item("desired_days", desired_days)?;
    fields.set_item("unmanaged", unmanaged)?;
    fields.set_item("source", &classes.retention_source)?;
    classes.resolved_retention.call((), Some(&fields))
}

pub(crate) fn resolved_table_type<'py>(
    classes: &ConfigClasses<'py>,
    value: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let fields: Bound<'py, PyDict> = PyDict::new(classes.table_type.py());
    fields.set_item("value", classes.table_type.call1((value,))?)?;
    fields.set_item("source", &classes.table_type_source)?;
    fields.set_item("declared", true)?;
    classes.resolved_table_type.call((), Some(&fields))
}
